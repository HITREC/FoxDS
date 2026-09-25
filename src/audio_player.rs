#![allow(dead_code)]

use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, DeviceSinkBuilder, Player};
use std::io::Cursor;
use std::sync::Arc;

pub fn find_output_device(name: &str) -> Option<rodio::cpal::Device> {
    let host = rodio::cpal::default_host();
    if name == "Default" || name.trim().is_empty() {
        return host.default_output_device();
    }

    if let Ok(devices) = host.output_devices() {
        for dev in devices {
            if let Ok(desc) = dev.description() {
                if desc.name().eq_ignore_ascii_case(name) || desc.name().contains(name) {
                    return Some(dev);
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

    // 1. Play to Virtual Cable (for teammates in game/Discord)
    let cable_dev = find_output_device(cable_device_name);
    let cable_bytes = shared_bytes.clone();
    let cable_handle = std::thread::spawn(move || {
        let sink_res = if let Some(dev) = cable_dev {
            DeviceSinkBuilder::from_device(dev).and_then(|b| b.open_stream())
        } else {
            DeviceSinkBuilder::open_default_sink()
        };

        if let Ok(sink_handle) = sink_res {
            let player = Player::connect_new(sink_handle.mixer());
            player.set_volume(volume);
            let cursor = Cursor::new(cable_bytes.as_ref().clone());
            if let Ok(decoder) = Decoder::try_from(cursor) {
                player.append(decoder);
                player.sleep_until_end();
            }
        }
    });

    // 2. Play to Headphones (so user can hear what was synthesized, if play_self is enabled)
    if play_self {
        let hp_dev = find_output_device(headphones_device_name);
        let hp_bytes = shared_bytes.clone();
        let _ = std::thread::spawn(move || {
            let sink_res = if let Some(dev) = hp_dev {
                DeviceSinkBuilder::from_device(dev).and_then(|b| b.open_stream())
            } else {
                DeviceSinkBuilder::open_default_sink()
            };

            if let Ok(sink_handle) = sink_res {
                let player = Player::connect_new(sink_handle.mixer());
                player.set_volume(volume * 0.9);
                let cursor = Cursor::new(hp_bytes.as_ref().clone());
                if let Ok(decoder) = Decoder::try_from(cursor) {
                    player.append(decoder);
                    player.sleep_until_end();
                }
            }
        });
    }

    let _ = cable_handle.join();
    Ok(())
}
