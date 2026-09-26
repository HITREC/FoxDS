#![allow(dead_code)]

use crate::audio::{calculate_rms, get_audio_devices, get_beep_done_wav, get_beep_start_wav};
use crate::audio_player::play_tts_audio;
use crate::config::AppConfig;
use crate::hotkeys::is_key_pressed;
use crate::stt::recognize_speech;
use crate::translator::translate_text;
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
    GpuInfo(String),
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
    is_incoming_tts_playing: Arc<AtomicBool>,
    last_incoming_tts_time: Arc<Mutex<std::time::Instant>>,
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
            is_incoming_tts_playing: Arc::new(AtomicBool::new(false)),
            last_incoming_tts_time: Arc::new(Mutex::new(std::time::Instant::now())),
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
        crate::translator::warm_up_connection();
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

        // Only intentional loud voice into microphone (> 65%) counts as active speech
        if mic_level > 65.0 {
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
        let is_incoming_tts_playing = self.is_incoming_tts_playing.clone();
        let last_incoming_tts_time = self.last_incoming_tts_time.clone();

        thread::spawn(move || {
            let host = rodio::cpal::default_host();

            while is_running.load(Ordering::SeqCst) {
                let (incoming_enabled, incoming_tts_enabled, selected_hp) = {
                    let cfg = config.lock();
                    (cfg.incoming_enabled, cfg.incoming_tts_enabled, cfg.selected_headphones.clone())
                };

                if !incoming_enabled && !incoming_tts_enabled {
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
                let is_tts_f32 = is_incoming_tts_playing.clone();
                let last_tts_f32 = last_incoming_tts_time.clone();
                let is_tts_i16 = is_incoming_tts_playing.clone();
                let last_tts_i16 = last_incoming_tts_time.clone();

                // VAD state machine variables
                let preroll_capacity = (sample_rate as f32 * 0.14) as usize; // 140ms pre-roll
                let mut preroll_buf: std::collections::VecDeque<f32> = std::collections::VecDeque::with_capacity(preroll_capacity);
                let mut speech_buf: Vec<f32> = Vec::new();
                let mut is_speaking = false;
                let mut peak_rms = 0.0f32;
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
                                    &is_tts_f32,
                                    &last_tts_f32,
                                    &mut preroll_buf,
                                    preroll_capacity,
                                    &mut speech_buf,
                                    &mut is_speaking,
                                    &mut peak_rms,
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
                                    &is_tts_i16,
                                    &last_tts_i16,
                                    &mut preroll_buf,
                                    preroll_capacity,
                                    &mut speech_buf,
                                    &mut is_speaking,
                                    &mut peak_rms,
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
                        let (in_sub, in_tts) = {
                            let cfg = config.lock();
                            (cfg.incoming_enabled, cfg.incoming_tts_enabled)
                        };
                        if !in_sub && !in_tts {
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
        is_tts_playing: &Arc<AtomicBool>,
        last_tts_time: &Arc<Mutex<std::time::Instant>>,
        preroll_buf: &mut std::collections::VecDeque<f32>,
        preroll_cap: usize,
        speech_buf: &mut Vec<f32>,
        is_speaking: &mut bool,
        peak_rms: &mut f32,
        last_speech_time: &mut std::time::Instant,
        speech_start_time: &mut std::time::Instant,
    ) {
        let rms = calculate_rms(mono);
        let spk_level = (rms * 450.0).clamp(0.0, 100.0);
        *spk_level_arc.lock() = spk_level;

        let (incoming_enabled, incoming_tts_enabled, incoming_voice, selected_hp, speech_speed, incoming_tts_gain, rms_threshold, min_confidence, filter_russian, ignore_own_mic, tensor_accel, radio_effect, natural_prosody) = {
            let cfg = cfg_arc.lock();
            (
                cfg.incoming_enabled,
                cfg.incoming_tts_enabled,
                cfg.incoming_voice.clone(),
                cfg.selected_headphones.clone(),
                cfg.speech_speed,
                cfg.incoming_tts_gain,
                cfg.rms_threshold,
                cfg.min_confidence,
                cfg.filter_russian,
                cfg.ignore_own_mic,
                cfg.tensor_accel,
                cfg.radio_effect,
                cfg.natural_prosody,
            )
        };

        if !incoming_enabled && !incoming_tts_enabled {
            if *is_speaking {
                *is_speaking = false;
                speech_buf.clear();
                *peak_rms = 0.0;
            }
            return;
        }

        // Anti-bleed: if user holds PTT F4 or Russian TTS is playing in headphones, drop loopback
        let ptt_held = is_ptt.load(Ordering::SeqCst);
        let tts_playing = is_tts_playing.load(Ordering::SeqCst) || last_tts_time.lock().elapsed() < Duration::from_millis(300);

        if ptt_held || tts_playing {
            if *is_speaking {
                *is_speaking = false;
                speech_buf.clear();
                *peak_rms = 0.0;
            }
            return;
        }

        // If user is shouting into mic (mic_level > 65%), don't trigger a NEW incoming recording
        let mic_loud = ignore_own_mic && mic_speech_arc.lock().elapsed() < Duration::from_millis(400);
        if !*is_speaking && mic_loud {
            return;
        }

        // Sensitive & responsive VAD threshold: scale user setting (5..100, default 35) to RMS (0.0018 .. 0.035)
        let vad_threshold = (rms_threshold / 7000.0).clamp(0.0018, 0.035);

        if rms >= vad_threshold {
            if !*is_speaking {
                *is_speaking = true;
                *speech_start_time = std::time::Instant::now();
                speech_buf.clear();
                // Prepend rolling pre-roll audio so initial syllable is preserved
                speech_buf.extend(preroll_buf.iter());
                *peak_rms = rms;
                let _ = proxy.send_event(AppEvent::StatusEvent("listening".to_string()));
            }
            if rms > *peak_rms {
                *peak_rms = rms;
            }
            speech_buf.extend_from_slice(mono);
            *last_speech_time = std::time::Instant::now();
        } else if *is_speaking {
            speech_buf.extend_from_slice(mono);
            let silence_dur = last_speech_time.elapsed();
            let total_dur = speech_start_time.elapsed();

            // Fast dynamic silence limit: 280ms for longer utterance, 330ms for short callout
            let silence_timeout = if total_dur >= Duration::from_millis(850) {
                Duration::from_millis(280)
            } else {
                Duration::from_millis(330)
            };

            if silence_dur >= silence_timeout || total_dur >= Duration::from_secs(12) {
                *is_speaking = false;
                let utterance = std::mem::take(speech_buf);
                let utterance_peak_rms = *peak_rms;
                *peak_rms = 0.0;
                let _ = proxy.send_event(AppEvent::StatusEvent("idle".to_string()));

                // Minimum speech length 0.18s to allow fast one-word tactical callouts
                let min_samples = (sample_rate as f32 * 0.18) as usize;
                if utterance.len() >= min_samples {
                    let proxy_worker = proxy.clone();
                    let tts_flag_worker = is_tts_playing.clone();
                    let tts_time_worker = last_tts_time.clone();
                    thread::spawn(move || {
                        Self::recognize_and_translate_incoming(
                            &utterance,
                            sample_rate,
                            utterance_peak_rms,
                            filter_russian,
                            min_confidence,
                            &proxy_worker,
                            incoming_enabled,
                            incoming_tts_enabled,
                            &incoming_voice,
                            &selected_hp,
                            speech_speed,
                            incoming_tts_gain,
                            tts_flag_worker,
                            tts_time_worker,
                            tensor_accel,
                            radio_effect,
                            natural_prosody,
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

    /// Background task for recognizing incoming teammate speech, filtering Russian, and displaying translation / speaking TTS
    fn recognize_and_translate_incoming(
        samples: &[f32],
        sample_rate: u32,
        peak_rms: f32,
        filter_russian: bool,
        _min_confidence: f32,
        proxy: &EventLoopProxy<AppEvent>,
        incoming_enabled: bool,
        incoming_tts_enabled: bool,
        incoming_voice: &str,
        selected_hp: &str,
        speech_speed: u32,
        incoming_tts_gain: f32,
        is_tts_playing: Arc<AtomicBool>,
        last_tts_time: Arc<Mutex<std::time::Instant>>,
        tensor_accel: bool,
        radio_effect: bool,
        natural_prosody: bool,
    ) {
        match recognize_speech(samples, sample_rate, "en-US") {
            Ok((recognized, _confidence)) => {
                let clean = recognized.trim();
                if clean.is_empty() {
                    return;
                }

                // Russian filter: if teammate spoke Russian, do not duplicate into subtitles
                if filter_russian && contains_cyrillic(clean) {
                    return;
                }

                // Format original English speech with emotion/intonation (ALL CAPS if shouted)
                let formatted_en = format_with_emotion(clean, peak_rms);

                // Translate English speech to Russian
                match translate_text(&formatted_en.display_text, "en", "ru") {
                    Ok(trans) => {
                        let trans_clean = trans.trim();
                        if !trans_clean.is_empty() {
                            // Format Russian translation preserving the same emotion (ALL CAPS if shouted)
                            let formatted_ru = format_with_emotion(trans_clean, peak_rms);

                            if incoming_enabled {
                                let _ = proxy.send_event(AppEvent::SpeechEvent(
                                    "incoming".to_string(),
                                    formatted_en.display_text.clone(),
                                    formatted_ru.display_text.clone(),
                                ));
                            }

                            if incoming_tts_enabled {
                                let voice = incoming_voice.to_string();
                                let hp_name = selected_hp.to_string();
                                let trans_text = formatted_ru.display_text.clone();
                                let is_shout = formatted_ru.is_shout;
                                let flag = is_tts_playing.clone();
                                let last_t = last_tts_time.clone();
                                thread::spawn(move || {
                                    match crate::tts::synthesize_speech_advanced(&trans_text, &voice, speech_speed, is_shout, natural_prosody) {
                                        Ok(audio) => {
                                            let _ = crate::audio_player::play_headphones_audio_dsp(
                                                &audio,
                                                &hp_name,
                                                incoming_tts_gain,
                                                Some(flag),
                                                Some(last_t),
                                                tensor_accel,
                                                tensor_accel,
                                                radio_effect,
                                            );
                                        }
                                        Err(e) => eprintln!("[Incoming TTS] Synthesis error: {}", e),
                                    }
                                });
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[Incoming Translate] Error: {}", e);
                        if incoming_enabled {
                            let _ = proxy.send_event(AppEvent::SpeechEvent(
                                "incoming".to_string(),
                                formatted_en.display_text.clone(),
                                "[Ошибка сети перевода]".to_string(),
                            ));
                        }
                    }
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

                        let peak_rms = calculate_peak_rms(&recorded_samples, (s_rate as f32 * 0.02) as usize);

                        // STT -> Translate -> TTS -> Output to Virtual Cable & Headphones
                        match recognize_speech(&recorded_samples, s_rate, "ru-RU") {
                            Ok((original_text, _conf)) => {
                                let orig = original_text.trim();
                                if orig.is_empty() {
                                    let _ = proxy_worker.send_event(AppEvent::StatusEvent("idle".to_string()));
                                    return;
                                }

                                let formatted_ru = format_with_emotion(orig, peak_rms);

                                let _ = proxy_worker.send_event(AppEvent::SpeechEvent(
                                    "outgoing".to_string(),
                                    formatted_ru.display_text.clone(),
                                    "".to_string(),
                                ));

                                match translate_text(&formatted_ru.display_text, "ru", "en") {
                                    Ok(translated) => {
                                        let trans = translated.trim();
                                        let formatted_en = format_with_emotion(trans, peak_rms);

                                        let _ = proxy_worker.send_event(AppEvent::SpeechEvent(
                                            "outgoing".to_string(),
                                            formatted_ru.display_text,
                                            formatted_en.display_text.clone(),
                                        ));

                                        let (voice, speed, cable, hp, play_self, tts_gain, tensor_accel, radio_effect, natural_prosody) = {
                                            let cfg = cfg_worker.lock();
                                            (
                                                cfg.voice.clone(),
                                                cfg.speech_speed,
                                                cfg.selected_cable_in.clone(),
                                                cfg.selected_headphones.clone(),
                                                cfg.play_self_audio,
                                                cfg.tts_gain,
                                                cfg.tensor_accel,
                                                cfg.radio_effect,
                                                cfg.natural_prosody,
                                            )
                                        };

                                        match crate::tts::synthesize_speech_advanced(
                                            &formatted_en.display_text,
                                            &voice,
                                            speed,
                                            formatted_en.is_shout,
                                            natural_prosody,
                                        ) {
                                            Ok(audio_bytes) => {
                                                if let Err(e) = crate::audio_player::play_tts_audio_dsp(
                                                    &audio_bytes,
                                                    &cable,
                                                    &hp,
                                                    play_self,
                                                    tts_gain,
                                                    tensor_accel,
                                                    tensor_accel,
                                                    radio_effect,
                                                ) {
                                                    eprintln!("[AudioPlayer] Error playing outgoing TTS: {}", e);
                                                }
                                            }
                                            Err(e) => eprintln!("[Outgoing TTS] Synthesis error: {}", e),
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

#[derive(Debug, Clone)]
pub struct FormattedSpeech {
    pub display_text: String,
    pub is_shout: bool,
    pub is_question: bool,
}

pub fn calculate_peak_rms(samples: &[f32], chunk_size: usize) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let step = chunk_size.max(256);
    let mut peak = 0.0f32;
    for chunk in samples.chunks(step) {
        let rms = calculate_rms(chunk);
        if rms > peak {
            peak = rms;
        }
    }
    peak
}

pub fn format_with_emotion(text: &str, peak_rms: f32) -> FormattedSpeech {
    let clean = text.trim();
    if clean.is_empty() {
        return FormattedSpeech {
            display_text: String::new(),
            is_shout: false,
            is_question: false,
        };
    }

    // Acoustic shouting threshold (RMS >= 0.065 is clearly loud/shouted speech in voice chat / mic)
    let acoustic_shout = peak_rms >= 0.065;
    let alpha_chars: Vec<char> = clean.chars().filter(|c| c.is_alphabetic()).collect();
    let text_all_caps = alpha_chars.len() >= 3 && alpha_chars.iter().all(|c| c.is_uppercase());
    let text_exclaim = clean.contains('!') || text_all_caps;
    let is_shout = acoustic_shout || text_exclaim;

    // Question detection: ending question mark or typical question words
    let clean_lower = clean.to_lowercase();
    let is_question = clean.ends_with('?')
        || clean_lower.starts_with("where ")
        || clean_lower.starts_with("who ")
        || clean_lower.starts_with("what ")
        || clean_lower.starts_with("why ")
        || clean_lower.starts_with("how ")
        || clean_lower.starts_with("when ")
        || clean_lower.starts_with("is ")
        || clean_lower.starts_with("are ")
        || clean_lower.starts_with("can ")
        || clean_lower.starts_with("где ")
        || clean_lower.starts_with("кто ")
        || clean_lower.starts_with("что ")
        || clean_lower.starts_with("почему ")
        || clean_lower.starts_with("зачем ")
        || clean_lower.starts_with("куда ")
        || clean_lower.starts_with("откуда ")
        || clean_lower.starts_with("как ");

    let mut result = clean.to_string();

    if is_shout {
        result = result.to_uppercase();
        if is_question {
            if !result.ends_with("?!") && !result.ends_with("!?") {
                if result.ends_with('?') {
                    result.pop();
                }
                result.push_str("?!");
            }
        } else if !result.ends_with('!') {
            result.push('!');
        }
    } else if is_question && !result.ends_with('?') {
        result.push('?');
    }

    FormattedSpeech {
        display_text: result,
        is_shout,
        is_question,
    }
}

fn contains_cyrillic(text: &str) -> bool {
    text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emotion_shout_caps() {
        // Normal speech
        let normal = format_with_emotion("enemy spotted", 0.02);
        assert!(!normal.is_shout);
        assert_eq!(normal.display_text, "enemy spotted");

        // Acoustic shouting (peak RMS >= 0.065)
        let shout = format_with_emotion("watch out sniper", 0.09);
        assert!(shout.is_shout);
        assert_eq!(shout.display_text, "WATCH OUT SNIPER!");

        // Question detection
        let q = format_with_emotion("where is the tank", 0.03);
        assert!(q.is_question);
        assert_eq!(q.display_text, "where is the tank?");

        // Shouted question
        let q_shout = format_with_emotion("what are you doing", 0.11);
        assert!(q_shout.is_shout);
        assert!(q_shout.is_question);
        assert_eq!(q_shout.display_text, "WHAT ARE YOU DOING?!");
    }

    #[test]
    fn test_cyrillic_filter() {
        assert!(contains_cyrillic("Привет"));
        assert!(contains_cyrillic("Hello мир"));
        assert!(!contains_cyrillic("Hello world, push B!"));
        assert!(!contains_cyrillic("Enemy spotted!"));
    }
}
