/// Physical desktop coordinates, including negative origins on secondary monitors.
#[derive(Clone, Copy)]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub fn position(
    area: WorkArea,
    size: (u32, u32),
    mode: &str,
    anchor: Option<(i32, i32)>,
    margin: i32,
) -> (i32, i32) {
    let max_x = (area.x + area.width as i32 - size.0 as i32 - margin).max(area.x);
    let max_y = (area.y + area.height as i32 - size.1 as i32 - margin).max(area.y);
    let left = (area.x + margin).min(max_x);
    match (mode, anchor) {
        ("bottom_right", _) => (max_x, max_y),
        ("tray", Some((x, y))) => {
            let above = y - size.1 as i32 - margin;
            let y = if above < area.y {
                y + margin * 2
            } else {
                above
            };
            (
                (x - size.0 as i32 + margin * 2).clamp(area.x, max_x),
                y.clamp(area.y, max_y),
            )
        }
        ("tray", None) => (max_x, max_y),
        _ => (left, max_y),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bottom_left_stays_above_taskbar() {
        assert_eq!(
            position(
                WorkArea {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1152
                },
                (440, 690),
                "bottom_left",
                None,
                10
            ),
            (10, 452)
        );
    }
    #[test]
    fn old_bottom_right_is_available() {
        assert_eq!(
            position(
                WorkArea {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1152
                },
                (440, 690),
                "bottom_right",
                None,
                10
            ),
            (1470, 452)
        );
    }
    #[test]
    fn secondary_monitor_and_small_work_area() {
        let area = WorkArea {
            x: -1280,
            y: 0,
            width: 1280,
            height: 720,
        };
        let p = position(area, (440, 690), "bottom_left", None, 10);
        assert_eq!(p, (-1270, 20));
        assert_eq!(
            position(area, (2000, 1000), "bottom_left", None, 10),
            (-1280, 0)
        );
    }
    #[test]
    fn tray_anchor_is_clamped_within_work_area() {
        let p = position(
            WorkArea {
                x: 0,
                y: 40,
                width: 800,
                height: 560,
            },
            (440, 500),
            "tray",
            Some((5, 40)),
            10,
        );
        assert_eq!(p, (0, 60));
    }
}
