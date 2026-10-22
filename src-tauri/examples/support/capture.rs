//! Test-only OS-composited frame capture. The harness first covers the area with its white fixture.
use windows::Win32::{Foundation::HWND, Graphics::Gdi::*, UI::WindowsAndMessaging::GetWindowRect};

pub unsafe fn save(window: HWND, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut rect = windows::Win32::Foundation::RECT::default();
    GetWindowRect(window, &mut rect)?;
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
        return Err("Invalid fixture dimensions".into());
    }
    struct Resources {
        screen: HDC,
        dc: HDC,
        bitmap: Option<HBITMAP>,
        previous: Option<HGDIOBJ>,
    }
    impl Drop for Resources {
        fn drop(&mut self) {
            unsafe {
                if let Some(previous) = self.previous {
                    SelectObject(self.dc, previous);
                }
                if let Some(bitmap) = self.bitmap {
                    let _ = DeleteObject(HGDIOBJ(bitmap.0));
                }
                let _ = DeleteDC(self.dc);
                ReleaseDC(None, self.screen);
            }
        }
    }
    let screen = GetDC(None);
    let dc = CreateCompatibleDC(Some(screen));
    let mut resources = Resources {
        screen,
        dc,
        bitmap: None,
        previous: None,
    };
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let bitmap = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
    resources.bitmap = Some(bitmap);
    resources.previous = Some(SelectObject(dc, HGDIOBJ(bitmap.0)));
    BitBlt(
        dc,
        0,
        0,
        width,
        height,
        Some(screen),
        rect.left,
        rect.top,
        SRCCOPY | CAPTUREBLT,
    )?;
    let mut rgba =
        std::slice::from_raw_parts(bits.cast::<u8>(), width as usize * height as usize * 4)
            .to_vec();
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    let mut encoder = png::Encoder::new(std::fs::File::create(path)?, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgba)?;
    Ok(())
}
