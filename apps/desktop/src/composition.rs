use crate::battle_selection::BattleSelectionInferer;
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

pub fn run(debug_mode: bool) -> iced::Result {
    DesktopComposition::compose(debug_mode)
        .expect("failed to compose desktop app")
        .run()
}

struct DesktopComposition {
    app_services: DesktopAppServices,
}

impl DesktopComposition {
    fn compose(debug_mode: bool) -> Result<Self> {
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
            "composing desktop application",
        );

        let repositories = DesktopRepositories::load(&app_paths)?;
        let battle_selection_inferer =
            RuntimeBootstrap::start(&app_paths, repositories.usage_repo.clone(), debug_mode)?;

        Ok(Self {
            app_services: DesktopAppServices::new(
                repositories.catalog_repo,
                repositories.party_repo,
                repositories.usage_fetcher,
                repositories.usage_repo,
                battle_selection_inferer,
                debug_mode,
            ),
        })
    }

    fn run(self) -> iced::Result {
        let app_services = self.app_services;
        tracing::info!("launching iced daemon");

        iced::daemon(
            move || PokeEditorApp::new(app_services.clone()),
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
    fn load(app_paths: &AppPaths) -> Result<Self> {
        let usage_json_path = app_paths.usage_json_path();
        let party_json_path = app_paths.party_json_path();
        tracing::info!(
            master_data_dir = %app_paths.master_data_dir.display(),
            usage_json_path = %usage_json_path.display(),
            party_json_path = %party_json_path.display(),
            "loading desktop repositories",
        );

        let catalog_repo: Arc<dyn CatalogRepository> = Arc::new(CsvCatalogRepository::new(
            &app_paths.master_data_dir,
            Some(&usage_json_path),
        )?);
        let party_repo: Arc<dyn PartyRepository> =
            Arc::new(JsonPartyRepository::new(party_json_path));
        let usage_fetcher: Arc<dyn UsageFetcher> = Arc::new(GameWithUsageFetcher::new());
        let usage_repo: Arc<dyn UsageRepository> =
            Arc::new(JsonUsageRepository::new(usage_json_path));

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
    ) -> Result<Option<Arc<BattleSelectionInferer>>> {
        let _span = tracing::info_span!("runtime_bootstrap").entered();
        let capture_config = CaptureConfig::default();
        tracing::info!(
            device_index = capture_config.device_index,
            width = capture_config.width,
            height = capture_config.height,
            fps = capture_config.fps,
            "starting runtime bootstrap",
        );
        let frame_source = OpenCvFrameSource::open(&capture_config)?;
        let preview_converter = RgbaPreviewConverter;

        let mut builder = RuntimeBuilder::new()
            .frame_source(Box::new(frame_source))
            .preview_converter(Box::new(preview_converter))
            .preview_max_width(1920)
            .preview_target_fps(60);

        let mut battle_selection_inferer = None;

        if let Some(components) = build_recognition_components(app_paths, usage_repo, debug_mode) {
            battle_selection_inferer = Some(components.battle_selection_inferer.clone());
            builder = builder.recognition_port(components.recognition_port);
        } else {
            tracing::warn!("recognition runtime disabled because model initialization failed");
        }

        let (handle, workers) = builder.build();
        let (command_sender, event_receiver, preview_receiver) = handle.split();

        ui::subscriptions::init_runtime(
            command_sender.clone(),
            Arc::new(tokio::sync::Mutex::new(preview_receiver)),
            Arc::new(tokio::sync::Mutex::new(event_receiver)),
        );

        spawn_runtime_workers(command_sender, workers);
        tracing::info!("runtime bootstrap completed");
        Ok(battle_selection_inferer)
    }
}

struct RecognitionComponents {
    recognition_port: Box<dyn RecognitionPort>,
    battle_selection_inferer: Arc<BattleSelectionInferer>,
}

fn build_recognition_components(
    app_paths: &AppPaths,
    usage_repo: Arc<dyn UsageRepository>,
    debug_mode: bool,
) -> Option<RecognitionComponents> {
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

    let ocr_engine = match MangaOcrEngine::new(&ocr_model_dir) {
        Ok(engine) => engine,
        Err(error) => {
            tracing::warn!(%error, "OCR initialization failed; continuing without recognition");
            return None;
        }
    };

    let party_identifier = match OnnxPartyIdentifier::new(&onnx_path, &master_img_dir, debug_mode) {
        Ok(identifier) => Arc::new(identifier),
        Err(error) => {
            tracing::warn!(
                %error,
                "party identifier initialization failed; continuing without recognition",
            );
            return None;
        }
    };

    let cropper = Arc::new(OpenCvCropper::with_debug_party_slot_dump(debug_mode));
    let battle_selection_inferer = Arc::new(BattleSelectionInferer::new(
        party_identifier.clone(),
        cropper.clone(),
    ));
    tracing::info!("recognition runtime initialized");

    Some(RecognitionComponents {
        recognition_port: Box::new(RecognitionRuntimePort::new(
            ocr_engine,
            party_identifier,
            cropper,
            usage_repo,
        )),
        battle_selection_inferer,
    })
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
