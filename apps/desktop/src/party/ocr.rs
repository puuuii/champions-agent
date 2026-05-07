use crate::party::cutout::mat_to_rgb_bytes;
use anyhow::Result;
use manga_ocr_rs::MangaOcr;
use opencv::prelude::*;

pub struct JapaneseOcr {
    inner: MangaOcr,
}

impl JapaneseOcr {
    pub fn new() -> Result<Self> {
        // ローカルのモデルディレクトリを指定
        let inner = MangaOcr::new(std::path::Path::new("models/manga-ocr/"))
            .map_err(|e| anyhow::anyhow!("OCRエンジンの初期化に失敗しました: {e}"))?;

        Ok(Self { inner })
    }

    pub fn recognize(&self, mat: &Mat) -> Result<String> {
        let (bytes, h, w) = mat_to_rgb_bytes(mat)?;
        let img = image::RgbImage::from_raw(w as u32, h as u32, bytes)
            .ok_or_else(|| anyhow::anyhow!("画像の変換に失敗しました"))?;

        let dynamic_img = image::DynamicImage::ImageRgb8(img);

        let text = self
            .inner
            .recognize(&dynamic_img)
            .map_err(|e| anyhow::anyhow!("推論エラー: {e}"))?;

        Ok(text)
    }
}
