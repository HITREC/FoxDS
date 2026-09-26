#![allow(dead_code)]

use std::io::Write;
use std::process::{Command, Stdio};

/// Capture a rectangular region of the screen on Windows using GDI BitBlt and encode as BMP in memory
#[cfg(windows)]
pub fn capture_screen_rect_bmp(x: i32, y: i32, w: i32, h: i32) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Graphics::Gdi::*;

    if w <= 0 || h <= 0 {
        return Err("Invalid rectangle dimensions".to_string());
    }

    unsafe {
        let hdc_screen = GetDC(std::ptr::null_mut());
        if hdc_screen.is_null() {
            return Err("Failed to get desktop device context".to_string());
        }

        let hdc_mem = CreateCompatibleDC(hdc_screen);
        if hdc_mem.is_null() {
            ReleaseDC(std::ptr::null_mut(), hdc_screen);
            return Err("Failed to create compatible DC".to_string());
        }

        let hbm = CreateCompatibleBitmap(hdc_screen, w, h);
        if hbm.is_null() {
            DeleteDC(hdc_mem);
            ReleaseDC(std::ptr::null_mut(), hdc_screen);
            return Err("Failed to create compatible bitmap".to_string());
        }

        let old_bm = SelectObject(hdc_mem, hbm);
        BitBlt(hdc_mem, 0, 0, w, h, hdc_screen, x, y, SRCCOPY);
        SelectObject(hdc_mem, old_bm);

        let row_size = ((w * 3 + 3) / 4) * 4; // 24-bit alignment to 4 bytes
        let image_size = (row_size * h) as usize;

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = h; // bottom-up BMP
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 24;
        bmi.bmiHeader.biCompression = BI_RGB;

        let mut pixel_data = vec![0u8; image_size];
        GetDIBits(
            hdc_mem,
            hbm,
            0,
            h as u32,
            pixel_data.as_mut_ptr() as *mut _,
            &mut bmi,
            DIB_RGB_COLORS,
        );

        DeleteObject(hbm);
        DeleteDC(hdc_mem);
        ReleaseDC(std::ptr::null_mut(), hdc_screen);

        // Build standard BMP file in memory
        let file_header_size = 14u32;
        let info_header_size = 40u32;
        let total_file_size = file_header_size + info_header_size + image_size as u32;

        let mut bmp = Vec::with_capacity(total_file_size as usize);

        // 1. BITMAPFILEHEADER
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&total_file_size.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes()); // Reserved
        let offset = file_header_size + info_header_size;
        bmp.extend_from_slice(&offset.to_le_bytes());

        // 2. BITMAPINFOHEADER
        bmp.extend_from_slice(&info_header_size.to_le_bytes());
        bmp.extend_from_slice(&w.to_le_bytes());
        bmp.extend_from_slice(&h.to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
        bmp.extend_from_slice(&24u16.to_le_bytes()); // biBitCount
        bmp.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
        bmp.extend_from_slice(&(image_size as u32).to_le_bytes());
        bmp.extend_from_slice(&2835i32.to_le_bytes()); // 72 DPI
        bmp.extend_from_slice(&2835i32.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());

        // 3. Pixel Data
        bmp.extend_from_slice(&pixel_data);

        Ok(bmp)
    }
}

#[cfg(not(windows))]
pub fn capture_screen_rect_bmp(_x: i32, _y: i32, _w: i32, _h: i32) -> Result<Vec<u8>, String> {
    Err("Screen capture currently implemented for Windows".to_string())
}

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Recognize text from BMP bytes using high-speed offline RapidOCR or Windows.Media.Ocr
pub fn recognize_text_from_bmp(bmp_data: &[u8]) -> Result<String, String> {
    if bmp_data.is_empty() {
        return Err("Empty image data".to_string());
    }

    // 1. Try RapidOCR via Python (completely silent, CREATE_NO_WINDOW, zero console popup)
    let script = r#"
import sys
try:
    from rapidocr_onnxruntime import RapidOCR
    data = sys.stdin.buffer.read()
    if data:
        ocr = RapidOCR()
        res, _ = ocr(data)
        if res:
            texts = [item[1].strip() for item in res if item and len(item) > 1 and item[1].strip()]
            print(" ".join(texts))
except Exception:
    pass
"#;

    let candidates = ["pythonw", "python"];
    for exe in candidates {
        let mut cmd = Command::new(exe);
        cmd.arg("-c").arg(script);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        if let Ok(mut child) = cmd.spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(bmp_data);
            }
            if let Ok(output) = child.wait_with_output() {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !text.is_empty() {
                        return Ok(text);
                    }
                }
            }
        }
    }

    // 2. Fallback: Windows 10/11 built-in WinRT Windows.Media.Ocr (offline, native, silent)
    #[cfg(windows)]
    {
        if let Ok(text) = recognize_text_winrt(bmp_data) {
            if !text.is_empty() {
                return Ok(text);
            }
        }
    }

    Err("No text detected in selected area".to_string())
}

#[cfg(windows)]
fn recognize_text_winrt(bmp_data: &[u8]) -> Result<String, String> {
    let ps_script = r#"
[Windows.Media.Ocr.OcrEngine, Windows.Foundation, ContentType = WindowsRuntime] | Out-Null
[Windows.Graphics.Imaging.BitmapDecoder, Windows.Graphics.Imaging, ContentType = WindowsRuntime] | Out-Null
$b64 = [System.Console]::In.ReadToEnd()
if (-not [string]::IsNullOrWhiteSpace($b64)) {
    try {
        $bytes = [System.Convert]::FromBase64String($b64)
        $ms = New-Object System.IO.MemoryStream(,$bytes)
        $stream = [System.IO.WindowsRuntimeStreamExtensions]::AsRandomAccessStream($ms)
        $decoder = [Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream).GetAwaiter().GetResult()
        $bitmap = $decoder.GetSoftwareBitmapAsync().GetAwaiter().GetResult()
        $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
        if ($engine) {
            $result = $engine.RecognizeAsync($bitmap).GetAwaiter().GetResult()
            if ($result -and $result.Text) {
                [System.Console]::Out.Write($result.Text.Trim())
            }
        }
    } catch {}
}
"#;

    let b64 = fast_base64_encode(bmp_data);
    let mut cmd = Command::new("powershell");
    cmd.arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-WindowStyle")
        .arg("Hidden")
        .arg("-Command")
        .arg(ps_script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b64.as_bytes());
    }

    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !text.is_empty() {
            return Ok(text);
        }
    }

    Err("WinRT OCR failed".to_string())
}

/// Simple, super-fast Base64 encoder without external dependencies
pub fn fast_base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        result.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
