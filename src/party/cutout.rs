//! # cutout
//!
//! キャプチャ画像からポケモンアイコン領域を切り出すモジュール。
//! Python版 cutout.py の Rust 移植。

use opencv::{core, imgproc, prelude::*};
use std::collections::HashMap;

/// 切り出し設定（片側分）
#[derive(Debug, Clone)]
pub struct SideCropConfig {
    /// アイコン列の中心X座標（画像幅に対する割合）
    pub center_x: f64,
    /// 最初のアイコンの中心Y座標（画像高に対する割合）
    pub y_start: f64,
    /// アイコン間の垂直ギャップ（画像高に対する割合）
    pub y_gap: f64,
    /// アイコンの一辺サイズ（画像幅に対する割合）
    pub size_w: f64,
}

impl Default for SideCropConfig {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            y_start: 0.0,
            y_gap: 0.1165,
            size_w: 0.057,
        }
    }
}

/// 自分側・相手側のデフォルト設定を返す
pub fn default_crop_config() -> HashMap<&'static str, SideCropConfig> {
    let mut m = HashMap::new();
    m.insert(
        "my",
        SideCropConfig {
            center_x: 0.286,
            y_start: 0.197,
            y_gap: 0.1165,
            size_w: 0.057,
        },
    );
    m.insert(
        "opp",
        SideCropConfig {
            center_x: 0.87,
            y_start: 0.20,
            y_gap: 0.1165,
            size_w: 0.057,
        },
    );
    m
}

/// 切り出し結果。`None` はその枠が画像外または空だったことを示す。
pub type CropResult = Vec<Option<core::Mat>>;

/// 設定に基づき画像を切り出す。
///
/// # 戻り値
/// `{ "my" => [Option<Mat>; 6], "opp" => [Option<Mat>; 6] }` 形式の HashMap。
pub fn get_pokemon_crops(
    img: &core::Mat,
    config: &HashMap<&str, SideCropConfig>,
) -> anyhow::Result<HashMap<String, CropResult>> {
    let h = img.rows() as f64;
    let w = img.cols() as f64;
    let mut result: HashMap<String, CropResult> = HashMap::new();

    for (&side, conf) in config {
        let side_px = conf.size_w * w;
        let half = side_px / 2.0;
        let cx = conf.center_x * w;

        let mut crops: CropResult = Vec::with_capacity(6);

        for i in 0..6 {
            let cy = (conf.y_start * h) + (i as f64 * conf.y_gap * h);

            let x1 = (cx - half).max(0.0) as i32;
            let x2 = (cx + half).min(w) as i32;
            let y1 = (cy - half).max(0.0) as i32;
            let y2 = (cy + half).min(h) as i32;

            if x2 <= x1 || y2 <= y1 {
                crops.push(None);
                continue;
            }

            let roi = core::Rect::new(x1, y1, x2 - x1, y2 - y1);
            let crop = core::Mat::roi(img, roi)?;

            // 空チェック
            if crop.empty() {
                crops.push(None);
            } else {
                // clone してROIの参照を切る
                crops.push(Some(crop.try_clone()?));
            }
        }

        result.insert(side.to_string(), crops);
    }

    Ok(result)
}

/// BGR Mat → RGB Vec<u8> に変換して返す（ONNX推論の前処理に使用）
pub fn mat_to_rgb_bytes(mat: &core::Mat) -> anyhow::Result<(Vec<u8>, i32, i32)> {
    let mut rgb = core::Mat::default();
    imgproc::cvt_color(mat, &mut rgb, imgproc::COLOR_BGR2RGB, 0)?;

    let h = rgb.rows();
    let w = rgb.cols();
    let bytes = rgb.data_bytes()?.to_vec();
    Ok((bytes, h, w))
}
