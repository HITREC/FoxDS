#![allow(dead_code)]

use parking_lot::Mutex;
use reqwest::blocking::Client;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

pub fn get_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(5))
            .tcp_nodelay(true)
            .pool_max_idle_per_host(8)
            .pool_idle_timeout(Duration::from_secs(120))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_default()
    })
}

#[derive(Clone, Debug)]
struct BingToken {
    key: String,
    token: String,
    ig: String,
    acquired_at: Instant,
}

static BING_SESSION: Mutex<Option<BingToken>> = Mutex::new(None);

/// Fetch fresh Bing tokens from bing.com/translator
fn fetch_bing_token(client: &Client) -> Option<BingToken> {
    let resp = client
        .get("https://www.bing.com/translator")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .header("Accept-Language", "ru,en;q=0.9")
        .send()
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let html = resp.text().ok()?;

    // 1. Extract params_AbusePreventionHelper = [key, "token", time]
    let pattern = "params_AbusePreventionHelper";
    let idx = html.find(pattern)?;
    let slice_h = &html[idx..];
    let start_b = slice_h.find('[')?;
    let end_b = slice_h[start_b..].find(']')?;
    let content = &slice_h[start_b + 1..start_b + end_b];
    let parts: Vec<&str> = content.split(',').collect();
    if parts.len() < 2 {
        return None;
    }
    let key = parts[0].trim().to_string();
    let token = parts[1].trim().trim_matches('"').to_string();

    // 2. Extract IG:"..."
    let ig_pattern = "IG:\"";
    let ig_idx = html.find(ig_pattern)?;
    let ig_slice = &html[ig_idx + ig_pattern.len()..];
    let ig_end = ig_slice.find('"')?;
    let ig = ig_slice[..ig_end].to_string();

    if key.is_empty() || token.is_empty() || ig.is_empty() {
        return None;
    }

    Some(BingToken {
        key,
        token,
        ig,
        acquired_at: Instant::now(),
    })
}

fn get_valid_bing_token(client: &Client, force_refresh: bool) -> Option<BingToken> {
    if !force_refresh {
        let lock = BING_SESSION.lock();
        if let Some(ref tok) = *lock {
            // Bing tokens stay valid for 30 minutes
            if tok.acquired_at.elapsed() < Duration::from_secs(1800) {
                return Some(tok.clone());
            }
        }
    }

    // Refresh token
    let fresh = fetch_bing_token(client)?;
    let mut lock = BING_SESSION.lock();
    *lock = Some(fresh.clone());
    Some(fresh)
}

/// Pre-warms HTTP connection and acquires Bing translation token in background
pub fn warm_up_connection() {
    std::thread::spawn(|| {
        let client = get_client();
        let _ = get_valid_bing_token(client, false);
    });
}

/// Translate via Microsoft Bing Translator API (high quality, maintains casing and punctuation)
fn translate_bing(client: &Client, text: &str, from_lang: &str, to_lang: &str) -> Option<String> {
    for attempt in 0..2 {
        let force_refresh = attempt > 0;
        let token = get_valid_bing_token(client, force_refresh)?;

        let url = format!(
            "https://www.bing.com/ttranslatev3?isVertical=1&IG={}&IID=translator.5028",
            token.ig
        );

        let params = [
            ("fromLang", from_lang),
            ("text", text),
            ("to", to_lang),
            ("token", &token.token),
            ("key", &token.key),
        ];

        let resp = client
            .post(&url)
            .form(&params)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .header("Referer", "https://www.bing.com/translator")
            .send()
            .ok()?;

        if !resp.status().is_success() {
            continue;
        }

        let json = resp.json::<serde_json::Value>().ok()?;
        if let Some(trans) = json
            .get(0)
            .and_then(|item| item.get("translations"))
            .and_then(|arr| arr.get(0))
            .and_then(|t| t.get("text"))
            .and_then(|s| s.as_str())
        {
            let res = trans.trim();
            if !res.is_empty() {
                return Some(res.to_string());
            }
        }
    }
    None
}

/// Translate via MyMemory API with user contact parameter
fn translate_mymemory(client: &Client, text: &str, from_lang: &str, to_lang: &str) -> Option<String> {
    let encoded = urlencoding(text);
    let hour_seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 3600;
    let email = format!("foxds_user_{}@foxds.app", hour_seed);
    let url = format!(
        "https://api.mymemory.translated.net/get?q={}&langpair={}|{}&de={}",
        encoded, from_lang, to_lang, email
    );

    let resp = client.get(&url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let json = resp.json::<serde_json::Value>().ok()?;
    let trans = json
        .get("responseData")
        .and_then(|d| d.get("translatedText"))
        .and_then(|t| t.as_str())?;

    let clean = trans.trim();
    if clean.is_empty() || clean.contains("MYMEMORY WARNING") {
        return None;
    }
    Some(clean.to_string())
}

/// Translate via Lingva API instances
fn translate_lingva(client: &Client, text: &str, from_lang: &str, to_lang: &str) -> Option<String> {
    let instances = [
        "https://lingva.ml",
        "https://translate.plausibility.cloud",
        "https://lingva.lunar.icu",
    ];
    let encoded = urlencoding(text);

    for inst in instances {
        let url = format!("{}/api/v1/{}/{}/{}", inst, from_lang, to_lang, encoded);
        if let Ok(resp) = client.get(&url).timeout(Duration::from_secs(3)).send() {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(trans) = json.get("translation").and_then(|t| t.as_str()) {
                        let clean = trans.trim();
                        if !clean.is_empty() && clean != text {
                            return Some(clean.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Translate via Google Translate web RPC
fn translate_google_gtx(client: &Client, text: &str, from_lang: &str, to_lang: &str) -> Option<String> {
    let encoded = urlencoding(text);
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl={}&tl={}&dt=t&q={}",
        from_lang, to_lang, encoded
    );

    let resp = client.get(&url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let json = resp.json::<serde_json::Value>().ok()?;
    let sentences = json.get(0).and_then(|v| v.as_array())?;
    let mut result = String::new();
    for s in sentences {
        if let Some(trans) = s.get(0).and_then(|v| v.as_str()) {
            result.push_str(trans);
        }
    }
    let clean = result.trim();
    if !clean.is_empty() {
        Some(clean.to_string())
    } else {
        None
    }
}

/// Fast Multi-Engine Translation with Automatic Fallback
pub fn translate_text(text: &str, from_lang: &str, to_lang: &str) -> Result<String, String> {
    let clean = text.trim();
    if clean.is_empty() {
        return Ok(String::new());
    }

    let client = get_client();

    // 1. Primary: Microsoft Bing Neural Translator (fast, accurate emotion/casing, high rate limits)
    if let Some(translated) = translate_bing(client, clean, from_lang, to_lang) {
        if !translated.trim().is_empty() {
            return Ok(translated);
        }
    }

    // 2. Secondary: MyMemory Translation API
    if let Some(translated) = translate_mymemory(client, clean, from_lang, to_lang) {
        if !translated.trim().is_empty() {
            return Ok(translated);
        }
    }

    // 3. Tertiary: Lingva Decentralized Instances
    if let Some(translated) = translate_lingva(client, clean, from_lang, to_lang) {
        if !translated.trim().is_empty() {
            return Ok(translated);
        }
    }

    // 4. Quaternary: Google GTX fallback
    if let Some(translated) = translate_google_gtx(client, clean, from_lang, to_lang) {
        if !translated.trim().is_empty() {
            return Ok(translated);
        }
    }

    eprintln!("[Translator] All translation backends failed for: \"{}\"", clean);
    Err(format!("Translation failed for: {}", clean))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_english_to_russian() {
        let phrases = [
            "WHAT IS?!",
            "LET'S GO!",
            "WHAT IS THE WEATHER TOMORROW?!",
        ];

        for phrase in phrases {
            let res = translate_text(phrase, "en", "ru");
            println!("Testing \"{}\" -> {:?}", phrase, res);
            assert!(res.is_ok(), "Translation failed for phrase: {}", phrase);
            let translated = res.unwrap();
            assert!(!translated.is_empty(), "Translation is empty");
            // Must contain cyrillic characters
            assert!(
                translated.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)),
                "Translation does not contain Cyrillic: {}",
                translated
            );
        }
    }

    #[test]
    fn test_translate_russian_to_english() {
        let res = translate_text("ПРИВЕТ КАК ДЕЛА?!", "ru", "en");
        println!("Testing RU->EN -> {:?}", res);
        assert!(res.is_ok());
        let translated = res.unwrap();
        assert!(!translated.is_empty());
        assert!(
            translated.to_lowercase().contains("hello")
                || translated.to_lowercase().contains("how are you")
                || translated.to_lowercase().contains("hi")
        );
    }
}

