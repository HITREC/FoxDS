#![allow(dead_code)]

use rodio::cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_input: bool,
    pub is_default: bool,
}

pub struct AudioEngineState {
    pub is_f4_held: Arc<AtomicBool>,
    pub mute_passthrough: Arc<AtomicBool>,
    pub is_recording: Arc<AtomicBool>,
    pub is_playing_tts: Arc<AtomicBool>,
}

impl AudioEngineState {
    pub fn new() -> Self {
        Self {
            is_f4_held: Arc::new(AtomicBool::new(false)),
            mute_passthrough: Arc::new(AtomicBool::new(false)),
            is_recording: Arc::new(AtomicBool::new(false)),
            is_playing_tts: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub fn get_audio_devices() -> (Vec<AudioDeviceInfo>, Vec<AudioDeviceInfo>) {
    let host = rodio::cpal::default_host();
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    let default_in_name = host
        .default_input_device()
        .and_then(|d| d.description().map(|desc| desc.name().to_string()).ok());
    let default_out_name = host
        .default_output_device()
        .and_then(|d| d.description().map(|desc| desc.name().to_string()).ok());

    if let Ok(devices) = host.input_devices() {
        for dev in devices {
            if let Ok(desc) = dev.description() {
                let name = desc.name().to_string();
                let is_default = default_in_name.as_ref().map(|d| d == &name).unwrap_or(false);
                inputs.push(AudioDeviceInfo {
                    name,
                    is_input: true,
                    is_default,
                });
            }
        }
    }

    if let Ok(devices) = host.output_devices() {
        for dev in devices {
            if let Ok(desc) = dev.description() {
                let name = desc.name().to_string();
                let is_default = default_out_name.as_ref().map(|d| d == &name).unwrap_or(false);
                outputs.push(AudioDeviceInfo {
                    name,
                    is_input: false,
                    is_default,
                });
            }
        }
    }

    (inputs, outputs)
}

/// Tactical Radio / Walkie-Talkie Filter (300Hz-3400Hz military bandpass + analog saturation)
pub fn apply_radio_filter(audio: &mut [f32], sample_rate: f32) {
    if audio.is_empty() {
        return;
    }
    let dt = 1.0 / sample_rate;
    let rc_hp = 1.0 / (2.0 * std::f32::consts::PI * 300.0);
    let alpha_hp = rc_hp / (rc_hp + dt);

    let rc_lp = 1.0 / (2.0 * std::f32::consts::PI * 3400.0);
    let alpha_lp = dt / (rc_lp + dt);

    let mut prev_hp_x = 0.0f32;
    let mut prev_hp_y = 0.0f32;
    let mut prev_lp_y = 0.0f32;

    for sample in audio.iter_mut() {
        let x = *sample;
        let hp = alpha_hp * (prev_hp_y + x - prev_hp_x);
        prev_hp_x = x;
        prev_hp_y = hp;

        prev_lp_y += alpha_lp * (hp - prev_lp_y);
        *sample = (prev_lp_y * 1.35).tanh() * 0.92;
    }
}

pub fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Generate a sine wave tone in memory (WAV format bytes)
pub fn generate_beep_wav(frequency: f32, duration_secs: f32, sample_rate: u32, volume: f32) -> Vec<u8> {
    let num_samples = (sample_rate as f32 * duration_secs) as usize;
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let mut sample = (2.0 * std::f32::consts::PI * frequency * t).sin() * volume;
        // Fade in and out envelope to eliminate clicks
        if i < 120 {
            sample *= i as f32 / 120.0;
        } else if i > num_samples.saturating_sub(120) {
            sample *= (num_samples - i) as f32 / 120.0;
        }
        samples.push((sample.clamp(-1.0, 1.0) * 32767.0) as i16);
    }

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = std::io::Cursor::new(Vec::new());
    if let Ok(mut writer) = hound::WavWriter::new(&mut cursor, spec) {
        for s in samples {
            let _ = writer.write_sample(s);
        }
        let _ = writer.finalize();
    }
    cursor.into_inner()
}

/// Tactical radio start beep (750 Hz for 80ms)
pub fn get_beep_start_wav() -> Vec<u8> {
    generate_beep_wav(750.0, 0.08, 44100, 0.3)
}

/// Tactical radio release double-beep (600 Hz 50ms + 900 Hz 70ms)
pub fn get_beep_done_wav() -> Vec<u8> {
    let num_samples = (44100.0 * (0.05 + 0.02 + 0.07)) as usize;
    let mut samples = Vec::with_capacity(num_samples);

    // Part 1: 600 Hz for 50ms
    let n1 = (44100.0 * 0.05) as usize;
    for i in 0..n1 {
        let t = i as f32 / 44100.0;
        let mut s = (2.0 * std::f32::consts::PI * 600.0 * t).sin() * 0.28;
        if i < 80 { s *= i as f32 / 80.0; }
        else if i > n1.saturating_sub(80) { s *= (n1 - i) as f32 / 80.0; }
        samples.push((s * 32767.0) as i16);
    }
    // Pause: 20ms
    let n_pause = (44100.0 * 0.02) as usize;
    samples.extend(std::iter::repeat(0i16).take(n_pause));

    // Part 2: 900 Hz for 70ms
    let n2 = (44100.0 * 0.07) as usize;
    for i in 0..n2 {
        let t = i as f32 / 44100.0;
        let mut s = (2.0 * std::f32::consts::PI * 900.0 * t).sin() * 0.3;
        if i < 80 { s *= i as f32 / 80.0; }
        else if i > n2.saturating_sub(80) { s *= (n2 - i) as f32 / 80.0; }
        samples.push((s * 32767.0) as i16);
    }

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = std::io::Cursor::new(Vec::new());
    if let Ok(mut writer) = hound::WavWriter::new(&mut cursor, spec) {
        for s in samples {
            let _ = writer.write_sample(s);
        }
        let _ = writer.finalize();
    }
    cursor.into_inner()
}
