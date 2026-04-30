use opencv::{core, highgui, imgcodecs, prelude::*, videoio};
use std::{
    fs,
    time::{Duration, Instant},
};

use usage_fetcher::fetch_usage;

mod party;
use party::{PartyIdentifier, default_crop_config};

const USAGE_URL: &str = "https://gamewith.jp/pokemon-champions/555373";
const USAGE_OUT: &str = "master_data/usage.json";
const CAPTURE_PATH: &str = "capture.png";
const ONNX_PATH: &str = "models/dinov2_vits14.onnx";
const MASTER_IMG_DIR: &str = "master_data/pokemon_images";

fn main() -> anyhow::Result<()> {
    // ─── Step 1: 使用率データの取得 ──────────────────────────────────────────
    println!("[1/3] 使用率データを取得中: {USAGE_URL}");
    let usage_data = fetch_usage(USAGE_URL)?;
    fs::create_dir_all("master_data")?;
    fs::write(USAGE_OUT, serde_json::to_string_pretty(&usage_data)?)?;
    println!("[1/3] 完了: {}体分 → {USAGE_OUT}", usage_data.len());

    // ─── Step 2: パーティ判定エンジンの初期化 ────────────────────────────────
    println!("[2/3] パーティ判定エンジンを初期化中...");
    let mut identifier = PartyIdentifier::new(ONNX_PATH, MASTER_IMG_DIR)?;
    let crop_config = default_crop_config();
    println!("[2/3] 完了");

    // ─── Step 3: キャプチャパイプライン ──────────────────────────────────────
    println!("[3/3] キャプチャを開始します。'q'キーで終了。");

    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_V4L2)?;
    anyhow::ensure!(
        videoio::VideoCapture::is_opened(&cam)?,
        "キャプチャボードが開けません"
    );

    cam.set(videoio::CAP_PROP_FRAME_WIDTH, 1920.0)?;
    cam.set(videoio::CAP_PROP_FRAME_HEIGHT, 1080.0)?;
    cam.set(videoio::CAP_PROP_FPS, 60.0)?;

    let mut frame = core::Mat::default();
    let mut last_save = Instant::now();
    let interval = Duration::from_secs(1);

    loop {
        cam.read(&mut frame)?;
        if frame.empty() {
            continue;
        }

        if last_save.elapsed() >= interval {
            // 1. capture.png として保存
            let params = core::Vector::new();
            imgcodecs::imwrite(CAPTURE_PATH, &frame, &params)?;
            println!("Saved: {CAPTURE_PATH}");
            last_save = Instant::now();

            // 2. 保存済みフレームに対してパーティ判定
            match identifier.identify_party(&frame, &crop_config) {
                Ok(results) => {
                    // キーをソートして出力を安定させる
                    let mut keys: Vec<_> = results.keys().collect();
                    keys.sort();
                    for key in keys {
                        let (name, score) = &results[key];
                        println!("  [{key}] {name}  (similarity: {score:.4})");
                    }
                }
                Err(e) => eprintln!("  判定エラー: {e}"),
            }
        }

        highgui::imshow("Switch 2 Rust Stream", &frame)?;
        // 'q' as i32 のレガシーキャストを修正
        if highgui::wait_key(1)? == i32::from(b'q') {
            break;
        }
    }

    Ok(())
}
