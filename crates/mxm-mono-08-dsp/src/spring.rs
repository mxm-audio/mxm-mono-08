//! Provisional mono post-mix spring (`research:effects/buchla-208-spring-reverb.md`).
//!
//! Card 12 establishes mono voltage drive and placement; no original tank was measured. This is a
//! bounded dispersive two-path model in the compatible replacement tank's published decay class.

use crate::{flush, soft_clip};

pub const TRANSITS_MS: [f32; 2] = [31.0, 43.0]; // Chosen.
pub const T60_S: f32 = 2.3; // Chosen within replacement tank's published 1.75-3.0 s class.
pub const BAND_LOW_HZ: f32 = 180.0; // Chosen.
pub const BAND_HIGH_HZ: f32 = 3_200.0; // Chosen.
pub const DISPERSION_STAGES: usize = 8; // Chosen.
pub const DISPERSION_HZ: f32 = 700.0; // Chosen.
pub const SNAP_LEVEL: f32 = 1e-6; // Chosen: -120 dB.
pub const SNAP_HOLD_S: f32 = 0.1; // Chosen.
const DEFAULT_FS: f32 = 48_000.;

#[derive(Debug, Clone, Copy, Default)]
struct Pole {
    lp: f64,
}
impl Pole {
    fn low(&mut self, x: f32, hz: f32, fs: f32) -> f32 {
        let c = 1.0
            - (-std::f64::consts::TAU * hz.clamp(1., fs * 0.45) as f64 / fs.max(1.) as f64).exp();
        self.lp += c * (x as f64 - self.lp);
        if self.lp.abs() < 1e-20 {
            self.lp = 0.;
        }
        self.lp as f32
    }
    fn reset(&mut self) {
        self.lp = 0.;
    }
}
#[derive(Debug, Clone, Copy, Default)]
struct AllPass {
    x1: f32,
    y1: f32,
}
impl AllPass {
    fn process(&mut self, x: f32, a: f32) -> f32 {
        let y = -a * x + self.x1 + a * self.y1;
        self.x1 = x;
        self.y1 = flush(y);
        y
    }
    fn reset(&mut self) {
        *self = Self::default();
    }
}
fn ap_coef(hz: f32, fs: f32) -> f32 {
    let w = (std::f32::consts::PI * hz.clamp(1., fs * 0.45) / fs.max(1.)).tan();
    ((1. - w) / (1. + w)).clamp(-0.999, 0.999)
}

#[derive(Debug, Clone)]
struct Path {
    line: Vec<f32>,
    write: usize,
    valid: usize,
    aps: [AllPass; DISPERSION_STAGES],
}
impl Path {
    fn new(fs: f32) -> Self {
        Self {
            line: vec![0.; (TRANSITS_MS[1] * 0.001 * fs).ceil() as usize + 32],
            write: 0,
            valid: 0,
            aps: [AllPass::default(); DISPERSION_STAGES],
        }
    }
    fn reset(&mut self) {
        self.write = 0;
        self.valid = 0;
        for a in &mut self.aps {
            a.reset();
        }
    }
    fn process(&mut self, x: f32, delay: usize, feedback: f32, a: f32) -> f32 {
        let n = self.line.len();
        let d = delay.clamp(1, n - 1);
        let mut y = if d > self.valid {
            0.
        } else {
            self.line[(self.write + n - d) % n]
        };
        for ap in &mut self.aps {
            y = ap.process(y, a);
        }
        self.line[self.write] = flush(x + y * feedback);
        self.write = (self.write + 1) % n;
        self.valid = (self.valid + 1).min(n);
        y
    }
}

#[derive(Debug, Clone)]
pub struct Spring {
    paths: [Path; 2],
    lo: Pole,
    hi: Pole,
    quiet: u32,
    armed: bool,
}
impl Default for Spring {
    fn default() -> Self {
        Self::new()
    }
}
impl Spring {
    pub fn new() -> Self {
        Self {
            paths: [Path::new(DEFAULT_FS), Path::new(DEFAULT_FS)],
            lo: Pole::default(),
            hi: Pole::default(),
            quiet: 0,
            armed: false,
        }
    }
    /// Allocates and resets; call only at activation/sample-rate changes, never per sample.
    pub fn set_sample_rate(&mut self, fs: f32) {
        self.paths = [Path::new(fs), Path::new(fs)];
        self.reset();
    }
    pub fn reset(&mut self) {
        for p in &mut self.paths {
            p.reset();
        }
        self.lo.reset();
        self.hi.reset();
        self.quiet = 0;
        self.armed = false;
    }
    pub fn active(&self) -> bool {
        self.armed
    }
    pub fn tail_samples(fs: f32) -> u32 {
        ((T60_S * 2.0 + TRANSITS_MS[1] * 0.001 + SNAP_HOLD_S) * fs) as u32
    }
    #[inline]
    pub fn process(&mut self, x: f32, amount: f32, fs: f32) -> f32 {
        let amount = amount.clamp(0., 1.);
        if amount == 0.0 {
            if self.armed {
                self.reset();
            }
            return x;
        }
        // An enabled but empty tank does no work and does not manufacture a tail from silence.
        if !self.armed && x.abs() < SNAP_LEVEL {
            return x;
        }
        self.armed = true;
        let low = self.lo.low(soft_clip(x), BAND_LOW_HZ, fs);
        let band = self.hi.low(soft_clip(x) - low, BAND_HIGH_HZ, fs);
        let a = ap_coef(DISPERSION_HZ, fs);
        let mut wet = 0.;
        for (i, p) in self.paths.iter_mut().enumerate() {
            let transit = TRANSITS_MS[i] * 0.001;
            let feedback = 10f32.powf(-3. * transit / T60_S);
            wet += p.process(band * 0.5, (transit * fs) as usize, feedback, a);
        }
        // Once no new send is audible, two T60 periods put a unity excitation at -120 dB.
        // Wait that decay plus the longest transit and hold, then clear in O(1). Chosen digital
        // housekeeping: the tank's vintage noise floor and exact tail are unmeasured.
        if band.abs() < SNAP_LEVEL {
            self.quiet = self.quiet.saturating_add(1);
        } else {
            self.quiet = 0;
        }
        let memory = ((2.0 * T60_S + TRANSITS_MS[1] * 0.001 + SNAP_HOLD_S) * fs) as u32;
        if self.quiet > memory {
            self.reset();
            return flush(x);
        }
        flush((x + amount * wet).clamp(-4., 4.))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FS: f32 = 48_000.;
    #[test]
    fn zero_is_bit_dry_and_clears_the_tank() {
        let mut s = Spring::new();
        for i in 0..4000 {
            s.process(if i < 10 { 1. } else { 0. }, 1., FS);
        }
        assert!(s.active());
        assert_eq!(s.process(0.37, 0., FS), 0.37);
        assert!(!s.active());
        for _ in 0..5000 {
            assert_eq!(s.process(0., 1., FS), 0.);
        }
    }
    #[test]
    fn impulse_has_two_dispersive_arrivals_and_a_finite_tail() {
        let mut s = Spring::new();
        let mut ir = Vec::new();
        for i in 0..(FS * 0.2) as usize {
            ir.push(s.process(if i == 0 { 1. } else { 0. }, 1., FS));
        }
        for &ms in &TRANSITS_MS {
            let n = (ms * 0.001 * FS) as usize;
            let peak = ir[n.saturating_sub(64)..(n + 500).min(ir.len())]
                .iter()
                .fold(0f32, |m, x| m.max(x.abs()));
            assert!(peak > 1e-5, "no arrival near {ms} ms: {peak}");
        }
        for _ in 0..Spring::tail_samples(FS) {
            s.process(0., 1., FS);
        }
        assert_eq!(s.process(0., 1., FS), 0.);
        assert!(
            !s.active(),
            "spring remained armed; quiet={} valid={:?}",
            s.quiet,
            s.paths.iter().map(|p| p.valid).collect::<Vec<_>>()
        );
    }
    #[test]
    fn stable_at_every_supported_rate() {
        for fs in [1_000., 44_100., 48_000., 96_000., 192_000., 768_000.] {
            let mut s = Spring::new();
            s.set_sample_rate(fs);
            for i in 0..(fs.min(48_000.) as usize) {
                let y = s.process(if i % 13 == 0 { 20. } else { -20. }, 1., fs);
                assert!(y.is_finite() && y.abs() <= 4.);
            }
        }
    }
}
