use champions_domain::usage::PokemonUsageSummary;
use champions_infrastructure::RgbaPreviewConverter;
use champions_infrastructure::config::AppPaths;
use champions_infrastructure::persistence::{
    CsvCatalogRepository, JsonPartyRepository, JsonUsageRepository,
};
use champions_infrastructure::{
    MangaOcrEngine, OnnxPartyIdentifier, OpenCvCropper, RecognitionAdapter,
};
use champions_runtime::RuntimeBuilder;
use iced::Size;
use std::sync::Arc;

mod capture;
pub mod ui;

use capture::{CaptureConfig, OpenCvFrameSource};
use ui::app::PokeEditorApp;

fn main() -> iced::Result {
    let project_root = std::env::current_dir().expect("failed to get current dir");
    let app_paths = AppPaths::from_project_root(&project_root);
    app_paths
        .ensure_writable_dirs()
        .expect("failed to create data dirs");

    let master_data_dir = &app_paths.master_data_dir;
    let model_dir = &app_paths.model_dir;
    let pokemon_images_dir = &app_paths.pokemon_images_dir;
    let usage_json_path = app_paths.usage_json_path();

    let onnx_path = model_dir.join("dinov2_vits14.onnx");
    let ocr_model_dir = model_dir.join("manga-ocr");
    let master_img_dir = pokemon_images_dir.clone();

    // --- Repositories ---
    let catalog_repo = Arc::new(
        CsvCatalogRepository::new(master_data_dir, Some(&usage_json_path))
<<<<<<< HEAD
            .expect("failed to load catalog repository"),
=======
            .expect("failed to load catalog"),
>>>>>>> f2f2b34b886871c5178965a723e4967226edb5b4
    );
    let party_repo = Arc::new(JsonPartyRepository::new(app_paths.party_json_path()));
    let usage_repo = Arc::new(JsonUsageRepository::new(usage_json_path));

    // --- Recognition setup ---
    let recognition_port = build_recognition_port(
        &onnx_path,
        &ocr_model_dir,
        &master_img_dir,
        usage_repo.clone(),
    );

    // --- Runtime setup ---
    let capture_config = CaptureConfig::default();
    let frame_source =
        OpenCvFrameSource::open(&capture_config).expect("failed to open capture device");
    let preview_converter = RgbaPreviewConverter;

    let mut builder = RuntimeBuilder::new()
        .frame_source(Box::new(frame_source))
        .preview_converter(Box::new(preview_converter))
        .preview_max_width(960)
        .preview_target_fps(15);

    if let Some(port) = recognition_port {
        builder = builder.recognition_port(Box::new(port));
    }

    let (handle, workers) = builder.build();

    let (command_sender, event_receiver, preview_receiver) = handle.split();
    let command_sender = Arc::new(command_sender);
    let event_receiver = Arc::new(tokio::sync::Mutex::new(event_receiver));
    let preview_receiver = Arc::new(tokio::sync::Mutex::new(preview_receiver));

    // Initialize static receivers for Iced subscriptions
    ui::subscriptions::init_receivers(preview_receiver.clone(), event_receiver.clone());

    // --- info channel for legacy UI compatibility ---
    let (_info_tx, info_rx) = std::sync::mpsc::channel::<Vec<PokemonUsageSummary>>();
    let info_rx = Arc::new(std::sync::Mutex::new(info_rx));

    // --- Spawn runtime workers ---
    let command_sender_for_start = command_sender.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        rt.block_on(async {
            // Start capture and recognition immediately
            let _ = command_sender_for_start
                .send(champions_interface::RuntimeCommand::StartCapture)
                .await;
            let _ = command_sender_for_start
                .send(champions_interface::RuntimeCommand::StartRecognition)
                .await;
            workers.run().await;
        });
    });

    // --- Main thread (UI) ---
    let info_rx_for_ui = info_rx.clone();
    let catalog_repo_for_ui = catalog_repo.clone();
    let party_repo_for_ui = party_repo.clone();
    let command_sender_for_ui = command_sender.clone();

    iced::application(
        move || {
            PokeEditorApp::new(
                info_rx_for_ui.clone(),
                catalog_repo_for_ui.clone(),
                party_repo_for_ui.clone(),
                command_sender_for_ui.clone(),
            )
        },
        PokeEditorApp::update,
        PokeEditorApp::view,
    )
    .subscription(PokeEditorApp::subscription)
    .title("Pokemon Editor")
    .window(iced::window::Settings {
        size: Size {
            width: 1200.0,
            height: 800.0,
        },
        exit_on_close_request: true,
        ..Default::default()
    })
    .font(include_bytes!(
        "../../../assets/fonts/NotoSansJP-Regular.ttf"
    ))
    .default_font(iced::Font {
        family: iced::font::Family::Name("Noto Sans JP"),
        ..Default::default()
    })
    .run()
}

fn build_recognition_port(
    onnx_path: &std::path::Path,
    ocr_model_dir: &std::path::Path,
    master_img_dir: &std::path::Path,
    usage_repo: Arc<JsonUsageRepository>,
) -> Option<RecognitionAdapter> {
    let ocr_engine = match MangaOcrEngine::new(ocr_model_dir) {
        Ok(engine) => engine,
        Err(e) => {
            eprintln!("[Recognition] OCR initialization failed: {e}");
            return None;
        }
    };

    let party_identifier = match OnnxPartyIdentifier::new(onnx_path, master_img_dir) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("[Recognition] ONNX initialization failed: {e}");
            return None;
        }
    };

    let cropper = OpenCvCropper::new();

    Some(RecognitionAdapter::new(
        ocr_engine,
        party_identifier,
        cropper,
        usage_repo,
    ))
}
