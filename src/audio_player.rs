#![allow(dead_code)]

use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, DeviceSinkBuilder, Player};
use std::io::Cursor;
use std::sync::Arc;

pub fn find_output_device(name: &str) -> Option<rodio::cpal::Device> {
    let host = rodio::cpal::default_host();
    let trimmed = name.trim();
    if trimmed.eq_ignore_ascii_case("default") || trimmed.is_empty() {
        return host.default_output_device();
    }

    if let Ok(devices) = host.output_devices() {
        let dev_list: Vec<_> = devices.filter_map(|d| {
            d.description().ok().map(|desc| (desc.name().to_string(), d))
        }).collect();

        // 1. Exact match
        for (desc_name, dev) in &dev_list {
            if desc_name.eq_ignore_ascii_case(trimmed) {
                return Some(dev.clone());
            }
        }

        let name_lower = trimmed.to_lowercase();

        // 2. Substring match (either desc contains name or name contains desc)
        for (desc_name, dev) in &dev_list {
            let desc_lower = desc_name.to_lowercase();
            if desc_lower.contains(&name_lower) || name_lower.contains(&desc_lower) {
                return Some(dev.clone());
            }
        }

        // 3. Fallback for Virtual Cable keywords
        if name_lower.contains("cable") {
            for (desc_name, dev) in &dev_list {
                if desc_name.to_lowercase().contains("cable") {
                    return Some(dev.clone());
                }
            }
        }
    }
    host.default_output_device()
}

/// Play audio bytes (MP3/WAV) to one or two output devices (Virtual Cable and Headphones)
pub fn play_tts_audio(
    audio_bytes: &[u8],
    cable_device_name: &str,
    headphones_device_name: &str,
    play_self: bool,
    volume: f32,
) -> Result<(), String> {
    if audio_bytes.is_empty() {
        return Ok(());
    }

    let shared_bytes = Arc::new(audio_bytes.to_vec());

    let cable_dev = find_output_device(cable_device_name);
    let hp_dev = if play_self {
        find_output_device(headphones_device_name)
    } else {
        None
    };

    // Check if cable and headphones are the same device
    let cable_name = cable_dev.as_ref().and_then(|d| d.description().ok()).map(|d| d.name().to_string()).unwrap_or_default();
    let hp_name = hp_dev.as_ref().and_then(|d| d.description().ok()).map(|d| d.name().to_string()).unwrap_or_default();

    let is_same_device = !cable_name.is_empty() && cable_name == hp_name;

    if is_same_device || !play_self || hp_dev.is_none() {
        // Only one output device needed
        let target_dev = if cable_dev.is_some() { cable_dev } else { hp_dev };
        let bytes = shared_bytes.clone();
        let handle = std::thread::spawn(move || {
            let sink_res = if let Some(dev) = target_dev {
                DeviceSinkBuilder::from_device(dev).and_then(|b| b.open_stream())
            } else {
                DeviceSinkBuilder::open_default_sink()
            };

            if let Ok(sink_handle) = sink_res {
                let player = Player::connect_new(sink_handle.mixer());
                player.set_volume(volume);
                let cursor = Cursor::new(bytes.as_ref().clone());
                if let Ok(decoder) = Decoder::try_from(cursor) {
                    player.append(decoder);
                    player.sleep_until_end();
                }
            }
        });
        let _ = handle.join();
    } else {
        // Two distinct devices: play to Cable and to Headphones simultaneously
        let c_dev = cable_dev;
        let c_bytes = shared_bytes.clone();
        let cable_handle = std::thread::spawn(move || {
            let sink_res = if let Some(dev) = c_dev {
                DeviceSinkBuilder::from_device(dev).and_then(|b| b.open_stream())
            } else {
                DeviceSinkBuilder::open_default_sink()
            };

            if let Ok(sink_handle) = sink_res {
                let player = Player::connect_new(sink_handle.mixer());
                player.set_volume(volume);
                let cursor = Cursor::new(c_bytes.as_ref().clone());
                if let Ok(decoder) = Decoder::try_from(cursor) {
                    player.append(decoder);
                    player.sleep_until_end();
                }
            }
        });

        let h_dev = hp_dev;
        let h_bytes = shared_bytes.clone();
        let hp_handle = std::thread::spawn(move || {
            let sink_res = if let Some(dev) = h_dev {
                DeviceSinkBuilder::from_device(dev).and_then(|b| b.open_stream())
            } else {
                DeviceSinkBuilder::open_default_sink()
            };

            if let Ok(sink_handle) = sink_res {
                let player = Player::connect_new(sink_handle.mixer());
                player.set_volume(volume * 0.95);
                let cursor = Cursor::new(h_bytes.as_ref().clone());
                if let Ok(decoder) = Decoder::try_from(cursor) {
                    player.append(decoder);
                    player.sleep_until_end();
                }
            }
        });

        let _ = cable_handle.join();
        let _ = hp_handle.join();
    }

    Ok(())
}

/// Play audio bytes directly to the user's headphones with anti-echo synchronization flags
pub fn play_headphones_audio(
    audio_bytes: &[u8],
    headphones_device_name: &str,
    volume: f32,
    is_playing_flag: Option<Arc<std::sync::atomic::AtomicBool>>,
    last_played_time: Option<Arc<parking_lot::Mutex<std::time::Instant>>>,
) -> Result<(), String> {
    if audio_bytes.is_empty() {
        return Ok(());
    }

    let hp_dev = find_output_device(headphones_device_name);
    let bytes = audio_bytes.to_vec();

    std::thread::spawn(move || {
        if let Some(ref flag) = is_playing_flag {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }

        let sink_res = if let Some(dev) = hp_dev {
            DeviceSinkBuilder::from_device(dev).and_then(|b| b.open_stream())
        } else {
            DeviceSinkBuilder::open_default_sink()
        };

        if let Ok(sink_handle) = sink_res {
            let player = Player::connect_new(sink_handle.mixer());
            player.set_volume(volume);
            let cursor = Cursor::new(bytes);
            if let Ok(decoder) = Decoder::try_from(cursor) {
                player.append(decoder);
                player.sleep_until_end();
            }
        }

        if let Some(ref flag) = is_playing_flag {
            flag.store(false, std::sync::atomic::Ordering::SeqCst);
        }
        if let Some(ref t) = last_played_time {
            *t.lock() = std::time::Instant::now();
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_matching() {
        let dev = find_output_device("Default");
        assert!(dev.is_some());
    }
}
