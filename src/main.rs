use iced::Size;
use opencv::{core, highgui, imgcodecs, prelude::*, videoio};
use std::{
    fs, process,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

mod party;
use party::{PartyIdentifier, default_crop_config};
pub mod damage;

pub mod ui;
use ui::app::PokeEditorApp;

// 警告を避けるため未使用定数に _ を付与
const _USAGE_URL: &str = "https://gamewith.jp/pokemon-champions/555373";
const _USAGE_OUT: &str = "master_data/usage.json";
const CAPTURE_PATH: &str = "capture.png";
const ONNX_PATH: &str = "models/dinov2_vits14.onnx";
const MASTER_IMG_DIR: &str = "master_data/pokemon_images";

fn main() -> iced::Result {
    let _ = fs::create_dir_all("master_data");
    let (tx, rx) = mpsc::sync_channel::<core::Mat>(1);

    // ─── ワーカースレッド (推論処理) ───
    thread::spawn(move || {
        println!("[Worker] 推論エンジンを初期化中...");
        let mut identifier = match PartyIdentifier::new(ONNX_PATH, MASTER_IMG_DIR) {
            Ok(id) => {
                println!("[Worker] 初期化完了。");
                id
            }
            Err(e) => {
                eprintln!("[Worker] 初期化失敗: {e}");
                return;
            }
        };
        let crop_config = default_crop_config();

        for frame in rx {
            let _ = imgcodecs::imwrite(CAPTURE_PATH, &frame, &core::Vector::new());

            match identifier.identify_party_batch(&frame, &crop_config) {
                Ok(results) => {
                    if !results.is_empty() {
                        let mut keys: Vec<_> = results.keys().collect();
                        keys.sort();
                        println!("--- 推論結果 ---");
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
        // warning回避のため cam_opt への mut は付与しない
        let cam_opt = videoio::VideoCapture::new(0, videoio::CAP_V4L2).ok();
        if let Some(mut cam) = cam_opt {
            let _ = cam.set(videoio::CAP_PROP_FRAME_WIDTH, 1920.0);
            let _ = cam.set(videoio::CAP_PROP_FRAME_HEIGHT, 1080.0);
            let mut frame = core::Mat::default();
            let mut last_save = Instant::now();
            loop {
                if cam.read(&mut frame).is_ok() && !frame.empty() {
                    if last_save.elapsed() >= Duration::from_secs(1) {
                        if let Ok(cloned) = frame.try_clone() {
                            let _ = tx.try_send(cloned);
                        }
                        last_save = Instant::now();
                    }
                    let _ = highgui::imshow("Switch 2 Rust Stream", &frame);

                    // 'q' が押されたらプロセスごと終了させる
                    if highgui::wait_key(1).unwrap_or(-1) == b'q' as i32 {
                        println!("'q' が押されたため終了します...");
                        process::exit(0);
                    }
                }
            }
        } else {
            eprintln!("[Capture] カメラが見つかりません。");
        }
    });

    // ─── メインスレッド (iced v0.14) ───
    iced::application(
        PokeEditorApp::new,
        PokeEditorApp::update,
        PokeEditorApp::view,
    )
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
