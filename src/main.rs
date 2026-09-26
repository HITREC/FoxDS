#![cfg_attr(windows, windows_subsystem = "windows")]

mod audio;
mod audio_engine;
mod audio_player;
mod config;
mod hotkeys;
mod ocr;
mod stt;
mod system_stats;
mod translator;
mod tts;

use audio_engine::{AppEvent, AudioEngine};
use config::AppConfig;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

const HTML_INDEX: &str = include_str!("../ui/index.html");
const CSS_STYLE: &str = include_str!("../ui/style.css");
const JS_APP: &str = include_str!("../ui/app.js");

const ICON_RGBA: &[u8] = include_bytes!("../ui/icon_64.rgba");
const APP_ICON_PNG: &[u8] = include_bytes!("../ui/app_icon.png");

fn load_app_icon() -> Option<tao::window::Icon> {
    tao::window::Icon::from_rgba(ICON_RGBA.to_vec(), 64, 64).ok()
}

const SNIPER_HTML: &str = r#"<!DOCTYPE html>
<html lang="ru">
<head>
<meta charset="UTF-8">
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; user-select: none; }
  html, body {
    width: 100vw; height: 100vh;
    background: transparent !important;
    overflow: hidden;
    cursor: crosshair;
    font-family: 'Segoe UI', system-ui, -apple-system, sans-serif;
  }
  #sniper-canvas {
    position: absolute;
    top: 0; left: 0;
    width: 100vw; height: 100vh;
    display: block;
    cursor: crosshair;
  }
  .sniper-banner {
    position: fixed;
    top: 24px;
    left: 50%;
    transform: translateX(-50%);
    background: rgba(14, 18, 26, 0.95);
    border: 1.5px solid #00f2fe;
    border-radius: 30px;
    padding: 8px 24px;
    color: #ffffff;
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.6px;
    box-shadow: 0 10px 30px rgba(0,0,0,0.85), 0 0 20px rgba(0, 242, 254, 0.4);
    pointer-events: none;
    display: flex;
    align-items: center;
    gap: 12px;
    z-index: 99999;
  }
  .sniper-tag {
    background: #f25c05;
    color: #fff;
    padding: 2px 8px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 800;
  }
  #selection-label {
    position: absolute;
    background: #00f2fe;
    color: #0b0f19;
    font-size: 11px;
    font-weight: 800;
    padding: 3px 8px;
    border-radius: 4px;
    box-shadow: 0 4px 12px rgba(0,0,0,0.5);
    pointer-events: none;
    display: none;
    z-index: 100000;
    white-space: nowrap;
  }
</style>
</head>
<body>
  <canvas id="sniper-canvas"></canvas>
  <div class="sniper-banner">
    <span class="sniper-tag">FOXDS SNIPER</span>
    <span>✂️ Выделите рамкой область с текстом для перевода | [ESC] или Правый клик — Отмена</span>
  </div>
  <div id="selection-label">0 × 0 px</div>
  <script>
    const canvas = document.getElementById('sniper-canvas');
    const ctx = canvas.getContext('2d');
    const label = document.getElementById('selection-label');
    let isDrawing = false;
    let startX = 0, startY = 0;
    let curX = 0, curY = 0;

    function resizeCanvas() {
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
      renderScene();
    }
    window.addEventListener('resize', resizeCanvas);

    function renderScene() {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      // Dim background slightly (35% dark overlay like Windows Snipping Tool)
      ctx.fillStyle = 'rgba(10, 15, 26, 0.35)';
      ctx.fillRect(0, 0, canvas.width, canvas.height);

      if (isDrawing) {
        const sx = Math.min(startX, curX);
        const sy = Math.min(startY, curY);
        const sw = Math.abs(curX - startX);
        const sh = Math.abs(curY - startY);

        if (sw > 0 && sh > 0) {
          // Clear the selection rectangle so the live desktop underneath is 100% crystal clear!
          ctx.clearRect(sx, sy, sw, sh);

          // Draw neon border with dash
          ctx.save();
          ctx.strokeStyle = '#00f2fe';
          ctx.lineWidth = 2;
          ctx.setLineDash([6, 4]);
          ctx.shadowColor = '#00f2fe';
          ctx.shadowBlur = 8;
          ctx.strokeRect(sx, sy, sw, sh);
          ctx.restore();

          // Corner accent notches
          const cs = Math.min(10, Math.min(sw, sh) / 2);
          ctx.strokeStyle = '#ffffff';
          ctx.lineWidth = 2.5;
          ctx.beginPath();
          // Top-left
          ctx.moveTo(sx, sy + cs); ctx.lineTo(sx, sy); ctx.lineTo(sx + cs, sy);
          // Top-right
          ctx.moveTo(sx + sw - cs, sy); ctx.lineTo(sx + sw, sy); ctx.lineTo(sx + sw, sy + cs);
          // Bottom-left
          ctx.moveTo(sx, sy + sh - cs); ctx.lineTo(sx, sy + sh); ctx.lineTo(sx + cs, sy + sh);
          // Bottom-right
          ctx.moveTo(sx + sw - cs, sy + sh); ctx.lineTo(sx + sw, sy + sh); ctx.lineTo(sx + sw, sy + sh - cs);
          ctx.stroke();

          // Update label position
          label.style.display = 'block';
          label.style.left = (sx + sw - 80) + 'px';
          label.style.top = (sy + sh + 8) + 'px';
          label.innerText = Math.round(sw) + ' × ' + Math.round(sh) + ' px';
        }
      } else {
        label.style.display = 'none';
      }
    }

    window.initSniper = function() {
      isDrawing = false;
      label.style.display = 'none';
      resizeCanvas();
    };

    window.addEventListener('mousedown', function(e) {
      if (e.button === 2) {
        cancelSnip();
        return;
      }
      if (e.button === 0) {
        isDrawing = true;
        startX = e.clientX;
        startY = e.clientY;
        curX = e.clientX;
        curY = e.clientY;
        renderScene();
      }
    });

    window.addEventListener('mousemove', function(e) {
      if (!isDrawing) return;
      curX = e.clientX;
      curY = e.clientY;
      renderScene();
    });

    window.addEventListener('mouseup', function(e) {
      if (!isDrawing) return;
      isDrawing = false;
      const sx = Math.min(startX, curX);
      const sy = Math.min(startY, curY);
      const sw = Math.abs(curX - startX);
      const sh = Math.abs(curY - startY);
      label.style.display = 'none';

      if (sw > 15 && sh > 15) {
        if (window.ipc) {
          window.ipc.postMessage(JSON.stringify({
            cmd: 'snip_complete',
            x: Math.round(sx),
            y: Math.round(sy),
            w: Math.round(sw),
            h: Math.round(sh)
          }));
        }
      } else {
        cancelSnip();
      }
    });

    window.addEventListener('keydown', function(e) {
      if (e.key === 'Escape') cancelSnip();
    });

    window.addEventListener('contextmenu', function(e) {
      e.preventDefault();
      cancelSnip();
    });

    function cancelSnip() {
      isDrawing = false;
      label.style.display = 'none';
      if (window.ipc) {
        window.ipc.postMessage(JSON.stringify({ cmd: 'snip_cancel' }));
      }
    }
  </script>
</body>
</html>"#;

const HUD_HTML: &str = r#"<!DOCTYPE html>
<html lang="ru">
<head>
<meta charset="UTF-8">
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; user-select: none; }
  html, body {
    width: 100%; height: 100%;
    background: transparent !important;
    overflow: hidden;
    font-family: 'Segoe UI', system-ui, -apple-system, sans-serif;
    padding: 3px;
  }
  .hud-card {
    width: 100%; height: 100%;
    background: rgba(14, 18, 26, 0.90);
    border: 2px solid #f25c05;
    border-radius: 10px;
    padding: 8px 14px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    box-shadow: 0 8px 24px rgba(0,0,0,0.8), 0 0 14px rgba(242, 92, 5, 0.4);
    cursor: move;
    touch-action: none;
    user-select: none;
  }
  .hud-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 3px;
  }
  .hud-header-left {
    display: flex;
    align-items: center;
    gap: 8px;
    overflow: hidden;
  }
  .hud-dot {
    width: 8px; height: 8px; border-radius: 50%; background: #3fb950;
    box-shadow: 0 0 8px #3fb950;
    transition: background 0.2s, box-shadow 0.2s;
    flex-shrink: 0;
  }
  .hud-badge {
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.8px;
    text-transform: uppercase;
    color: #f25c05;
    transition: color 0.15s ease;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .hud-lock-btn {
    background: #f25c05;
    border: none;
    color: #ffffff;
    font-size: 11px;
    font-weight: 700;
    padding: 3px 10px;
    border-radius: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 4px;
    transition: all 0.15s ease;
    font-family: inherit;
    white-space: nowrap;
    user-select: none;
    flex-shrink: 0;
    box-shadow: 0 2px 6px rgba(242, 92, 5, 0.4);
  }
  .hud-lock-btn:hover {
    background: #ff7b29;
    box-shadow: 0 0 10px rgba(242, 92, 5, 0.7);
    transform: scale(1.03);
  }
  .hud-trans {
    font-size: 15px;
    font-weight: 700;
    color: #00f2fe;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: font-size 0.15s ease;
  }
  .hud-orig {
    font-size: 12px;
    color: #8b949e;
    margin-top: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: font-size 0.15s ease;
  }
</style>
</head>
<body>
  <div class="hud-card" id="hud">
    <div class="hud-header">
      <div class="hud-header-left">
        <div class="hud-dot" id="dot"></div>
        <div class="hud-badge" id="badge">FOXDS TACTICAL HUD</div>
      </div>
      <button class="hud-lock-btn" id="btn-hud-lock">🔒 Зафиксировать</button>
    </div>
    <div class="hud-trans" id="trans">FoxDS Pro онлайн</div>
    <div class="hud-orig" id="orig" style="display: none;"></div>
  </div>
  <script>
    window.updateHud = function(badge, orig, trans, color) {
      if (badge) document.getElementById('badge').innerText = badge;
      const origEl = document.getElementById('orig');
      if (orig && orig.trim().length > 0) {
        origEl.innerText = orig;
        origEl.style.display = 'block';
      } else {
        origEl.innerText = '';
        origEl.style.display = 'none';
      }
      if (trans) document.getElementById('trans').innerText = trans;
      if (color) {
        const hud = document.getElementById('hud');
        if (hud) {
          hud.dataset.borderColor = color;
          const curBw = parseInt(hud.dataset.borderWidth !== undefined ? hud.dataset.borderWidth : '2', 10);
          if (curBw > 0) {
            hud.style.borderColor = color;
            hud.style.boxShadow = '0 8px 24px rgba(0,0,0,0.8), 0 0 14px ' + color + '55';
          }
        }
        const dot = document.getElementById('dot');
        if (dot) {
          dot.style.background = color;
          dot.style.boxShadow = '0 0 8px ' + color;
        }
      }
    };

    window.applyOverlayStyle = function(fontSize, alpha, borderColor, borderWidth, textColor) {
      const hud = document.getElementById('hud');
      if (!hud) return;
      if (fontSize) {
        document.getElementById('trans').style.fontSize = fontSize + 'px';
        document.getElementById('orig').style.fontSize = Math.max(10, fontSize - 3) + 'px';
      }
      if (alpha !== undefined && alpha !== null) {
        // Set background color with alpha, keep text 100% sharp and readable
        hud.style.backgroundColor = 'rgba(14, 18, 26, ' + alpha + ')';
      }
      if (textColor) {
        document.getElementById('trans').style.color = textColor;
      }
      if (borderColor) {
        hud.dataset.borderColor = borderColor;
        const dot = document.getElementById('dot');
        if (dot) {
          dot.style.background = borderColor;
          dot.style.boxShadow = '0 0 8px ' + borderColor;
        }
        const badge = document.getElementById('badge');
        if (badge) badge.style.color = borderColor;
      }
      if (borderWidth !== undefined && borderWidth !== null) {
        hud.dataset.borderWidth = borderWidth;
      }
      const curBw = parseInt(hud.dataset.borderWidth !== undefined ? hud.dataset.borderWidth : '2', 10);
      const curBc = hud.dataset.borderColor || '#f25c05';
      if (curBw === 0) {
        hud.style.border = 'none';
        hud.style.borderWidth = '0px';
        hud.style.boxShadow = '0 8px 24px rgba(0,0,0,0.85)';
      } else {
        hud.style.border = curBw + 'px solid ' + curBc;
        hud.style.borderWidth = curBw + 'px';
        hud.style.boxShadow = '0 8px 24px rgba(0,0,0,0.8), 0 0 14px ' + curBc + '55';
      }
    };

    let isDragging = false;
    let startScreenX = 0;
    let startScreenY = 0;
    let pendingDx = 0;
    let pendingDy = 0;
    let rafId = null;
    const hudCard = document.getElementById('hud');

    hudCard.addEventListener('pointerdown', function(e) {
      if (e.target.closest('#btn-hud-lock')) return;
      if (e.button !== 0) return;
      isDragging = true;
      startScreenX = e.screenX;
      startScreenY = e.screenY;
      pendingDx = 0;
      pendingDy = 0;
      try { hudCard.setPointerCapture(e.pointerId); } catch(err) {}
      if (window.ipc) {
        window.ipc.postMessage(JSON.stringify({ cmd: 'start_drag_hud' }));
      }
    });

    hudCard.addEventListener('pointermove', function(e) {
      if (!isDragging) return;
      pendingDx += (e.screenX - startScreenX);
      pendingDy += (e.screenY - startScreenY);
      startScreenX = e.screenX;
      startScreenY = e.screenY;

      if (!rafId) {
        rafId = requestAnimationFrame(function() {
          rafId = null;
          if (!isDragging) return;
          if (pendingDx !== 0 || pendingDy !== 0) {
            const dx = pendingDx;
            const dy = pendingDy;
            pendingDx = 0;
            pendingDy = 0;
            if (window.ipc) {
              window.ipc.postMessage(JSON.stringify({ cmd: 'move_hud', dx: dx, dy: dy }));
            }
          }
        });
      }
    });

    function endDrag(e) {
      if (!isDragging) return;
      isDragging = false;
      if (rafId) {
        cancelAnimationFrame(rafId);
        rafId = null;
      }
      try { hudCard.releasePointerCapture(e.pointerId); } catch(err) {}
      if (pendingDx !== 0 || pendingDy !== 0) {
        const dx = pendingDx;
        const dy = pendingDy;
        pendingDx = 0;
        pendingDy = 0;
        if (window.ipc) {
          window.ipc.postMessage(JSON.stringify({ cmd: 'move_hud', dx: dx, dy: dy }));
        }
      }
      if (window.ipc) {
        window.ipc.postMessage(JSON.stringify({ cmd: 'save_hud_pos' }));
      }
    }

    hudCard.addEventListener('pointerup', endDrag);
    hudCard.addEventListener('pointercancel', endDrag);

    const lockBtn = document.getElementById('btn-hud-lock');
    lockBtn.addEventListener('pointerdown', function(e) {
      e.stopPropagation();
    });
    lockBtn.addEventListener('click', function(e) {
      e.stopPropagation();
      if (window.ipc) {
        window.ipc.postMessage(JSON.stringify({ cmd: 'lock_hud' }));
      }
    });
  </script>
</body>
</html>"#;

#[cfg(windows)]
mod win_composition {
    use std::ffi::c_void;

    #[repr(C)]
    #[allow(dead_code)]
    pub enum AccentState {
        AccentDisabled = 0,
        AccentEnableGradient = 1,
        AccentEnableTransparentGradient = 2,
        AccentEnableBlurbehind = 3,
        AccentEnableAcrylicBlurbehind = 4,
        AccentInvalidState = 5,
    }

    #[repr(C)]
    pub struct AccentPolicy {
        pub accent_state: AccentState,
        pub accent_flags: u32,
        pub gradient_color: u32,
        pub animation_id: u32,
    }

    #[repr(C)]
    pub struct WindowCompositionAttributeData {
        pub attribute: u32,
        pub data: *mut c_void,
        pub size_of_data: usize,
    }

    type SetWindowCompositionAttributeFn = unsafe extern "system" fn(
        hwnd: isize,
        data: *const WindowCompositionAttributeData,
    ) -> i32;

    pub unsafe fn set_transparent(hwnd: isize) {
        let user32 = windows_sys::Win32::System::LibraryLoader::GetModuleHandleA(b"user32.dll\0".as_ptr());
        let user32 = if user32.is_null() {
            windows_sys::Win32::System::LibraryLoader::LoadLibraryA(b"user32.dll\0".as_ptr())
        } else {
            user32
        };
        if user32.is_null() {
            return;
        }
        let func = windows_sys::Win32::System::LibraryLoader::GetProcAddress(
            user32,
            b"SetWindowCompositionAttribute\0".as_ptr(),
        );
        if let Some(func) = func {
            let set_comp: SetWindowCompositionAttributeFn = std::mem::transmute(func);
            let mut policy = AccentPolicy {
                accent_state: AccentState::AccentEnableTransparentGradient,
                accent_flags: 2,
                gradient_color: 0x00000000,
                animation_id: 0,
            };
            let data = WindowCompositionAttributeData {
                attribute: 19, // WCA_ACCENT_POLICY
                data: &mut policy as *mut _ as *mut c_void,
                size_of_data: std::mem::size_of::<AccentPolicy>(),
            };
            set_comp(hwnd, &data);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    {
        std::env::set_var("WEBVIEW2_DEFAULT_BACKGROUND_COLOR", "0");
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::SetProcessDPIAware();
        }
    }

    // 1. Load configuration
    let config_path = config::get_config_path();
    let config = Arc::new(Mutex::new(AppConfig::load_or_default(&config_path)));

    // 2. Set up Tao Window & Event Loop with UserEvent
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    // Main Control Panel Window
    let app_icon = load_app_icon();

    let window = WindowBuilder::new()
        .with_title("FoxDS Voice Translator Pro")
        .with_window_icon(app_icon.clone())
        .with_decorations(false)
        .with_inner_size(tao::dpi::LogicalSize::new(1200.0, 780.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(1000.0, 680.0))
        .build(&event_loop)?;

    let window = Arc::new(window);
    let win_clone = window.clone();

    // Floating In-Game HUD Subtitle Overlay Window
    let (init_hud_x, init_hud_y, hud_w, hud_h) = {
        let cfg = config.lock();
        (cfg.overlay_x, cfg.overlay_y, cfg.overlay_w as f64, cfg.overlay_h as f64)
    };
    let hud_pos_x = Arc::new(std::sync::atomic::AtomicI32::new(init_hud_x));
    let hud_pos_y = Arc::new(std::sync::atomic::AtomicI32::new(init_hud_y));

    #[cfg(windows)]
    use tao::platform::windows::WindowBuilderExtWindows;

    #[allow(unused_mut)]
    let mut hud_builder = WindowBuilder::new()
        .with_title("FoxDS Tactical Subtitle HUD")
        .with_window_icon(app_icon.clone())
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .with_inner_size(tao::dpi::LogicalSize::new(hud_w.max(300.0), hud_h.max(50.0)))
        .with_min_inner_size(tao::dpi::LogicalSize::new(300.0, 50.0))
        .with_position(tao::dpi::LogicalPosition::new(init_hud_x as f64, init_hud_y as f64));

    #[cfg(windows)]
    {
        hud_builder = hud_builder.with_undecorated_shadow(false);
    }

    let hud_window = hud_builder.build(&event_loop)?;
    let hud_window = Arc::new(hud_window);

    #[cfg(windows)]
    {
        use tao::platform::windows::WindowExtWindows;
        unsafe {
            win_composition::set_transparent(hud_window.hwnd() as isize);
        }
    }

    // Floating In-Game Sniper / Area Selection Overlay Window
    let (scr_x, scr_y, scr_w, scr_h) = {
        #[cfg(windows)]
        unsafe {
            let vx = windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(windows_sys::Win32::UI::WindowsAndMessaging::SM_XVIRTUALSCREEN) as f64;
            let vy = windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(windows_sys::Win32::UI::WindowsAndMessaging::SM_YVIRTUALSCREEN) as f64;
            let vw = windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(windows_sys::Win32::UI::WindowsAndMessaging::SM_CXVIRTUALSCREEN) as f64;
            let vh = windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(windows_sys::Win32::UI::WindowsAndMessaging::SM_CYVIRTUALSCREEN) as f64;
            if vw > 100.0 && vh > 100.0 {
                (vx, vy, vw, vh)
            } else {
                (0.0, 0.0, 1920.0, 1080.0)
            }
        }
        #[cfg(not(windows))]
        (0.0, 0.0, 1920.0, 1080.0)
    };

    #[allow(unused_mut)]
    let mut sniper_builder = WindowBuilder::new()
        .with_title("FoxDS Tactical Sniper")
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .with_visible(false)
        .with_inner_size(tao::dpi::LogicalSize::new(scr_w, scr_h))
        .with_position(tao::dpi::LogicalPosition::new(scr_x, scr_y));

    #[cfg(windows)]
    {
        sniper_builder = sniper_builder.with_undecorated_shadow(false);
    }

    let sniper_window = sniper_builder.build(&event_loop)?;
    let sniper_window = Arc::new(sniper_window);
    let sniper_win_ipc = sniper_window.clone();

    #[cfg(windows)]
    {
        use tao::platform::windows::WindowExtWindows;
        unsafe {
            win_composition::set_transparent(sniper_window.hwnd() as isize);
        }
    }

    // 3. Assemble embedded HTML with inlined CSS and JS
    let icon_b64 = ocr::fast_base64_encode(APP_ICON_PNG);
    let full_html = HTML_INDEX
        .replace(
            "<link rel=\"stylesheet\" href=\"style.css\">",
            &format!("<style>{}</style>", CSS_STYLE),
        )
        .replace(
            "<script src=\"app.js\"></script>",
            &format!("<script>{}</script>", JS_APP),
        )
        .replace(
            "app_icon.png",
            &format!("data:image/png;base64,{}", icon_b64),
        );

    // 4. Start Pure Rust Audio Engine (Mic capture, PTT F4, RMS VU-meter, STT, Translation, TTS)
    let is_running = Arc::new(AtomicBool::new(true));
    let engine = AudioEngine::new(config.clone(), proxy.clone(), is_running.clone());
    engine.start();

    // 5. System Stats Background Thread
    let is_running_stats = is_running.clone();
    thread::spawn(move || {
        while is_running_stats.load(Ordering::SeqCst) {
            let _stats = system_stats::SystemStats::current();
            thread::sleep(Duration::from_millis(2000));
        }
    });

    // 6. IPC handler for Main Control Panel
    let config_clone = config.clone();
    let config_path_clone = config_path.clone();
    let is_running_clone = is_running.clone();
    let proxy_ipc = proxy.clone();

    let webview = WebViewBuilder::new()
        .with_html(full_html)
        .with_ipc_handler(move |req| {
            let body = req.body();
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
                if let Some(cmd) = val.get("cmd").and_then(|v| v.as_str()) {
                    match cmd {
                        "drag_window" => {
                            let _ = win_clone.drag_window();
                        }
                        "minimize" => {
                            win_clone.set_minimized(true);
                        }
                        "maximize" => {
                            let is_max = win_clone.is_maximized();
                            win_clone.set_maximized(!is_max);
                        }
                        "close" => {
                            is_running_clone.store(false, Ordering::SeqCst);
                            std::process::exit(0);
                        }
                        "app_ready" => {
                            let cfg = config_clone.lock();
                            if let Ok(cfg_json) = serde_json::to_string(&*cfg) {
                                proxy_ipc.send_event(AppEvent::InitConfig(cfg_json)).ok();
                            }
                            let dev_json = AudioEngine::get_devices_json();
                            proxy_ipc.send_event(AppEvent::AudioDevices(dev_json)).ok();
                        }
                        "slider_change" => {
                            if let (Some(id), Some(val_num)) = (
                                val.get("id").and_then(|v| v.as_str()),
                                val.get("value").and_then(|v| v.as_f64()),
                            ) {
                                let mut cfg = config_clone.lock();
                                match id {
                                    "range-voice-speed" => cfg.speech_speed = val_num as u32,
                                    "range-mic-gain" => cfg.mic_gain = (val_num / 100.0) as f32,
                                    "range-tts-gain" => cfg.tts_gain = (val_num / 100.0) as f32,
                                    "range-incoming-tts-gain" => cfg.incoming_tts_gain = (val_num / 100.0) as f32,
                                    "range-incoming-thresh" => cfg.rms_threshold = val_num as f32,
                                    "range-ai-confidence" => cfg.min_confidence = (val_num / 100.0) as f32,
                                    "range-ocr-delay" => cfg.ocr_appear_delay = (val_num / 10.0) as f32,
                                    "range-ocr-duration" => cfg.ocr_display_duration = val_num as u32,
                                    "range-overlay-font" => {
                                        cfg.overlay_font_size = val_num as u32;
                                        proxy_ipc.send_event(AppEvent::UpdateHudStyle {
                                            font_size: cfg.overlay_font_size,
                                            alpha: cfg.overlay_alpha,
                                            border_color: cfg.overlay_border_color.clone(),
                                            border_width: cfg.overlay_border_width,
                                            text_color: cfg.overlay_text_color.clone(),
                                        }).ok();
                                    }
                                    "range-overlay-alpha" => {
                                        cfg.overlay_alpha = (val_num / 100.0) as f32;
                                        proxy_ipc.send_event(AppEvent::UpdateHudStyle {
                                            font_size: cfg.overlay_font_size,
                                            alpha: cfg.overlay_alpha,
                                            border_color: cfg.overlay_border_color.clone(),
                                            border_width: cfg.overlay_border_width,
                                            text_color: cfg.overlay_text_color.clone(),
                                        }).ok();
                                    }
                                    "range-overlay-border-w" => {
                                        cfg.overlay_border_width = val_num as u32;
                                        proxy_ipc.send_event(AppEvent::UpdateHudStyle {
                                            font_size: cfg.overlay_font_size,
                                            alpha: cfg.overlay_alpha,
                                            border_color: cfg.overlay_border_color.clone(),
                                            border_width: cfg.overlay_border_width,
                                            text_color: cfg.overlay_text_color.clone(),
                                        }).ok();
                                    }
                                    "range-overlay-width" => {
                                        cfg.overlay_w = val_num as u32;
                                        proxy_ipc.send_event(AppEvent::ResizeHud {
                                            width: cfg.overlay_w,
                                            height: cfg.overlay_h,
                                        }).ok();
                                    }
                                    "range-overlay-height" => {
                                        cfg.overlay_h = val_num as u32;
                                        proxy_ipc.send_event(AppEvent::ResizeHud {
                                            width: cfg.overlay_w,
                                            height: cfg.overlay_h,
                                        }).ok();
                                    }
                                    _ => {}
                                }
                                let _ = cfg.save(&config_path_clone);
                            }
                        }
                        "toggle_change" => {
                            if let (Some(id), Some(checked)) = (
                                val.get("id").and_then(|v| v.as_str()),
                                val.get("checked").and_then(|v| v.as_bool()),
                            ) {
                                let mut cfg = config_clone.lock();
                                match id {
                                    "chk-passthrough" => cfg.passthrough_enabled = checked,
                                    "chk-radio-filter" => cfg.radio_effect = checked,
                                    "chk-play-self" => cfg.play_self_audio = checked,
                                    "chk-incoming-subtitles" => cfg.incoming_enabled = checked,
                                    "chk-incoming-tts" => cfg.incoming_tts_enabled = checked,
                                    "chk-filter-ru" => cfg.filter_russian = checked,
                                    "chk-ignore-mic" => cfg.ignore_own_mic = checked,
                                    "chk-auto-match" => cfg.auto_volume_match = checked,
                                    "chk-ocr-enabled" => cfg.ocr_enabled = checked,
                                    _ => {}
                                }
                                let _ = cfg.save(&config_path_clone);
                            }
                        }
                        "voice_change" => {
                            if let Some(voice) = val.get("voice").and_then(|v| v.as_str()) {
                                let mut cfg = config_clone.lock();
                                cfg.voice = voice.to_string();
                                let _ = cfg.save(&config_path_clone);
                            }
                        }
                        "incoming_voice_change" => {
                            if let Some(voice) = val.get("voice").and_then(|v| v.as_str()) {
                                let mut cfg = config_clone.lock();
                                cfg.incoming_voice = voice.to_string();
                                let _ = cfg.save(&config_path_clone);
                            }
                        }
                        "device_change" => {
                            let dtype = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            let name = val.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let mut cfg = config_clone.lock();
                            match dtype {
                                "mic" => cfg.selected_mic = name.to_string(),
                                "spk" => cfg.selected_headphones = name.to_string(),
                                "cable" => cfg.selected_cable_in = name.to_string(),
                                _ => {}
                            }
                            let _ = cfg.save(&config_path_clone);
                        }
                        "test_f4" => {
                            // Synthesize test phrase and play to virtual cable AND headphones
                            let cfg = config_clone.lock();
                            let voice = cfg.voice.clone();
                            let speed = cfg.speech_speed;
                            let cable = cfg.selected_cable_in.clone();
                            let hp = cfg.selected_headphones.clone();
                            let play_self = cfg.play_self_audio;
                            let tts_gain = cfg.tts_gain;
                            drop(cfg);

                            thread::spawn(move || {
                                if let Ok(audio) = tts::synthesize_speech("Voice transmission test. FoxDS Pro is online.", &voice, speed) {
                                    let _ = audio_player::play_tts_audio(&audio, &cable, &hp, play_self, tts_gain);
                                }
                            });
                        }
                        "test_incoming" => {
                            let en_text = "WATCH OUT, SNIPER IN THE CLOCK TOWER!";
                            let ru_text = "ОСТОРОЖНО, СНАЙПЕР НА ЧАСОВОЙ БАШНЕ!";
                            proxy_ipc.send_event(AppEvent::SpeechEvent(
                                "incoming".to_string(),
                                en_text.to_string(),
                                format!("ТИММЕЙТ: {}", ru_text),
                            )).ok();

                            let cfg = config_clone.lock();
                            let incoming_tts = cfg.incoming_tts_enabled;
                            let voice = cfg.incoming_voice.clone();
                            let hp = cfg.selected_headphones.clone();
                            let speed = cfg.speech_speed;
                            let gain = cfg.incoming_tts_gain;
                            drop(cfg);

                            if incoming_tts {
                                let text_to_speak = ru_text.to_string();
                                thread::spawn(move || {
                                    if let Ok(audio) = tts::synthesize_speech_opt(&text_to_speak, &voice, speed, true) {
                                        let _ = audio_player::play_headphones_audio(&audio, &hp, gain, None, None);
                                    }
                                });
                            }
                        }
                        "set_click_through" => {
                            let locked = val.get("locked").and_then(|v| v.as_bool()).unwrap_or(false);
                            let mut cfg = config_clone.lock();
                            cfg.overlay_locked = locked;
                            let _ = cfg.save(&config_path_clone);
                            proxy_ipc.send_event(AppEvent::SetHudLocked(locked)).ok();
                        }
                        "set_overlay_preset" => {
                            if let Some(pos) = val.get("preset").and_then(|v| v.as_str()) {
                                let mut cfg = config_clone.lock();
                                cfg.overlay_preset = pos.to_string();
                                let _ = cfg.save(&config_path_clone);
                                proxy_ipc.send_event(AppEvent::SetHudPreset(pos.to_string())).ok();
                            }
                        }
                        "set_overlay_color" => {
                            if let Some(col) = val.get("color").and_then(|v| v.as_str()) {
                                let mut cfg = config_clone.lock();
                                cfg.overlay_border_color = col.to_string();
                                let _ = cfg.save(&config_path_clone);
                                proxy_ipc.send_event(AppEvent::UpdateHudStyle {
                                    font_size: cfg.overlay_font_size,
                                    alpha: cfg.overlay_alpha,
                                    border_color: col.to_string(),
                                    border_width: cfg.overlay_border_width,
                                    text_color: cfg.overlay_text_color.clone(),
                                }).ok();
                            }
                        }
                        "set_overlay_text_color" => {
                            if let Some(col) = val.get("color").and_then(|v| v.as_str()) {
                                let mut cfg = config_clone.lock();
                                cfg.overlay_text_color = col.to_string();
                                let _ = cfg.save(&config_path_clone);
                                proxy_ipc.send_event(AppEvent::UpdateHudStyle {
                                    font_size: cfg.overlay_font_size,
                                    alpha: cfg.overlay_alpha,
                                    border_color: cfg.overlay_border_color.clone(),
                                    border_width: cfg.overlay_border_width,
                                    text_color: col.to_string(),
                                }).ok();
                            }
                        }
                        "save_voice_hotkey" => {
                            if let Some(key) = val.get("key").and_then(|v| v.as_str()) {
                                let mut cfg = config_clone.lock();
                                cfg.hotkey = key.to_string();
                                let _ = cfg.save(&config_path_clone);
                            }
                        }
                        "save_ocr_hotkey" => {
                            if let Some(key) = val.get("key").and_then(|v| v.as_str()) {
                                let mut cfg = config_clone.lock();
                                cfg.ocr_hotkey = key.to_string();
                                let _ = cfg.save(&config_path_clone);
                            }
                        }
                        "save_overlay_settings" => {
                            let cfg = config_clone.lock();
                            let _ = cfg.save(&config_path_clone);
                            proxy_ipc.send_event(AppEvent::UpdateHudStyle {
                                font_size: cfg.overlay_font_size,
                                alpha: cfg.overlay_alpha,
                                border_color: cfg.overlay_border_color.clone(),
                                border_width: cfg.overlay_border_width,
                                text_color: cfg.overlay_text_color.clone(),
                            }).ok();
                        }
                        "trigger_ocr" | "test_ocr" => {
                            proxy_ipc.send_event(AppEvent::OpenSniper).ok();
                        }
                        _ => {}
                    }
                }
            }
        })
        .build(&*window)?;

    // 7. HUD Webview
    let proxy_hud = proxy.clone();
    let hud_win_ipc = hud_window.clone();
    let config_hud = config.clone();
    let config_path_hud = config_path.clone();
    let hud_pos_x_ipc = hud_pos_x.clone();
    let hud_pos_y_ipc = hud_pos_y.clone();

    let hud_webview = WebViewBuilder::new()
        .with_transparent(true)
        .with_html(HUD_HTML)
        .with_ipc_handler(move |req| {
            let body = req.body();
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
                if let Some(cmd) = val.get("cmd").and_then(|v| v.as_str()) {
                    match cmd {
                        "start_drag_hud" => {
                            if let Ok(cur_pos) = hud_win_ipc.outer_position() {
                                let scale = hud_win_ipc.scale_factor();
                                let log_pos = cur_pos.to_logical::<f64>(scale);
                                hud_pos_x_ipc.store(log_pos.x.round() as i32, Ordering::Relaxed);
                                hud_pos_y_ipc.store(log_pos.y.round() as i32, Ordering::Relaxed);
                            }
                        }
                        "move_hud" => {
                            let dx = val.get("dx").and_then(|v| v.as_f64()).unwrap_or(0.0).round() as i32;
                            let dy = val.get("dy").and_then(|v| v.as_f64()).unwrap_or(0.0).round() as i32;
                            if dx != 0 || dy != 0 {
                                let nx = hud_pos_x_ipc.fetch_add(dx, Ordering::Relaxed) + dx;
                                let ny = hud_pos_y_ipc.fetch_add(dy, Ordering::Relaxed) + dy;
                                hud_win_ipc.set_outer_position(tao::dpi::LogicalPosition::new(nx as f64, ny as f64));
                            }
                        }
                        "save_hud_pos" => {
                            if let Ok(cur_pos) = hud_win_ipc.outer_position() {
                                let scale = hud_win_ipc.scale_factor();
                                let log_pos = cur_pos.to_logical::<f64>(scale);
                                let mut cfg = config_hud.lock();
                                cfg.overlay_x = log_pos.x as i32;
                                cfg.overlay_y = log_pos.y as i32;
                                hud_pos_x_ipc.store(cfg.overlay_x, Ordering::Relaxed);
                                hud_pos_y_ipc.store(cfg.overlay_y, Ordering::Relaxed);
                                let _ = cfg.save(&config_path_hud);
                            }
                        }
                        "lock_hud" => {
                            let mut cfg = config_hud.lock();
                            cfg.overlay_locked = true;
                            let _ = cfg.save(&config_path_hud);
                            proxy_hud.send_event(AppEvent::SetHudLocked(true)).ok();
                        }
                        _ => {}
                    }
                }
            } else if body.contains("drag_hud") {
                let _ = hud_win_ipc.drag_window();
            } else if body.contains("lock_hud") {
                let mut cfg = config_hud.lock();
                cfg.overlay_locked = true;
                let _ = cfg.save(&config_path_hud);
                proxy_hud.send_event(AppEvent::SetHudLocked(true)).ok();
            }
        })
        .build(&*hud_window)?;

    #[cfg(windows)]
    {
        use tao::platform::windows::WindowExtWindows;
        unsafe {
            win_composition::set_transparent(hud_window.hwnd() as isize);
        }
        let cur_sz = hud_window.inner_size();
        hud_window.set_inner_size(tao::dpi::PhysicalSize::new(cur_sz.width + 1, cur_sz.height));
        hud_window.set_inner_size(cur_sz);
    }

    // 8. Sniper / Screen Selection Webview
    let proxy_sniper = proxy.clone();

    let sniper_webview = WebViewBuilder::new()
        .with_transparent(true)
        .with_html(SNIPER_HTML)
        .with_ipc_handler(move |req| {
            let body = req.body();
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
                if let Some(cmd) = val.get("cmd").and_then(|v| v.as_str()) {
                    match cmd {
                        "snip_cancel" => {
                            sniper_win_ipc.set_visible(false);
                            proxy_sniper.send_event(AppEvent::StatusEvent("idle".to_string())).ok();
                        }
                        "snip_complete" => {
                            sniper_win_ipc.set_visible(false);
                            let rx = val.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                            let ry = val.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                            let w = val.get("w").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                            let h = val.get("h").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

                            let target_x = scr_x as i32 + rx;
                            let target_y = scr_y as i32 + ry;

                            proxy_sniper.send_event(AppEvent::StatusEvent("ocr_processing".to_string())).ok();
                            let proxy_worker = proxy_sniper.clone();

                            thread::spawn(move || {
                                match ocr::capture_screen_rect_bmp(target_x, target_y, w, h) {
                                    Ok(bmp) => {
                                        match ocr::recognize_text_from_bmp(&bmp) {
                                            Ok(recognized) => {
                                                let clean = recognized.trim();
                                                if clean.is_empty() {
                                                    proxy_worker.send_event(AppEvent::StatusEvent("idle".to_string())).ok();
                                                    return;
                                                }
                                                match translator::translate_text(clean, "en", "ru") {
                                                    Ok(trans) => {
                                                        proxy_worker.send_event(AppEvent::SpeechEvent(
                                                            "ocr".to_string(),
                                                            clean.to_string(),
                                                            trans.trim().to_string(),
                                                        )).ok();
                                                    }
                                                    Err(e) => eprintln!("[OCR Translate] Error: {}", e),
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!("[OCR Recognize] Error: {}", e);
                                                proxy_worker.send_event(AppEvent::SpeechEvent(
                                                    "ocr".to_string(),
                                                    "Выделенная область".to_string(),
                                                    "Текст в рамке не обнаружен".to_string(),
                                                )).ok();
                                            }
                                        }
                                    }
                                    Err(e) => eprintln!("[OCR Capture] Error: {}", e),
                                }
                                proxy_worker.send_event(AppEvent::StatusEvent("idle".to_string())).ok();
                            });
                        }
                        _ => {}
                    }
                }
            }
        })
        .build(&*sniper_window)?;

    #[cfg(windows)]
    {
        use tao::platform::windows::WindowExtWindows;
        unsafe {
            win_composition::set_transparent(sniper_window.hwnd() as isize);
        }
    }

    // 9. Run Main Tao Event Loop
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(app_event) => match app_event {
                AppEvent::InitConfig(json_str) => {
                    let script = format!("window.initFromConfig({});", json_str);
                    let _ = webview.evaluate_script(&script);

                    // Apply loaded HUD style to the live subtitle window
                    let cfg = config.lock();
                    let hud_init = format!(
                        "window.applyOverlayStyle({}, {}, '{}', {}, '{}');",
                        cfg.overlay_font_size, cfg.overlay_alpha, cfg.overlay_border_color, cfg.overlay_border_width, cfg.overlay_text_color
                    );
                    let _ = hud_webview.evaluate_script(&hud_init);
                    let _ = hud_window.set_ignore_cursor_events(cfg.overlay_locked);

                    // Ensure window size from config is applied
                    let w = (cfg.overlay_w as f64).max(300.0);
                    let h = (cfg.overlay_h as f64).max(50.0);
                    hud_window.set_inner_size(tao::dpi::LogicalSize::new(w, h));

                    // Safety clamp overlay position to visible screen bounds
                    let monitor = hud_window.current_monitor().or_else(|| hud_window.primary_monitor());
                    if let Some(m) = monitor {
                        let scale = m.scale_factor();
                        let pos = m.position().to_logical::<f64>(scale);
                        let size = m.size().to_logical::<f64>(scale);
                        let cur_x = cfg.overlay_x as f64;
                        let cur_y = cfg.overlay_y as f64;
                        let clamped_x = cur_x.max(pos.x).min(pos.x + size.width - 100.0);
                        let clamped_y = cur_y.max(pos.y).min(pos.y + size.height - 50.0);
                        if (clamped_x - cur_x).abs() > 1.0 || (clamped_y - cur_y).abs() > 1.0 {
                            hud_window.set_outer_position(tao::dpi::LogicalPosition::new(clamped_x, clamped_y));
                            hud_pos_x.store(clamped_x.round() as i32, Ordering::Relaxed);
                            hud_pos_y.store(clamped_y.round() as i32, Ordering::Relaxed);
                        }
                    }
                }
                AppEvent::AudioDevices(json_str) => {
                    let script = format!("window.setAudioDevices({});", json_str);
                    let _ = webview.evaluate_script(&script);
                }
                AppEvent::SpeechEvent(stype, orig, trans) => {
                    if let (Ok(s_json), Ok(o_json), Ok(t_json)) = (
                        serde_json::to_string(&stype),
                        serde_json::to_string(&orig),
                        serde_json::to_string(&trans),
                    ) {
                        let script = format!("window.onRustSpeechEvent({}, {}, {});", s_json, o_json, t_json);
                        let _ = webview.evaluate_script(&script);

                        // Update In-Game HUD Subtitles
                        let badge = if stype == "incoming" {
                            "💬 ТИММЕЙТ"
                        } else if stype == "ocr" {
                            "📸 ПЕРЕВОД ЭКРАНА"
                        } else {
                            "🎙️ ВЫ СКАЗАЛИ"
                        };
                        let col = if stype == "incoming" {
                            "#38d9a9"
                        } else if stype == "ocr" {
                            "#bc8cff"
                        } else {
                            "#00f2fe"
                        };
                        let hud_script = format!("window.updateHud('{}', {}, {}, '{}');", badge, o_json, t_json, col);
                        let _ = hud_webview.evaluate_script(&hud_script);
                    }
                }
                AppEvent::StatusEvent(status) => {
                    let (txt, col) = match status.as_str() {
                        "recording" => ("🔴 Запись речи...", "#f85149"),
                        "processing" => ("⏳ Перевод речи ИИ...", "#d29922"),
                        "listening" => ("💬 Тиммейт говорит...", "#38d9a9"),
                        "ocr_processing" => ("📸 Считывание экрана...", "#bc8cff"),
                        _ => ("🟢 Ожидание речи...", "#3fb950"),
                    };
                    let script = format!("window.updateEngineStatus('{}', '{}');", txt, col);
                    let _ = webview.evaluate_script(&script);

                    // Update HUD badge & dot indicator
                    let hud_script = format!("window.updateHud('{}', '', '', '{}');", txt, col);
                    let _ = hud_webview.evaluate_script(&hud_script);
                }
                AppEvent::VuEvent(mic, spk, active) => {
                    let script = format!("window.updateRealVU({}, {}, {});", mic, spk, active);
                    let _ = webview.evaluate_script(&script);
                }
                AppEvent::UpdateHudStyle { font_size, alpha, border_color, border_width, text_color } => {
                    let script = format!(
                        "window.applyOverlayStyle({}, {}, '{}', {}, '{}');",
                        font_size, alpha, border_color, border_width, text_color
                    );
                    let _ = hud_webview.evaluate_script(&script);
                }
                AppEvent::ResizeHud { width, height } => {
                    let w = (width as f64).max(300.0);
                    let h = (height as f64).max(50.0);
                    hud_window.set_inner_size(tao::dpi::LogicalSize::new(w, h));
                }
                AppEvent::SetHudPreset(preset) => {
                    let monitor = hud_window
                        .current_monitor()
                        .or_else(|| hud_window.primary_monitor());

                    let (mon_x, mon_y, mon_w, mon_h) = if let Some(m) = monitor {
                        let scale = m.scale_factor();
                        let pos = m.position().to_logical::<f64>(scale);
                        let size = m.size().to_logical::<f64>(scale);
                        (pos.x, pos.y, size.width, size.height)
                    } else {
                        (0.0, 0.0, 1920.0, 1080.0)
                    };

                    let scale = hud_window.scale_factor();
                    let cur_sz = hud_window.inner_size().to_logical::<f64>(scale);
                    let w = cur_sz.width.max(200.0);
                    let h = cur_sz.height.max(50.0);

                    let margin = 30.0;
                    let (rel_x, rel_y) = match preset.as_str() {
                        "top_center" => ((mon_w - w) / 2.0, margin),
                        "top_right" => (mon_w - w - margin, margin),
                        "bottom_right" => (mon_w - w - margin, mon_h - h - margin - 20.0),
                        _ => ((mon_w - w) / 2.0, mon_h - h - margin - 20.0), // bottom_center
                    };

                    let px = mon_x + rel_x;
                    let py = mon_y + rel_y;

                    hud_window.set_outer_position(tao::dpi::LogicalPosition::new(px, py));
                    hud_pos_x.store(px.round() as i32, Ordering::Relaxed);
                    hud_pos_y.store(py.round() as i32, Ordering::Relaxed);
                    let mut cfg = config.lock();
                    cfg.overlay_x = px.round() as i32;
                    cfg.overlay_y = py.round() as i32;
                    cfg.overlay_preset = preset;
                    let _ = cfg.save(&config_path);
                }
                AppEvent::SetHudLocked(locked) => {
                    let _ = hud_window.set_ignore_cursor_events(locked);
                    let script = format!("if (window.onOverlayLockChanged) window.onOverlayLockChanged({});", locked);
                    let _ = webview.evaluate_script(&script);
                }
                AppEvent::OpenSniper => {
                    sniper_window.set_visible(true);
                    sniper_window.set_focus();
                    #[cfg(windows)]
                    {
                        use tao::platform::windows::WindowExtWindows;
                        unsafe {
                            win_composition::set_transparent(sniper_window.hwnd() as isize);
                        }
                    }
                    let _ = sniper_webview.evaluate_script("window.initSniper();");
                }
            },
            Event::WindowEvent {
                window_id,
                event,
                ..
            } => {
                match event {
                    WindowEvent::CloseRequested => {
                        is_running.store(false, Ordering::SeqCst);
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::Moved(pos) => {
                        if window_id == hud_window.id() {
                            let mut cfg = config.lock();
                            cfg.overlay_x = pos.x;
                            cfg.overlay_y = pos.y;
                            let _ = cfg.save(&config_path);
                        }
                    }
                    _ => ()
                }
            }
            _ => (),
        }
    });
}
