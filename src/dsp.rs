#![allow(dead_code)]

use std::f32::consts::PI;

/// High-performance audio DSP processor for real-time voice enhancement
pub struct AudioDsp;

impl AudioDsp {
    /// Process 16-bit 24kHz or 48kHz mono/stereo PCM samples in-place
    pub fn process_samples(
        samples: &mut [f32],
        sample_rate: u32,
        apply_warmth_eq: bool,
        apply_tube_saturation: bool,
        apply_radio_filter: bool,
    ) {
        if samples.is_empty() {
            return;
        }

        let sr = sample_rate as f32;

        // 1. Radio Walkie-Talkie Filter (Foxhole / Military Radio)
        if apply_radio_filter {
            // High-pass at 350 Hz (cuts boominess and room rumble)
            Self::apply_high_pass(samples, 350.0, sr);
            // Low-pass at 3400 Hz (standard walkie-talkie / telephone band)
            Self::apply_low_pass(samples, 3400.0, sr);

            // Radio Drive / Soft Overdrive
            for s in samples.iter_mut() {
                let x = *s * 1.6;
                *s = x.clamp(-0.85, 0.85);
            }
            return;
        }

        // 2. Studio Warmth & De-Harshing Equalizer (removes robotic vocoder glare)
        if apply_warmth_eq {
            // Cut harsh digital resonance at 3100 Hz (typical TTS metallic peak)
            Self::apply_notch(samples, 3100.0, 1.2, sr);

            // Gentle low-end warmth at 140 Hz (adds chest resonance)
            Self::apply_low_shelf(samples, 140.0, 2.5, sr);

            // Breath / Air boost at 9500 Hz
            Self::apply_high_shelf(samples, 9500.0, 1.8, sr);
        }

        // 3. Analog Tube Harmonic Warmth (Soft non-linear saturation)
        if apply_tube_saturation {
            for s in samples.iter_mut() {
                let x = *s;
                // Soft-knee cubic saturation curve: f(x) = 1.25 * (x - 0.2 * x^3)
                // Adds subtle 2nd and 3rd harmonics to give warmth to synthetic voices
                let abs_x = x.abs();
                if abs_x < 0.8 {
                    *s = x * 1.15 - 0.18 * x * x * x;
                } else {
                    let sign = if x >= 0.0 { 1.0 } else { -1.0 };
                    *s = sign * (0.8 + (abs_x - 0.8) / (1.0 + (abs_x - 0.8)));
                }
            }
        }
    }

    /// Second-order IIR Notch / Peaking filter
    fn apply_notch(samples: &mut [f32], freq: f32, q: f32, sample_rate: f32) {
        let w0 = 2.0 * PI * (freq / sample_rate);
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();

        // -3.5 dB notch gain
        let a = 10.0f32.powf(-3.5 / 40.0);
        let b0 = 1.0 + alpha / a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha / a;
        let a0 = 1.0 + alpha * a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha * a;

        Self::biquad_filter(samples, b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
    }

    /// First-order Low-pass filter
    fn apply_low_pass(samples: &mut [f32], cutoff_freq: f32, sample_rate: f32) {
        let rc = 1.0 / (2.0 * PI * cutoff_freq);
        let dt = 1.0 / sample_rate;
        let alpha = dt / (rc + dt);

        let mut prev = 0.0f32;
        for s in samples.iter_mut() {
            prev += alpha * (*s - prev);
            *s = prev;
        }
    }

    /// First-order High-pass filter
    fn apply_high_pass(samples: &mut [f32], cutoff_freq: f32, sample_rate: f32) {
        let rc = 1.0 / (2.0 * PI * cutoff_freq);
        let dt = 1.0 / sample_rate;
        let alpha = rc / (rc + dt);

        let mut prev_in = 0.0f32;
        let mut prev_out = 0.0f32;
        for s in samples.iter_mut() {
            let current_in = *s;
            let current_out = alpha * (prev_out + current_in - prev_in);
            prev_in = current_in;
            prev_out = current_out;
            *s = current_out;
        }
    }

    /// Low-shelf filter for vocal chest resonance
    fn apply_low_shelf(samples: &mut [f32], freq: f32, gain_db: f32, sample_rate: f32) {
        let a = 10.0f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * (freq / sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / 2.0 * (2.0f32).sqrt();

        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha);
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha;

        Self::biquad_filter(samples, b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
    }

    /// High-shelf filter for airy human breath
    fn apply_high_shelf(samples: &mut [f32], freq: f32, gain_db: f32, sample_rate: f32) {
        let a = 10.0f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * (freq / sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / 2.0 * (2.0f32).sqrt();

        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha;

        Self::biquad_filter(samples, b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
    }

    #[inline(always)]
    fn biquad_filter(samples: &mut [f32], b0: f32, b1: f32, b2: f32, a1: f32, a2: f32) {
        let mut x1 = 0.0f32;
        let mut x2 = 0.0f32;
        let mut y1 = 0.0f32;
        let mut y2 = 0.0f32;

        for s in samples.iter_mut() {
            let x0 = *s;
            let y0 = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            x2 = x1;
            x1 = x0;
            y2 = y1;
            y1 = y0;
            *s = y0.clamp(-1.0, 1.0);
        }
    }
}
