//! Behavioural 208 Card 10/11 low-pass gate.
//!
//! The established topology is a switched two-pole network controlled by two optical elements,
//! not a resonant VCF plus VCA (`research:filters/machines/buchla-lowpass-gate.md` §1.1). Exact 208
//! curves and timing are unmeasured; the constants below are chosen and deliberately are not the
//! published 292/VTL5C3 model constants.

use crate::{Memo, flush};

pub const OPTICAL_RISE_S: f64 = 0.008;
pub const OPTICAL_FALL_S: f64 = 0.220;
pub const CUTOFF_MIN_HZ: f32 = 35.0;
pub const CUTOFF_MAX_HZ: f32 = 18_000.0;
pub const OUTPUT_BOUND: f32 = 1.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GateMode {
    Vca,
    LowPass,
    #[default]
    Combination,
}

#[derive(Debug, Clone, Copy, Default)]
struct Pole {
    s: f64,
    /// Keyed on `(hz, fs)` because the coefficient depends on both. In `Vca` mode `hz` is the
    /// constant `fs * 0.44` and this always hits; in the other modes it tracks the optical level
    /// and hits once that has settled.
    c: Memo<2>,
}
impl Pole {
    #[inline]
    fn process(&mut self, x: f32, hz: f32, fs: f32) -> f32 {
        let c = self.c.get([hz as f64, fs as f64], || {
            1.0 - (-std::f64::consts::TAU * hz.clamp(1.0, fs * 0.45) as f64 / fs.max(1.0) as f64)
                .exp()
        });
        self.s += c * (x as f64 - self.s);
        if self.s.abs() < 1e-20 {
            self.s = 0.0;
        }
        self.s as f32
    }
    fn reset(&mut self) {
        self.s = 0.0;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LowPassGate {
    optical: f64,
    poles: [Pole; 2],
    /// **One slot per direction, not one memo.** The rise/fall selection can alternate every
    /// sample under a moving drive, and a single slot would then miss every time and pay a
    /// comparison for it. Each slot is keyed on the sample rate its `tau` was built at.
    optical_c: [Memo<1>; 2],
    /// Keyed on the optical level the cutoff is raised from.
    cutoff: Memo<1>,
}
impl Default for LowPassGate {
    fn default() -> Self {
        Self::new()
    }
}
impl LowPassGate {
    pub const fn new() -> Self {
        Self {
            optical: 0.0,
            poles: [Pole {
                s: 0.0,
                c: Memo::new(),
            }; 2],
            optical_c: [Memo::new(); 2],
            cutoff: Memo::new(),
        }
    }
    pub fn reset(&mut self) {
        self.optical = 0.0;
        for p in &mut self.poles {
            p.reset();
        }
    }
    pub fn optical_level(&self) -> f32 {
        self.optical as f32
    }
    pub fn settled(&self) -> bool {
        self.optical == 0.0 && self.poles.iter().all(|p| p.s == 0.0)
    }
    #[inline]
    pub fn process(&mut self, x: f32, drive: f32, mode: GateMode, fs: f32) -> f32 {
        let target = drive.clamp(0.0, 1.0) as f64;
        let rising = target > self.optical;
        let tau = if rising {
            OPTICAL_RISE_S
        } else {
            OPTICAL_FALL_S
        };
        let c = self.optical_c[usize::from(!rising)].get([fs as f64], || {
            1.0 - (-1.0 / (tau * fs.max(1.0) as f64)).exp()
        });
        self.optical += c * (target - self.optical);
        if self.optical.abs() < 1e-9 && target == 0.0 {
            self.optical = 0.0;
        }
        let v = self.optical as f32;
        // `f32 -> f64 -> f32` round-trips exactly, so the cached value is the one `powf` gave.
        let cutoff = self.cutoff.get([v as f64], || {
            (CUTOFF_MIN_HZ * (CUTOFF_MAX_HZ / CUTOFF_MIN_HZ).powf(v)) as f64
        }) as f32;
        let (gain, poles, fc) = match mode {
            GateMode::Vca => (v, 1, fs * 0.44),
            GateMode::LowPass => (1.0, 2, cutoff),
            GateMode::Combination => (v, 2, cutoff),
        };
        let mut y = x;
        for p in &mut self.poles[..poles] {
            y = p.process(y, fc, fs);
        }
        if poles == 1 {
            self.poles[1].process(0.0, fc, fs);
        }
        flush((y * gain).clamp(-OUTPUT_BOUND, OUTPUT_BOUND))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FS: f32 = 48_000.;
    fn tone(mode: GateMode, drive: f32, hz: f32) -> f64 {
        let mut g = LowPassGate::new();
        let mut re = 0.;
        let mut im = 0.;
        for i in 0..48_000 {
            let t = i as f32 / FS;
            let x = (std::f32::consts::TAU * hz * t).sin();
            let y = g.process(x, drive, mode, FS);
            if i > 24_000 {
                let a = std::f64::consts::TAU * hz as f64 * i as f64 / FS as f64;
                re += y as f64 * a.cos();
                im += y as f64 * a.sin();
            }
        }
        (re * re + im * im).sqrt()
    }
    /// The shipped gate's arithmetic with **no memo anywhere**: the independent reference the
    /// cached implementation has to equal bit for bit.
    ///
    /// The duplication is the point. `BASELINE-M0.md`'s digests pin one rate and one set of
    /// patches, so they cannot speak for a rate change, a reset, the fall branch or the two modes
    /// the factory bank happens not to exercise — which is exactly where a memo keyed on the wrong
    /// thing would hide.
    #[derive(Clone, Copy, Default)]
    struct Reference {
        optical: f64,
        s: [f64; 2],
    }
    impl Reference {
        fn pole(&mut self, i: usize, x: f32, hz: f32, fs: f32) -> f32 {
            let c = 1.0
                - (-std::f64::consts::TAU * hz.clamp(1.0, fs * 0.45) as f64 / fs.max(1.0) as f64)
                    .exp();
            self.s[i] += c * (x as f64 - self.s[i]);
            if self.s[i].abs() < 1e-20 {
                self.s[i] = 0.0;
            }
            self.s[i] as f32
        }
        fn process(&mut self, x: f32, drive: f32, mode: GateMode, fs: f32) -> f32 {
            let target = drive.clamp(0.0, 1.0) as f64;
            let tau = if target > self.optical {
                OPTICAL_RISE_S
            } else {
                OPTICAL_FALL_S
            };
            let c = 1.0 - (-1.0 / (tau * fs.max(1.0) as f64)).exp();
            self.optical += c * (target - self.optical);
            if self.optical.abs() < 1e-9 && target == 0.0 {
                self.optical = 0.0;
            }
            let v = self.optical as f32;
            let cutoff = CUTOFF_MIN_HZ * (CUTOFF_MAX_HZ / CUTOFF_MIN_HZ).powf(v);
            let (gain, poles, fc) = match mode {
                GateMode::Vca => (v, 1, fs * 0.44),
                GateMode::LowPass => (1.0, 2, cutoff),
                GateMode::Combination => (v, 2, cutoff),
            };
            let mut y = x;
            for i in 0..poles {
                y = self.pole(i, y, fc, fs);
            }
            if poles == 1 {
                self.pole(1, 0.0, fc, fs);
            }
            flush((y * gain).clamp(-OUTPUT_BOUND, OUTPUT_BOUND))
        }
        fn reset(&mut self) {
            *self = Self::default();
        }
    }

    /// Memoising the gate's three transcendentals changes **no sample**, at every supported rate,
    /// in every mode, across rate changes and resets, and through a drive schedule built to force
    /// cache hits and misses in both directions.
    #[test]
    fn memoisation_is_bit_identical_to_recomputing() {
        for &(fs, other) in &[
            (1_000.0f32, 48_000.0f32),
            (44_100.0, 48_000.0),
            (48_000.0, 96_000.0),
            (96_000.0, 48_000.0),
            (192_000.0, 44_100.0),
            (768_000.0, 48_000.0),
        ] {
            for mode in [GateMode::Vca, GateMode::LowPass, GateMode::Combination] {
                let mut cached = LowPassGate::new();
                let mut reference = Reference::default();
                for i in 0..40_000usize {
                    // Ramp in, hold long enough for the optical value to stop moving (a hit),
                    // alternate every sample (the branch thrash a one-slot memo would lose on),
                    // fall away, then sit at zero.
                    let drive = match i {
                        0..=2_000 => i as f32 / 2_000.0,
                        2_001..=20_000 => 1.0,
                        20_001..=24_000 => {
                            if i % 2 == 0 {
                                0.3
                            } else {
                                0.9
                            }
                        }
                        24_001..=30_000 => 0.045,
                        _ => 0.0,
                    };
                    // A rate change mid-schedule, and back again.
                    let rate = if (12_000..14_000).contains(&i) {
                        other
                    } else {
                        fs
                    };
                    if i == 32_000 {
                        cached.reset();
                        reference.reset();
                    }
                    let x = (i as f32 * 0.037).sin();
                    let got = cached.process(x, drive, mode, rate);
                    let want = reference.process(x, drive, mode, rate);
                    assert_eq!(
                        got.to_bits(),
                        want.to_bits(),
                        "sample {i} at {fs} Hz in {mode:?}: {got} != {want}"
                    );
                    assert_eq!(
                        cached.optical_level().to_bits(),
                        (reference.optical as f32).to_bits(),
                        "optical level diverged at sample {i}, {fs} Hz, {mode:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn optical_memory_is_fast_open_and_slow_closed() {
        let mut g = LowPassGate::new();
        for _ in 0..(FS * 0.03) as usize {
            g.process(0., 1., GateMode::Combination, FS);
        }
        let peak = g.optical_level();
        for _ in 0..(FS * 0.03) as usize {
            g.process(0., 0., GateMode::Combination, FS);
        }
        assert!(
            peak > 0.9 && g.optical_level() > 0.7,
            "{} {}",
            peak,
            g.optical_level()
        );
    }
    #[test]
    fn combination_mode_loses_brightness_faster_than_vca() {
        let bright_open =
            tone(GateMode::Combination, 1., 5000.) / tone(GateMode::Combination, 1., 400.);
        let bright_closed =
            tone(GateMode::Combination, 0.25, 5000.) / tone(GateMode::Combination, 0.25, 400.);
        let vca_open = tone(GateMode::Vca, 1., 5000.) / tone(GateMode::Vca, 1., 400.);
        let vca_closed = tone(GateMode::Vca, 0.25, 5000.) / tone(GateMode::Vca, 0.25, 400.);
        assert!(
            bright_closed < bright_open * 0.2,
            "combination brightness {} {}",
            bright_open,
            bright_closed
        );
        assert!(
            vca_closed > vca_open * 0.9,
            "VCA brightness changed {} {}",
            vca_open,
            vca_closed
        );
    }
    #[test]
    fn all_modes_are_finite_bounded_and_settle_exactly() {
        for fs in [1_000., 44_100., 48_000., 96_000., 192_000., 768_000.] {
            for mode in [GateMode::Vca, GateMode::LowPass, GateMode::Combination] {
                let mut g = LowPassGate::new();
                for i in 0..10_000 {
                    let y = g.process(
                        if i < 100 { 1000.0 } else { 0.0 },
                        if i < 100 { 1.0 } else { 0.0 },
                        mode,
                        fs,
                    );
                    assert!(y.is_finite() && y.abs() <= OUTPUT_BOUND);
                }
                g.reset();
                assert_eq!(g.process(0., 0., mode, fs), 0.0);
            }
        }
    }
}
