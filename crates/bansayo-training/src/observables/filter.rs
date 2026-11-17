//! Filters of 04 §6.3: a live critically damped second-order low-pass and a zero-phase
//! fourth-order Butterworth (second order forward and backward) for completed repetitions.
use std::f64::consts::{PI, SQRT_2};

/// Two equal first-order poles whose cascade has its −3 dB point at `cutoff_hz`.
#[derive(Debug, Clone, Copy)]
pub struct CriticallyDamped {
    tau_s: f64,
    stages: Option<[f64; 2]>,
}

impl CriticallyDamped {
    pub fn new(cutoff_hz: f64) -> Self {
        // For n equal poles the cascade's −3 dB frequency is f_pole·sqrt(2^(1/n) − 1).
        let pole_hz = cutoff_hz / (SQRT_2 - 1.0).sqrt();
        Self {
            tau_s: 1.0 / (2.0 * PI * pole_hz),
            stages: None,
        }
    }

    /// Filters `x` taken `dt_s` after the previous input. The first input initialises the state.
    pub fn update(&mut self, x: f64, dt_s: f64) -> f64 {
        let Some([a, b]) = self.stages.as_mut() else {
            self.stages = Some([x, x]);
            return x;
        };
        let alpha = 1.0 - (-dt_s.max(0.0) / self.tau_s).exp();
        *a += alpha * (x - *a);
        *b += alpha * (*a - *b);
        *b
    }

    pub fn reset(&mut self) {
        self.stages = None;
    }
}

/// Second-order Butterworth low-pass biquad coefficients (bilinear transform, pre-warped).
fn butterworth2(cutoff_hz: f64, rate_hz: f64) -> ([f64; 3], [f64; 2]) {
    let k = (PI * (cutoff_hz / rate_hz).min(0.49)).tan();
    let norm = 1.0 / (1.0 + SQRT_2 * k + k * k);
    let b0 = k * k * norm;
    (
        [b0, 2.0 * b0, b0],
        [
            2.0 * (k * k - 1.0) * norm,
            (1.0 - SQRT_2 * k + k * k) * norm,
        ],
    )
}

fn pass(x: &[f64], b: [f64; 3], a: [f64; 2]) -> Vec<f64> {
    // Start from the steady state of the first sample so a constant input yields itself.
    let (mut x1, mut x2, mut y1, mut y2) = (x[0], x[0], x[0], x[0]);
    x.iter()
        .map(|&x0| {
            let y0 = b[0] * x0 + b[1] * x1 + b[2] * x2 - a[0] * y1 - a[1] * y2;
            (x2, x1, y2, y1) = (x1, x0, y1, y0);
            y0
        })
        .collect()
}

/// Zero-phase low-pass of a uniformly sampled series: forward, then backward.
pub fn filtfilt(x: &[f64], cutoff_hz: f64, rate_hz: f64) -> Vec<f64> {
    if x.len() < 3 || !(rate_hz.is_finite() && rate_hz > 0.0) {
        return x.to_vec();
    }
    let (b, a) = butterworth2(cutoff_hz, rate_hz);
    let mut forward = pass(x, b, a);
    forward.reverse();
    let mut out = pass(&forward, b, a);
    out.reverse();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_input_is_preserved_by_both_filters() {
        let mut f = CriticallyDamped::new(12.0);
        for _ in 0..50 {
            assert!((f.update(2.5, 1.0 / 60.0) - 2.5).abs() < 1e-12);
        }
        let y = filtfilt(&[1.0; 40], 12.0, 60.0);
        assert!(y.iter().all(|v| (v - 1.0).abs() < 1e-9));
    }

    #[test]
    fn zero_phase_filter_keeps_a_peak_in_place_and_live_filter_lags() {
        let rate = 100.0;
        let x: Vec<f64> = (0..200)
            .map(|i| (-(((i as f64) - 100.0) / 12.0).powi(2)).exp())
            .collect();
        let y = filtfilt(&x, 12.0, rate);
        let peak = |v: &[f64]| {
            v.iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map(|(i, _)| i)
                .unwrap_or_default()
        };
        assert_eq!(peak(&y), 100);
        let mut f = CriticallyDamped::new(12.0);
        let live: Vec<f64> = x.iter().map(|v| f.update(*v, 1.0 / rate)).collect();
        assert!(peak(&live) > 100);
    }

    #[test]
    fn live_filter_cutoff_is_at_minus_three_decibels() {
        let (fc, rate) = (5.0, 2000.0);
        let mut f = CriticallyDamped::new(fc);
        let mut amplitude: f64 = 0.0;
        for i in 0..8000 {
            let t = i as f64 / rate;
            let y = f.update((2.0 * PI * fc * t).sin(), 1.0 / rate);
            if i > 4000 {
                amplitude = amplitude.max(y.abs());
            }
        }
        assert!(
            (amplitude - std::f64::consts::FRAC_1_SQRT_2).abs() < 0.02,
            "{amplitude}"
        );
    }
}
