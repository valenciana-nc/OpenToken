use std::f64::consts::TAU;
use tauri::image::Image;
const SIZE: usize = 64;

/// High-resolution Fluent badge: restrained surface, rounded progress arc, native UI type.
pub(super) fn render(value: Option<u8>, error: bool, stale: bool, light: bool) -> Image<'static> {
    let value = value.map(|v| v.min(100));
    let fill = if light {
        [245, 250, 255, 246]
    } else {
        [29, 39, 53, 246]
    };
    let mut accent = if light {
        [0, 103, 192, 255]
    } else {
        [96, 205, 255, 255]
    };
    if error {
        accent = if light {
            [196, 43, 28, 255]
        } else {
            [255, 153, 164, 255]
        };
    } else if stale || value.is_none() {
        accent = if light {
            [111, 125, 143, 215]
        } else {
            [159, 173, 192, 215]
        };
    } else if value.is_some_and(|v| v <= 10) {
        accent = if light {
            [161, 94, 0, 255]
        } else {
            [255, 194, 91, 255]
        };
    }
    let mut track = accent;
    track[3] = if light { 42 } else { 57 };
    let ink = if error {
        accent
    } else if stale || value.is_none() {
        if light {
            [91, 106, 123, 255]
        } else {
            [171, 185, 204, 255]
        }
    } else if light {
        [23, 43, 62, 255]
    } else {
        [244, 249, 255, 255]
    };
    let label = if error {
        "!".into()
    } else {
        value.map_or_else(|| "—".into(), |v| v.to_string())
    };
    let mask = super::text::mask(&label, SIZE).unwrap_or_else(|| bitmap_mask(&label));
    let sweep = if error {
        TAU
    } else {
        value.unwrap_or(0) as f64 / 100.0 * TAU
    };
    let mut rgba = vec![0; SIZE * SIZE * 4];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f64 + 0.5 - SIZE as f64 / 2.0;
            let dy = y as f64 + 0.5 - SIZE as f64 / 2.0;
            let radius = dx.hypot(dy);
            let offset = (y * SIZE + x) * 4;
            let pixel = &mut rgba[offset..offset + 4];
            over(pixel, fill, (31.0 + 0.5 - radius).clamp(0.0, 1.0));
            over(pixel, track, (2.0 - (radius - 28.5).abs()).clamp(0.0, 1.0));
            if sweep > 0.0 {
                let angle = dx.atan2(-dy).rem_euclid(TAU);
                let distance = if angle <= sweep || sweep >= TAU {
                    (radius - 28.5).abs()
                } else {
                    dx.hypot(dy + 28.5)
                        .min((dx - 28.5 * sweep.sin()).hypot(dy + 28.5 * sweep.cos()))
                };
                over(pixel, accent, (2.0 - distance).clamp(0.0, 1.0));
            }
            over(pixel, ink, mask[y * SIZE + x] as f64 / 255.0);
        }
    }
    Image::new_owned(rgba, SIZE as u32, SIZE as u32)
}
fn over(pixel: &mut [u8], color: [u8; 4], coverage: f64) {
    let source = coverage * color[3] as f64 / 255.0;
    if source == 0.0 {
        return;
    }
    let destination = pixel[3] as f64 / 255.0;
    let alpha = source + destination * (1.0 - source);
    for i in 0..3 {
        pixel[i] = ((color[i] as f64 * source + pixel[i] as f64 * destination * (1.0 - source))
            / alpha)
            .round() as u8;
    }
    pixel[3] = (alpha * 255.0).round() as u8;
}
fn bitmap_mask(text: &str) -> Vec<u8> {
    let mut mask = vec![0; SIZE * SIZE];
    let scale = if text.len() >= 3 { 4 } else { 6 };
    let text = if text == "—" { "-" } else { text };
    let width = text.len() * 4 * scale - scale;
    let left = (SIZE - width) / 2;
    let top = (SIZE - 5 * scale) / 2;
    for (i, ch) in text.chars().enumerate() {
        let pattern = ch
            .to_digit(10)
            .map(|n| super::DIGITS[n as usize])
            .unwrap_or(if ch == '!' {
                [2, 2, 2, 0, 2]
            } else {
                [0, 0, 7, 0, 0]
            });
        for (row, bits) in pattern.iter().enumerate() {
            for col in 0..3 {
                if bits & (1 << (2 - col)) != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            mask[(top + row * scale + dy) * SIZE
                                + left
                                + i * 4 * scale
                                + col * scale
                                + dx] = 255;
                        }
                    }
                }
            }
        }
    }
    mask
}
