#![allow(dead_code)]

use sha2::{Digest, Sha256};
use std::time::SystemTime;
use tungstenite::client::IntoClientRequest;
use tungstenite::Message;

const WIN_EPOCH: u64 = 11644473600;
const TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const WSS_URL: &str = "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1";

pub fn generate_sec_ms_gec() -> String {
    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut ticks = now + WIN_EPOCH;
    ticks -= ticks % 300;
    ticks *= 10_000_000;

    let str_to_hash = format!("{}{}", ticks, TRUSTED_CLIENT_TOKEN);
    let mut hasher = Sha256::new();
    hasher.update(str_to_hash.as_bytes());
    let result = hasher.finalize();

    let mut hex = String::with_capacity(64);
    for b in result {
        hex.push_str(&format!("{:02X}", b));
    }
    hex
}

pub fn generate_request_id() -> String {
    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut hasher = Sha256::new();
    hasher.update(format!("req_{}", now).as_bytes());
    let res = hasher.finalize();
    let mut hex = String::with_capacity(32);
    for b in &res[..16] {
        hex.push_str(&format!("{:02x}", b));
    }
    hex
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Synthesize text to MP3 audio bytes using Microsoft Edge-TTS neural speech service
pub fn synthesize_speech(text: &str, voice: &str, speed_percent: u32) -> Result<Vec<u8>, String> {
    synthesize_speech_opt(text, voice, speed_percent, false)
}

/// Synthesize text to MP3 with emotion/shouting support
pub fn synthesize_speech_opt(text: &str, voice: &str, speed_percent: u32, is_shout: bool) -> Result<Vec<u8>, String> {
    let clean_text = text.trim();
    if clean_text.is_empty() {
        return Ok(Vec::new());
    }

    let gec = generate_sec_ms_gec();
    let conn_id = generate_request_id();
    let url_str = format!(
        "{}?TrustedClientToken={}&ConnectionId={}&Sec-MS-GEC={}&Sec-MS-GEC-Version=1-143.0.3650.75",
        WSS_URL, TRUSTED_CLIENT_TOKEN, conn_id, gec
    );

    let mut request = url_str
        .into_client_request()
        .map_err(|e| format!("Invalid WebSocket URL: {}", e))?;

    let headers = request.headers_mut();
    headers.insert(
        "User-Agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0"
            .parse()
            .unwrap(),
    );
    headers.insert(
        "Origin",
        "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold"
            .parse()
            .unwrap(),
    );
    headers.insert(
        "Accept-Encoding",
        "gzip, deflate, br, zstd".parse().unwrap(),
    );
    headers.insert(
        "Accept-Language",
        "en-US,en;q=0.9".parse().unwrap(),
    );
    headers.insert(
        "Pragma",
        "no-cache".parse().unwrap(),
    );
    headers.insert(
        "Cache-Control",
        "no-cache".parse().unwrap(),
    );
    let muid = generate_request_id().to_uppercase();
    headers.insert(
        "Cookie",
        format!("muid={};", muid).parse().unwrap(),
    );

    let (mut socket, _resp) = tungstenite::connect(request)
        .map_err(|e| format!("Edge-TTS WebSocket connect failed: {}", e))?;

    // 1. Send speech.config
    let config_msg = "Content-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n{\"context\":{\"synthesis\":{\"audio\":{\"metadataoptions\":{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"false\"},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}\r\n";
    socket
        .send(Message::Text(config_msg.to_string().into()))
        .map_err(|e| format!("Failed to send config: {}", e))?;

    // 2. Format rate and prosody with shouting emotion support
    let (volume_str, pitch_str, rate_delta) = if is_shout {
        ("+25%", "+3Hz", 10)
    } else {
        ("+0%", "+0Hz", 0)
    };

    let rate_val = (speed_percent as i32 - 100 + rate_delta).clamp(-50, 100);
    let rate_str = if rate_val >= 0 {
        format!("+{}%", rate_val)
    } else {
        format!("{}%", rate_val)
    };

    let req_id = generate_request_id();
    let escaped = escape_xml(clean_text);
    let xml_lang = if voice.to_lowercase().starts_with("ru-") {
        "ru-RU"
    } else {
        "en-US"
    };
    let ssml_body = format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='{}'><voice name='{}'><prosody rate='{}' pitch='{}' volume='{}'>{}</prosody></voice></speak>",
        xml_lang, voice, rate_str, pitch_str, volume_str, escaped
    );

    let ssml_msg = format!(
        "X-RequestId:{}\r\nContent-Type:application/ssml+xml\r\nPath:ssml\r\n\r\n{}",
        req_id, ssml_body
    );

    socket
        .send(Message::Text(ssml_msg.into()))
        .map_err(|e| format!("Failed to send SSML: {}", e))?;

    // 3. Receive audio chunks until turn.end
    let mut audio_data = Vec::new();

    loop {
        let msg = match socket.read() {
            Ok(m) => m,
            Err(e) => {
                if !audio_data.is_empty() {
                    break;
                }
                return Err(format!("Edge-TTS read error: {}", e));
            }
        };

        match msg {
            Message::Text(txt) => {
                if txt.contains("Path:turn.end") {
                    break;
                }
            }
            Message::Binary(bin) => {
                if bin.len() >= 2 {
                    let header_len = u16::from_be_bytes([bin[0], bin[1]]) as usize;
                    if bin.len() > 2 + header_len {
                        let chunk = &bin[2 + header_len..];
                        audio_data.extend_from_slice(chunk);
                    }
                }
            }
            Message::Close(_) => break,
            _ => (),
        }
    }

    if audio_data.is_empty() {
        return Err("Edge-TTS returned no audio data".to_string());
    }

    Ok(audio_data)
}
