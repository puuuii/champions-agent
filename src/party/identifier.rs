use std::{
    collections::HashMap,
    path::Path, // PathBuf を削除
};

use anyhow::{Context, Result};
use image::{DynamicImage, RgbImage, imageops};
use ndarray::{Array1, Array3, Axis, s, stack}; // Array4 を削除
use opencv::core::Mat;
use ort::{
    execution_providers::{CPUExecutionProvider, CUDAExecutionProvider},
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};

use crate::party::cutout::{SideCropConfig, get_pokemon_crops, mat_to_rgb_bytes};

const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
const INPUT_SIZE: u32 = 224;

pub struct PartyIdentifier {
    session: Session,
    master_embeddings: Vec<Array1<f32>>,
    master_names: Vec<String>,
}

impl PartyIdentifier {
    pub fn new(onnx_path: impl AsRef<Path>, master_dir: impl AsRef<Path>) -> Result<Self> {
        let session = Session::builder()
            .map_err(|e| anyhow::anyhow!("SessionBuilder error: {e}"))?
            .with_execution_providers([
                CUDAExecutionProvider::default().build(),
                CPUExecutionProvider::default().build(),
            ])
            .map_err(|e| anyhow::anyhow!("EP error: {e}"))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("Opt error: {e}"))?
            .commit_from_file(onnx_path)
            .map_err(|e| anyhow::anyhow!("Model load error: {e}"))?;

        let mut identifier = Self {
            session,
            master_embeddings: Vec::new(),
            master_names: Vec::new(),
        };

        identifier.cache_master_data(master_dir)?;
        Ok(identifier)
    }

    pub fn identify_party_batch(
        &mut self,
        img: &Mat,
        config: &HashMap<&str, SideCropConfig>,
    ) -> Result<HashMap<String, (String, f32)>> {
        let crops_map = get_pokemon_crops(img, config)?;

        let mut batch_tensors = Vec::new();
        let mut result_keys = Vec::new();

        for (side, crops) in &crops_map {
            for (i, crop_opt) in crops.iter().enumerate() {
                let Some(crop) = crop_opt else { continue };

                let (bytes, h, w) = mat_to_rgb_bytes(crop)?;
                let rgb = RgbImage::from_raw(w as u32, h as u32, bytes)
                    .context("RgbImage conversion failed")?;

                let tensor = preprocess_single(&DynamicImage::ImageRgb8(rgb));
                batch_tensors.push(tensor);
                result_keys.push(format!("{side}_{i}"));
            }
        }

        if batch_tensors.is_empty() {
            return Ok(HashMap::new());
        }

        let views: Vec<_> = batch_tensors.iter().map(|a| a.view()).collect();
        let batch_input = stack(Axis(0), &views)?;

        // --- 修正ポイント：スコープを分けて借用を解除する ---
        let embeddings = {
            let input = Tensor::from_array(batch_input)?;
            // ここで self を mutable に借用する
            let outputs = self.session.run(ort::inputs!["pixel_values" => input])?;
            // データを所有権付きの ArrayD にコピーして取り出す
            outputs["embedding"].try_extract_array::<f32>()?.to_owned()
            // ブロックを抜けることで outputs がドロップされ、self の借用が解除される
        };

        let mut results = HashMap::new();
        for (idx, key) in result_keys.into_iter().enumerate() {
            // ここでは self はもう自由なので find_best_match を呼べる
            let emb = embeddings.slice(s![idx, ..]).to_owned();
            let (name, score) = self.find_best_match(&l2_normalize(emb));
            results.insert(key, (name, score));
        }

        Ok(results)
    }

    fn cache_master_data(&mut self, master_dir: impl AsRef<Path>) -> Result<()> {
        let master_dir = master_dir.as_ref();
        let paths: Vec<_> = std::fs::read_dir(master_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "png"))
            .collect();

        for path in paths {
            let img = image::open(&path)?;
            let tensor = preprocess_single(&img);

            // ここも同様に所有権を分離
            let emb = {
                let input = Tensor::from_array(tensor.insert_axis(Axis(0)))?;
                let outputs = self.session.run(ort::inputs!["pixel_values" => input])?;
                outputs["embedding"]
                    .try_extract_array::<f32>()?
                    .slice(s![0, ..])
                    .to_owned()
            };

            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            self.master_embeddings.push(l2_normalize(emb));
            self.master_names.push(name);
        }
        Ok(())
    }

    fn find_best_match(&self, query: &Array1<f32>) -> (String, f32) {
        let mut best_score = f32::NEG_INFINITY;
        let mut best_idx = 0;
        for (i, emb) in self.master_embeddings.iter().enumerate() {
            let score = query.dot(emb);
            if score > best_score {
                best_score = score;
                best_idx = i;
            }
        }
        (self.master_names[best_idx].clone(), best_score)
    }
}

fn preprocess_single(img: &DynamicImage) -> Array3<f32> {
    let resized = img.resize_exact(INPUT_SIZE, INPUT_SIZE, imageops::FilterType::Triangle);
    let rgb = resized.to_rgb8();

    let mut tensor = Array3::<f32>::zeros((3, INPUT_SIZE as usize, INPUT_SIZE as usize));
    for (x, y, pixel) in rgb.enumerate_pixels() {
        for c in 0..3 {
            let val = pixel[c] as f32 / 255.0;
            tensor[[c, y as usize, x as usize]] = (val - MEAN[c]) / STD[c];
        }
    }
    tensor
}

fn l2_normalize(v: Array1<f32>) -> Array1<f32> {
    let norm = v.dot(&v).sqrt().max(1e-12);
    v / norm
}
