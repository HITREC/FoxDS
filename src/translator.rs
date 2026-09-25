#![allow(dead_code)]

use reqwest::blocking::Client;
use std::sync::OnceLock;
use std::time::Duration;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

pub fn get_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(90))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_default()
    })
}

/// Fast Neural Google Translation (dict-chrome-ex) with fallback to MyMemory
pub fn translate_text(text: &str, from_lang: &str, to_lang: &str) -> Result<String, String> {
    let clean = text.trim();
    if clean.is_empty() {
        return Ok(String::new());
    }

    let encoded = urlencoding(clean);
    let client = get_client();

    // 1. Primary: Google Translate dict-chrome-ex endpoint (~40ms latency)
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?client=dict-chrome-ex&sl={}&tl={}&dt=t&q={}",
        from_lang, to_lang, encoded
    );

    if let Ok(resp) = client.get(&url).send() {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<serde_json::Value>() {
                if let Some(sentences) = json.get(0).and_then(|v| v.as_array()) {
                    let mut result = String::new();
                    for s in sentences {
                        if let Some(trans) = s.get(0).and_then(|v| v.as_str()) {
                            result.push_str(trans);
                        }
                    }
                    if !result.is_empty() {
                        return Ok(result);
                    }
                }
            }
        }
    }

    // 2. Secondary fallback: MyMemory Translation API
    let mm_url = format!(
        "https://api.mymemory.translated.net/get?q={}&langpair={}|{}",
        encoded, from_lang, to_lang
    );
    if let Ok(resp) = client.get(&mm_url).send() {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<serde_json::Value>() {
                if let Some(trans) = json
                    .get("responseData")
                    .and_then(|d| d.get("translatedText"))
                    .and_then(|t| t.as_str())
                {
                    if !trans.is_empty() && !trans.contains("MYMEMORY WARNING") {
                        return Ok(trans.to_string());
                    }
                }
            }
        }
    }

    // Return original text if network failed
    Ok(clean.to_string())
}

pub fn urlencoding(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}
