use std::{fs, path::Path};

use champions_application::{OcrImage, PartyImageSet, RecognitionImageExtractor, SlotImage};
use champions_domain::recognition::SelectionSlot;
use image::RgbImage;

#[derive(Debug, Clone)]
pub struct CropConfig {
    pub center_x: f64,
    pub y_start: f64,
    pub y_gap: f64,
    pub size_w: f64,
    pub width_ratio: f64,
}

#[derive(Debug, Clone, Copy)]
struct CropRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

pub struct OpenCvCropper {
    opponent_config: CropConfig,
    ocr_config: CropConfig,
    battle_result_config: CropConfig,
    save_debug_party_slots: bool,
}

impl OpenCvCropper {
    pub fn new() -> Self {
        Self::with_debug_party_slot_dump(false)
    }

    pub fn with_debug_party_slot_dump(save_debug_party_slots: bool) -> Self {
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
            save_debug_party_slots,
        }
    }

    fn compute_crop_rect(
        &self,
        frame_width: u32,
        frame_height: u32,
        config: &CropConfig,
        index: usize,
    ) -> Option<CropRect> {
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

        Some(CropRect {
            x: x1,
            y: y1,
            width: crop_w,
            height: crop_h,
        })
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
        let rect = self.compute_crop_rect(frame_width, frame_height, config, index)?;

        let stride = frame_width as usize * channels;
        let mut rgb_bytes = Vec::with_capacity((rect.width * rect.height * 3) as usize);

        for row in 0..rect.height {
            let src_y = (rect.y + row) as usize;
            for col in 0..rect.width {
                let src_x = (rect.x + col) as usize;
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

        Some((rgb_bytes, rect.width, rect.height))
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

    fn save_party_slot_debug_image(
        &self,
        slot_index: usize,
        width: u32,
        height: u32,
        rgb_bytes: &[u8],
    ) {
        if !self.save_debug_party_slots {
            return;
        }

        let output_dir = Path::new("tmp");
        if let Err(error) = fs::create_dir_all(output_dir) {
            tracing::warn!(
                slot = slot_index + 1,
                path = %output_dir.display(),
                %error,
                "failed to create opponent crop debug directory",
            );
            return;
        }

        let Some(image) = RgbImage::from_raw(width, height, rgb_bytes.to_vec()) else {
            tracing::warn!(
                slot = slot_index + 1,
                width,
                height,
                bytes = rgb_bytes.len(),
                "failed to build opponent crop debug image",
            );
            return;
        };

        let path = output_dir.join(format!("opp_poke{}.png", slot_index + 1));
        if let Err(error) = image.save(&path) {
            tracing::warn!(
                slot = slot_index + 1,
                path = %path.display(),
                %error,
                "failed to save opponent crop debug image",
            );
        }
    }

    fn log_party_slot_debug(
        &self,
        slot_index: usize,
        frame_width: u32,
        frame_height: u32,
        rect: CropRect,
    ) {
        if !self.save_debug_party_slots {
            return;
        }

        tracing::info!(
            slot = slot_index + 1,
            frame_width,
            frame_height,
            x1 = rect.x,
            y1 = rect.y,
            crop_width = rect.width,
            crop_height = rect.height,
            "opponent crop debug",
        );
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
            Some((rgb_bytes, w, h)) => OcrImage {
                width: w,
                height: h,
                rgb_bytes,
            },
            None => OcrImage {
                width: 0,
                height: 0,
                rgb_bytes: Vec::new(),
            },
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
            Some((rgb_bytes, w, h)) => OcrImage {
                width: w,
                height: h,
                rgb_bytes,
            },
            None => OcrImage {
                width: 0,
                height: 0,
                rgb_bytes: Vec::new(),
            },
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
            let slot_index = i as usize;
            let Some(rect) = self.compute_crop_rect(
                frame_width,
                frame_height,
                &self.opponent_config,
                slot_index,
            ) else {
                continue;
            };

            self.log_party_slot_debug(slot_index, frame_width, frame_height, rect);

            if let Some((rgb_bytes, w, h)) = self.crop_region(
                frame_width,
                frame_height,
                frame_bytes,
                channels,
                &self.opponent_config,
                slot_index,
            ) {
                self.save_party_slot_debug_image(slot_index, w, h, &rgb_bytes);
                slots.push(SlotImage {
                    slot: SelectionSlot(i),
                    width: w,
                    height: h,
                    rgb_bytes,
                });
            }
        }

        PartyImageSet { slots }
    }
}
