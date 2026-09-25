#![allow(dead_code)]
#![allow(unused_imports)]

use serde::Serialize;
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
pub struct SystemStats {
    pub rating: u32,
    pub gpu_score: u32,
    pub cpu_score: u32,
    pub memory_score: u32,
    pub optimization_pct: u32,
    pub audio_latency_ms: f32,
    pub memory_mb: u32,
    pub buffer_drops_pct: f32,
    pub status_text: String,
    pub is_optimal: bool,
}

impl SystemStats {
    pub fn current() -> Self {
        Self {
            rating: 28_450,
            gpu_score: 9_420,
            cpu_score: 11_980,
            memory_score: 7_050,
            optimization_pct: 85,
            audio_latency_ms: 11.2,
            memory_mb: 28,
            buffer_drops_pct: 0.0,
            status_text: "Звуковой тракт оптимизирован. Задержка минимальна.".to_string(),
            is_optimal: true,
        }
    }
}
