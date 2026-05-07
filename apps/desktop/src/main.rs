use iced::Size;
use opencv::{core, highgui, imgcodecs, prelude::*, videoio};
use std::{
    collections::HashMap,
    fs, process,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

mod domain;
mod party;
use party::ocr::JapaneseOcr;
use party::{
    PartyIdentifier, cutout::default_ocr_config, cutout::get_pokemon_crops, default_crop_config,
};

pub mod damage;
pub mod ui;

use domain::master_data::MasterData;
// app.rsのPokemonUsageを使用できるようにインポート
use ui::app::{PokeEditorApp, PokemonUsage};

const CAPTURE_PATH: &str = "capture.png";
const ONNX_PATH: &str = "models/dinov2_vits14.onnx";
const MASTER_IMG_DIR: &str = "master_data/pokemon_images";
const USAGE_JSON_PATH: &str = "master_data/usage.json";
const MASTER_DATA_DIR: &str = "master_data";

fn main() -> iced::Result {
    let _ = fs::create_dir_all("master_data");

    // --- マスターデータのロード ---
    let master_data = MasterData::load(MASTER_DATA_DIR).unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load master data: {}", e);
        MasterData::default()
    });
    let master_data = Arc::new(master_data);

    // --- JSONのメモリロード ---
    let usage_data = fs::read_to_string(USAGE_JSON_PATH).unwrap_or_else(|_| "[]".to_string());
    let usages: Vec<PokemonUsage> = serde_json::from_str(&usage_data).unwrap_or_default();
    let usage_map: HashMap<String, PokemonUsage> =
        usages.into_iter().map(|u| (u.name.clone(), u)).collect();
    let usage_map = Arc::new(usage_map);

    // カメラ → OCRワーカー間のチャンネル
    let (tx, rx) = mpsc::sync_channel::<core::Mat>(1);

    // OCRワーカー → UIへの詳細情報通知チャンネル (StringからVec<PokemonUsage>に変更)
    let (info_tx, info_rx) = mpsc::channel::<Vec<PokemonUsage>>();
    let info_rx = Arc::new(Mutex::new(info_rx));

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

        // [1回前の判定, 2回前の判定]
        let mut history = [false, false];

        while let Ok(frame) = rx.recv() {
            let _ = imgcodecs::imwrite(CAPTURE_PATH, &frame, &core::Vector::new());

            let mut is_selection_screen = false;

            // 1. OCR処理で画面判定
            if let Ok(ocr_crops) = get_pokemon_crops(&frame, &ocr_config) {
                if let Some(Some(crop)) = ocr_crops.get("target_text").and_then(|v| v.get(0)) {
                    if let Ok(text) = ocr_processor.recognize(crop) {
                        let clean_text = text.trim();
                        // 判定条件は維持（不要な標準出力や単発送信は削除）
                        if clean_text.contains("シングル") && clean_text.contains("バトル") {
                            is_selection_screen = true;
                        }
                    }
                }
            }

            // 2. 選出画面の場合のみ推論処理
            if is_selection_screen {
                // 前回または前々回が既に「選出画面」だった場合はDINOv2推論をスキップ
                if history[0] || history[1] {
                    history[1] = history[0];
                    history[0] = true;
                    continue;
                }

                match identifier.identify_party_batch(&frame, &crop_config) {
                    Ok(results) => {
                        if !results.is_empty() {
                            let mut keys: Vec<_> = results.keys().collect();
                            keys.sort();

                            let mut party_info = Vec::new();
                            for key in keys {
                                let (name, _) = &results[key];
                                // 推論した名前をキーにメモリから詳細情報を引き当てる
                                if let Some(usage) = usage_map.get(name) {
                                    party_info.push(usage.clone());
                                }
                            }

                            // UIへ推論＆JSON抽出結果を送信
                            let _ = info_tx.send(party_info);
                        }
                    }
                    Err(e) => eprintln!("[Worker] 推論エラー: {e}"),
                }
            }

            // 履歴の更新
            history[1] = history[0];
            history[0] = is_selection_screen;
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
    let info_rx_for_ui = info_rx.clone();
    let master_data_for_ui = master_data.clone();
    iced::application(
        move || PokeEditorApp::new(info_rx_for_ui.clone(), master_data_for_ui.clone()),
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
    .font(include_bytes!("../../../assets/fonts/NotoSansJP-Regular.ttf"))
    .default_font(iced::Font {
        family: iced::font::Family::Name("Noto Sans JP"),
        ..Default::default()
    })
    .run()
}
