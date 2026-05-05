use iced::Size;
use opencv::{core, highgui, imgcodecs, prelude::*, videoio};
use std::{
    fs, process,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

mod party;
use party::ocr::JapaneseOcr;
use party::{
    PartyIdentifier, cutout::default_ocr_config, cutout::get_pokemon_crops, default_crop_config,
};

pub mod damage;
pub mod ui;
use ui::app::PokeEditorApp;

const CAPTURE_PATH: &str = "capture.png";
const ONNX_PATH: &str = "models/dinov2_vits14.onnx";
const MASTER_IMG_DIR: &str = "master_data/pokemon_images";

fn main() -> iced::Result {
    let _ = fs::create_dir_all("master_data");

    // カメラ → OCRワーカー間のチャンネル
    let (tx, rx) = mpsc::sync_channel::<core::Mat>(1);
    // OCRワーカー → UIへの結果通知チャンネル
    let (ocr_tx, ocr_rx) = mpsc::channel::<String>();
    let ocr_rx = Arc::new(Mutex::new(ocr_rx));

    // ─── ワーカースレッド (OCR + アイコン推論) ───
    thread::spawn(move || {
        println!("[Worker] システムエンジンを初期化中...");

        let mut identifier = match PartyIdentifier::new(ONNX_PATH, MASTER_IMG_DIR) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("[Worker] 初期化失敗: {e}");
                return;
            }
        };

        let ocr_processor = match JapaneseOcr::new() {
            Ok(ocr) => ocr,
            Err(e) => {
                eprintln!(
                    "[Worker] OCRの初期化に失敗しました。 models/manga-ocr/ が正しく配置されているか確認してください: {e}"
                );
                return;
            }
        };

        let crop_config = default_crop_config();
        let ocr_config = default_ocr_config();

        while let Ok(frame) = rx.recv() {
            let _ = imgcodecs::imwrite(CAPTURE_PATH, &frame, &core::Vector::new());

            // 1. OCR処理 (標準出力せず、チャンネル経由でUIへ送信)
            if let Ok(ocr_crops) = get_pokemon_crops(&frame, &ocr_config) {
                if let Some(Some(crop)) = ocr_crops.get("target_text").and_then(|v| v.get(0)) {
                    match ocr_processor.recognize(crop) {
                        Ok(text) => {
                            let clean_text = text.trim().to_string();
                            if !clean_text.is_empty() {
                                let _ = ocr_tx.send(clean_text);
                            }
                        }
                        Err(e) => eprintln!("[OCR Error] {}", e),
                    }
                }
            }

            // 2. 敵ポケモン推論処理 (DINOv2)
            match identifier.identify_party_batch(&frame, &crop_config) {
                Ok(results) => {
                    if !results.is_empty() {
                        let mut keys: Vec<_> = results.keys().collect();
                        keys.sort();
                        println!("--- アイコン推論結果 ---");
                        for key in keys {
                            let (name, score) = &results[key];
                            println!("  [{key}] {name} ({score:.4})");
                        }
                    }
                }
                Err(e) => eprintln!("[Worker] 推論エラー: {e}"),
            }
        }
    });

    // ─── サブスレッド (OpenCV映像キャプチャ) ───
    thread::spawn(move || {
        let cam_opt = videoio::VideoCapture::new(0, videoio::CAP_V4L2).ok();
        if let Some(mut cam) = cam_opt {
            let _ = cam.set(videoio::CAP_PROP_FRAME_WIDTH, 1920.0);
            let _ = cam.set(videoio::CAP_PROP_FRAME_HEIGHT, 1080.0);
            let mut frame = core::Mat::default();
            loop {
                if cam.read(&mut frame).is_ok() && !frame.empty() {
                    if let Ok(cloned) = frame.try_clone() {
                        let _ = tx.try_send(cloned);
                    }
                    let _ = highgui::imshow("Switch 2 Rust Stream", &frame);

                    if highgui::wait_key(1).unwrap_or(-1) == b'q' as i32 {
                        process::exit(0);
                    }
                    thread::sleep(Duration::from_millis(33));
                }
            }
        }
    });

    // ─── メインスレッド (UI) ───
    let ocr_rx_for_ui = ocr_rx.clone();
    iced::application(
        move || PokeEditorApp::new(ocr_rx_for_ui.clone()),
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
        ..Default::default()
    })
    .run()
}
