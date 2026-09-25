#![allow(dead_code)]

use flacenc::bitsink::ByteSink;
use flacenc::component::BitRepr;
use flacenc::config::Encoder;
use flacenc::error::Verify;
use flacenc::source::MemSource;
use reqwest::blocking::Client;
use std::sync::OnceLock;
use std::time::Duration;

static STT_CLIENT: OnceLock<Client> = OnceLock::new();

fn get_client() -> &'static Client {
    STT_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_default()
    })
}

/// Convert float audio samples at any sample rate to 16-bit 16000Hz mono PCM
pub fn resample_to_16k(input: &[f32], source_rate: u32) -> Vec<i32> {
    if input.is_empty() {
        return Vec::new();
    }
    if source_rate == 16000 {
        return input
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i32)
            .collect();
    }

    let ratio = source_rate as f64 / 16000.0;
    let target_len = (input.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(target_len);

    for i in 0..target_len {
        let src_idx = i as f64 * ratio;
        let idx0 = src_idx.floor() as usize;
        let idx1 = (idx0 + 1).min(input.len().saturating_sub(1));
        let frac = (src_idx - idx0 as f64) as f32;

        let sample = if idx0 < input.len() {
            input[idx0] * (1.0 - frac) + input[idx1] * frac
        } else {
            0.0
        };
        output.push((sample.clamp(-1.0, 1.0) * 32767.0) as i32);
    }
    output
}

/// Encode 16kHz mono 16-bit audio samples into FLAC format bytes in memory
pub fn encode_flac(samples: &[i32]) -> Result<Vec<u8>, String> {
    if samples.is_empty() {
        return Err("No samples to encode".to_string());
    }

    let config = Encoder::default()
        .into_verified()
        .map_err(|e| format!("FLAC config error: {:?}", e))?;

    let source = MemSource::from_samples(samples, 1, 16, 16000);
    let flac_stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| format!("FLAC encode error: {:?}", e))?;

    let mut sink = ByteSink::new();
    flac_stream
        .write(&mut sink)
        .map_err(|e| format!("FLAC write error: {:?}", e))?;

    Ok(sink.into_inner())
}

/// Recognize speech using Google Chromium Speech-to-Text API (Pure Rust)
pub fn recognize_speech(audio_samples: &[f32], sample_rate: u32, lang: &str) -> Result<(String, f32), String> {
    if audio_samples.is_empty() {
        return Err("Empty audio buffer".to_string());
    }

    // 1. Resample to 16kHz 16-bit PCM
    let pcm16 = resample_to_16k(audio_samples, sample_rate);

    // 2. Encode to FLAC
    let flac_bytes = encode_flac(&pcm16)?;

    // 3. Post to Google Chromium Speech API
    let url = format!(
        "https://www.google.com/speech-api/v2/recognize?client=chromium&lang={}&key=AIzaSyBOti4mM-6x9WDnZIjIeyEU21OpBXqWBgw&pFilter=0",
        lang
    );

    let client = get_client();
    let resp = client
        .post(&url)
        .header("Content-Type", "audio/x-flac; rate=16000")
        .body(flac_bytes)
        .send()
        .map_err(|e| format!("Google STT network error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Google STT HTTP status: {}", resp.status()));
    }

    let text = resp
        .text()
        .map_err(|e| format!("Google STT read error: {}", e))?;

    // 4. Parse Google response lines
    for line in text.lines() {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(res) = json.get("result").and_then(|r| r.as_array()) {
                if let Some(first) = res.first() {
                    if let Some(alt) = first.get("alternative").and_then(|a| a.as_array()) {
                        if let Some(first_alt) = alt.first() {
                            let transcript = first_alt
                                .get("transcript")
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .to_string();
                            let confidence = first_alt
                                .get("confidence")
                                .and_then(|c| c.as_f64())
                                .unwrap_or(0.9) as f32;
                            if !transcript.is_empty() {
                                return Ok((transcript, confidence));
                            }
                        }
                    }
                }
            }
        }
    }

    Err("No speech recognized".to_string())
}
