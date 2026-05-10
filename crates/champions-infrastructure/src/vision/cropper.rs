use champions_application::{OcrImage, PartyImageSet, RecognitionImageExtractor, SlotImage};
use champions_domain::recognition::SelectionSlot;

const BATTLE_RESULT_DEBUG_CAPTURE_PATH: &str = "capture.png";

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
}

impl OpenCvCropper {
    pub fn new() -> Self {
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

    fn write_battle_result_debug_capture(&self, image: &OcrImage) {
        if image.width == 0 || image.height == 0 || image.rgb_bytes.is_empty() {
            tracing::warn!("battle result debug capture skipped: extracted image is empty");
            return;
        }

        let Some(rgb_image) =
            image::RgbImage::from_raw(image.width, image.height, image.rgb_bytes.clone())
        else {
            tracing::warn!(
                width = image.width,
                height = image.height,
                bytes = image.rgb_bytes.len(),
                "battle result debug capture skipped: invalid RGB buffer"
            );
            return;
        };

        if let Err(error) = rgb_image.save(BATTLE_RESULT_DEBUG_CAPTURE_PATH) {
            tracing::warn!(
                path = BATTLE_RESULT_DEBUG_CAPTURE_PATH,
                "battle result debug capture save failed: {error}"
            );
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
            Some((rgb_bytes, w, h)) => {
                let image = OcrImage {
                    width: w,
                    height: h,
                    rgb_bytes,
                };
                self.write_battle_result_debug_capture(&image);
                image
            }
            None => {
                tracing::warn!("battle result debug capture skipped: crop region is empty");
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
            if let Some((rgb_bytes, w, h)) = self.crop_region(
                frame_width,
                frame_height,
                frame_bytes,
                channels,
                &self.opponent_config,
                i as usize,
            ) {
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
