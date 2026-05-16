use crate::capture::{CaptureConfig, OpenCvFrameSource};
use crate::recognition::RecognitionRuntimePort;
use crate::services::DesktopAppServices;
use crate::ui::{self, app::PokeEditorApp};
use anyhow::Result;
use champions_application::ports::{
    CatalogRepository, PartyRepository, UsageFetcher, UsageRepository,
};
use champions_infrastructure::config::AppPaths;
use champions_infrastructure::persistence::{
    CsvCatalogRepository, JsonPartyRepository, JsonUsageRepository,
};
use champions_infrastructure::{
    GameWithUsageFetcher, MangaOcrEngine, OnnxPartyIdentifier, OpenCvCropper,
};
use champions_runtime::{
    CommandSender, RecognitionPort, RgbaPreviewConverter, RuntimeBuilder, RuntimeWorkers,
};
use std::sync::Arc;
use std::time::Instant;

pub fn run(debug_mode: bool) -> iced::Result {
    DesktopComposition::compose(debug_mode)
        .expect("failed to compose desktop app")
        .run(debug_mode)
}

struct DesktopComposition {
    app_services: DesktopAppServices,
}

impl DesktopComposition {
    fn compose(debug_mode: bool) -> Result<Self> {
        let compose_started_at = Instant::now();
        let project_root = std::env::current_dir()?;
        let app_paths = AppPaths::from_project_root(&project_root);
        app_paths.ensure_writable_dirs()?;
        crate::observability::init(&app_paths)?;

        let _span = tracing::info_span!(
            "desktop_compose",
            project_root = %project_root.display(),
        )
        .entered();
        tracing::info!(
            user_data_dir = %app_paths.user_data_dir.display(),
            cache_dir = %app_paths.cache_dir.display(),
            model_dir = %app_paths.model_dir.display(),
            debug_mode,
            "composing desktop application",
        );
        if debug_mode {
            tracing::info!("startup profiling enabled");
        }

        let repositories_started_at = Instant::now();
        let repositories = DesktopRepositories::load(&app_paths, debug_mode)?;
        log_startup_profile(
            debug_mode,
            "desktop_repositories.load",
            repositories_started_at,
        );

        let runtime_started_at = Instant::now();
        RuntimeBootstrap::start(&app_paths, repositories.usage_repo.clone(), debug_mode)?;
        log_startup_profile(debug_mode, "runtime_bootstrap.start", runtime_started_at);
        log_startup_profile(debug_mode, "desktop_compose.total", compose_started_at);

        Ok(Self {
            app_services: DesktopAppServices::new(
                repositories.catalog_repo,
                repositories.party_repo,
                repositories.usage_fetcher,
                repositories.usage_repo,
            ),
        })
    }

    fn run(self, debug_mode: bool) -> iced::Result {
        let app_services = self.app_services;
        tracing::info!("launching iced daemon");

        iced::daemon(
            move || PokeEditorApp::new(app_services.clone(), debug_mode),
            PokeEditorApp::update,
            PokeEditorApp::view,
        )
        .subscription(PokeEditorApp::subscription)
        .title(PokeEditorApp::title)
        .font(include_bytes!(
            "../../../assets/fonts/NotoSansJP-Regular.ttf"
        ))
        .default_font(iced::Font {
            family: iced::font::Family::Name("Noto Sans JP"),
            ..Default::default()
        })
        .run()
    }
}

struct DesktopRepositories {
    catalog_repo: Arc<dyn CatalogRepository>,
    party_repo: Arc<dyn PartyRepository>,
    usage_fetcher: Arc<dyn UsageFetcher>,
    usage_repo: Arc<dyn UsageRepository>,
}

impl DesktopRepositories {
    fn load(app_paths: &AppPaths, debug_mode: bool) -> Result<Self> {
        let usage_json_path = app_paths.usage_json_path();
        let party_json_path = app_paths.party_json_path();
        tracing::info!(
            master_data_dir = %app_paths.master_data_dir.display(),
            usage_json_path = %usage_json_path.display(),
            party_json_path = %party_json_path.display(),
            "loading desktop repositories",
        );

        let catalog_started_at = Instant::now();
        let catalog_repo: Arc<dyn CatalogRepository> = Arc::new(CsvCatalogRepository::new(
            &app_paths.master_data_dir,
            Some(&usage_json_path),
        )?);
        log_startup_profile(debug_mode, "repository.catalog.load", catalog_started_at);

        let party_started_at = Instant::now();
        let party_repo: Arc<dyn PartyRepository> =
            Arc::new(JsonPartyRepository::new(party_json_path));
        log_startup_profile(debug_mode, "repository.party.init", party_started_at);

        let usage_fetcher_started_at = Instant::now();
        let usage_fetcher: Arc<dyn UsageFetcher> = Arc::new(GameWithUsageFetcher::new());
        log_startup_profile(
            debug_mode,
            "repository.usage_fetcher.init",
            usage_fetcher_started_at,
        );

        let usage_repo_started_at = Instant::now();
        let usage_repo: Arc<dyn UsageRepository> =
            Arc::new(JsonUsageRepository::new(usage_json_path));
        log_startup_profile(
            debug_mode,
            "repository.usage_repo.init",
            usage_repo_started_at,
        );

        Ok(Self {
            catalog_repo,
            party_repo,
            usage_fetcher,
            usage_repo,
        })
    }
}

struct RuntimeBootstrap;

impl RuntimeBootstrap {
    fn start(
        app_paths: &AppPaths,
        usage_repo: Arc<dyn UsageRepository>,
        debug_mode: bool,
    ) -> Result<()> {
        let _span = tracing::info_span!("runtime_bootstrap").entered();
        let capture_config = CaptureConfig::default();
        tracing::info!(
            device_index = capture_config.device_index,
            width = capture_config.width,
            height = capture_config.height,
            fps = capture_config.fps,
            "starting runtime bootstrap",
        );
        let capture_started_at = Instant::now();
        let frame_source = OpenCvFrameSource::open(&capture_config)?;
        log_startup_profile(debug_mode, "runtime.capture.open", capture_started_at);
        let preview_converter = RgbaPreviewConverter;

        let builder_prep_started_at = Instant::now();
        let mut builder = RuntimeBuilder::new()
            .frame_source(Box::new(frame_source))
            .preview_converter(Box::new(preview_converter))
            .preview_max_width(1920)
            .preview_target_fps(60);
        log_startup_profile(
            debug_mode,
            "runtime.builder.prepare",
            builder_prep_started_at,
        );

        let recognition_started_at = Instant::now();
        if let Some(recognition_port) = build_recognition_port(app_paths, usage_repo, debug_mode) {
            builder = builder.recognition_port(recognition_port);
        } else {
            tracing::warn!("recognition runtime disabled because model initialization failed");
        }
        log_startup_profile(
            debug_mode,
            "runtime.recognition_port.build",
            recognition_started_at,
        );

        let builder_started_at = Instant::now();
        let (handle, workers) = builder.build();
        log_startup_profile(debug_mode, "runtime.builder.build", builder_started_at);
        let (command_sender, event_receiver, preview_receiver) = handle.split();

        let subscriptions_started_at = Instant::now();
        ui::subscriptions::init_runtime(
            command_sender.clone(),
            Arc::new(tokio::sync::Mutex::new(preview_receiver)),
            Arc::new(tokio::sync::Mutex::new(event_receiver)),
        );
        log_startup_profile(
            debug_mode,
            "runtime.subscriptions.init",
            subscriptions_started_at,
        );

        let worker_spawn_started_at = Instant::now();
        spawn_runtime_workers(command_sender, workers);
        log_startup_profile(debug_mode, "runtime.workers.spawn", worker_spawn_started_at);
        tracing::info!("runtime bootstrap completed");
        Ok(())
    }
}

fn build_recognition_port(
    app_paths: &AppPaths,
    usage_repo: Arc<dyn UsageRepository>,
    debug_mode: bool,
) -> Option<Box<dyn RecognitionPort>> {
    let onnx_path = app_paths.model_dir.join("dinov2_vits14.onnx");
    let ocr_model_dir = app_paths.model_dir.join("manga-ocr");
    let master_img_dir = app_paths.pokemon_images_dir.clone();
    let _span = tracing::info_span!(
        "build_recognition_port",
        onnx_path = %onnx_path.display(),
        ocr_model_dir = %ocr_model_dir.display(),
        master_img_dir = %master_img_dir.display(),
        debug_mode,
    )
    .entered();

    let ocr_started_at = Instant::now();
    let ocr_engine = match MangaOcrEngine::new(&ocr_model_dir) {
        Ok(engine) => engine,
        Err(error) => {
            log_startup_profile(debug_mode, "recognition.ocr.init", ocr_started_at);
            tracing::warn!(%error, "OCR initialization failed; continuing without recognition");
            return None;
        }
    };
    log_startup_profile(debug_mode, "recognition.ocr.init", ocr_started_at);

    let identifier_started_at = Instant::now();
    let party_identifier = match OnnxPartyIdentifier::new(&onnx_path, &master_img_dir) {
        Ok(identifier) => identifier,
        Err(error) => {
            log_startup_profile(
                debug_mode,
                "recognition.party_identifier.init",
                identifier_started_at,
            );
            tracing::warn!(
                %error,
                "party identifier initialization failed; continuing without recognition",
            );
            return None;
        }
    };
    log_startup_profile(
        debug_mode,
        "recognition.party_identifier.init",
        identifier_started_at,
    );

    let cropper = OpenCvCropper::with_debug_party_slot_dump(debug_mode);
    tracing::info!("recognition runtime initialized");

    Some(Box::new(RecognitionRuntimePort::new(
        ocr_engine,
        party_identifier,
        cropper,
        usage_repo,
    )))
}

fn spawn_runtime_workers(command_sender: CommandSender, workers: RuntimeWorkers) {
    std::thread::spawn(move || {
        tracing::info!("runtime worker thread started");
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(async move {
            start_runtime(&command_sender).await;
            workers.run().await;
        });
        tracing::info!("runtime worker thread finished");
    });
}

async fn start_runtime(command_sender: &CommandSender) {
    match command_sender
        .send(champions_interface::RuntimeCommand::StartCapture)
        .await
    {
        Ok(()) => tracing::info!("initial capture start command sent"),
        Err(error) => tracing::error!(%error, "failed to send initial capture start command"),
    }
}

fn log_startup_profile(enabled: bool, step: &'static str, started_at: Instant) {
    if !enabled {
        return;
    }

    tracing::info!(
        startup_step = step,
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        "startup profile",
    );
}
