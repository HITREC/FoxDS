#![allow(dead_code)]
#![allow(unused_imports)]

use std::collections::HashMap;

#[cfg(windows)]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

pub fn parse_key_str(s: &str) -> Vec<u16> {
    let mut codes = Vec::new();
    let parts = s.split('+');
    for part in parts {
        let p = part.trim().to_lowercase();
        #[cfg(windows)]
        match p.as_str() {
            "f1" => codes.push(VK_F1),
            "f2" => codes.push(VK_F2),
            "f3" => codes.push(VK_F3),
            "f4" => codes.push(VK_F4),
            "f5" => codes.push(VK_F5),
            "f6" => codes.push(VK_F6),
            "f7" => codes.push(VK_F7),
            "f8" => codes.push(VK_F8),
            "f9" => codes.push(VK_F9),
            "f10" => codes.push(VK_F10),
            "f11" => codes.push(VK_F11),
            "f12" => codes.push(VK_F12),
            "ctrl" | "control" => codes.push(VK_CONTROL),
            "alt" | "menu" => codes.push(VK_MENU),
            "shift" => codes.push(VK_SHIFT),
            "space" => codes.push(VK_SPACE),
            "tab" => codes.push(VK_TAB),
            "caps" | "capslock" => codes.push(VK_CAPITAL),
            "mouse4" => codes.push(VK_XBUTTON1),
            "mouse5" => codes.push(VK_XBUTTON2),
            ch if ch.len() == 1 => {
                let c = ch.chars().next().unwrap();
                if c.is_ascii_alphabetic() {
                    codes.push(c.to_ascii_uppercase() as u16);
                } else if c.is_ascii_digit() {
                    codes.push(c as u16);
                }
            }
            _ => {}
        }
        #[cfg(not(windows))]
        {
            match p.as_str() {
                "f1" => codes.push(1),
                "f2" => codes.push(2),
                "f3" => codes.push(3),
                "f4" => codes.push(4),
                "f5" => codes.push(5),
                "f6" => codes.push(6),
                "f7" => codes.push(7),
                "f8" => codes.push(8),
                "f9" => codes.push(9),
                "f10" => codes.push(10),
                "f11" => codes.push(11),
                "f12" => codes.push(12),
                _ => {}
            }
        }
    }
    codes
}

#[cfg(windows)]
pub fn is_key_down(vk_code: u16) -> bool {
    unsafe {
        let state = GetAsyncKeyState(vk_code as i32);
        (state as u16 & 0x8000) != 0
    }
}

#[cfg(not(windows))]
pub fn is_key_down(_vk_code: u16) -> bool {
    false
}

pub fn are_keys_down(codes: &[u16]) -> bool {
    if codes.is_empty() {
        return false;
    }
    codes.iter().all(|&c| is_key_down(c))
}

pub fn is_key_pressed(key_str: &str) -> bool {
    let codes = parse_key_str(key_str);
    are_keys_down(&codes)
}
