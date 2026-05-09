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

pub fn run() -> iced::Result {
    DesktopComposition::compose()
        .expect("failed to compose desktop app")
        .run()
}

struct DesktopComposition {
    app_services: DesktopAppServices,
}

impl DesktopComposition {
    fn compose() -> Result<Self> {
        let project_root = std::env::current_dir()?;
        let app_paths = AppPaths::from_project_root(&project_root);
        app_paths.ensure_writable_dirs()?;

        let repositories = DesktopRepositories::load(&app_paths)?;
        RuntimeBootstrap::start(&app_paths, repositories.usage_repo.clone())?;

        Ok(Self {
            app_services: DesktopAppServices::new(
                repositories.catalog_repo,
                repositories.party_repo,
                repositories.usage_fetcher,
                repositories.usage_repo,
            ),
        })
    }

    fn run(self) -> iced::Result {
        let app_services = self.app_services;

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

        let catalog_repo: Arc<dyn CatalogRepository> = Arc::new(CsvCatalogRepository::new(
            &app_paths.master_data_dir,
            Some(&usage_json_path),
        )?);
        let party_repo: Arc<dyn PartyRepository> =
            Arc::new(JsonPartyRepository::new(app_paths.party_json_path()));
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
    fn start(app_paths: &AppPaths, usage_repo: Arc<dyn UsageRepository>) -> Result<()> {
        let capture_config = CaptureConfig::default();
        let frame_source = OpenCvFrameSource::open(&capture_config)?;
        let preview_converter = RgbaPreviewConverter;

        let mut builder = RuntimeBuilder::new()
            .frame_source(Box::new(frame_source))
            .preview_converter(Box::new(preview_converter))
            .preview_max_width(960)
            .preview_target_fps(15);

        if let Some(recognition_port) = build_recognition_port(app_paths, usage_repo) {
            builder = builder.recognition_port(recognition_port);
        }

        let (handle, workers) = builder.build();
        let (command_sender, event_receiver, preview_receiver) = handle.split();

        ui::subscriptions::init_receivers(
            Arc::new(tokio::sync::Mutex::new(preview_receiver)),
            Arc::new(tokio::sync::Mutex::new(event_receiver)),
        );

        spawn_runtime_workers(command_sender, workers);
        Ok(())
    }
}

fn build_recognition_port(
    app_paths: &AppPaths,
    usage_repo: Arc<dyn UsageRepository>,
) -> Option<Box<dyn RecognitionPort>> {
    let onnx_path = app_paths.model_dir.join("dinov2_vits14.onnx");
    let ocr_model_dir = app_paths.model_dir.join("manga-ocr");
    let master_img_dir = app_paths.pokemon_images_dir.clone();

    let ocr_engine = match MangaOcrEngine::new(&ocr_model_dir) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("[Recognition] OCR initialization failed: {error}");
            return None;
        }
    };

    let party_identifier = match OnnxPartyIdentifier::new(&onnx_path, &master_img_dir) {
        Ok(identifier) => identifier,
        Err(error) => {
            eprintln!("[Recognition] ONNX initialization failed: {error}");
            return None;
        }
    };

    let cropper = OpenCvCropper::new();

    Some(Box::new(RecognitionRuntimePort::new(
        ocr_engine,
        party_identifier,
        cropper,
        usage_repo,
    )))
}

fn spawn_runtime_workers(command_sender: CommandSender, workers: RuntimeWorkers) {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        runtime.block_on(async move {
            start_runtime(&command_sender).await;
            workers.run().await;
        });
    });
}

async fn start_runtime(command_sender: &CommandSender) {
    let _ = command_sender
        .send(champions_interface::RuntimeCommand::StartCapture)
        .await;
    let _ = command_sender
        .send(champions_interface::RuntimeCommand::StartRecognition)
        .await;
}
