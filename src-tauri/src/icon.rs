use opentoken_core::model::Provider;
use tauri::image::Image;
mod badge;
mod text;

const DIGITS: [[u8; 5]; 10] = [
    [7, 5, 5, 5, 7],
    [2, 6, 2, 2, 7],
    [7, 1, 7, 4, 7],
    [7, 1, 7, 1, 7],
    [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7],
    [7, 4, 7, 5, 7],
    [7, 1, 2, 2, 2],
    [7, 5, 7, 5, 7],
    [7, 5, 7, 1, 7],
];
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Glyph {
    Badge {
        primary: Option<u8>,
        error: bool,
        stale: bool,
    },
    Provider(Provider),
    Usage {
        primary: Option<u8>,
        secondary: Option<u8>,
        dual: bool,
        error: bool,
        stale: bool,
    },
    App,
}
pub fn cursor_quota(
    report: &opentoken_core::model::Report,
) -> Option<&opentoken_core::model::Quota> {
    if report.source == opentoken_core::model::ReportSource::CursorDashboard {
        // Match the dashboard summary: prefer the reported aggregate, not the
        // most exhausted independent Auto/API bucket. Never fabricate a total.
        return ["cursor_monthly", "cursor_api", "cursor_auto"]
            .iter()
            .find_map(|id| {
                report
                    .quotas
                    .iter()
                    .find(|q| q.id == *id && q.percent.is_some_and(f64::is_finite))
            });
    }
    report
        .quotas
        .iter()
        .filter(|q| q.percent.is_some_and(f64::is_finite))
        .max_by(|a, b| a.percent.unwrap().total_cmp(&b.percent.unwrap()))
}
pub fn remaining(used: f64) -> u8 {
    (100.0 - used).round().clamp(0.0, 100.0) as u8
}

pub fn render(glyph: Glyph, light_taskbar: bool) -> Image<'static> {
    if let Glyph::Badge {
        primary,
        error,
        stale,
    } = glyph
    {
        return badge::render(primary, error, stale, light_taskbar);
    }
    if glyph == Glyph::App {
        return Image::from_bytes(include_bytes!("../icons/32x32.png"))
            .expect("bundled application icon");
    }
    let foreground = if light_taskbar {
        [25, 25, 25]
    } else {
        [250, 250, 250]
    };
    if let Glyph::Provider(provider) = glyph {
        let bytes: &[u8] = match provider {
            Provider::Codex => include_bytes!("../icons/openai-tray.png"),
            Provider::Claude => include_bytes!("../icons/claude-tray.png"),
            Provider::Cursor => include_bytes!("../icons/cursor-tray.png"),
            Provider::Copilot => include_bytes!("../icons/copilot-tray.png"),
            Provider::Zai => include_bytes!("../icons/zai-tray.png"),
            Provider::Deepseek => include_bytes!("../icons/deepseek-tray.png"),
            Provider::Kimi => include_bytes!("../icons/kimi-tray.png"),
            Provider::Opencode => include_bytes!("../icons/opencode-tray.png"),
            Provider::Grok => include_bytes!("../icons/grok-tray.png"),
            Provider::Antigravity => include_bytes!("../icons/antigravity-tray.png"),
            Provider::Muse => include_bytes!("../icons/muse-tray.png"),
            Provider::Devin => include_bytes!("../icons/devin-tray.png"),
        };
        let image = Image::from_bytes(bytes).expect("bundled provider icon");
        let mut pixels = image.rgba().to_vec();
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel[..3].copy_from_slice(&foreground);
        }
        return Image::new_owned(pixels, image.width(), image.height());
    }
    let Glyph::Usage {
        primary,
        secondary,
        dual,
        error,
        stale,
    } = glyph
    else {
        unreachable!()
    };
    let color = if error {
        [196, 43, 28, 255]
    } else {
        [
            foreground[0],
            foreground[1],
            foreground[2],
            if stale { 150 } else { 255 },
        ]
    };
    let mut rgba = vec![0; 32 * 32 * 4];
    let text = |value: Option<u8>| {
        if error {
            "!".into()
        } else {
            value.map_or_else(|| "-".into(), |v| format!("{v}%"))
        }
    };
    draw_text(&mut rgba, &text(primary), if dual { 2 } else { 10 }, color);
    if dual {
        draw_text(&mut rgba, &text(secondary), 18, color);
    }
    Image::new_owned(rgba, 32, 32)
}
fn draw_text(rgba: &mut [u8], text: &str, top: usize, color: [u8; 4]) {
    let scale = 2;
    let width = text.len() * 4 * scale - scale;
    let left = (32 - width) / 2;
    for (i, ch) in text.chars().enumerate() {
        let pattern = match ch.to_digit(10) {
            Some(n) => DIGITS[n as usize],
            None => match ch {
                '%' => [5, 1, 2, 4, 5],
                '!' => [2, 2, 2, 0, 2],
                _ => [0, 0, 7, 0, 0],
            },
        };
        for (row, bits) in pattern.iter().enumerate() {
            for col in 0..3 {
                if bits & (1 << (2 - col)) != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let x = left + i * 4 * scale + col * scale + dx;
                            let y = top + row * scale + dy;
                            let offset = (y * 32 + x) * 4;
                            rgba[offset..offset + 4].copy_from_slice(&color);
                        }
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fluent_badges_are_valid_for_every_percentage_and_status() {
        for primary in (0..=100).map(Some).chain([None]) {
            for light in [true, false] {
                for (error, stale) in [(false, false), (true, false), (false, true)] {
                    let image = render(
                        Glyph::Badge {
                            primary,
                            error,
                            stale,
                        },
                        light,
                    );
                    assert_eq!(image.rgba().len(), 64 * 64 * 4);
                    assert_eq!(image.rgba()[3], 0);
                    assert!(image
                        .rgba()
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|p| p[3] > 0 && p[3] < 255));
                }
            }
        }
        // Optional design artifact, never used by the production executable.
        if let Some(dir) = std::env::var_os("OPENTOKEN_ICON_PREVIEW_DIR") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            for light in [true, false] {
                for primary in [None, Some(0), Some(8), Some(29), Some(72), Some(100)] {
                    let image = render(
                        Glyph::Badge {
                            primary,
                            error: false,
                            stale: false,
                        },
                        light,
                    );
                    let path = dir.join(format!(
                        "badge-{}-{}.png",
                        if light { "light" } else { "dark" },
                        primary.map_or_else(|| "unknown".into(), |p| p.to_string())
                    ));
                    let mut encoder = png::Encoder::new(
                        std::fs::File::create(path).unwrap(),
                        image.width(),
                        image.height(),
                    );
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    encoder
                        .write_header()
                        .unwrap()
                        .write_image_data(image.rgba())
                        .unwrap();
                }
            }
        }
    }
    #[test]
    fn personal_cursor_badge_uses_reported_monthly_total_and_explicit_bucket_fallbacks() {
        use opentoken_core::model::{Quota, Report, ReportSource};
        let mut report = Report {
            plan: Some("Pro".into()),
            source: ReportSource::CursorDashboard,
            verified_at: None,
            observed_at: chrono::Utc::now(),
            quotas: vec![
                Quota::window("cursor_monthly", "Monthly plan", 25.0, None),
                Quota::window("cursor_auto", "Auto", 100.0, None),
                Quota::window("cursor_api", "API", 42.0, None),
            ],
        };
        let q = cursor_quota(&report).unwrap();
        assert_eq!(q.id, "cursor_monthly");
        assert_eq!(remaining(q.percent.unwrap()), 75);
        report.quotas[0].percent = Some(64.27777777777777);
        assert_eq!(
            remaining(cursor_quota(&report).unwrap().percent.unwrap()),
            36
        );
        report.quotas[0].percent = Some(100.0);
        assert_eq!(
            remaining(cursor_quota(&report).unwrap().percent.unwrap()),
            0
        );
        report.quotas[0].percent = Some(f64::NAN);
        assert_eq!(cursor_quota(&report).unwrap().id, "cursor_api");
        assert_eq!(
            remaining(cursor_quota(&report).unwrap().percent.unwrap()),
            58
        );
        report.quotas.retain(|q| q.id == "cursor_auto");
        assert_eq!(
            remaining(cursor_quota(&report).unwrap().percent.unwrap()),
            0
        );
        report.quotas.clear();
        assert!(cursor_quota(&report).is_none());
    }
    #[test]
    fn remaining_agrees_with_dashboard() {
        assert_eq!(remaining(40.0), 60);
        assert_eq!(remaining(0.0), 100);
        assert_eq!(remaining(100.0), 0);
        assert_eq!(remaining(150.0), 0);
    }
    #[test]
    fn icons_are_transparent_and_valid_for_all_values() {
        for primary in [None, Some(0), Some(7), Some(39), Some(100)] {
            for light in [true, false] {
                let glyph = Glyph::Usage {
                    primary,
                    secondary: Some(100),
                    dual: true,
                    error: false,
                    stale: false,
                };
                let image = render(glyph, light);
                assert_eq!(image.rgba().len(), 32 * 32 * 4);
                assert_eq!(image.rgba()[3], 0, "no dark disc/background");
                assert!(image.rgba().as_chunks::<4>().0.iter().any(|p| p[3] == 255));
            }
        }
    }
    #[test]
    fn providers_have_distinct_marks_and_adapt_to_taskbar_theme() {
        let providers = [
            Provider::Codex,
            Provider::Claude,
            Provider::Cursor,
            Provider::Copilot,
        ];
        let images: Vec<_> = providers
            .iter()
            .map(|p| render(Glyph::Provider(*p), true))
            .collect();
        for (i, provider) in providers.iter().enumerate() {
            assert_ne!(
                images[i].rgba(),
                render(Glyph::Provider(*provider), false).rgba()
            );
            assert_eq!(images[i].rgba()[3], 0);
            for prior in &images[..i] {
                assert_ne!(prior.rgba(), images[i].rgba());
            }
        }
    }
    #[test]
    fn weekly_changes_update_the_percentage_image() {
        let glyph = |secondary| Glyph::Usage {
            primary: Some(39),
            secondary: Some(secondary),
            dual: true,
            error: false,
            stale: false,
        };
        assert_ne!(
            render(glyph(43), true).rgba(),
            render(glyph(44), true).rgba()
        );
    }
}
