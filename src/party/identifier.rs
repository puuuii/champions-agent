//! # identifier
//!
//! DINOv2 ONNX モデルを使ってポケモンアイコンを同定するモジュール。
//! ort 2.0.0-rc.x API 対応版。

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use image::{DynamicImage, RgbImage, imageops};
use ndarray::{Array1, Array4, s};
use opencv::core::Mat;
use ort::{
    execution_providers::{CPUExecutionProvider, CUDAExecutionProvider},
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};

use crate::party::cutout::{SideCropConfig, get_pokemon_crops, mat_to_rgb_bytes};

// ─── 前処理定数（ImageNet正規化）────────────────────────────────────────────

const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
const INPUT_SIZE: u32 = 224;

// ─── 公開 API ─────────────────────────────────────────────────────────────────

pub struct PartyIdentifier {
    session: Session,
    master_embeddings: Vec<Array1<f32>>,
    master_names: Vec<String>,
}

impl PartyIdentifier {
    pub fn new(onnx_path: impl AsRef<Path>, master_dir: impl AsRef<Path>) -> Result<Self> {
        // BuilderのエラーがSend/Syncを実装していないため、map_errでStringベースのエラーに変換
        let session = Session::builder()
            .map_err(|e| anyhow::anyhow!("SessionBuilder初期化エラー: {e}"))?
            .with_execution_providers([
                CUDAExecutionProvider::default().build(),
                CPUExecutionProvider::default().build(),
            ])
            .map_err(|e| anyhow::anyhow!("ExecutionProvider設定エラー: {e}"))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("最適化レベル設定エラー: {e}"))?
            .commit_from_file(onnx_path)
            .map_err(|e| anyhow::anyhow!("モデル読み込みエラー: {e}"))?;

        let mut identifier = Self {
            session,
            master_embeddings: Vec::new(),
            master_names: Vec::new(),
        };

        identifier.cache_master_data(master_dir)?;
        Ok(identifier)
    }

    pub fn identify_party(
        &mut self,
        img: &Mat,
        config: &HashMap<&str, SideCropConfig>,
    ) -> Result<HashMap<String, (String, f32)>> {
        let crops_map = get_pokemon_crops(img, config)?;
        let mut results = HashMap::new();

        for (side, crops) in &crops_map {
            for (i, crop_opt) in crops.iter().enumerate() {
                let Some(crop) = crop_opt else { continue };
                let embedding = self.embed_mat(crop)?;
                let (name, score) = self.find_best_match(&embedding);
                results.insert(format!("{side}_{i}"), (name, score));
            }
        }

        Ok(results)
    }

    // ─── 内部実装 ───────────────────────────────────────────────────────────

    fn cache_master_data(&mut self, master_dir: impl AsRef<Path>) -> Result<()> {
        let master_dir = master_dir.as_ref();
        anyhow::ensure!(
            master_dir.exists(),
            "master_dir が見つかりません: {master_dir:?}"
        );

        println!("キャッシュ中: {master_dir:?}");

        let entries: Vec<PathBuf> = std::fs::read_dir(master_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "png"))
            .collect();

        for path in &entries {
            let img = image::open(path).with_context(|| format!("画像読み込み失敗: {path:?}"))?;
            let embedding = self.embed_image(&img)?;
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            self.master_embeddings.push(embedding);
            self.master_names.push(name);
        }

        println!("完了: {}体分キャッシュ", self.master_embeddings.len());
        Ok(())
    }

    fn embed_mat(&mut self, mat: &Mat) -> Result<Array1<f32>> {
        let (bytes, h, w) = mat_to_rgb_bytes(mat)?;
        let img = RgbImage::from_raw(w as u32, h as u32, bytes).context("RgbImage変換失敗")?;
        self.embed_image(&DynamicImage::ImageRgb8(img))
    }

    fn embed_image(&mut self, img: &DynamicImage) -> Result<Array1<f32>> {
        let tensor = preprocess(img);

        let input = Tensor::from_array(tensor)?;
        let outputs = self.session.run(ort::inputs!["pixel_values" => input])?;

        let output = outputs["embedding"].try_extract_array::<f32>()?;
        let flat: Array1<f32> = output.slice(s![0, ..]).to_owned();
        Ok(l2_normalize(flat))
    }

    fn find_best_match(&self, query: &Array1<f32>) -> (String, f32) {
        let mut best_score = f32::NEG_INFINITY;
        let mut best_idx = 0;

        for (i, emb) in self.master_embeddings.iter().enumerate() {
            let score: f32 = query.dot(emb);
            if score > best_score {
                best_score = score;
                best_idx = i;
            }
        }

        (self.master_names[best_idx].clone(), best_score)
    }
}

// ─── 前処理 ─────────────────────────────────────────────────────────────────

fn preprocess(img: &DynamicImage) -> Array4<f32> {
    let (w, h) = (img.width(), img.height());
    let (nw, nh) = if w < h {
        (INPUT_SIZE, (INPUT_SIZE as f64 * h as f64 / w as f64) as u32)
    } else {
        ((INPUT_SIZE as f64 * w as f64 / h as f64) as u32, INPUT_SIZE)
    };
    // 修正: Bilinear は存在しないため Triangle を使用
    let resized = img.resize_exact(nw, nh, imageops::FilterType::Triangle);

    let x_off = (nw - INPUT_SIZE) / 2;
    let y_off = (nh - INPUT_SIZE) / 2;
    let cropped = resized.crop_imm(x_off, y_off, INPUT_SIZE, INPUT_SIZE);
    let rgb = cropped.to_rgb8();

    let mut tensor = Array4::<f32>::zeros((1, 3, INPUT_SIZE as usize, INPUT_SIZE as usize));
    for (x, y, pixel) in rgb.enumerate_pixels() {
        for c in 0..3 {
            let val = pixel[c] as f32 / 255.0;
            tensor[[0, c, y as usize, x as usize]] = (val - MEAN[c]) / STD[c];
        }
    }
    tensor
}

fn l2_normalize(v: Array1<f32>) -> Array1<f32> {
    let norm = v.dot(&v).sqrt().max(1e-12);
    v / norm
}
