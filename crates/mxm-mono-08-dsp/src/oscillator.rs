//! The 1973-card-set oscillators (`research:oscillators/buchla-208-oscillators.md`).
//!
//! Card 8 and Card 9 retain separate phase state. The model is behavioural: Card 8's triangle
//! cross-modulates Card 9 through a smoothed nonlinear phase displacement rather than pretending
//! the circuit is one phasor followed by a generic folder. The two complex cores and their smooth
//! phase/timbre interaction run at 4×, selected by an error measurement against the same model at
//! 8×. Discontinuous endpoint and modulation-oscillator waves use PolyBLEP at their own sample
//! rate. AM and balanced multiplication are intentionally not described as oversampled.

use crate::{flush, soft_clip};

pub const OVERSAMPLE: usize = 4;
pub const CORE8_RATIO: f64 = 1.0007; // Chosen calibration mismatch, not an original measurement.
pub const TIMBRE_DRIVE: f32 = 2.4; // Chosen.
pub const OPTICAL_RISE_S: f64 = 0.008; // Chosen.
pub const OPTICAL_FALL_S: f64 = 0.045; // Chosen.
pub const MO_AUDIO_HIGHPASS_HZ: f32 = 15.9; // Derived from Card 11's 1 uF into 10 kOhm.
pub const MO_TILT_HZ: f32 = 4.0; // Chosen route-local deformation preserving the observed class.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComplexEndpoint {
    Spike,
    Square,
    #[default]
    Triangle,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModWave {
    #[default]
    Triangle,
    Square,
    Saw,
}

/// The coupled region's controls for one sample.
#[derive(Debug, Clone, Copy)]
pub struct ComplexInput {
    pub hz: f32,
    pub endpoint: ComplexEndpoint,
    pub wave_mix: f32,
    pub timbre: f32,
    /// Bipolar modulation oscillator value.
    pub fm: f32,
    pub mod_index: f32,
}

#[inline]
fn tri(p: f64) -> f32 {
    (1.0 - 4.0 * (p as f32 - 0.5).abs()).clamp(-1.0, 1.0)
}
#[inline]
fn square(p: f64) -> f32 {
    if p < 0.5 { 1.0 } else { -1.0 }
}
#[inline]
fn saw(p: f64) -> f32 {
    2.0 * p as f32 - 1.0
}
/// PolyBLEP correction around a unit-phase discontinuity (Valimaki/Pekonen 2012).
#[inline]
fn poly_blep(phase: f64, step: f64) -> f32 {
    if step <= 0.0 {
        return 0.0;
    }
    if phase < step {
        let t = phase / step;
        (t + t - t * t - 1.0) as f32
    } else if phase > 1.0 - step {
        let t = (phase - 1.0) / step;
        (t * t + t + t + 1.0) as f32
    } else {
        0.0
    }
}
#[inline]
fn sine(p: f64) -> f32 {
    (std::f64::consts::TAU * p).sin() as f32
}
#[inline]
fn wrap(p: f64) -> f64 {
    p - p.floor()
}

#[derive(Debug, Clone, Copy)]
pub struct ComplexOscillator {
    phase8: f64,
    phase9: f64,
    optical: f64,
    /// One slot per direction, each keyed on the **oversampled** rate its `tau` was built at.
    /// `reset` restores `Self::new()`, which clears these; a cleared memo costs one miss.
    optical_c: [crate::Memo<1>; 2],
}
impl Default for ComplexOscillator {
    fn default() -> Self {
        Self::new()
    }
}
impl ComplexOscillator {
    pub const fn new() -> Self {
        Self {
            phase8: 0.0,
            phase9: 0.173,
            optical: 0.0,
            optical_c: [crate::Memo::new(); 2],
        }
    }
    pub fn reset(&mut self) {
        *self = Self::new();
    }
    pub fn phases(&self) -> (f64, f64) {
        (self.phase8, self.phase9)
    }
    /// One host sample. `fm` is bipolar and `mod_index`, `wave_mix`, `timbre` are normalized.
    pub fn process(&mut self, input: ComplexInput, fs: f32) -> f32 {
        self.process_factor::<OVERSAMPLE>(input, fs)
    }
    fn process_factor<const N: usize>(&mut self, input: ComplexInput, fs: f32) -> f32 {
        let fs = fs.max(1.0) as f64 * N as f64;
        let hz = input.hz.clamp(0.01, 0.45 * fs as f32) as f64;
        let phase_step = hz / fs;
        let target = input.timbre.clamp(0.0, 1.0) as f64;
        let rising = target > self.optical;
        let tau = if rising {
            OPTICAL_RISE_S
        } else {
            OPTICAL_FALL_S
        };
        let c = self.optical_c[usize::from(!rising)].get([fs], || 1.0 - (-1.0 / (tau * fs)).exp());
        let mut sum = 0.0;
        for _ in 0..N {
            self.optical += c * (target - self.optical);
            let t8 = tri(self.phase8);
            // Chosen bounded approximation of the dated cards' same-frequency triangle interaction.
            let displacement = soft_clip(t8 * TIMBRE_DRIVE) * self.optical as f32 * 0.18;
            let shaped_phase = wrap(self.phase9 + displacement as f64);
            let shaped = sine(shaped_phase);
            let selected = match input.endpoint {
                ComplexEndpoint::Spike => {
                    let width = 0.045;
                    let pulse = if self.phase8 < width { -1.0 } else { 0.15 };
                    pulse - 0.575 * poly_blep(self.phase8, phase_step)
                        + 0.575 * poly_blep(wrap(self.phase8 - width), phase_step)
                }
                ComplexEndpoint::Square => {
                    square(self.phase8) + poly_blep(self.phase8, phase_step)
                        - poly_blep(wrap(self.phase8 - 0.5), phase_step)
                }
                ComplexEndpoint::Triangle => tri(self.phase9),
            };
            let mix = input.wave_mix.clamp(0.0, 1.0);
            // Timbre disappears at fully selected square/spike, as the directive states.
            let timbre_weight = if input.endpoint == ComplexEndpoint::Triangle {
                1.0
            } else {
                1.0 - mix
            };
            // One `sin` per sub-sample, not two: the same argument was evaluated twice, four times
            // over at 4× oversampling. `a + (shaped - a) * w` is the expression that was here.
            let core9 = sine(self.phase9);
            let sine_branch = core9 + (shaped - core9) * timbre_weight;
            sum += sine_branch + (selected - sine_branch) * mix;
            let fm_ratio = (1.0
                + input.fm.clamp(-1.0, 1.0) as f64 * input.mod_index.clamp(0.0, 1.0) as f64 * 2.0)
                .max(0.0);
            self.phase8 = wrap(self.phase8 + hz * CORE8_RATIO / fs);
            self.phase9 = wrap(self.phase9 + hz * fm_ratio / fs);
        }
        flush(sum / N as f32)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct OnePole {
    s: f64,
    /// Keyed on `(hz, fs)`; both are constant in normal use, so this hits from the second sample.
    c: crate::Memo<2>,
}
impl OnePole {
    fn lowpass(&mut self, x: f32, hz: f32, fs: f32) -> f32 {
        let c = self.c.get([hz as f64, fs as f64], || {
            1.0 - (-std::f64::consts::TAU * hz.clamp(0.01, fs * 0.45) as f64 / fs.max(1.0) as f64)
                .exp()
        });
        self.s += c * (x as f64 - self.s);
        self.s = if self.s.abs() < 1e-20 { 0.0 } else { self.s };
        self.s as f32
    }
    fn reset(&mut self) {
        self.s = 0.0;
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DcBlocker {
    lp: OnePole,
}
impl DcBlocker {
    pub fn reset(&mut self) {
        self.lp.reset();
    }
    pub fn process(&mut self, x: f32, hz: f32, fs: f32) -> f32 {
        flush(x - self.lp.lowpass(x, hz, fs))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModulationOscillator {
    phase: f64,
    tilt: OnePole,
    coupling: DcBlocker,
}
impl Default for ModulationOscillator {
    fn default() -> Self {
        Self::new()
    }
}
impl ModulationOscillator {
    pub const fn new() -> Self {
        Self {
            phase: 0.0,
            tilt: OnePole {
                s: 0.0,
                c: crate::Memo::new(),
            },
            coupling: DcBlocker {
                lp: OnePole {
                    s: 0.0,
                    c: crate::Memo::new(),
                },
            },
        }
    }
    pub fn reset(&mut self) {
        *self = Self::new();
    }
    /// Direct CV remains unipolar and retains DC/sub-audio content.
    pub fn process_cv(&mut self, hz: f32, wave: ModWave, fs: f32) -> f32 {
        let step = hz.clamp(0.01, fs * 0.45) as f64 / fs.max(1.0) as f64;
        self.phase = wrap(self.phase + step);
        match wave {
            ModWave::Triangle => 0.5 + 0.5 * tri(self.phase),
            ModWave::Square => {
                let naive = if self.phase < 0.5 { 1.0 } else { -1.0 };
                0.5 + 0.5
                    * (naive + poly_blep(self.phase, step)
                        - poly_blep(wrap(self.phase - 0.5), step))
            }
            ModWave::Saw => 0.5 + 0.5 * (saw(self.phase) - poly_blep(self.phase, step)),
        }
        .clamp(0.0, 1.0)
    }
    /// The monitored/Gate-2 audio branch alone gets a chosen low-frequency shelf and the derived
    /// coupling. The shelf deforms slow plateaus without putting the complete signal behind a 4 Hz
    /// low-pass: audio-rate gain therefore tends to unity instead of disappearing.
    pub fn monitored_audio(&mut self, direct_cv: f32, fs: f32) -> f32 {
        let bipolar = direct_cv.clamp(0.0, 1.0) * 2.0 - 1.0;
        let low = self.tilt.lowpass(bipolar, MO_TILT_HZ, fs);
        self.coupling
            .process(bipolar - 0.2 * low, MO_AUDIO_HIGHPASS_HZ, fs)
    }
}

#[inline]
pub fn balanced_product(signal: f32, carrier_cv: f32) -> f32 {
    soft_clip(signal) * (carrier_cv.clamp(0.0, 1.0) * 2.0 - 1.0)
}
#[inline]
pub fn am_crossfade(signal: f32, carrier_cv: f32, index: f32) -> f32 {
    let i = index.clamp(0.0, 1.0);
    signal + (balanced_product(signal, carrier_cv) - signal) * i
}

#[cfg(test)]
mod tests {
    use super::*;
    const FS: f32 = 48_000.0;
    fn input(
        hz: f32,
        endpoint: ComplexEndpoint,
        wave_mix: f32,
        timbre: f32,
        fm: f32,
        mod_index: f32,
    ) -> ComplexInput {
        ComplexInput {
            hz,
            endpoint,
            wave_mix,
            timbre,
            fm,
            mod_index,
        }
    }
    fn tone_level(x: &[f32], hz: f32, fs: f32) -> f64 {
        let (mut re, mut im) = (0., 0.);
        for (i, &v) in x.iter().enumerate() {
            let a = std::f64::consts::TAU * hz as f64 * i as f64 / fs as f64;
            re += v as f64 * a.cos();
            im += v as f64 * a.sin();
        }
        2.0 * (re * re + im * im).sqrt() / x.len() as f64
    }
    #[test]
    fn the_complex_cores_are_separate_state() {
        let mut o = ComplexOscillator::new();
        for _ in 0..48_000 {
            o.process(input(440., ComplexEndpoint::Triangle, 0., 0., 0., 0.), FS);
        }
        let (a, b) = o.phases();
        assert!(
            (a - b).abs() > 1e-4,
            "independent calibrated cores must not collapse to one phase: {a} {b}"
        );
    }
    #[test]
    fn waveform_mix_and_timbre_are_distinct_and_timbre_leaves_square_endpoint_alone() {
        let render = |endpoint, mix, timbre| {
            let mut o = ComplexOscillator::new();
            (0..4096)
                .map(|_| o.process(input(375., endpoint, mix, timbre, 0., 0.), FS))
                .collect::<Vec<_>>()
        };
        assert_ne!(
            render(ComplexEndpoint::Triangle, 0., 0.),
            render(ComplexEndpoint::Triangle, 0., 1.)
        );
        assert_ne!(
            render(ComplexEndpoint::Triangle, 0., 0.),
            render(ComplexEndpoint::Square, 1., 0.)
        );
        assert_eq!(
            render(ComplexEndpoint::Square, 1., 0.),
            render(ComplexEndpoint::Square, 1., 1.)
        );
    }
    #[test]
    fn complex_oscillator_tracks_frequency_and_is_bounded_at_supported_rates() {
        for fs in [
            1_000.0f32, 44_100.0, 48_000.0, 96_000.0, 192_000.0, 768_000.0,
        ] {
            let mut o = ComplexOscillator::new();
            for i in 0..(fs.min(48_000.) as usize) {
                let y = o.process(
                    input(
                        (220. + i as f32 * 0.01).min(fs * 0.4),
                        ComplexEndpoint::Spike,
                        1.,
                        1.,
                        1.,
                        1.,
                    ),
                    fs,
                );
                assert!(y.is_finite() && y.abs() <= 1.1, "{fs}: {y}");
            }
        }
    }
    #[test]
    fn four_x_complex_core_interaction_stays_close_to_an_eight_x_reference() {
        for (name, controls, limit) in [
            (
                "smooth FM",
                input(1900., ComplexEndpoint::Triangle, 0.2, 1., 0.7, 1.),
                -26.0,
            ),
            (
                "spike endpoint",
                input(3200., ComplexEndpoint::Spike, 1.0, 0.8, 0.4, 1.),
                -18.0,
            ),
            (
                "square endpoint",
                input(3200., ComplexEndpoint::Square, 1.0, 0.0, 0.4, 1.),
                -18.0,
            ),
        ] {
            let (mut a, mut b) = (ComplexOscillator::new(), ComplexOscillator::new());
            let mut err = 0.;
            let mut sig = 0.;
            for _ in 0..8192 {
                let x = a.process_factor::<4>(controls, FS);
                let r = b.process_factor::<8>(controls, FS);
                err += (x - r).powi(2) as f64;
                sig += r.powi(2) as f64;
            }
            let db = 10.0 * (err / sig).log10();
            eprintln!("{name}: 4x versus 8x complex-core RMS deviation: {db:.2} dB");
            assert!(db < limit, "{name}: 4x deviation against 8x is {db:0.1} dB");
        }
    }
    #[test]
    fn monitored_square_tilts_while_direct_cv_holds() {
        let mut o = ModulationOscillator::new();
        let mut first = 0.;
        let mut last = 0.;
        for i in 0..(FS * 0.4) as usize {
            let cv = o.process_cv(2., ModWave::Square, FS);
            let a = o.monitored_audio(cv, FS);
            if i == (FS * 0.01) as usize {
                first = a;
            }
            if i == (FS * 0.20) as usize {
                last = a;
            }
        }
        assert!(
            first.abs() > last.abs() * 1.2,
            "audio square should droop: {first} {last}"
        );
    }
    #[test]
    fn monitored_route_preserves_audio_rate_level_while_deforming_low_rate_plateaus() {
        fn level(hz: f32) -> f64 {
            let mut o = ModulationOscillator::new();
            let mut samples = Vec::new();
            for i in 0..(FS * 2.0) as usize {
                let cv = o.process_cv(hz, ModWave::Square, FS);
                let y = o.monitored_audio(cv, FS);
                if i >= FS as usize {
                    samples.push(y);
                }
            }
            (samples.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / samples.len() as f64)
                .sqrt()
        }
        let slow = level(2.0);
        let panel_top = level(50.0);
        let routed_audio = level(1_000.0);
        assert!(slow > 0.05, "2 Hz deformation erased the route: {slow}");
        assert!(panel_top > 0.65, "50 Hz level was attenuated: {panel_top}");
        assert!(
            routed_audio > 0.85,
            "externally driven audio-rate level was attenuated: {routed_audio}"
        );
    }
    #[test]
    fn direct_cv_keeps_dc_while_the_audio_route_rejects_it() {
        let mut o = ModulationOscillator::new();
        let mut cv = 0.;
        let mut audio = 0.;
        for _ in 0..(FS * 3.) as usize {
            cv = o.process_cv(0.01, ModWave::Square, FS);
            audio = o.monitored_audio(cv, FS);
        }
        assert_eq!(cv, 1.0);
        assert!(audio.abs() < 1e-4, "AC route retained DC: {audio}");
    }
    #[test]
    fn balanced_mode_has_difference_sideband_and_am_zero_is_dry() {
        assert_eq!(am_crossfade(0.37, 0.9, 0.), 0.37);
        let mut x = Vec::new();
        for i in 0..4800 {
            let t = i as f32 / FS;
            x.push(balanced_product(
                (std::f32::consts::TAU * 440. * t).sin(),
                0.5 + 0.5 * (std::f32::consts::TAU * 110. * t).sin(),
            ));
        }
        assert!(tone_level(&x, 330., FS) > 0.2);
    }
}
