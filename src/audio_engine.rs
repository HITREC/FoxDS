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
        text_color: String,
    },
    ResizeHud {
        width: u32,
        height: u32,
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
    last_mic_speech: Arc<Mutex<std::time::Instant>>,
    last_mic_level: Arc<Mutex<f32>>,
    last_spk_level: Arc<Mutex<f32>>,
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
            last_mic_speech: Arc::new(Mutex::new(std::time::Instant::now())),
            last_mic_level: Arc::new(Mutex::new(0.0)),
            last_spk_level: Arc::new(Mutex::new(0.0)),
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

    /// Start the native background audio capture, loopback monitoring, VU ticker, and PTT hotkeys
    pub fn start(&self) {
        self.spawn_mic_stream();
        self.spawn_loopback_stream();
        self.spawn_vu_ticker();
        self.spawn_hotkeys_loop();
    }

    /// Spawns 30fps smooth VU meter ticker with decay for both Mic and Discord/Game audio
    fn spawn_vu_ticker(&self) {
        let is_running = self.is_running.clone();
        let proxy = self.proxy.clone();
        let last_mic = self.last_mic_level.clone();
        let last_spk = self.last_spk_level.clone();
        let is_ptt = self.is_ptt_held.clone();

        thread::spawn(move || {
            let mut smooth_mic = 0.0f32;
            let mut smooth_spk = 0.0f32;

            while is_running.load(Ordering::SeqCst) {
                let cur_mic = {
                    let mut m = last_mic.lock();
                    let val = *m;
                    *m = val * 0.72; // natural decay
                    val
                };
                let cur_spk = {
                    let mut s = last_spk.lock();
                    let val = *s;
                    *s = val * 0.72; // natural decay
                    val
                };

                smooth_mic = smooth_mic * 0.35 + cur_mic * 0.65;
                smooth_spk = smooth_spk * 0.35 + cur_spk * 0.65;

                let ptt = is_ptt.load(Ordering::SeqCst);
                let _ = proxy.send_event(AppEvent::VuEvent(smooth_mic, smooth_spk, ptt));

                thread::sleep(Duration::from_millis(35));
            }
        });
    }

    /// Spawns continuous microphone input stream with F32 and I16 support and RMS VU calculation
    fn spawn_mic_stream(&self) {
        let is_running = self.is_running.clone();
        let is_ptt_held = self.is_ptt_held.clone();
        let recording_buffer = self.recording_buffer.clone();
        let current_sample_rate = self.current_sample_rate.clone();
        let config = self.config.clone();
        let last_mic_level = self.last_mic_level.clone();
        let last_mic_speech = self.last_mic_speech.clone();

        thread::spawn(move || {
            let host = rodio::cpal::default_host();

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
                let is_running_clone = is_running.clone();
                let cfg_clone = config.clone();
                let mic_level_clone = last_mic_level.clone();
                let mic_speech_clone = last_mic_speech.clone();

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
                                    &mic_level_clone,
                                    &mic_speech_clone,
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
                                    &mic_level_clone,
                                    &mic_speech_clone,
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
        mic_level_arc: &Arc<Mutex<f32>>,
        mic_speech_arc: &Arc<Mutex<std::time::Instant>>,
    ) {
        let rms = calculate_rms(mono);
        let mic_gain = { cfg_arc.lock().mic_gain };
        let mic_level = (rms * 350.0 * mic_gain).clamp(0.0, 100.0);

        *mic_level_arc.lock() = mic_level;

        if mic_level > 15.0 {
            *mic_speech_arc.lock() = std::time::Instant::now();
        }

        let ptt_active = is_ptt.load(Ordering::SeqCst);
        if ptt_active {
            *mic_speech_arc.lock() = std::time::Instant::now();
            let mut buf = rec_buf.lock();
            buf.extend_from_slice(mono);
        }
    }

    /// Spawns continuous WASAPI Loopback capture from Headphones/Discord/Game audio,
    /// runs real-time Voice Activity Detection (VAD) with rolling pre-roll buffer,
    /// performs Google STT (en-US), filters Russian speech, translates to RU, and displays subtitles.
    fn spawn_loopback_stream(&self) {
        let is_running = self.is_running.clone();
        let is_ptt_held = self.is_ptt_held.clone();
        let config = self.config.clone();
        let proxy = self.proxy.clone();
        let last_spk_level = self.last_spk_level.clone();
        let last_mic_speech = self.last_mic_speech.clone();

        thread::spawn(move || {
            let host = rodio::cpal::default_host();

            while is_running.load(Ordering::SeqCst) {
                let (incoming_enabled, selected_hp) = {
                    let cfg = config.lock();
                    (cfg.incoming_enabled, cfg.selected_headphones.clone())
                };

                if !incoming_enabled {
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }

                // Find the render / output device to capture from via WASAPI loopback
                let device = if selected_hp == "Default" || selected_hp.is_empty() {
                    host.default_output_device()
                } else {
                    host.output_devices()
                        .ok()
                        .and_then(|mut devs| {
                            devs.find(|d| {
                                d.description()
                                    .map(|desc| desc.name().contains(&selected_hp))
                                    .unwrap_or(false)
                            })
                        })
                        .or_else(|| host.default_output_device())
                };

                let dev = match device {
                    Some(d) => d,
                    None => {
                        thread::sleep(Duration::from_millis(1000));
                        continue;
                    }
                };

                let default_cfg = match dev.default_output_config() {
                    Ok(c) => c,
                    Err(_) => {
                        thread::sleep(Duration::from_millis(1000));
                        continue;
                    }
                };

                let sample_rate = default_cfg.sample_rate();
                let channels = default_cfg.channels() as usize;
                let sample_format = default_cfg.sample_format();
                let stream_config: rodio::cpal::StreamConfig = default_cfg.into();

                let err_fn = |err| eprintln!("[AudioEngine] Loopback stream error: {}", err);

                let is_running_clone = is_running.clone();
                let is_ptt_clone = is_ptt_held.clone();
                let cfg_clone = config.clone();
                let proxy_clone = proxy.clone();
                let spk_level_clone = last_spk_level.clone();
                let mic_speech_clone = last_mic_speech.clone();

                // VAD state machine variables
                let preroll_capacity = (sample_rate as f32 * 0.25) as usize; // 250ms pre-roll
                let mut preroll_buf: std::collections::VecDeque<f32> = std::collections::VecDeque::with_capacity(preroll_capacity);
                let mut speech_buf: Vec<f32> = Vec::new();
                let mut is_speaking = false;
                let mut last_speech_time = std::time::Instant::now();
                let mut speech_start_time = std::time::Instant::now();

                let stream_res = match sample_format {
                    rodio::cpal::SampleFormat::F32 => {
                        dev.build_input_stream(
                            &stream_config,
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

                                Self::process_loopback_chunk(
                                    &mono,
                                    sample_rate,
                                    &cfg_clone,
                                    &proxy_clone,
                                    &is_ptt_clone,
                                    &spk_level_clone,
                                    &mic_speech_clone,
                                    &mut preroll_buf,
                                    preroll_capacity,
                                    &mut speech_buf,
                                    &mut is_speaking,
                                    &mut last_speech_time,
                                    &mut speech_start_time,
                                );
                            },
                            err_fn,
                            None,
                        )
                    }
                    rodio::cpal::SampleFormat::I16 => {
                        dev.build_input_stream(
                            &stream_config,
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

                                Self::process_loopback_chunk(
                                    &mono,
                                    sample_rate,
                                    &cfg_clone,
                                    &proxy_clone,
                                    &is_ptt_clone,
                                    &spk_level_clone,
                                    &mic_speech_clone,
                                    &mut preroll_buf,
                                    preroll_capacity,
                                    &mut speech_buf,
                                    &mut is_speaking,
                                    &mut last_speech_time,
                                    &mut speech_start_time,
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
                        let incoming_on = { config.lock().incoming_enabled };
                        if !incoming_on {
                            break;
                        }
                        thread::sleep(Duration::from_millis(250));
                    }
                } else {
                    thread::sleep(Duration::from_millis(1000));
                }
            }
        });
    }

    fn process_loopback_chunk(
        mono: &[f32],
        sample_rate: u32,
        cfg_arc: &Arc<Mutex<AppConfig>>,
        proxy: &EventLoopProxy<AppEvent>,
        is_ptt: &Arc<AtomicBool>,
        spk_level_arc: &Arc<Mutex<f32>>,
        mic_speech_arc: &Arc<Mutex<std::time::Instant>>,
        preroll_buf: &mut std::collections::VecDeque<f32>,
        preroll_cap: usize,
        speech_buf: &mut Vec<f32>,
        is_speaking: &mut bool,
        last_speech_time: &mut std::time::Instant,
        speech_start_time: &mut std::time::Instant,
    ) {
        let rms = calculate_rms(mono);
        let spk_level = (rms * 450.0).clamp(0.0, 100.0);
        *spk_level_arc.lock() = spk_level;

        let (incoming_enabled, rms_threshold, min_confidence, filter_russian, ignore_own_mic) = {
            let cfg = cfg_arc.lock();
            (
                cfg.incoming_enabled,
                cfg.rms_threshold,
                cfg.min_confidence,
                cfg.filter_russian,
                cfg.ignore_own_mic,
            )
        };

        if !incoming_enabled {
            if *is_speaking {
                *is_speaking = false;
                speech_buf.clear();
            }
            return;
        }

        // Anti-bleed check: if user is holding PTT F4 or spoke into mic recently (<650ms), ignore loopback
        let ptt_held = is_ptt.load(Ordering::SeqCst);
        let mic_recent = ignore_own_mic && mic_speech_arc.lock().elapsed() < Duration::from_millis(650);

        if ptt_held || mic_recent {
            if *is_speaking {
                *is_speaking = false;
                speech_buf.clear();
            }
            return;
        }

        // VAD threshold: scale user setting (5..100, default 35) to RMS (0.002 .. 0.040)
        let vad_threshold = (rms_threshold / 2500.0).clamp(0.002, 0.080);

        if rms >= vad_threshold {
            if !*is_speaking {
                *is_speaking = true;
                *speech_start_time = std::time::Instant::now();
                speech_buf.clear();
                // Prepend rolling pre-roll audio so initial syllable isn't lost
                speech_buf.extend(preroll_buf.iter());
                let _ = proxy.send_event(AppEvent::StatusEvent("listening".to_string()));
            }
            speech_buf.extend_from_slice(mono);
            *last_speech_time = std::time::Instant::now();
        } else if *is_speaking {
            speech_buf.extend_from_slice(mono);
            let silence_dur = last_speech_time.elapsed();
            let total_dur = speech_start_time.elapsed();

            // End of utterance detected after 500ms of silence or max 14s duration
            if silence_dur >= Duration::from_millis(500) || total_dur >= Duration::from_secs(14) {
                *is_speaking = false;
                let utterance = std::mem::take(speech_buf);
                let _ = proxy.send_event(AppEvent::StatusEvent("idle".to_string()));

                // Minimum speech length 0.30s to filter short clicks / noise
                let min_samples = (sample_rate as f32 * 0.30) as usize;
                if utterance.len() >= min_samples {
                    let proxy_worker = proxy.clone();
                    thread::spawn(move || {
                        Self::recognize_and_translate_incoming(
                            &utterance,
                            sample_rate,
                            filter_russian,
                            min_confidence,
                            &proxy_worker,
                        );
                    });
                }
            }
        } else {
            // When idle, keep rolling pre-roll buffer
            for &s in mono {
                if preroll_buf.len() >= preroll_cap {
                    preroll_buf.pop_front();
                }
                preroll_buf.push_back(s);
            }
        }
    }

    /// Background task for recognizing incoming teammate speech, filtering Russian, and displaying translation
    fn recognize_and_translate_incoming(
        samples: &[f32],
        sample_rate: u32,
        filter_russian: bool,
        min_confidence: f32,
        proxy: &EventLoopProxy<AppEvent>,
    ) {
        match recognize_speech(samples, sample_rate, "en-US") {
            Ok((recognized, confidence)) => {
                let clean = recognized.trim();
                if clean.is_empty() {
                    return;
                }

                // Check AI confidence threshold (lenient so game chatter is never dropped)
                if confidence < (min_confidence * 0.6).max(0.30) {
                    return;
                }

                // Russian filter: if teammate spoke Russian, do not duplicate into subtitles
                if filter_russian && contains_cyrillic(clean) {
                    return;
                }

                // Translate English speech to Russian
                match translate_text(clean, "en", "ru") {
                    Ok(trans) => {
                        let trans_clean = trans.trim();
                        if !trans_clean.is_empty() {
                            let _ = proxy.send_event(AppEvent::SpeechEvent(
                                "incoming".to_string(),
                                clean.to_string(),
                                trans_clean.to_string(),
                            ));
                        }
                    }
                    Err(e) => eprintln!("[Incoming Translate] Error: {}", e),
                }
            }
            Err(_) => {
                // Background game audio / inaudible sound - silently ignore
            }
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

fn contains_cyrillic(text: &str) -> bool {
    text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpal_wasapi_loopback() {
        let host = rodio::cpal::default_host();
        if let Some(out_dev) = host.default_output_device() {
            println!("Default output device: {:?}", out_dev.description().map(|d| d.name().to_string()));
            if let Ok(out_cfg) = out_dev.default_output_config() {
                println!("Default output config: sample_rate={}, channels={}, format={:?}",
                    out_cfg.sample_rate(), out_cfg.channels(), out_cfg.sample_format());
                let config: rodio::cpal::StreamConfig = out_cfg.into();
                let stream_res = out_dev.build_input_stream(
                    &config,
                    move |_data: &[f32], _| {},
                    move |err| eprintln!("Error: {}", err),
                    None,
                );
                println!("build_input_stream on output device result: {:?}", stream_res.is_ok());
                assert!(stream_res.is_ok());
            }
        }
    }

    #[test]
    fn test_cyrillic_filter() {
        assert!(contains_cyrillic("Привет"));
        assert!(contains_cyrillic("Hello мир"));
        assert!(!contains_cyrillic("Hello world, push B!"));
        assert!(!contains_cyrillic("Enemy spotted!"));
    }
}
