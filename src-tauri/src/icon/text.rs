//! Rasterize the installed Windows UI font, without bundling/extracting font files.
//! All DCs, bitmaps, fonts and selections are scoped RAII resources.

#[cfg(windows)]
pub(super) fn mask(text: &str, size: usize) -> Option<Vec<u8>> {
    use std::{mem::size_of, ptr};
    use windows::{
        core::w,
        Win32::{
            Foundation::{COLORREF, RECT},
            Graphics::Gdi::*,
        },
    };
    struct Dc(HDC);
    impl Drop for Dc {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteDC(self.0);
            }
        }
    }
    struct Object(HGDIOBJ);
    impl Drop for Object {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteObject(self.0);
            }
        }
    }
    struct Selection(HDC, HGDIOBJ);
    impl Drop for Selection {
        fn drop(&mut self) {
            unsafe {
                let _ = SelectObject(self.0, self.1);
            }
        }
    }
    // SAFETY: all GDI operations use this private memory DC. Selections are restored
    // before their objects are deleted. The DIB slice is read only while it is alive,
    // after GdiFlush, and exactly matches its validated dimensions/32-bit format.
    unsafe {
        let dc = Dc(CreateCompatibleDC(None));
        if dc.0.is_invalid() {
            return None;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size as i32,
                biHeight: -(size as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = ptr::null_mut();
        let bitmap = Object(
            CreateDIBSection(Some(dc.0), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                .ok()?
                .into(),
        );
        if bits.is_null() {
            return None;
        }
        let previous_bitmap = SelectObject(dc.0, bitmap.0);
        if previous_bitmap.is_invalid() {
            return None;
        }
        let _selected_bitmap = Selection(dc.0, previous_bitmap);
        let height = if text.len() >= 3 { 28 } else { 36 };
        let font = CreateFontW(
            -height,
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            w!("Segoe UI"),
        );
        if font.is_invalid() {
            return None;
        }
        let font = Object(font.into());
        let previous_font = SelectObject(dc.0, font.0);
        if previous_font.is_invalid() {
            return None;
        }
        let _selected_font = Selection(dc.0, previous_font);
        ptr::write_bytes(bits.cast::<u8>(), 0, size * size * 4);
        SetBkMode(dc.0, TRANSPARENT);
        SetTextColor(dc.0, COLORREF(0x00ff_ffff));
        let mut utf16: Vec<_> = text.encode_utf16().collect();
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: size as i32,
            bottom: size as i32,
        };
        if DrawTextW(
            dc.0,
            &mut utf16,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        ) == 0
        {
            return None;
        }
        let _ = GdiFlush();
        let pixels = std::slice::from_raw_parts(bits.cast::<u8>(), size * size * 4);
        Some(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[0].max(p[1]).max(p[2]))
                .collect(),
        )
    }
}
#[cfg(not(windows))]
pub(super) fn mask(_text: &str, _size: usize) -> Option<Vec<u8>> {
    None
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn windows_type_is_antialiased_centered_and_not_clipped() {
        for text in ["0", "8", "29", "72", "100", "!"] {
            let mask = super::mask(text, 64).expect("Windows UI font rasterization");
            assert_eq!(mask.len(), 64 * 64);
            assert!(
                mask.iter().any(|a| *a > 0 && *a < 255),
                "font must be antialiased"
            );
            assert!(mask[..64].iter().all(|a| *a == 0));
            assert!(mask[63 * 64..].iter().all(|a| *a == 0));
            for y in 0..64 {
                assert_eq!(mask[y * 64], 0);
                assert_eq!(mask[y * 64 + 63], 0);
            }
        }
    }
}
