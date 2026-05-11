use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use champions_application::{OcrImage, PartyImageSet, RecognitionImageExtractor, SlotImage};
use champions_domain::recognition::SelectionSlot;
use image::ExtendedColorType;

const TARGET_TEXT_DEBUG_FILENAME: &str = "latest_crop_selection_text.png";
const BATTLE_RESULT_DEBUG_FILENAME: &str = "latest_crop_battle_result_text.png";

#[derive(Debug, Clone)]
pub struct CropConfig {
    pub center_x: f64,
    pub y_start: f64,
    pub y_gap: f64,
    pub size_w: f64,
    pub width_ratio: f64,
}

pub struct OpenCvCropper {
    opponent_config: CropConfig,
    ocr_config: CropConfig,
    battle_result_config: CropConfig,
    debug_output_dir: PathBuf,
}

impl OpenCvCropper {
    pub fn new() -> Self {
        Self::with_debug_output_dir(std::env::current_dir().unwrap_or_else(|_| ".".into()))
    }

    fn with_debug_output_dir(debug_output_dir: impl Into<PathBuf>) -> Self {
        Self {
            opponent_config: CropConfig {
                center_x: 0.87,
                y_start: 0.20,
                y_gap: 0.1165,
                size_w: 0.057,
                width_ratio: 1.0,
            },
            ocr_config: CropConfig {
                center_x: 0.50,
                y_start: 0.04,
                y_gap: 0.0,
                size_w: 0.04,
                width_ratio: 6.0,
            },
            // Matches scripts/crop_image_with_config.py for WIN/LOSE detection.
            battle_result_config: CropConfig {
                center_x: 0.51,
                y_start: 0.65,
                y_gap: 0.0,
                size_w: 0.13,
                width_ratio: 6.0,
            },
            debug_output_dir: debug_output_dir.into(),
        }
    }

    fn crop_region(
        &self,
        frame_width: u32,
        frame_height: u32,
        frame_bytes: &[u8],
        channels: usize,
        config: &CropConfig,
        index: usize,
    ) -> Option<(Vec<u8>, u32, u32)> {
        let w = frame_width as f64;
        let h = frame_height as f64;

        let size_h = config.size_w * w;
        let size_w = size_h * config.width_ratio;
        let cx = config.center_x * w;
        let cy = (config.y_start * h) + (index as f64 * config.y_gap * h);

        let x1 = ((cx - size_w / 2.0).max(0.0)) as u32;
        let y1 = ((cy - size_h / 2.0).max(0.0)) as u32;
        let crop_w = (size_w as u32).min(frame_width.saturating_sub(x1));
        let crop_h = (size_h as u32).min(frame_height.saturating_sub(y1));

        if crop_w == 0 || crop_h == 0 {
            return None;
        }

        let stride = frame_width as usize * channels;
        let mut rgb_bytes = Vec::with_capacity((crop_w * crop_h * 3) as usize);

        for row in 0..crop_h {
            let src_y = (y1 + row) as usize;
            for col in 0..crop_w {
                let src_x = (x1 + col) as usize;
                let offset = src_y * stride + src_x * channels;

                if channels >= 3 && offset + 2 < frame_bytes.len() {
                    // BGR -> RGB
                    rgb_bytes.push(frame_bytes[offset + 2]);
                    rgb_bytes.push(frame_bytes[offset + 1]);
                    rgb_bytes.push(frame_bytes[offset]);
                } else if channels == 1 && offset < frame_bytes.len() {
                    let g = frame_bytes[offset];
                    rgb_bytes.push(g);
                    rgb_bytes.push(g);
                    rgb_bytes.push(g);
                }
            }
        }

        Some((rgb_bytes, crop_w, crop_h))
    }

    fn detect_channels(&self, frame_width: u32, frame_height: u32, frame_bytes: &[u8]) -> usize {
        let pixel_count = (frame_width as usize) * (frame_height as usize);
        if pixel_count == 0 {
            return 3;
        }
        let byte_count = frame_bytes.len();
        if byte_count >= pixel_count * 4 {
            4
        } else if byte_count >= pixel_count * 3 {
            3
        } else {
            1
        }
    }

    fn debug_output_path(&self, file_name: &str) -> PathBuf {
        self.debug_output_dir.join(file_name)
    }

    fn save_debug_crop(&self, file_name: &str, width: u32, height: u32, rgb_bytes: &[u8]) {
        let path = self.debug_output_path(file_name);

        if let Err(error) = ensure_parent_dir(&path) {
            tracing::warn!(
                "failed to prepare debug crop output directory for {}: {}",
                path.display(),
                error
            );
            return;
        }

        if let Err(error) =
            image::save_buffer(&path, rgb_bytes, width, height, ExtendedColorType::Rgb8)
        {
            tracing::warn!("failed to save debug crop {}: {}", path.display(), error);
        }
    }

    fn clear_debug_crop(&self, file_name: &str) {
        let path = self.debug_output_path(file_name);

        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                tracing::warn!(
                    "failed to remove stale debug crop {}: {}",
                    path.display(),
                    error
                )
            }
        }
    }
}

impl Default for OpenCvCropper {
    fn default() -> Self {
        Self::new()
    }
}

impl RecognitionImageExtractor for OpenCvCropper {
    fn extract_target_text_image(
        &self,
        frame_width: u32,
        frame_height: u32,
        frame_bytes: &[u8],
    ) -> OcrImage {
        let channels = self.detect_channels(frame_width, frame_height, frame_bytes);

        match self.crop_region(
            frame_width,
            frame_height,
            frame_bytes,
            channels,
            &self.ocr_config,
            0,
        ) {
            Some((rgb_bytes, w, h)) => {
                self.save_debug_crop(TARGET_TEXT_DEBUG_FILENAME, w, h, &rgb_bytes);

                OcrImage {
                    width: w,
                    height: h,
                    rgb_bytes,
                }
            }
            None => {
                self.clear_debug_crop(TARGET_TEXT_DEBUG_FILENAME);

                OcrImage {
                    width: 0,
                    height: 0,
                    rgb_bytes: Vec::new(),
                }
            }
        }
    }

    fn extract_battle_result_text_image(
        &self,
        frame_width: u32,
        frame_height: u32,
        frame_bytes: &[u8],
    ) -> OcrImage {
        let channels = self.detect_channels(frame_width, frame_height, frame_bytes);

        match self.crop_region(
            frame_width,
            frame_height,
            frame_bytes,
            channels,
            &self.battle_result_config,
            0,
        ) {
            Some((rgb_bytes, w, h)) => {
                self.save_debug_crop(BATTLE_RESULT_DEBUG_FILENAME, w, h, &rgb_bytes);

                OcrImage {
                    width: w,
                    height: h,
                    rgb_bytes,
                }
            }
            None => {
                self.clear_debug_crop(BATTLE_RESULT_DEBUG_FILENAME);

                OcrImage {
                    width: 0,
                    height: 0,
                    rgb_bytes: Vec::new(),
                }
            }
        }
    }

    fn extract_party_slots(
        &self,
        frame_width: u32,
        frame_height: u32,
        frame_bytes: &[u8],
    ) -> PartyImageSet {
        let channels = self.detect_channels(frame_width, frame_height, frame_bytes);
        let mut slots = Vec::with_capacity(6);

        for i in 0..6u8 {
            let file_name = opponent_slot_debug_filename(i as usize);

            if let Some((rgb_bytes, w, h)) = self.crop_region(
                frame_width,
                frame_height,
                frame_bytes,
                channels,
                &self.opponent_config,
                i as usize,
            ) {
                self.save_debug_crop(&file_name, w, h, &rgb_bytes);
                slots.push(SlotImage {
                    slot: SelectionSlot(i),
                    width: w,
                    height: h,
                    rgb_bytes,
                });
            } else {
                self.clear_debug_crop(&file_name);
            }
        }

        PartyImageSet { slots }
    }
}

fn opponent_slot_debug_filename(slot_index: usize) -> String {
    format!("latest_crop_opponent_slot_{}.png", slot_index + 1)
}

fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        BATTLE_RESULT_DEBUG_FILENAME, OpenCvCropper, TARGET_TEXT_DEBUG_FILENAME,
        opponent_slot_debug_filename,
    };
    use champions_application::RecognitionImageExtractor;

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "champions-agent-cropper-{name}-{}-{unique}",
                std::process::id()
            ));

            fs::create_dir_all(&path).expect("failed to create test output dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn extract_target_text_image_saves_debug_crop_png() {
        let output_dir = TestDir::new("selection");
        let cropper = OpenCvCropper::with_debug_output_dir(output_dir.path());
        let frame = sample_rgb_frame(100, 100);

        let image = cropper.extract_target_text_image(100, 100, &frame);
        let path = output_dir.path().join(TARGET_TEXT_DEBUG_FILENAME);

        assert!(path.exists(), "expected {}", path.display());
        assert_eq!(
            image::image_dimensions(&path).expect("failed to read saved png"),
            (image.width, image.height)
        );
    }

    #[test]
    fn extract_battle_result_text_image_saves_debug_crop_png() {
        let output_dir = TestDir::new("battle-result");
        let cropper = OpenCvCropper::with_debug_output_dir(output_dir.path());
        let frame = sample_rgb_frame(100, 100);

        let image = cropper.extract_battle_result_text_image(100, 100, &frame);
        let path = output_dir.path().join(BATTLE_RESULT_DEBUG_FILENAME);

        assert!(path.exists(), "expected {}", path.display());
        assert_eq!(
            image::image_dimensions(&path).expect("failed to read saved png"),
            (image.width, image.height)
        );
    }

    #[test]
    fn extract_party_slots_saves_each_slot_debug_crop_png() {
        let output_dir = TestDir::new("opponent-slots");
        let cropper = OpenCvCropper::with_debug_output_dir(output_dir.path());
        let frame = sample_rgb_frame(100, 100);

        let party = cropper.extract_party_slots(100, 100, &frame);

        assert_eq!(party.slots.len(), 6);

        for slot_index in 0..6 {
            let path = output_dir
                .path()
                .join(opponent_slot_debug_filename(slot_index));
            assert!(path.exists(), "expected {}", path.display());
        }
    }

    #[test]
    fn empty_target_crop_removes_stale_debug_png() {
        let output_dir = TestDir::new("stale-cleanup");
        let cropper = OpenCvCropper::with_debug_output_dir(output_dir.path());
        let stale_path = output_dir.path().join(TARGET_TEXT_DEBUG_FILENAME);
        fs::write(&stale_path, b"stale").expect("failed to seed stale file");

        let image = cropper.extract_target_text_image(0, 0, &[]);

        assert_eq!(image.width, 0);
        assert_eq!(image.height, 0);
        assert!(
            !stale_path.exists(),
            "stale debug crop should be removed: {}",
            stale_path.display()
        );
    }

    fn sample_rgb_frame(width: u32, height: u32) -> Vec<u8> {
        let pixel_count = width as usize * height as usize;
        let mut frame = Vec::with_capacity(pixel_count * 3);

        for idx in 0..pixel_count {
            frame.push((idx % 251) as u8);
            frame.push(((idx + 53) % 251) as u8);
            frame.push(((idx + 101) % 251) as u8);
        }

        frame
    }
}
