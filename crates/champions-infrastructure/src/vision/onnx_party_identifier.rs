use std::path::Path;
use std::sync::Mutex;

use champions_application::{
    PartyIdentifier, PartyIdentifierError, PartyImageSet, RecognitionConfig,
};
use champions_domain::recognition::{
    ConfidenceScore, RecognitionCandidate, RecognizedParty, RecognizedPokemon, SpeciesId,
};
use image::{DynamicImage, RgbImage, imageops};
use ndarray::{Array1, Array3, Axis, s, stack};
use ort::{
    execution_providers::{CPUExecutionProvider, CUDAExecutionProvider},
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use rayon::prelude::*;

const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
const INPUT_SIZE: u32 = 224;
const HIGH_CONFIDENCE_THRESHOLD: f32 = 0.85;
const LOW_CONFIDENCE_THRESHOLD: f32 = 0.5;

pub struct OnnxPartyIdentifier {
    session: Mutex<Session>,
    master_embeddings: Vec<Array1<f32>>,
    master_names: Vec<String>,
}

impl OnnxPartyIdentifier {
    pub fn new(
        onnx_path: impl AsRef<Path>,
        master_images_dir: impl AsRef<Path>,
    ) -> Result<Self, PartyIdentifierError> {
        let onnx_path = onnx_path.as_ref();
        let master_images_dir = master_images_dir.as_ref();
        tracing::info!(
            onnx_path = %onnx_path.display(),
            master_images_dir = %master_images_dir.display(),
            "initializing ONNX party identifier",
        );
        let session = Session::builder()
            .map_err(|e| PartyIdentifierError::ModelNotFound(format!("SessionBuilder error: {e}")))?
            .with_execution_providers([
                CUDAExecutionProvider::default().build(),
                CPUExecutionProvider::default().build(),
            ])
            .map_err(|e| PartyIdentifierError::ModelNotFound(format!("EP error: {e}")))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| PartyIdentifierError::ModelNotFound(format!("Opt error: {e}")))?
            .commit_from_file(onnx_path)
            .map_err(|e| {
                PartyIdentifierError::ModelNotFound(format!(
                    "Model load error ({}): {e}",
                    onnx_path.display()
                ))
            })?;

        let mut identifier = Self {
            session: Mutex::new(session),
            master_embeddings: Vec::new(),
            master_names: Vec::new(),
        };

        identifier.cache_master_data(master_images_dir)?;
        tracing::info!(
            species_count = identifier.master_names.len(),
            "ONNX party identifier initialized",
        );
        Ok(identifier)
    }

    fn cache_master_data(
        &mut self,
        master_dir: impl AsRef<Path>,
    ) -> Result<(), PartyIdentifierError> {
        let master_dir = master_dir.as_ref();
        let paths: Vec<std::path::PathBuf> = std::fs::read_dir(master_dir)
            .map_err(|e| {
                PartyIdentifierError::MasterDataNotLoaded(format!("{}: {e}", master_dir.display()))
            })?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "png"))
            .collect();

        if paths.is_empty() {
            return Err(PartyIdentifierError::MasterDataNotLoaded(
                "no PNG files found in master images directory".into(),
            ));
        }

        tracing::info!(
            master_dir = %master_dir.display(),
            image_count = paths.len(),
            "loading master images",
        );

        let processed_data: Vec<(String, Array3<f32>)> = paths
            .into_par_iter()
            .filter_map(|path| {
                let img = image::open(&path).ok()?;
                let tensor = preprocess_single(&img);
                let name = path.file_stem()?.to_string_lossy().to_string();
                Some((name, tensor))
            })
            .collect();

        for (name, tensor) in processed_data {
            let emb = self.run_single_embedding(tensor)?;
            self.master_embeddings.push(l2_normalize(emb));
            self.master_names.push(name);
        }

        tracing::info!(
            species_count = self.master_names.len(),
            "master data cached",
        );
        Ok(())
    }

    fn run_single_embedding(
        &self,
        tensor: Array3<f32>,
    ) -> Result<Array1<f32>, PartyIdentifierError> {
        let input = Tensor::from_array(tensor.insert_axis(Axis(0)))
            .map_err(|e| PartyIdentifierError::InferenceFailed(format!("tensor error: {e}")))?;
        let mut session = self.session.lock().unwrap();
        let outputs = session
            .run(ort::inputs!["pixel_values" => input])
            .map_err(|e| PartyIdentifierError::InferenceFailed(format!("run error: {e}")))?;
        let emb = outputs["embedding"]
            .try_extract_array::<f32>()
            .map_err(|e| PartyIdentifierError::InferenceFailed(format!("extract error: {e}")))?
            .slice(s![0, ..])
            .to_owned();
        Ok(emb)
    }

    fn find_top_matches(&self, query: &Array1<f32>, top_n: usize) -> Vec<(usize, f32)> {
        let mut scores: Vec<(usize, f32)> = self
            .master_embeddings
            .iter()
            .enumerate()
            .map(|(i, emb)| (i, query.dot(emb)))
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_n);
        scores
    }

    fn find_top_matches_in_candidates(
        &self,
        query: &Array1<f32>,
        candidate_indices: &[usize],
        top_n: usize,
    ) -> Vec<(usize, f32)> {
        let mut scores: Vec<(usize, f32)> = candidate_indices
            .iter()
            .copied()
            .map(|index| (index, query.dot(&self.master_embeddings[index])))
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_n.min(scores.len()));
        scores
    }

    fn embedding_from_slot_image(
        &self,
        slot_image: &SlotImage,
    ) -> Result<Option<Array1<f32>>, PartyIdentifierError> {
        if slot_image.rgb_bytes.is_empty() || slot_image.width == 0 || slot_image.height == 0 {
            return Ok(None);
        }

        let Some(rgb) = RgbImage::from_raw(
            slot_image.width,
            slot_image.height,
            slot_image.rgb_bytes.clone(),
        ) else {
            return Ok(None);
        };

        let tensor = preprocess_single(&DynamicImage::ImageRgb8(rgb));
        let embedding = self.run_single_embedding(tensor)?;
        Ok(Some(l2_normalize(embedding)))
    }

    pub fn identify_from_candidate_names(
        &self,
        slot_image: &SlotImage,
        candidate_names: &[String],
        config: &RecognitionConfig,
    ) -> Result<Option<RecognizedPokemon>, PartyIdentifierError> {
        let candidate_indices = candidate_names
            .iter()
            .filter_map(|candidate_name| self.master_names.iter().position(|name| name == candidate_name))
            .collect::<Vec<_>>();

        if candidate_indices.is_empty() {
            return Ok(None);
        }

        let Some(embedding) = self.embedding_from_slot_image(slot_image)? else {
            return Ok(None);
        };

        let top_matches = self.find_top_matches_in_candidates(
            &embedding,
            &candidate_indices,
            config.top_candidates,
        );
        let best_match = top_matches.first();

        let best_score = best_match.map(|(_, score)| *score).unwrap_or(0.0);
        let confidence = ConfidenceScore::from_score(
            best_score,
            HIGH_CONFIDENCE_THRESHOLD,
            LOW_CONFIDENCE_THRESHOLD,
        );
        let display_name = best_match.map(|(index, _)| self.master_names[*index].clone());
        let species_id = best_match.map(|(index, _)| SpeciesId(*index as u32));
        let candidates = top_matches
            .iter()
            .map(|(index, score)| RecognitionCandidate {
                species_id: Some(SpeciesId(*index as u32)),
                display_name: self.master_names[*index].clone(),
                score: *score,
            })
            .collect();

        Ok(Some(RecognizedPokemon {
            slot: slot_image.slot,
            species_id,
            display_name,
            confidence,
            candidates,
        }))
    }
}

impl PartyIdentifier for OnnxPartyIdentifier {
    fn identify_opponent_party(
        &self,
        input: &PartyImageSet,
        config: &RecognitionConfig,
    ) -> Result<RecognizedParty, PartyIdentifierError> {
        if input.slots.is_empty() {
            return Ok(RecognizedParty {
                pokemons: Vec::new(),
            });
        }

        let mut batch_tensors = Vec::new();
        let mut slot_indices = Vec::new();

        for slot_image in &input.slots {
            if slot_image.rgb_bytes.is_empty() || slot_image.width == 0 || slot_image.height == 0 {
                continue;
            }

            let rgb = match RgbImage::from_raw(
                slot_image.width,
                slot_image.height,
                slot_image.rgb_bytes.clone(),
            ) {
                Some(img) => img,
                None => continue,
            };

            let tensor = preprocess_single(&DynamicImage::ImageRgb8(rgb));
            batch_tensors.push(tensor);
            slot_indices.push(slot_image.slot);
        }

        if batch_tensors.is_empty() {
            return Ok(RecognizedParty {
                pokemons: Vec::new(),
            });
        }

        let views: Vec<_> = batch_tensors.iter().map(|a| a.view()).collect();
        let batch_input = stack(Axis(0), &views)
            .map_err(|e| PartyIdentifierError::InferenceFailed(format!("stack error: {e}")))?;

        let embeddings = {
            let input_tensor = Tensor::from_array(batch_input)
                .map_err(|e| PartyIdentifierError::InferenceFailed(format!("tensor error: {e}")))?;
            let mut session = self.session.lock().unwrap();
            let outputs = session
                .run(ort::inputs!["pixel_values" => input_tensor])
                .map_err(|e| PartyIdentifierError::InferenceFailed(format!("run error: {e}")))?;
            outputs["embedding"]
                .try_extract_array::<f32>()
                .map_err(|e| PartyIdentifierError::InferenceFailed(format!("extract error: {e}")))?
                .to_owned()
        };

        let mut pokemons = Vec::new();
        for (idx, slot) in slot_indices.into_iter().enumerate() {
            let emb = l2_normalize(embeddings.slice(s![idx, ..]).to_owned());
            let top_matches = self.find_top_matches(&emb, config.top_candidates);
            let best_match = top_matches.first();

            let best_score = best_match.map(|(_, s)| *s).unwrap_or(0.0);
            let confidence = ConfidenceScore::from_score(
                best_score,
                HIGH_CONFIDENCE_THRESHOLD,
                LOW_CONFIDENCE_THRESHOLD,
            );

            let display_name = best_match.map(|(i, _)| self.master_names[*i].clone());
            let species_id = best_match.map(|(i, _)| SpeciesId(*i as u32));

            let candidates = top_matches
                .iter()
                .map(|(i, score)| RecognitionCandidate {
                    species_id: Some(SpeciesId(*i as u32)),
                    display_name: self.master_names[*i].clone(),
                    score: *score,
                })
                .collect();

            pokemons.push(RecognizedPokemon {
                slot,
                species_id,
                display_name,
                confidence,
                candidates,
            });
        }

        Ok(RecognizedParty { pokemons })
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
