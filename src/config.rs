use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub hotkey: String,
    pub voice: String,
    pub passthrough_enabled: bool,
    pub incoming_enabled: bool,
    pub incoming_tts_enabled: bool,
    pub incoming_voice: String,
    pub incoming_tts_gain: f32,
    pub play_self_audio: bool,
    pub auto_volume_match: bool,
    pub radio_effect: bool,
    pub tensor_accel: bool,
    pub tensor_backend: String,
    pub natural_prosody: bool,
    pub mic_gain: f32,
    pub tts_gain: f32,
    pub rms_threshold: f32,
    pub speech_speed: u32,
    pub filter_russian: bool,
    pub min_confidence: f32,
    pub ignore_own_mic: bool,
    pub ocr_hotkey: String,
    pub ocr_enabled: bool,
    pub ocr_appear_delay: f32,
    pub ocr_display_duration: u32,
    pub overlay_preset: String,
    pub overlay_font_size: u32,
    pub overlay_alpha: f32,
    pub overlay_border_color: String,
    pub overlay_border_width: u32,
    pub overlay_text_color: String,
    pub overlay_locked: bool,
    pub overlay_x: i32,
    pub overlay_y: i32,
    pub overlay_w: u32,
    pub overlay_h: u32,
    pub outgoing_enabled: bool,
    pub selected_mic: String,
    pub selected_headphones: String,
    pub selected_cable_in: String,
    pub preset_name: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey: "F4".to_string(),
            voice: "en-US-AndrewMultilingualNeural".to_string(),
            passthrough_enabled: true,
            incoming_enabled: true,
            incoming_tts_enabled: true,
            incoming_voice: "ru-RU-DmitryNeural".to_string(),
            incoming_tts_gain: 1.2,
            play_self_audio: true,
            auto_volume_match: true,
            radio_effect: false,
            tensor_accel: true,
            tensor_backend: "auto".to_string(),
            natural_prosody: true,
            mic_gain: 1.0,
            tts_gain: 1.2,
            rms_threshold: 35.0,
            speech_speed: 100,
            filter_russian: true,
            min_confidence: 0.62,
            ignore_own_mic: true,
            ocr_hotkey: "ALT+Q".to_string(),
            ocr_enabled: true,
            ocr_appear_delay: 0.6,
            ocr_display_duration: 12,
            overlay_preset: "bottom_center".to_string(),
            overlay_font_size: 12,
            overlay_alpha: 0.92,
            overlay_border_color: "#f25c05".to_string(),
            overlay_border_width: 2,
            overlay_text_color: "#00f2fe".to_string(),
            overlay_locked: false,
            overlay_x: 400,
            overlay_y: 650,
            overlay_w: 660,
            overlay_h: 95,
            outgoing_enabled: true,
            selected_mic: "Default".to_string(),
            selected_headphones: "Default".to_string(),
            selected_cable_in: "CABLE Input (VB-Audio Virtual Cable)".to_string(),
            preset_name: "Тактический Foxhole".to_string(),
        }
    }
}

pub fn get_config_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("FOXDS_CONFIG_PATH") {
        if !p.trim().is_empty() {
            return std::path::PathBuf::from(p);
        }
    }
    let cwd_cfg = std::path::PathBuf::from("config.json");
    if cwd_cfg.exists() {
        return cwd_cfg;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let next_to_exe = parent.join("config.json");
            if next_to_exe.exists() {
                return next_to_exe;
            }
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let xdg_dir = std::path::PathBuf::from(home).join(".config").join("foxds-voice-translator");
            let _ = std::fs::create_dir_all(&xdg_dir);
            return xdg_dir.join("config.json");
        }
    }
    std::path::PathBuf::from("config.json")
}

impl AppConfig {
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        let p_ref = path.as_ref();
        let target_path = if p_ref.as_os_str().is_empty() || p_ref == Path::new("config.json") && !p_ref.exists() {
            get_config_path()
        } else {
            p_ref.to_path_buf()
        };

        if target_path.exists() {
            if let Ok(content) = fs::read_to_string(&target_path) {
                if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                    return cfg;
                }
            }
        }
        let default_cfg = Self::default();
        let _ = default_cfg.save(&target_path);
        default_cfg
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), std::io::Error> {
        let p_ref = path.as_ref();
        let target_path = if p_ref.as_os_str().is_empty() || p_ref == Path::new("config.json") && !p_ref.exists() {
            get_config_path()
        } else {
            p_ref.to_path_buf()
        };
        let json = serde_json::to_string_pretty(self)?;
        fs::write(target_path, json)
    }
}
