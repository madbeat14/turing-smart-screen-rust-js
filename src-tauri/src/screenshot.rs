/// Webview screenshot capture using Win32 PrintWindow API.
///
/// Captures the Tauri webview window's client area to an RGBA pixel buffer,
/// resized to the display's native resolution.
///
/// Uses PrintWindow with PW_CLIENTONLY | PW_RENDERFULLCONTENT flags to:
/// - PW_CLIENTONLY (0x1): capture only the client area (no title bar/borders)
/// - PW_RENDERFULLCONTENT (0x2): capture composited/DirectX content (WebView2)
///
/// This works even when the window is off-screen (positioned at -9999,-9999)
/// because PrintWindow asks the window to paint itself, not a screen capture.
use anyhow::{anyhow, Result};
use image::{imageops::FilterType, ImageBuffer, Rgba};

#[cfg(target_os = "windows")]
use std::mem;

/// Capture a screenshot of the webview and return resized RGBA pixels.
pub fn capture_webview_screenshot(
    _window: &tauri::WebviewWindow,
    target_w: u32,
    target_h: u32,
) -> Result<Vec<u8>> {
    #[cfg(target_os = "windows")]
    {
        capture_win32(_window, target_w, target_h)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (_window, target_w, target_h);
        log::warn!("Screenshot capture not implemented for this platform, using placeholder");
        Ok(vec![0u8; (target_w * target_h * 4) as usize])
    }
}

/// RAII Guard to ensure all Win32 GDI handles are released even on early returns or errors.
#[cfg(target_os = "windows")]
struct GdiContextGuard {
    hwnd: windows::Win32::Foundation::HWND,
    hdc_window: windows::Win32::Graphics::Gdi::HDC,
    hdc_mem: windows::Win32::Graphics::Gdi::HDC,
    hbmp: windows::Win32::Graphics::Gdi::HBITMAP,
    old_bmp: windows::Win32::Graphics::Gdi::HGDIOBJ,
}

#[cfg(target_os = "windows")]
impl Drop for GdiContextGuard {
    fn drop(&mut self) {
        unsafe {
            use windows::Win32::Graphics::Gdi::{DeleteDC, DeleteObject, ReleaseDC, SelectObject};
            if !self.old_bmp.is_invalid() && !self.hdc_mem.is_invalid() {
                SelectObject(self.hdc_mem, self.old_bmp);
            }
            if !self.hbmp.is_invalid() {
                let _ = DeleteObject(self.hbmp.into());
            }
            if !self.hdc_mem.is_invalid() {
                let _ = DeleteDC(self.hdc_mem);
            }
            if !self.hdc_window.is_invalid() {
                ReleaseDC(Some(self.hwnd), self.hdc_window);
            }
        }
    }
}

/// Win32 screenshot: capture client area only (no window chrome).
#[cfg(target_os = "windows")]
fn capture_win32(window: &tauri::WebviewWindow, target_w: u32, target_h: u32) -> Result<Vec<u8>> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, GetDC, GetDIBits,
        SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    let raw_hwnd = window
        .hwnd()
        .map_err(|e| anyhow!("Failed to get HWND: {}", e))?;
    let hwnd = HWND(raw_hwnd.0 as *mut _);

    unsafe {
        // Get client area dimensions
        let mut client_rect = mem::zeroed::<windows::Win32::Foundation::RECT>();
        let _ = GetClientRect(hwnd, &mut client_rect);
        let win_w = (client_rect.right - client_rect.left) as u32;
        let win_h = (client_rect.bottom - client_rect.top) as u32;

        if win_w == 0 || win_h == 0 {
            return Err(anyhow!(
                "Window has zero client dimensions ({}x{})",
                win_w,
                win_h
            ));
        }

        let hdc_window = GetDC(Some(hwnd));
        if hdc_window.is_invalid() {
            return Err(anyhow!("GetDC failed"));
        }

        let hdc_mem = CreateCompatibleDC(Some(hdc_window));
        if hdc_mem.is_invalid() {
            use windows::Win32::Graphics::Gdi::ReleaseDC;
            ReleaseDC(Some(hwnd), hdc_window);
            return Err(anyhow!("CreateCompatibleDC failed"));
        }

        let hbmp = CreateCompatibleBitmap(hdc_window, win_w as i32, win_h as i32);
        if hbmp.is_invalid() {
            use windows::Win32::Graphics::Gdi::{DeleteDC, ReleaseDC};
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(Some(hwnd), hdc_window);
            return Err(anyhow!("CreateCompatibleBitmap failed"));
        }

        let old_bmp = SelectObject(hdc_mem, hbmp.into());

        // Establish RAII guard for the allocated GDI resources
        let _guard = GdiContextGuard {
            hwnd,
            hdc_window,
            hdc_mem,
            hbmp,
            old_bmp,
        };

        // PW_CLIENTONLY (0x1) | PW_RENDERFULLCONTENT (0x2) = 0x3
        // Captures only the client area with composited DirectX/WebView2 content.
        let pw_ok = PrintWindow(hwnd, hdc_mem, PRINT_WINDOW_FLAGS(0x3));
        if !pw_ok.as_bool() {
            log::warn!("PrintWindow with PW_RENDERFULLCONTENT failed, trying PW_CLIENTONLY only");
            let _ = PrintWindow(hwnd, hdc_mem, PRINT_WINDOW_FLAGS(0x1));
        }

        // Read bitmap pixels as BGRA
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: win_w as i32,
                biHeight: -(win_h as i32), // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..mem::zeroed()
            },
            ..mem::zeroed()
        };

        let mut pixels = vec![0u8; (win_w * win_h * 4) as usize];
        GetDIBits(
            hdc_mem,
            hbmp,
            0,
            win_h,
            Some(pixels.as_mut_ptr().cast()),
            &bmi as *const _ as *mut _,
            DIB_RGB_COLORS,
        );

        // Convert BGRA → RGBA
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }

        // Resize to target display resolution if needed
        if win_w == target_w && win_h == target_h {
            Ok(pixels)
        } else {
            let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
                ImageBuffer::from_raw(win_w, win_h, pixels)
                    .ok_or_else(|| anyhow!("Failed to create image buffer"))?;

            let resized = image::imageops::resize(&img, target_w, target_h, FilterType::Triangle);
            Ok(resized.into_raw())
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_pixel_buffer_calculation() {
        let w = 480u32;
        let h = 320u32;
        let expected_len = (w * h * 4) as usize;
        assert_eq!(expected_len, 614400);
    }

    #[test]
    fn test_bgra_to_rgba_swap() {
        let mut pixels = vec![255u8, 128, 64, 255]; // B=255, G=128, R=64, A=255
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        assert_eq!(pixels, vec![64u8, 128, 255, 255]); // R=64, G=128, B=255, A=255
    }
}
