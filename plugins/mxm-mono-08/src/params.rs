//! Permanent host parameters for the patchable voice.
//!
//! Values that are audio/CV signals are smoothed. Times and selectors configure state machines and
//! are deliberately unsmoothed. The route graph is finite but broad: every weighted CV edge and
//! every pulse edge is an ordinary parameter, so host state and presets restore the whole patch.

use mxm_mono_08_dsp::control::EnvelopeMode;
use mxm_mono_08_dsp::lpg::GateMode;
use mxm_mono_08_dsp::oscillator::{ComplexEndpoint, ModWave};
use mxm_mono_08_dsp::voice::{Gate2Source, MOD_HIGH_RATIO, ModulationMode};
use nice_plug::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexEndpointKind {
    #[id = "spike"]
    #[name = "Spike"]
    Spike,
    #[id = "square"]
    #[name = "Square"]
    Square,
    #[id = "triangle"]
    #[name = "Triangle"]
    Triangle,
}
impl From<ComplexEndpointKind> for ComplexEndpoint {
    fn from(value: ComplexEndpointKind) -> Self {
        match value {
            ComplexEndpointKind::Spike => Self::Spike,
            ComplexEndpointKind::Square => Self::Square,
            ComplexEndpointKind::Triangle => Self::Triangle,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModWaveKind {
    #[id = "triangle"]
    #[name = "Triangle"]
    Triangle,
    #[id = "square"]
    #[name = "Square"]
    Square,
    #[id = "saw"]
    #[name = "Sawtooth"]
    Saw,
}
impl From<ModWaveKind> for ModWave {
    fn from(value: ModWaveKind) -> Self {
        match value {
            ModWaveKind::Triangle => Self::Triangle,
            ModWaveKind::Square => Self::Square,
            ModWaveKind::Saw => Self::Saw,
        }
    }
}

/// `modtype`'s options. **Two, where the retired `modmode` had three**: its third, *balanced
/// external*, processed the external input, which the owner dropped (2026-09-23). A stepped id
/// keeps its exact option list (`plans/plan-modulation-routing.md` §5.0), so the list shrank under
/// a new id rather than under the old one.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModulationModeKind {
    #[id = "am"]
    #[name = "AM"]
    Am,
    #[id = "fm"]
    #[name = "FM"]
    Fm,
}
impl From<ModulationModeKind> for ModulationMode {
    fn from(value: ModulationModeKind) -> Self {
        match value {
            ModulationModeKind::Am => Self::Am,
            ModulationModeKind::Fm => Self::Fm,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateModeKind {
    #[id = "vca"]
    #[name = "VCA"]
    Vca,
    #[id = "lowpass"]
    #[name = "VCF"]
    LowPass,
    #[id = "combination"]
    #[name = "VCA + VCF"]
    Combination,
}
impl From<GateModeKind> for GateMode {
    fn from(value: GateModeKind) -> Self {
        match value {
            GateModeKind::Vca => Self::Vca,
            GateModeKind::LowPass => Self::LowPass,
            GateModeKind::Combination => Self::Combination,
        }
    }
}

/// `gate2input`'s options, the retired `gate2source`'s without *External*; see
/// [`ModulationModeKind`] for why that is a new id.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate2SourceKind {
    #[id = "mod"]
    #[name = "Mod oscillator"]
    ModOsc,
    #[id = "gate1"]
    #[name = "LPG 1"]
    Gate1,
}
impl From<Gate2SourceKind> for Gate2Source {
    fn from(value: Gate2SourceKind) -> Self {
        match value {
            Gate2SourceKind::ModOsc => Self::ModOsc,
            Gate2SourceKind::Gate1 => Self::Gate1,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeModeKind {
    #[id = "transient"]
    #[name = "One-shot"]
    Transient,
    #[id = "sustained"]
    #[name = "Sustained"]
    Sustained,
}
impl From<EnvelopeModeKind> for EnvelopeMode {
    fn from(value: EnvelopeModeKind) -> Self {
        match value {
            EnvelopeModeKind::Transient => Self::Transient,
            EnvelopeModeKind::Sustained => Self::Sustained,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceLengthKind {
    #[id = "2"]
    #[name = "2 steps"]
    Two,
    #[id = "3"]
    #[name = "3 steps"]
    Three,
    #[id = "4"]
    #[name = "4 steps"]
    Four,
    #[id = "5"]
    #[name = "5 steps"]
    Five,
}
impl SequenceLengthKind {
    pub const fn stages(self) -> usize {
        match self {
            Self::Two => 2,
            Self::Three => 3,
            Self::Four => 4,
            Self::Five => 5,
        }
    }
}

type ValueToString = Arc<dyn Fn(f32) -> String + Send + Sync>;
type StringToValue = Arc<dyn Fn(&str) -> Option<f32> + Send + Sync>;
/// A whole percentage — **never a negative zero**. A bipolar amount just below zero prints `-0`,
/// which parses to zero and prints `0`, so the text would change through the host's conversion.
fn percent_to_string() -> ValueToString {
    Arc::new(|v| {
        let percent = format!("{:.0}", v * 100.0);
        match percent.as_str() {
            "-0" => String::from("0 %"),
            _ => format!("{percent} %"),
        }
    })
}
fn percent_from_string() -> StringToValue {
    Arc::new(|s| {
        s.trim()
            .trim_end_matches('%')
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| v / 100.0)
    })
}
/// Whole milliseconds below a second, hundredths of a second from it — **the unit chosen from the
/// rounded milliseconds, not the raw value.** Chosen from the raw value, 0.9996 s printed `1000 ms`,
/// which parses to one second and prints `1.00 s`.
fn time_to_string() -> ValueToString {
    Arc::new(|v| {
        let ms = v * 1000.0;
        if ms.round() >= 1000.0 {
            format!("{v:.2} s")
        } else {
            format!("{ms:.0} ms")
        }
    })
}
fn time_from_string() -> StringToValue {
    Arc::new(|s| {
        let s = s.trim().to_lowercase();
        let (n, scale) = if let Some(v) = s.strip_suffix("ms") {
            (v, 0.001)
        } else if let Some(v) = s.strip_suffix('s') {
            (v, 1.0)
        } else {
            (s.as_str(), 0.001)
        };
        n.trim().parse::<f32>().ok().map(|v| v * scale)
    })
}
fn amount(name: impl Into<String>, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_smoother(SmoothingStyle::Linear(10.0))
        .with_value_to_string(percent_to_string())
        .with_string_to_value(percent_from_string())
}
fn time(name: &str, default: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min: 0.002,
            max: 10.0,
            factor: FloatRange::skew_factor(-2.0),
        },
    )
    .with_value_to_string(time_to_string())
    .with_string_to_value(time_from_string())
}

/// **The clock period's tempo sync** (`plans/plan-tempo-sync-controls.md`): 1/64 to four bars, the
/// slice of the ladder the period's 2 ms – 10 s holds at 120 bpm, the top the longest.
pub const PULSER_SYNC: mxm_tempo::Ladder = mxm_tempo::Ladder::new(
    mxm_tempo::Span::new(
        mxm_tempo::Division::SixtyFourth,
        mxm_tempo::Division::FourBars,
    ),
    mxm_tempo::Direction::Time,
);

#[derive(Params)]
pub struct MxmMono08Params {
    #[id = "complexfreq"]
    pub complex_frequency: FloatParam,
    #[id = "complexendpoint"]
    pub complex_endpoint: EnumParam<ComplexEndpointKind>,
    #[id = "wavemix"]
    pub wave_mix: FloatParam,
    #[id = "timbre"]
    pub timbre: FloatParam,
    #[id = "complexkeyboard"]
    pub complex_keyboard: BoolParam,
    #[id = "modfreq"]
    pub mod_frequency: FloatParam,
    #[id = "modwave"]
    pub mod_wave: EnumParam<ModWaveKind>,
    #[id = "modkeyboard"]
    pub mod_keyboard: BoolParam,
    /// The modulation oscillator's high range: [`MOD_HIGH_RATIO`] times the frequency control, so
    /// the same knob reaches audio rates. `mod_frequency` reads in the range this selects.
    #[id = "modhigh"]
    pub mod_high: BoolParam,
    #[id = "modtype"]
    pub modulation_mode: EnumParam<ModulationModeKind>,
    #[id = "modindex"]
    pub modulation_index: FloatParam,
    #[id = "gate1mode"]
    pub gate1_mode: EnumParam<GateModeKind>,
    #[id = "gate1level"]
    pub gate1_level: FloatParam,
    #[id = "gate2mode"]
    pub gate2_mode: EnumParam<GateModeKind>,
    #[id = "gate2level"]
    pub gate2_level: FloatParam,
    #[id = "gate2input"]
    pub gate2_source: EnumParam<Gate2SourceKind>,
    #[id = "mix1"]
    pub mix1: FloatParam,
    #[id = "mix2"]
    pub mix2: FloatParam,
    #[id = "reverb"]
    pub reverb: FloatParam,
    #[id = "master"]
    pub master: FloatParam,
    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "duration"]
    pub duration: FloatParam,
    #[id = "decay"]
    pub decay: FloatParam,
    #[id = "envmode"]
    pub envelope_mode: EnumParam<EnvelopeModeKind>,
    #[id = "pulserperiod"]
    pub pulser_period: FloatParam,
    /// The clock period's tempo sync: its position picks a division of the host's tempo.
    #[id = "pulsersync"]
    pub pulser_sync: BoolParam,
    #[id = "pulserself"]
    pub pulser_self: BoolParam,
    #[id = "seqlength"]
    pub sequence_length: EnumParam<SequenceLengthKind>,
    #[id = "seq1level"]
    pub sequence_1_level: FloatParam,
    #[id = "seq1pulse"]
    pub sequence_1_pulse: BoolParam,
    #[id = "seq2level"]
    pub sequence_2_level: FloatParam,
    #[id = "seq2pulse"]
    pub sequence_2_pulse: BoolParam,
    #[id = "seq3level"]
    pub sequence_3_level: FloatParam,
    #[id = "seq3pulse"]
    pub sequence_3_pulse: BoolParam,
    #[id = "seq4level"]
    pub sequence_4_level: FloatParam,
    #[id = "seq4pulse"]
    pub sequence_4_pulse: BoolParam,
    #[id = "seq5level"]
    pub sequence_5_level: FloatParam,
    #[id = "seq5pulse"]
    pub sequence_5_pulse: BoolParam,
    #[id = "portamento"]
    pub portamento: FloatParam,
    #[id = "bendrange"]
    pub bend_range: FloatParam,

    /// **The routing**: a presence and a signed amount for every *(destination, source)* pair,
    /// and the twelve pulse enables, which are presence only (plan D1). Declared in
    /// `crate::routes`, which owns the permanent ids.
    #[nested(group = "Modulation")]
    pub routes: crate::routes::Routes,

    #[persist = "preset"]
    pub preset: RwLock<mxm_preset::PresetIdentity>,
}

/// The modulation frequency's reading, in whichever range `high` says is selected.
///
/// **The one reading here that depends on another parameter.** The frequency stores the low-range
/// control and the high range multiplies it, so the number a person sees — on the knob and in the
/// host — follows the switch, and a typed frequency is taken in the range showing. `high` is the
/// switch's own value, which its callback keeps; the high reading has one decimal, as the complex
/// oscillator's does.
fn mod_frequency_text(high: Arc<AtomicBool>) -> ValueToString {
    Arc::new(move |hz| {
        if high.load(Ordering::Relaxed) {
            format!("{:.1}", hz * MOD_HIGH_RATIO)
        } else {
            format!("{hz:.2}")
        }
    })
}
fn mod_frequency_from_text(high: Arc<AtomicBool>) -> StringToValue {
    Arc::new(move |s| {
        let hz = s.trim().trim_end_matches("Hz").trim().parse::<f32>().ok()?;
        Some(if high.load(Ordering::Relaxed) {
            hz / MOD_HIGH_RATIO
        } else {
            hz
        })
    })
}

impl Default for MxmMono08Params {
    fn default() -> Self {
        // The high-range switch's value, shared with the frequency's reading.
        let mod_high = Arc::new(AtomicBool::new(false));
        Self {
            complex_frequency: FloatParam::new(
                "Complex frequency",
                220.0,
                FloatRange::Skewed {
                    min: 27.5,
                    max: 1760.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            complex_endpoint: EnumParam::new("Complex wave", ComplexEndpointKind::Triangle),
            wave_mix: amount("Complex wave mix", 0.35),
            timbre: amount("Complex timbre", 0.15),
            complex_keyboard: BoolParam::new("Complex key tracking", true),
            mod_frequency: FloatParam::new(
                "Mod frequency",
                5.0,
                FloatRange::Skewed {
                    min: 0.16,
                    max: 50.0,
                    factor: FloatRange::skew_factor(-2.5),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_unit(" Hz")
            .with_value_to_string(mod_frequency_text(mod_high.clone()))
            .with_string_to_value(mod_frequency_from_text(mod_high.clone())),
            mod_wave: EnumParam::new("Mod wave", ModWaveKind::Triangle),
            mod_keyboard: BoolParam::new("Mod key tracking", false),
            mod_high: BoolParam::new("Mod high range", false).with_callback(Arc::new(
                move |high| mod_high.store(high, Ordering::Relaxed),
            )),
            modulation_mode: EnumParam::new("Mod type", ModulationModeKind::Am),
            modulation_index: amount("Mod depth", 0.0),
            gate1_mode: EnumParam::new("LPG 1 mode", GateModeKind::Combination),
            gate1_level: amount("LPG 1 level", 0.0),
            gate2_mode: EnumParam::new("LPG 2 mode", GateModeKind::Combination),
            gate2_level: amount("LPG 2 level", 0.0),
            gate2_source: EnumParam::new("LPG 2 input", Gate2SourceKind::Gate1),
            mix1: amount("LPG 1 mix", 0.8),
            mix2: amount("LPG 2 mix", 0.0),
            reverb: amount("Reverb", 0.0),
            master: amount("Output", 0.8),
            attack: time("Envelope attack", 0.01),
            duration: time("Envelope hold", 0.12),
            decay: time("Envelope decay", 0.35),
            envelope_mode: EnumParam::new("Envelope mode", EnvelopeModeKind::Transient),
            pulser_period: time("Clock period", 0.5),
            pulser_sync: BoolParam::new("Clock period sync", false),
            pulser_self: BoolParam::new("Clock loop", false),
            sequence_length: EnumParam::new("Sequence length", SequenceLengthKind::Five),
            sequence_1_level: amount("Step 1 level", 0.0),
            sequence_1_pulse: BoolParam::new("Step 1 trigger", true),
            sequence_2_level: amount("Step 2 level", 0.25),
            sequence_2_pulse: BoolParam::new("Step 2 trigger", true),
            sequence_3_level: amount("Step 3 level", 0.5),
            sequence_3_pulse: BoolParam::new("Step 3 trigger", true),
            sequence_4_level: amount("Step 4 level", 0.75),
            sequence_4_pulse: BoolParam::new("Step 4 trigger", true),
            sequence_5_level: amount("Step 5 level", 1.0),
            sequence_5_pulse: BoolParam::new("Step 5 trigger", true),
            portamento: FloatParam::new(
                "Glide time",
                0.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 10.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_value_to_string(time_to_string())
            .with_string_to_value(time_from_string()),
            bend_range: FloatParam::new(
                "Bend range",
                2.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 24.0,
                },
            )
            // Smoothed because it scales a held bend into the pitch CV: a range edit under a held
            // bend would otherwise be a pitch step (`docs/code-review-notes.md` §2).
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit(" st")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            routes: crate::routes::Routes::new(),
            preset: RwLock::new(mxm_preset::PresetIdentity::none()),
        }
    }
}

impl MxmMono08Params {
    /// The clock period while its sync follows the host, or `None` for its free value: the modulated
    /// position picks a division on [`PULSER_SYNC`]. Resolved once a buffer by the plugin.
    pub fn synced_pulser_period(&self, tempo: Option<f64>) -> Option<f32> {
        let param = &self.pulser_period;
        PULSER_SYNC
            .resolve(
                self.pulser_sync.value(),
                tempo,
                param.modulated_normalized_value(),
                f64::from(param.preview_plain(0.0)),
                f64::from(param.preview_plain(1.0)),
            )
            .map(|seconds| seconds as f32)
    }

    /// Every parameter in host declaration order. This is also the editor's future complete testing
    /// surface and the preset file order, so those consumers cannot quietly disagree.
    pub fn all_parameters(&self) -> Vec<(&'static str, &dyn mxm_preset::ErasedParam)> {
        let mut out: Vec<(&'static str, &dyn mxm_preset::ErasedParam)> = vec![
            ("complexfreq", &self.complex_frequency),
            ("complexendpoint", &self.complex_endpoint),
            ("wavemix", &self.wave_mix),
            ("timbre", &self.timbre),
            ("complexkeyboard", &self.complex_keyboard),
            ("modfreq", &self.mod_frequency),
            ("modwave", &self.mod_wave),
            ("modkeyboard", &self.mod_keyboard),
            ("modhigh", &self.mod_high),
            ("modtype", &self.modulation_mode),
            ("modindex", &self.modulation_index),
            ("gate1mode", &self.gate1_mode),
            ("gate1level", &self.gate1_level),
            ("gate2mode", &self.gate2_mode),
            ("gate2level", &self.gate2_level),
            ("gate2input", &self.gate2_source),
            ("mix1", &self.mix1),
            ("mix2", &self.mix2),
            ("reverb", &self.reverb),
            ("master", &self.master),
            ("attack", &self.attack),
            ("duration", &self.duration),
            ("decay", &self.decay),
            ("envmode", &self.envelope_mode),
            ("pulserperiod", &self.pulser_period),
            ("pulsersync", &self.pulser_sync),
            ("pulserself", &self.pulser_self),
            ("seqlength", &self.sequence_length),
            ("seq1level", &self.sequence_1_level),
            ("seq1pulse", &self.sequence_1_pulse),
            ("seq2level", &self.sequence_2_level),
            ("seq2pulse", &self.sequence_2_pulse),
            ("seq3level", &self.sequence_3_level),
            ("seq3pulse", &self.sequence_3_pulse),
            ("seq4level", &self.sequence_4_level),
            ("seq4pulse", &self.sequence_4_pulse),
            ("seq5level", &self.sequence_5_level),
            ("seq5pulse", &self.sequence_5_pulse),
            ("portamento", &self.portamento),
            ("bendrange", &self.bend_range),
        ];
        out.extend(self.routes.parameters());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The clock period's sync picks a division and is inert without a tempo**
    /// (`plans/plan-tempo-sync-controls.md`): off, or with no tempo, the knob's own time stands; on
    /// at 120 bpm the ends are the ladder's ends that the range can hold, the top the longest.
    #[test]
    fn clock_period_sync_picks_a_division_and_is_inert_without_a_tempo() {
        use nice_plug::params::InternalParamMut;
        fn set<P: InternalParamMut>(param: &P, normalized: f32) {
            unsafe {
                let _ = param._internal_set_normalized_value(normalized);
            }
        }
        let p = MxmMono08Params::default();
        set(&p.pulser_period, 1.0);
        assert_eq!(
            p.synced_pulser_period(Some(120.0)),
            None,
            "off is the free time"
        );
        set(&p.pulser_sync, 1.0);
        assert_eq!(
            p.synced_pulser_period(None),
            None,
            "no tempo is the free time"
        );

        let top = p
            .synced_pulser_period(Some(120.0))
            .expect("synced at a tempo");
        set(&p.pulser_period, 0.0);
        let bottom = p
            .synced_pulser_period(Some(120.0))
            .expect("synced at a tempo");
        let (lo, hi) = (
            f64::from(p.pulser_period.preview_plain(0.0)),
            f64::from(p.pulser_period.preview_plain(1.0)),
        );
        assert!(
            top > bottom,
            "the top of a time is the longest: {bottom} to {top}"
        );
        let reach = PULSER_SYNC.reachable(120.0, lo, hi).divisions();
        let shortest = reach[0].seconds(120.0) as f32;
        let longest = reach[reach.len() - 1].seconds(120.0) as f32;
        assert!(
            (bottom - shortest).abs() < 1e-5,
            "{bottom} against {shortest}"
        );
        assert!((top - longest).abs() < 1e-5, "{top} against {longest}");
    }

    /// Plain values either side of every point where a formatter here changes unit, precision or
    /// sign. Each parameter clamps what lies outside its own range, so one list serves them all.
    const BOUNDARIES: [f32; 15] = [
        // The bipolar routing amounts cross zero, and `{:.0} %` rounds a small negative to `-0`:
        // a ten-thousandth and a thousandth of the unit, and the half-percent where rounding ties.
        -0.005, -1.0e-3, -1.0e-4, 0.0, 1.0e-4, 1.0e-3, 0.005,
        // The envelope times, pulser period and portamento read whole milliseconds below a second
        // and hundredths of a second above: both rounding edges below one second and the `1.00 s`
        // bucket above it.
        0.9994, 0.9995, 0.9996, 0.99995, 1.0, 1.004, 1.005, 1.006,
    ];

    /// **Every parameter's text survives the host's own conversion.** The CLAP wrapper formats a
    /// normalised value, parses the text back to a normalised value and formats that again, so a
    /// reading that chooses its unit or its sign from the raw value can print one text, parse to the
    /// other side of its own switch and print another — which `clap-validator`'s
    /// `param-conversions` fails only when its values land in that sliver, so a clean run proves
    /// nothing (`docs/code-review-notes.md` §6). This walks every parameter, with the unit on as the
    /// host sees it, across clap-validator 0.4.1's own grid, the collection's `i / 19` grid, and
    /// the normalised neighbours of every value in [`BOUNDARIES`] — once in each modulation range,
    /// because the frequency reads in the one selected.
    #[test]
    fn every_parameter_text_is_idempotent_through_the_hosts_conversion() {
        let mut failures: Vec<String> = Vec::new();
        for high in [false, true] {
            let params = MxmMono08Params::default();
            if high {
                set_mod_high(&params);
            }
            walk_every_parameter_text(&params, &mut failures);
        }
        assert!(
            failures.is_empty(),
            "{} parameter texts changed through the host's conversion:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    /// Selects the high range the way a host does, so the switch's callback runs.
    fn set_mod_high(params: &MxmMono08Params) {
        let ptr = params.mod_high.as_ptr();
        // SAFETY: `params` owns the switch and outlives the call.
        unsafe {
            ptr._internal_set_normalized_value(1.0);
        }
    }

    fn walk_every_parameter_text(params: &MxmMono08Params, failures: &mut Vec<String>) {
        let map = params.param_map();
        let validator_values = 4_000_usize.div_ceil(map.len()).clamp(5, 100);
        for (id, ptr, _group) in &map {
            // SAFETY: `params` owns every parameter these pointers refer to and outlives the loop;
            // this is the same access `param-conversions` makes through CLAP.
            unsafe {
                // The wrapper hands CLAP `normalised × step count` and divides by it on the way in.
                let steps = ptr.step_count().unwrap_or(1) as f64;
                let from_clap = |value: f64| value as f32 / steps as f32;
                let grid = (0..=19).map(|i| (i as f32 / 19.0, "grid"));
                let validator = (0..validator_values).map(|i| {
                    let value = steps * (i as f64 / (validator_values - 1) as f64);
                    (from_clap(value), "validator grid")
                });
                let boundary = BOUNDARIES.into_iter().flat_map(|plain| {
                    let at = ptr.preview_normalized(plain).clamp(0.0, 1.0);
                    [at.next_down().max(0.0), at, at.next_up().min(1.0)].map(|n| (n, "boundary"))
                });
                for (value, from) in grid.chain(validator).chain(boundary) {
                    let first = ptr.normalized_value_to_string(value, true);
                    let second = ptr.string_to_normalized_value(&first).map(|parsed| {
                        ptr.normalized_value_to_string(from_clap(parsed as f64 * steps), true)
                    });
                    if second.as_deref() != Some(first.as_str()) {
                        let failure = format!("{id}: {first:?} reads back as {second:?}");
                        if failures
                            .last()
                            .is_none_or(|last| !last.starts_with(&failure))
                        {
                            let plain = ptr.preview_plain(value);
                            failures.push(format!("{failure} ({from}, plain {plain})"));
                        }
                    }
                }
            }
        }
    }

    /// **The frequency reads in the range the switch selects**, on the knob and in the host, and
    /// a typed frequency is taken in that range: the default reads 5.00 Hz low and **440.0 Hz
    /// high** — the owner's choice of ratio — and typing 220 in the high range stores 2.5 Hz.
    #[test]
    fn the_modulation_frequency_reads_and_parses_in_the_selected_range() {
        let params = MxmMono08Params::default();
        let at = params.mod_frequency.unmodulated_normalized_value();
        let text =
            |params: &MxmMono08Params| params.mod_frequency.normalized_value_to_string(at, true);
        assert_eq!(text(&params), "5.00 Hz");
        set_mod_high(&params);
        assert!(params.mod_high.value());
        assert_eq!(text(&params), "440.0 Hz");
        let typed = params
            .mod_frequency
            .string_to_normalized_value("220 Hz")
            .expect("a frequency parses");
        let stored = params.mod_frequency.preview_plain(typed);
        assert!((stored - 220.0 / MOD_HIGH_RATIO).abs() < 1e-3, "{stored}");
    }

    /// **A preset written before the high range loads in the low range**, silently: it was written
    /// for the one range there was, so it must not keep the instance's high range, and it is not
    /// an incomplete file.
    #[test]
    fn a_preset_that_does_not_name_the_high_range_loads_in_the_low_range() {
        let params = MxmMono08Params::default();
        set_mod_high(&params);
        let (name, text) = crate::preset::FACTORY_FILES[0];
        let mut preset = mxm_preset::Preset::parse(text, crate::CLAP_ID).expect(name);
        preset.params.remove("modhigh");
        let (writes, problems) = preset.resolve(&params);
        assert!(
            !problems.iter().any(|problem| problem.contains("modhigh")),
            "{problems:?}"
        );
        let write = writes
            .iter()
            .find(|(id, _, _)| *id == "modhigh")
            .expect("the resolver writes the switch");
        assert_eq!(write.2, 0.0, "the low range");
    }
}
