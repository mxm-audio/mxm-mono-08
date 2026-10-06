//! Framework-free DSP for mxm-mono-08's 1973-card-set patchable mono voice.
//!
//! The implementation is original code — MIT-licensed until the split of 2026-10-06, and
//! GPL-3.0-or-later with the rest of this repository since — informed by the facts in
//! `research:instruments/buchla-music-easel.md` and its specialist pages. No third-party
//! implementation is copied. Fidelity is unverified: constants not established by those sources
//! are explicitly labelled chosen in this crate's `NOTES.md` and beside their definitions.

#[cfg(any(test, feature = "conformance"))]
pub mod conformance;
pub mod control;
pub mod lpg;
pub mod oscillator;
pub mod routing;
pub mod spring;
pub mod voice;

/// The lowest host rate the plugin activates at; a non-finite rate is refused with it.
///
/// [`voice::Voice::set_sample_rate`] clamps the rate up to it, but `f32::clamp` passes a NaN
/// through, and the gates' `1 Hz ..= 0.45 × rate` clamp then panics on its NaN bound. The value is
/// no higher than the lowest rate clap-validator (1234.57 Hz) or the player's robustness sweeps
/// (1 kHz) ask for.
pub const MIN_SAMPLE_RATE: f32 = 1_000.0;

/// Flush recursive state before it can become denormal; about -400 dB for `f32`.
#[inline(always)]
pub fn flush(x: f32) -> f32 {
    if x.abs() < 1e-20 { 0.0 } else { x }
}

/// A one-entry memo for a per-sample transcendental whose argument rarely changes.
///
/// **It is a cost device, never a numeric one.** A hit returns the value the memoised expression
/// itself produced from this exact key, so a site that adopts one stays bit-identical to the site it
/// replaces; no approximation is introduced and none may be. `plan-mxm-mono-08-per-sample-cost.md`
/// has the measurement that motivated it: the two low-pass gates alone evaluated eight
/// transcendentals per sample, a quarter of the whole instrument, on arguments that are constant for
/// most of a patch's life.
///
/// **The key is the complete argument set**, which is what makes a sample-rate change an ordinary
/// miss rather than an invalidation path that can be got wrong. A `NaN` key never equals itself, so
/// it always misses and always recomputes — the correct fallback. An infinity compares equal and is
/// cached, which is equally correct: it returns what the expression produced from that infinity.
///
/// A memo is pure derived state, so clearing or keeping it across `reset` is unobservable beyond one
/// miss, and no caller has to decide.
#[derive(Debug, Clone, Copy)]
pub struct Memo<const N: usize> {
    key: [f64; N],
    value: f64,
}

impl<const N: usize> Memo<N> {
    pub const fn new() -> Self {
        Self {
            key: [f64::NAN; N],
            value: 0.0,
        }
    }

    /// The cached value for `key`, computing and storing it when the key has moved.
    #[inline(always)]
    pub fn get(&mut self, key: [f64; N], compute: impl FnOnce() -> f64) -> f64 {
        if self.key != key {
            self.key = key;
            self.value = compute();
        }
        self.value
    }
}

impl<const N: usize> Default for Memo<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Deterministic xorshift generator. A zero seed is replaced because it is a fixed point.
#[derive(Debug, Clone)]
pub struct Rng(u32);

impl Rng {
    pub const fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0x9e37_79b9 } else { seed })
    }

    #[inline]
    pub fn unipolar(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 16_777_216.0
    }
}

/// Bounded odd soft clip used only where a provisional analogue drive is named.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-4.0, 4.0);
    let x2 = x * x;
    let y = x * (27.0 + x2) / (27.0 + 9.0 * x2);
    y.clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A hit returns exactly what recomputing returns.** Everything the memoised sites claim
    /// rests on this one property: each adopted it by wrapping an expression in `get`, so if `get`
    /// is transparent then every site is bit-identical to the site it replaced.
    #[test]
    fn a_memo_is_transparent_for_every_key_including_the_hostile_ones() {
        let f = |k: [f64; 2]| (k[0] * 0.75).exp() + k[1].sqrt();
        let mut memo = Memo::<2>::new();
        // Repeats (hits), alternation (the branch thrash), a rate change, then the hostile keys.
        let keys = [
            [1.0, 48_000.0],
            [1.0, 48_000.0],
            [2.0, 48_000.0],
            [1.0, 48_000.0],
            [1.0, 96_000.0],
            [f64::INFINITY, 48_000.0],
            [f64::INFINITY, 48_000.0],
            [f64::NEG_INFINITY, 48_000.0],
            [f64::NAN, 48_000.0],
            [f64::NAN, 48_000.0],
            [0.0, 0.0],
            [-0.0, 0.0],
        ];
        for _ in 0..4 {
            for k in keys {
                let got = memo.get(k, || f(k));
                let want = f(k);
                assert_eq!(
                    got.to_bits(),
                    want.to_bits(),
                    "memo returned {got} for {k:?}, direct gives {want}"
                );
            }
        }
        // A NaN key never equals itself, so it must recompute every time rather than latch.
        let mut calls = 0;
        let mut nan_memo = Memo::<1>::new();
        for _ in 0..8 {
            nan_memo.get([f64::NAN], || {
                calls += 1;
                1.0
            });
        }
        assert_eq!(calls, 8, "a NaN key must always miss");
    }

    #[test]
    fn housekeeping_is_bounded_deterministic_and_exact() {
        assert_eq!(flush(1e-30), 0.0);
        assert_eq!(flush(-0.25), -0.25);
        let (mut a, mut b) = (Rng::new(0x208), Rng::new(0x208));
        for _ in 0..10_000 {
            let x = a.unipolar();
            assert_eq!(x, b.unipolar());
            assert!((0.0..1.0).contains(&x));
        }
        for i in -10_000..=10_000 {
            let y = soft_clip(i as f32);
            assert!(y.is_finite() && y.abs() <= 1.0);
        }
    }
}
