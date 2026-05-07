use opencv::{core, imgproc, prelude::*};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SideCropConfig {
    pub center_x: f64,
    pub y_start: f64,
    pub y_gap: f64,
    pub size_w: f64,
    pub width_ratio: f64, // 横長切り出し用の係数
}

impl Default for SideCropConfig {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            y_start: 0.0,
            y_gap: 0.1165,
            size_w: 0.057,
            width_ratio: 1.0,
        }
    }
}

pub fn default_crop_config() -> HashMap<&'static str, SideCropConfig> {
    let mut m = HashMap::new();
    m.insert(
        "opp",
        SideCropConfig {
            center_x: 0.87,
            y_start: 0.20,
            y_gap: 0.1165,
            size_w: 0.057,
            width_ratio: 1.0,
        },
    );
    m
}

/// OCR用の設定（相手の名前が表示されるあたり）
pub fn default_ocr_config() -> HashMap<&'static str, SideCropConfig> {
    let mut m = HashMap::new();
    m.insert(
        "target_text",
        SideCropConfig {
            center_x: 0.50, // (0.38 + 0.62) / 2
            y_start: 0.04,  // (0.02 + 0.06) / 2
            y_gap: 0.0,
            size_w: 0.04,   // height: 0.06 - 0.02
            width_ratio: 6.0, // width / height = 0.24 / 0.04
        },
    );
    m
}

pub fn get_pokemon_crops(
    img: &core::Mat,
    config: &HashMap<&str, SideCropConfig>,
) -> anyhow::Result<HashMap<String, Vec<Option<core::Mat>>>> {
    let h = img.rows() as f64;
    let w = img.cols() as f64;
    let mut result = HashMap::new();

    for (&side, conf) in config {
        let size_h = conf.size_w * w;
        let size_w = size_h * conf.width_ratio;
        let cx = conf.center_x * w;

        let mut crops = Vec::new();
        let count = if conf.y_gap == 0.0 { 1 } else { 6 };

        for i in 0..count {
            let cy = (conf.y_start * h) + (i as f64 * conf.y_gap * h);

            let x1 = (cx - size_w / 2.0).max(0.0) as i32;
            let y1 = (cy - size_h / 2.0).max(0.0) as i32;

            let roi = core::Rect::new(x1, y1, size_w as i32, size_h as i32);

            if roi.x + roi.width <= img.cols() && roi.y + roi.height <= img.rows() {
                let crop = core::Mat::roi(img, roi)?;
                crops.push(Some(crop.try_clone()?));
            } else {
                crops.push(None);
            }
        }
        result.insert(side.to_string(), crops);
    }
    Ok(result)
}

pub fn mat_to_rgb_bytes(mat: &core::Mat) -> anyhow::Result<(Vec<u8>, i32, i32)> {
    let mut rgb = core::Mat::default();
    imgproc::cvt_color(mat, &mut rgb, imgproc::COLOR_BGR2RGB, 0)?;
    let h = rgb.rows();
    let w = rgb.cols();
    let bytes = rgb.data_bytes()?.to_vec();
    Ok((bytes, h, w))
}
