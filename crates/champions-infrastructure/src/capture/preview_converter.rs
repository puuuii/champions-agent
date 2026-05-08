use champions_interface::{CapturedFrame, PixelFormat, PreviewFrame};
use champions_runtime::traits::PreviewFrameConverter;

pub struct RgbaPreviewConverter;

impl PreviewFrameConverter for RgbaPreviewConverter {
    fn convert(&self, frame: &CapturedFrame, max_width: u32) -> PreviewFrame {
        let src = &frame.image;
        let (src_w, src_h) = (src.width, src.height);

        let (dst_w, dst_h) = if src_w > max_width {
            let scale = max_width as f64 / src_w as f64;
            let new_h = (src_h as f64 * scale).round() as u32;
            (max_width, new_h)
        } else {
            (src_w, src_h)
        };

        let rgba_full = to_rgba(&src.bytes, src_w, src_h, src.pixel_format);

        let rgba_scaled = if dst_w == src_w && dst_h == src_h {
            rgba_full
        } else {
            nearest_neighbor_scale(&rgba_full, src_w, src_h, dst_w, dst_h)
        };

        PreviewFrame {
            frame_sequence: frame.frame_sequence,
            timestamp_millis: frame.captured_at_millis,
            width: dst_w,
            height: dst_h,
            rgba: rgba_scaled.into(),
        }
    }
}

fn to_rgba(src: &[u8], width: u32, height: u32, format: PixelFormat) -> Vec<u8> {
    let pixel_count = (width as usize) * (height as usize);
    let mut rgba = vec![255u8; pixel_count * 4];

    match format {
        PixelFormat::Bgr8 => {
            for i in 0..pixel_count {
                let si = i * 3;
                let di = i * 4;
                rgba[di] = src[si + 2]; // R
                rgba[di + 1] = src[si + 1]; // G
                rgba[di + 2] = src[si]; // B
                rgba[di + 3] = 255; // A
            }
        }
        PixelFormat::Rgb8 => {
            for i in 0..pixel_count {
                let si = i * 3;
                let di = i * 4;
                rgba[di] = src[si];
                rgba[di + 1] = src[si + 1];
                rgba[di + 2] = src[si + 2];
                rgba[di + 3] = 255;
            }
        }
        PixelFormat::Rgba8 => {
            rgba[..pixel_count * 4].copy_from_slice(&src[..pixel_count * 4]);
        }
        PixelFormat::Gray8 => {
            for (i, &g) in src.iter().enumerate().take(pixel_count) {
                let di = i * 4;
                rgba[di] = g;
                rgba[di + 1] = g;
                rgba[di + 2] = g;
                rgba[di + 3] = 255;
            }
        }
    }

    rgba
}

fn nearest_neighbor_scale(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut dst = vec![0u8; (dst_w as usize) * (dst_h as usize) * 4];
    let x_ratio = src_w as f64 / dst_w as f64;
    let y_ratio = src_h as f64 / dst_h as f64;

    for y in 0..dst_h {
        let src_y = (y as f64 * y_ratio) as u32;
        let src_y = src_y.min(src_h - 1);
        for x in 0..dst_w {
            let src_x = (x as f64 * x_ratio) as u32;
            let src_x = src_x.min(src_w - 1);

            let si = ((src_y * src_w + src_x) as usize) * 4;
            let di = ((y * dst_w + x) as usize) * 4;

            dst[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }

    dst
}
