#![allow(dead_code)]

use crate::audio::{calculate_rms, get_audio_devices, get_beep_done_wav, get_beep_start_wav};
use crate::audio_player::play_tts_audio;
use crate::config::AppConfig;
use crate::hotkeys::is_key_pressed;
use crate::stt::recognize_speech;
use crate::translator::translate_text;
use crate::tts::synthesize_speech;
use parking_lot::Mutex;
use rodio::cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tao::event_loop::EventLoopProxy;

#[derive(Debug, Clone)]
pub enum AppEvent {
    InitConfig(String),
    AudioDevices(String),
    SpeechEvent(String, String, String),
    StatusEvent(String),
    VuEvent(f32, f32, bool),
    OpenSniper,
    UpdateHudStyle {
        font_size: u32,
        alpha: f32,
        border_color: String,
        border_width: u32,
    },
    SetHudPreset(String),
    SetHudLocked(bool),
}

pub struct AudioEngine {
    config: Arc<Mutex<AppConfig>>,
    proxy: EventLoopProxy<AppEvent>,
    is_running: Arc<AtomicBool>,
    is_ptt_held: Arc<AtomicBool>,
    recording_buffer: Arc<Mutex<Vec<f32>>>,
    current_sample_rate: Arc<Mutex<u32>>,
}

impl AudioEngine {
    pub fn new(config: Arc<Mutex<AppConfig>>, proxy: EventLoopProxy<AppEvent>, is_running: Arc<AtomicBool>) -> Self {
        Self {
            config,
            proxy,
            is_running,
            is_ptt_held: Arc::new(AtomicBool::new(false)),
            recording_buffer: Arc::new(Mutex::new(Vec::new())),
            current_sample_rate: Arc::new(Mutex::new(48000)),
        }
    }

    /// Return all audio devices serialized as JSON for the web interface
    pub fn get_devices_json() -> String {
        let (inputs, outputs) = get_audio_devices();
        let payload = serde_json::json!({
            "inputs": inputs,
            "outputs": outputs
        });
        serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
    }

    /// Start the native background audio capture and PTT monitoring threads
    pub fn start(&self) {
        self.spawn_mic_stream();
        self.spawn_hotkeys_loop();
    }

    /// Spawns continuous microphone input stream with F32 and I16 support and RMS VU calculation
    fn spawn_mic_stream(&self) {
        let is_running = self.is_running.clone();
        let is_ptt_held = self.is_ptt_held.clone();
        let recording_buffer = self.recording_buffer.clone();
        let current_sample_rate = self.current_sample_rate.clone();
        let config = self.config.clone();
        let proxy = self.proxy.clone();

        thread::spawn(move || {
            let host = rodio::cpal::default_host();
            let mut last_vu_send = std::time::Instant::now();

            while is_running.load(Ordering::SeqCst) {
                let selected_mic = {
                    let cfg = config.lock();
                    cfg.selected_mic.clone()
                };

                // Find input device
                let device = if selected_mic == "Default" || selected_mic.is_empty() {
                    host.default_input_device()
                } else {
                    host.input_devices()
                        .ok()
                        .and_then(|mut devs| {
                            devs.find(|d| {
                                d.description()
                                    .map(|desc| desc.name().contains(&selected_mic))
                                    .unwrap_or(false)
                            })
                        })
                        .or_else(|| host.default_input_device())
                };

                let dev = match device {
                    Some(d) => d,
                    None => {
                        thread::sleep(Duration::from_millis(1000));
                        continue;
                    }
                };

                let default_cfg = match dev.default_input_config() {
                    Ok(c) => c,
                    Err(_) => {
                        thread::sleep(Duration::from_millis(1000));
                        continue;
                    }
                };

                let sample_rate = default_cfg.sample_rate();
                *current_sample_rate.lock() = sample_rate;
                let channels = default_cfg.channels() as usize;

                let err_fn = |err| eprintln!("[AudioEngine] Mic stream error: {}", err);

                let rec_buf_clone = recording_buffer.clone();
                let is_ptt_clone = is_ptt_held.clone();
                let proxy_clone = proxy.clone();
                let is_running_clone = is_running.clone();
                let cfg_clone = config.clone();

                // Support both F32 and I16 microphone formats (crucial for Windows USB / Realtek drivers)
                let stream_res = match default_cfg.sample_format() {
                    rodio::cpal::SampleFormat::F32 => {
                        dev.build_input_stream(
                            &default_cfg.into(),
                            move |data: &[f32], _| {
                                if !is_running_clone.load(Ordering::SeqCst) {
                                    return;
                                }
                                let mono: Vec<f32> = if channels > 1 {
                                    data.chunks(channels)
                                        .map(|ch| ch.iter().sum::<f32>() / channels as f32)
                                        .collect()
                                } else {
                                    data.to_vec()
                                };
                                Self::process_mic_samples(
                                    &mono,
                                    &cfg_clone,
                                    &rec_buf_clone,
                                    &is_ptt_clone,
                                    &proxy_clone,
                                    &mut last_vu_send,
                                );
                            },
                            err_fn,
                            None,
                        )
                    }
                    rodio::cpal::SampleFormat::I16 => {
                        dev.build_input_stream(
                            &default_cfg.into(),
                            move |data: &[i16], _| {
                                if !is_running_clone.load(Ordering::SeqCst) {
                                    return;
                                }
                                let mono: Vec<f32> = if channels > 1 {
                                    data.chunks(channels)
                                        .map(|ch| (ch.iter().map(|&s| s as f32).sum::<f32>() / channels as f32) / 32768.0)
                                        .collect()
                                } else {
                                    data.iter().map(|&s| s as f32 / 32768.0).collect()
                                };
                                Self::process_mic_samples(
                                    &mono,
                                    &cfg_clone,
                                    &rec_buf_clone,
                                    &is_ptt_clone,
                                    &proxy_clone,
                                    &mut last_vu_send,
                                );
                            },
                            err_fn,
                            None,
                        )
                    }
                    _ => {
                        thread::sleep(Duration::from_millis(1000));
                        continue;
                    }
                };

                if let Ok(s) = stream_res {
                    let _ = s.play();
                    while is_running.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_millis(200));
                    }
                } else {
                    thread::sleep(Duration::from_millis(1000));
                }
            }
        });
    }

    fn process_mic_samples(
        mono: &[f32],
        cfg_arc: &Arc<Mutex<AppConfig>>,
        rec_buf: &Arc<Mutex<Vec<f32>>>,
        is_ptt: &Arc<AtomicBool>,
        proxy: &EventLoopProxy<AppEvent>,
        last_vu: &mut std::time::Instant,
    ) {
        let rms = calculate_rms(mono);
        let mic_gain = { cfg_arc.lock().mic_gain };
        let mic_level = (rms * 350.0 * mic_gain).clamp(0.0, 100.0);

        let ptt_active = is_ptt.load(Ordering::SeqCst);
        if ptt_active {
            let mut buf = rec_buf.lock();
            buf.extend_from_slice(mono);
        }

        if last_vu.elapsed() >= Duration::from_millis(40) {
            *last_vu = std::time::Instant::now();
            let _ = proxy.send_event(AppEvent::VuEvent(mic_level, 0.0, ptt_active));
        }
    }

    /// Spawns the hotkeys monitor (F4 Push-to-Talk + Alt+Q OCR)
    fn spawn_hotkeys_loop(&self) {
        let is_running = self.is_running.clone();
        let is_ptt_held = self.is_ptt_held.clone();
        let recording_buffer = self.recording_buffer.clone();
        let current_sample_rate = self.current_sample_rate.clone();
        let config = self.config.clone();
        let proxy = self.proxy.clone();

        thread::spawn(move || {
            let mut last_f4_state = false;
            let mut last_ocr_state = false;

            while is_running.load(Ordering::SeqCst) {
                let (hotkey, ocr_hotkey, hp_device) = {
                    let cfg = config.lock();
                    (cfg.hotkey.clone(), cfg.ocr_hotkey.clone(), cfg.selected_headphones.clone())
                };

                // --- 1. Push-to-Talk F4 Key ---
                let f4_pressed = is_key_pressed(&hotkey);

                if f4_pressed && !last_f4_state {
                    // F4 Pressed: play tactical start beep immediately!
                    last_f4_state = true;
                    is_ptt_held.store(true, Ordering::SeqCst);
                    {
                        let mut buf = recording_buffer.lock();
                        buf.clear();
                    }
                    let _ = proxy.send_event(AppEvent::StatusEvent("recording".to_string()));

                    let hp = hp_device.clone();
                    thread::spawn(move || {
                        let beep = get_beep_start_wav();
                        let _ = play_tts_audio(&beep, "", &hp, true, 0.4);
                    });
                } else if !f4_pressed && last_f4_state {
                    // F4 Released: play tactical done beep immediately!
                    last_f4_state = false;
                    is_ptt_held.store(false, Ordering::SeqCst);
                    let _ = proxy.send_event(AppEvent::StatusEvent("processing".to_string()));

                    let hp = hp_device.clone();
                    thread::spawn(move || {
                        let beep = get_beep_done_wav();
                        let _ = play_tts_audio(&beep, "", &hp, true, 0.4);
                    });

                    let recorded_samples = {
                        let mut buf = recording_buffer.lock();
                        let taken = buf.clone();
                        buf.clear();
                        taken
                    };

                    let s_rate = *current_sample_rate.lock();
                    let proxy_worker = proxy.clone();
                    let cfg_worker = config.clone();

                    // Process speech pipeline in background
                    thread::spawn(move || {
                        if recorded_samples.len() < (s_rate as f32 * 0.2) as usize {
                            let _ = proxy_worker.send_event(AppEvent::StatusEvent("idle".to_string()));
                            return;
                        }

                        // STT -> Translate -> TTS -> Output to Virtual Cable & Headphones
                        match recognize_speech(&recorded_samples, s_rate, "ru-RU") {
                            Ok((original_text, _conf)) => {
                                let orig = original_text.trim();
                                if orig.is_empty() {
                                    let _ = proxy_worker.send_event(AppEvent::StatusEvent("idle".to_string()));
                                    return;
                                }

                                let _ = proxy_worker.send_event(AppEvent::SpeechEvent(
                                    "outgoing".to_string(),
                                    orig.to_string(),
                                    "".to_string(),
                                ));

                                match translate_text(orig, "ru", "en") {
                                    Ok(translated) => {
                                        let trans = translated.trim();
                                        let _ = proxy_worker.send_event(AppEvent::SpeechEvent(
                                            "outgoing".to_string(),
                                            orig.to_string(),
                                            trans.to_string(),
                                        ));

                                        let (voice, speed, cable, hp, play_self, tts_gain) = {
                                            let cfg = cfg_worker.lock();
                                            (
                                                cfg.voice.clone(),
                                                cfg.speech_speed,
                                                cfg.selected_cable_in.clone(),
                                                cfg.selected_headphones.clone(),
                                                cfg.play_self_audio,
                                                cfg.tts_gain,
                                            )
                                        };

                                        if let Ok(audio_bytes) = synthesize_speech(trans, &voice, speed) {
                                            let _ = play_tts_audio(
                                                &audio_bytes,
                                                &cable,
                                                &hp,
                                                play_self,
                                                tts_gain,
                                            );
                                        }
                                    }
                                    Err(e) => eprintln!("[Translate] Error: {}", e),
                                }
                            }
                            Err(e) => eprintln!("[STT] Error: {}", e),
                        }

                        let _ = proxy_worker.send_event(AppEvent::StatusEvent("idle".to_string()));
                    });
                }

                // --- 2. Screen OCR Alt+Q Key ---
                let ocr_pressed = is_key_pressed(&ocr_hotkey);
                if ocr_pressed && !last_ocr_state {
                    last_ocr_state = true;
                    let _ = proxy.send_event(AppEvent::OpenSniper);
                } else if !ocr_pressed && last_ocr_state {
                    last_ocr_state = false;
                }

                thread::sleep(Duration::from_millis(25));
            }
        });
    }
}
