use crate::persistence::atomic_write;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct FileSignature {
    file_name: String,
    len: u64,
    modified_unix_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct MasterEmbeddingCacheKey {
    onnx_model: FileSignature,
    master_images: Vec<FileSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct MasterEmbeddingCache {
    key: MasterEmbeddingCacheKey,
    master_names: Vec<String>,
    master_embeddings: Vec<Vec<f32>>,
}

pub struct OnnxPartyIdentifier {
    session: Mutex<Session>,
    master_embeddings: Vec<Array1<f32>>,
    master_names: Vec<String>,
}

impl OnnxPartyIdentifier {
    pub fn new(
        onnx_path: impl AsRef<Path>,
        master_images_dir: impl AsRef<Path>,
        cache_path: impl AsRef<Path>,
    ) -> Result<Self, PartyIdentifierError> {
        let onnx_path = onnx_path.as_ref();
        let master_images_dir = master_images_dir.as_ref();
        let cache_path = cache_path.as_ref();
        tracing::info!(
            onnx_path = %onnx_path.display(),
            master_images_dir = %master_images_dir.display(),
            cache_path = %cache_path.display(),
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

        identifier.cache_master_data(onnx_path, master_images_dir, &cache_path)?;
        tracing::info!(
            species_count = identifier.master_names.len(),
            "ONNX party identifier initialized",
        );
        Ok(identifier)
    }

    fn cache_master_data(
        &mut self,
        onnx_path: &Path,
        master_dir: impl AsRef<Path>,
        cache_path: &Path,
    ) -> Result<(), PartyIdentifierError> {
        let master_dir = master_dir.as_ref();
        let paths = collect_master_image_paths(master_dir)?;

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

        let cache_key = match build_cache_key(onnx_path, &paths) {
            Ok(key) => Some(key),
            Err(error) => {
                tracing::warn!(
                    %error,
                    cache_path = %cache_path.display(),
                    "failed to build master embedding cache key; regenerating embeddings",
                );
                None
            }
        };

        if let Some(cache_key) = cache_key.as_ref() {
            if let Some(cache) = try_read_master_embedding_cache(cache_path, cache_key) {
                self.master_names = cache.master_names;
                self.master_embeddings = cache
                    .master_embeddings
                    .into_iter()
                    .map(Array1::from_vec)
                    .collect();

                tracing::info!(
                    cache_path = %cache_path.display(),
                    species_count = self.master_names.len(),
                    "loaded master embeddings from cache",
                );
                return Ok(());
            }
        }

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

        if let Some(cache_key) = cache_key {
            let cache = MasterEmbeddingCache {
                key: cache_key,
                master_names: self.master_names.clone(),
                master_embeddings: self
                    .master_embeddings
                    .iter()
                    .map(|embedding| embedding.to_vec())
                    .collect(),
            };

            if let Err(error) = write_master_embedding_cache(cache_path, &cache) {
                tracing::warn!(
                    %error,
                    cache_path = %cache_path.display(),
                    "failed to persist master embedding cache",
                );
            } else {
                tracing::info!(
                    cache_path = %cache_path.display(),
                    species_count = self.master_names.len(),
                    "persisted master embeddings cache",
                );
            }
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

fn collect_master_image_paths(master_dir: &Path) -> Result<Vec<PathBuf>, PartyIdentifierError> {
    let mut paths: Vec<PathBuf> = fs::read_dir(master_dir)
        .map_err(|e| {
            PartyIdentifierError::MasterDataNotLoaded(format!("{}: {e}", master_dir.display()))
        })?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .collect();

    paths.sort_by(|left, right| file_name_for_cache(left).cmp(&file_name_for_cache(right)));

    Ok(paths)
}

fn build_cache_key(
    onnx_path: &Path,
    image_paths: &[PathBuf],
) -> Result<MasterEmbeddingCacheKey, PartyIdentifierError> {
    let onnx_model = build_file_signature(onnx_path)?;
    let master_images = image_paths
        .iter()
        .map(|path| build_file_signature(path))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(MasterEmbeddingCacheKey {
        onnx_model,
        master_images,
    })
}

fn build_file_signature(path: &Path) -> Result<FileSignature, PartyIdentifierError> {
    let metadata = fs::metadata(path).map_err(|e| {
        PartyIdentifierError::MasterDataNotLoaded(format!(
            "failed to read metadata for {}: {e}",
            path.display()
        ))
    })?;
    let modified = metadata.modified().map_err(|e| {
        PartyIdentifierError::MasterDataNotLoaded(format!(
            "failed to read modified time for {}: {e}",
            path.display()
        ))
    })?;
    let modified_unix_secs = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|e| {
            PartyIdentifierError::MasterDataNotLoaded(format!(
                "failed to normalize modified time for {}: {e}",
                path.display()
            ))
        })?
        .as_secs();

    Ok(FileSignature {
        file_name: file_name_for_cache(path),
        len: metadata.len(),
        modified_unix_secs,
    })
}

fn file_name_for_cache(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

fn try_read_master_embedding_cache(
    cache_path: &Path,
    expected_key: &MasterEmbeddingCacheKey,
) -> Option<MasterEmbeddingCache> {
    if !cache_path.is_file() {
        return None;
    }

    let data = match fs::read_to_string(cache_path) {
        Ok(data) => data,
        Err(error) => {
            tracing::warn!(
                %error,
                cache_path = %cache_path.display(),
                "failed to read master embedding cache; regenerating",
            );
            return None;
        }
    };

    let cache: MasterEmbeddingCache = match serde_json::from_str(&data) {
        Ok(cache) => cache,
        Err(error) => {
            tracing::warn!(
                %error,
                cache_path = %cache_path.display(),
                "failed to parse master embedding cache; regenerating",
            );
            return None;
        }
    };

    if cache.key != *expected_key {
        tracing::info!(
            cache_path = %cache_path.display(),
            "master embedding cache is stale; regenerating",
        );
        return None;
    }

    if cache.master_names.len() != cache.master_embeddings.len() || cache.master_names.is_empty() {
        tracing::warn!(
            cache_path = %cache_path.display(),
            "master embedding cache contents are invalid; regenerating",
        );
        return None;
    }

    let embedding_dim = cache.master_embeddings[0].len();
    if embedding_dim == 0
        || cache
            .master_embeddings
            .iter()
            .any(|embedding| embedding.len() != embedding_dim)
    {
        tracing::warn!(
            cache_path = %cache_path.display(),
            "master embedding cache dimensions are invalid; regenerating",
        );
        return None;
    }

    Some(cache)
}

fn write_master_embedding_cache(
    cache_path: &Path,
    cache: &MasterEmbeddingCache,
) -> Result<(), PartyIdentifierError> {
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            PartyIdentifierError::MasterDataNotLoaded(format!(
                "failed to create cache directory {}: {e}",
                parent.display()
            ))
        })?;
    }

    let json = serde_json::to_vec(cache).map_err(|e| {
        PartyIdentifierError::MasterDataNotLoaded(format!(
            "failed to serialize master embedding cache: {e}"
        ))
    })?;

    atomic_write(cache_path, &json).map_err(|e| {
        PartyIdentifierError::MasterDataNotLoaded(format!(
            "failed to write master embedding cache {}: {e}",
            cache_path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_test_dir(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("champions_{name}_{stamp}"))
    }

    #[test]
    fn master_embedding_cache_round_trips() {
        let dir = unique_test_dir("master_embedding_cache_round_trip");
        fs::create_dir_all(&dir).unwrap();
        let cache_path = dir.join("master_embeddings.json");
        let cache = MasterEmbeddingCache {
            key: MasterEmbeddingCacheKey {
                onnx_model: FileSignature {
                    file_name: "dinov2_vits14.onnx".to_string(),
                    len: 123,
                    modified_unix_secs: 456,
                },
                master_images: vec![FileSignature {
                    file_name: "pikachu.png".to_string(),
                    len: 789,
                    modified_unix_secs: 999,
                }],
            },
            master_names: vec!["pikachu".to_string()],
            master_embeddings: vec![vec![0.25, 0.5, 1.0]],
        };

        write_master_embedding_cache(&cache_path, &cache).unwrap();
        let loaded = try_read_master_embedding_cache(&cache_path, &cache.key).unwrap();

        assert_eq!(loaded, cache);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_master_image_paths_sorts_file_names() {
        let dir = unique_test_dir("collect_master_image_paths_sorts_file_names");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("zoroark.png"), b"z").unwrap();
        fs::write(dir.join("pikachu.png"), b"p").unwrap();
        fs::write(dir.join("ignore.txt"), b"x").unwrap();

        let paths = collect_master_image_paths(&dir).unwrap();
        let file_names: Vec<String> = paths
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert_eq!(file_names, vec!["pikachu.png", "zoroark.png"]);

        let _ = fs::remove_dir_all(&dir);
    }
}
