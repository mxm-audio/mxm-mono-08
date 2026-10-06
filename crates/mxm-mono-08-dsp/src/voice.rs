//! The complete deterministic 208/218 network.
//!
//! Signal order is frozen in `crates/mxm-mono-08-dsp/AGENTS.md`. Pulse routing runs before held
//! state, then continuous sources, then the audio path. A producer unavailable at a destination's
//! point reads `CvFrame`'s preceding sample, making every backward route a one-sample delay.

use crate::control::{Envelope, EnvelopeMode, Pulser, RandomVoltages, Sequencer};
use crate::flush;
use crate::lpg::{GateMode, LowPassGate};
use crate::oscillator::{
    ComplexEndpoint, ComplexInput, ComplexOscillator, ModWave, ModulationOscillator, am_crossfade,
};
use crate::routing::{
    CV_SOURCES, CvDestination, CvSource, Graph, KEY_UNIT_SEMITONES, PULSE_SOURCES,
    PulseDestination, PulseSource, Routing, complement,
};
use crate::spring::Spring;
use mxm_modulation::standard;

pub const OUTPUT_BOUND: f32 = 8.0;
pub const POST_TAIL_S: f32 = 0.25; // Chosen digital settle.
pub const SILENCE_FLOOR: f32 = 1e-7;
pub const PRESS_CAPACITY: usize = 16;
pub const CV_OCTAVES: f32 = 5.0; // Chosen destination scaling per normalized CV unit.
pub const MOD_CV_OCTAVES: f32 = 8.0; // Chosen.
/// What the modulation oscillator's high range multiplies its panel rate by, so the 0.16–50 Hz
/// control reads 14.08–4400 Hz. **Chosen so the control's 5 Hz default is 440 Hz** (owner,
/// 2026-09-23: *"This should probably be 440 Hz default in the high range to make better musical
/// sense"*), which puts a semitone step from the default on equal temperament at A440.
///
/// **A departure from the target revision** (owner, 2026-09-23): the original has one range, and a
/// range switch arrived with the 2013 reissue, whose high range is 55–1760 Hz
/// (`research:instruments/buchla-music-easel.md` §3.2, §9). A ratio rather than that span keeps a
/// pitch step on the control a pitch step in the sound.
pub const MOD_HIGH_RATIO: f32 = 88.0; // Chosen.
pub const DEFAULT_KEY: u8 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModulationMode {
    #[default]
    Am,
    Fm,
}
/// What Gate 2 processes. **No external input** (the owner's ruling, 2026-09-23): the original's
/// preamp, envelope detector and balanced external mode were dropped with the plugin's auxiliary
/// port, because the hosts it is played in do not offer that port by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Gate2Source {
    ModOsc,
    #[default]
    Gate1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    Live,
    Tailing,
    Inert,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteId {
    pub voice_id: Option<i32>,
    pub channel: u8,
    pub key: u8,
}
#[derive(Debug, Clone, Copy)]
struct Press {
    id: NoteId,
    serial: u64,
    poly_pressure: Option<f32>,
    tuning: f32,
    active: bool,
}
const EMPTY_PRESS: Press = Press {
    id: NoteId {
        voice_id: None,
        channel: 0,
        key: DEFAULT_KEY,
    },
    serial: 0,
    poly_pressure: None,
    tuning: 0.,
    active: false,
};

/// Plain values. Parameter smoothing belongs to the plugin shell.
#[derive(Debug, Clone, PartialEq)]
pub struct Params {
    pub complex_hz: f32,
    pub complex_endpoint: ComplexEndpoint,
    pub wave_mix: f32,
    pub timbre: f32,
    pub complex_keyboard: bool,
    pub mod_hz: f32,
    /// The high range: [`MOD_HIGH_RATIO`] times `mod_hz`, applied after the panel clamp.
    pub mod_high: bool,
    pub mod_wave: ModWave,
    pub mod_keyboard: bool,
    pub modulation_mode: ModulationMode,
    pub modulation_index: f32,
    pub gate1_mode: GateMode,
    pub gate1_level: f32,
    pub gate2_mode: GateMode,
    pub gate2_level: f32,
    pub gate2_source: Gate2Source,
    pub mix1: f32,
    pub mix2: f32,
    pub reverb: f32,
    pub master: f32,
    pub attack_s: f32,
    pub duration_s: f32,
    pub decay_s: f32,
    pub envelope_mode: EnvelopeMode,
    pub pulser_period_s: f32,
    pub pulser_self: bool,
    pub sequence_length: usize,
    pub sequence_levels: [f32; 5],
    pub sequence_pulses: [bool; 5],
    pub portamento_s: f32,
    /// Channel pitch bend supplied by the plugin, in semitones. Per-note tuning remains stored on
    /// the press it names so the two performance signals cannot lose their ownership.
    pub bend_semitones: f32,
    /// The last press's note-on velocity, `0..=1`, held through its release — every press is a
    /// keyboard pulse, so this is the press that last could trigger the envelope. Full before any
    /// press, so the standard Velocity rests at zero. **A routing source and nothing else**: this
    /// machine has no velocity sensitivity of its own.
    pub velocity: f32,
    /// CC 1 on the sounding press's channel, `0..=1`. A routing source and nothing else; it writes
    /// no parameter, which is what the control map's CC 1 reservation allows.
    pub wheel: f32,
    /// The bender's lever position, signed `-1..=1` — **not** its reach in semitones, which travels
    /// in `bend_semitones` beside it for the hard-wired path into complex pitch.
    pub bend: f32,
}
impl Default for Params {
    fn default() -> Self {
        // The two routes the machine itself wires now live in `Routing::init`, which travels beside
        // this patch rather than inside it.
        Self {
            complex_hz: 220.,
            complex_endpoint: ComplexEndpoint::Triangle,
            wave_mix: 0.35,
            timbre: 0.15,
            complex_keyboard: true,
            mod_hz: 5.,
            mod_high: false,
            mod_wave: ModWave::Triangle,
            mod_keyboard: false,
            modulation_mode: ModulationMode::Am,
            modulation_index: 0.,
            gate1_mode: GateMode::Combination,
            gate1_level: 0.,
            gate2_mode: GateMode::Combination,
            gate2_level: 0.,
            gate2_source: Gate2Source::Gate1,
            mix1: 0.8,
            mix2: 0.,
            reverb: 0.,
            master: 0.8,
            attack_s: 0.01,
            duration_s: 0.12,
            decay_s: 0.35,
            envelope_mode: EnvelopeMode::Transient,
            pulser_period_s: 0.5,
            pulser_self: false,
            sequence_length: 5,
            sequence_levels: [0., 0.25, 0.5, 0.75, 1.],
            sequence_pulses: [true; 5],
            portamento_s: 0.,
            bend_semitones: 0.,
            velocity: 1.,
            wheel: 0.,
            bend: 0.,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Voice {
    fs: f32,
    /// The two pitch exponentials. Their argument is constant in any patch that does not route
    /// something into that pitch, which is most of them; a patch that does modulate pitch misses
    /// every sample and pays one comparison. See `plan-mxm-mono-08-per-sample-cost.md`.
    mod_hz_exp: crate::Memo<1>,
    complex_hz_exp: crate::Memo<1>,
    presses: [Press; PRESS_CAPACITY],
    serial: u64,
    owner: NoteId,
    key: f64,
    pressure: f32,
    touch_pending: bool,
    once_pending: u16,
    pulser_end_pending: bool,
    graph: Graph,
    envelope: Envelope,
    pulser: Pulser,
    sequencer: Sequencer,
    random: RandomVoltages,
    complex: ComplexOscillator,
    mod_osc: ModulationOscillator,
    gate1: LowPassGate,
    gate2: LowPassGate,
    spring: Spring,
    activity: Activity,
    silent: u32,
    last_out: f32,
    last_complex: f32,
    last_mod_cv: f32,
    last_gate: [f32; 2],
    last_period: f32,
    last_stage_pulse: bool,
    stage_pulse_latch: bool,
    last_cv_sources: [f32; CV_SOURCES],
    pulse_latch: [bool; PULSE_SOURCES],
}
impl Default for Voice {
    fn default() -> Self {
        Self::new()
    }
}
impl Voice {
    pub fn new() -> Self {
        let mut v = Self {
            mod_hz_exp: crate::Memo::new(),
            complex_hz_exp: crate::Memo::new(),
            fs: 48_000.,
            presses: [EMPTY_PRESS; PRESS_CAPACITY],
            serial: 0,
            owner: NoteId {
                voice_id: None,
                channel: 0,
                key: DEFAULT_KEY,
            },
            key: DEFAULT_KEY as f64,
            pressure: 0.,
            touch_pending: false,
            once_pending: 0,
            pulser_end_pending: false,
            graph: Graph::new(),
            envelope: Envelope::new(),
            pulser: Pulser::new(),
            sequencer: Sequencer::new(),
            random: RandomVoltages::new(),
            complex: ComplexOscillator::new(),
            mod_osc: ModulationOscillator::new(),
            gate1: LowPassGate::new(),
            gate2: LowPassGate::new(),
            spring: Spring::new(),
            activity: Activity::Inert,
            silent: u32::MAX,
            last_out: 0.,
            last_complex: 0.,
            last_mod_cv: 0.,
            last_gate: [0.; 2],
            last_period: 0.5,
            last_stage_pulse: false,
            stage_pulse_latch: false,
            last_cv_sources: [0.0; CV_SOURCES],
            pulse_latch: [false; PULSE_SOURCES],
        };
        v.set_sample_rate(48_000.);
        v
    }
    /// Rebuilds which routes are live: **once per processing interval, never per sample.**
    ///
    /// Topology is discrete and changes only on a parameter event, which is the whole of the
    /// efficiency design — the per-sample sums then run over the routes that exist rather than over
    /// every source the instrument declares. Every caller that changes a presence owes this call
    /// before the next render.
    pub fn set_topology(&mut self, routing: &Routing) {
        self.graph.set_topology(routing);
    }

    /// Allocates spring memory; activation only.
    pub fn set_sample_rate(&mut self, fs: f32) {
        self.fs = fs.clamp(crate::MIN_SAMPLE_RATE, 768_000.);
        self.spring.set_sample_rate(self.fs);
    }
    /// Clear all runtime state without resizing the spring delay memory. Hosts may call this on
    /// the audio thread, so rebuilding `Self` here would be a realtime allocation.
    pub fn reset(&mut self) {
        self.all_sound_off();
        self.serial = 0;
    }
    pub fn all_sound_off(&mut self) {
        self.presses = [EMPTY_PRESS; PRESS_CAPACITY];
        self.owner = EMPTY_PRESS.id;
        self.key = f64::from(DEFAULT_KEY);
        self.pressure = 0.;
        self.touch_pending = false;
        self.once_pending = 0;
        self.pulser_end_pending = false;
        self.graph.reset();
        self.envelope.reset();
        self.pulser.reset();
        self.sequencer.reset();
        self.random.reset();
        self.complex.reset();
        self.mod_osc.reset();
        self.gate1.reset();
        self.gate2.reset();
        self.spring.reset();
        self.activity = Activity::Inert;
        self.silent = u32::MAX;
        self.last_out = 0.;
        self.last_complex = 0.;
        self.last_stage_pulse = false;
        self.stage_pulse_latch = false;
        self.pulse_latch = [false; PULSE_SOURCES];
    }
    fn sounding_index(&self) -> Option<usize> {
        self.presses
            .iter()
            .enumerate()
            .filter(|(_, p)| p.active)
            .max_by_key(|(_, p)| p.serial)
            .map(|(i, _)| i)
    }
    pub fn note_on(&mut self, id: NoteId) {
        self.serial = self.serial.wrapping_add(1).max(1);
        let slot = self
            .presses
            .iter()
            .position(|p| !p.active)
            .unwrap_or_else(|| {
                self.presses
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, p)| p.serial)
                    .unwrap()
                    .0
            });
        self.presses[slot] = Press {
            id,
            serial: self.serial,
            poly_pressure: None,
            tuning: 0.,
            active: true,
        };
        self.owner = id;
        self.touch_pending = true;
        self.activity = Activity::Live;
        self.silent = 0;
    }
    fn press_index(&self, voice_id: Option<i32>, channel: u8, key: u8) -> Option<usize> {
        let by_id = voice_id.and_then(|id| {
            self.presses
                .iter()
                .position(|p| p.active && p.id.voice_id == Some(id) && p.id.channel == channel)
        });
        by_id.or_else(|| {
            self.presses
                .iter()
                .enumerate()
                .filter(|(_, p)| p.active && p.id.channel == channel && p.id.key == key)
                .min_by_key(|(_, p)| p.serial)
                .map(|(i, _)| i)
        })
    }
    pub fn note_off(&mut self, voice_id: Option<i32>, channel: u8, key: u8) {
        if let Some(i) = self.press_index(voice_id, channel, key) {
            self.presses[i].active = false;
        }
        if let Some(i) = self.sounding_index() {
            self.owner = self.presses[i].id;
            self.pressure = self.presses[i].poly_pressure.unwrap_or(0.0);
        } else {
            self.pressure = 0.;
            self.envelope.set_gate(false);
        }
    }
    /// Immediately retire one named touch. Another held touch takes ownership; with none left the
    /// keyed envelope and low-pass gates close without their release, while the post-mix spring is
    /// allowed to finish. Independent pulser/routing state is not part of the named note.
    pub fn choke(&mut self, voice_id: Option<i32>, channel: u8, key: u8) {
        let Some(i) = self.press_index(voice_id, channel, key) else {
            return;
        };
        self.presses[i].active = false;
        if let Some(i) = self.sounding_index() {
            self.owner = self.presses[i].id;
            self.pressure = self.presses[i].poly_pressure.unwrap_or(0.0);
        } else {
            self.pressure = 0.0;
            self.touch_pending = false;
            self.envelope.reset();
            self.gate1.reset();
            self.gate2.reset();
            self.last_gate = [0.0; 2];
        }
    }
    pub fn all_notes_off(&mut self) {
        for p in &mut self.presses {
            p.active = false;
        }
        self.pressure = 0.;
        self.envelope.set_gate(false);
    }
    pub fn set_pressure(
        &mut self,
        voice_id: Option<i32>,
        channel: u8,
        key: u8,
        value: f32,
    ) -> bool {
        if let Some(p) = self.presses.iter_mut().find(|p| {
            p.active
                && p.id.channel == channel
                && p.id.key == key
                && (voice_id.is_none() || p.id.voice_id == voice_id)
        }) {
            p.poly_pressure = Some(value.clamp(0., 1.));
            if p.id == self.owner {
                self.pressure = p.poly_pressure.unwrap_or(0.0);
            }
            true
        } else {
            false
        }
    }
    /// Apply channel-owned pressure to the sounding touch without replacing a per-note value.
    pub fn set_channel_pressure(&mut self, value: f32) {
        let owner_has_poly = self
            .sounding_index()
            .is_some_and(|i| self.presses[i].poly_pressure.is_some());
        if !owner_has_poly {
            self.pressure = value.clamp(0.0, 1.0);
        }
    }
    pub fn pressure(&self) -> f32 {
        self.pressure
    }
    pub fn set_tuning(
        &mut self,
        voice_id: Option<i32>,
        channel: u8,
        key: u8,
        semitones: f32,
    ) -> bool {
        if let Some(p) = self.presses.iter_mut().find(|p| {
            p.active
                && p.id.channel == channel
                && p.id.key == key
                && (voice_id.is_none() || p.id.voice_id == voice_id)
        }) {
            p.tuning = semitones.clamp(-48., 48.);
            true
        } else {
            false
        }
    }
    /// Per-note tuning belongs to the sounding press and holds through its release. A new press
    /// replaces the owner and starts with zero tuning; a note-off must not snap an existing tail
    /// back to the key centre.
    fn owner_tuning(&self) -> f32 {
        self.presses
            .iter()
            .find(|press| press.id == self.owner)
            .map_or(0.0, |press| press.tuning)
    }
    /// The DSP-side operation invoked once per accepted transient command by the plugin shell.
    pub fn once(&mut self) -> bool {
        if self.once_pending == u16::MAX {
            false
        } else {
            self.once_pending += 1;
            self.activity = Activity::Live;
            true
        }
    }
    pub fn activity(&self) -> Activity {
        self.activity
    }
    /// The press whose channel owns bend and channel pressure, retained through its release tail.
    pub fn owner(&self) -> NoteId {
        self.owner
    }
    /// The complex oscillator sample before Gate 1, for observational editor telemetry only.
    pub fn complex_sample(&self) -> f32 {
        self.last_complex
    }
    pub fn stage(&self) -> usize {
        self.sequencer.stage()
    }
    pub fn sequence_value(&self) -> f32 {
        self.sequencer.value()
    }
    pub fn stage_pulsed(&self) -> bool {
        self.last_stage_pulse
    }
    pub fn modulation_cv(&self) -> f32 {
        self.last_mod_cv
    }
    pub fn gate_levels(&self) -> [f32; 2] {
        self.last_gate
    }
    pub fn pulser_period(&self) -> f32 {
        self.last_period
    }
    pub fn envelope_level(&self) -> f32 {
        self.envelope.level()
    }
    pub fn cv_sources(&self) -> [f32; CV_SOURCES] {
        self.last_cv_sources
    }
    /// What the frame holds for a source now — the conformance checks' view of a publisher.
    #[cfg(test)]
    pub(crate) fn published_for_test(&self, source: CvSource) -> f32 {
        self.graph.read(source)
    }
    /// Drain every one-sample pulse observed since the shell last published telemetry.
    pub fn take_pulse_telemetry(&mut self) -> (bool, [bool; PULSE_SOURCES]) {
        let stage = std::mem::take(&mut self.stage_pulse_latch);
        let sources = std::mem::replace(&mut self.pulse_latch, [false; PULSE_SOURCES]);
        (stage, sources)
    }
    fn routed(routing: &Routing, d: PulseDestination, pulses: [bool; PULSE_SOURCES]) -> bool {
        routing.pulsed(d, pulses)
    }
    fn gate_drives_reach_audio(p: &Params, gate1_drive: f32, gate2_drive: f32) -> bool {
        let gate1_open = gate1_drive > 0.0;
        let gate2_open = gate2_drive > 0.0;
        let gate1_audible = gate1_open
            && (p.mix1 > 0.0
                || (p.mix2 > 0.0 && p.gate2_source == Gate2Source::Gate1 && gate2_open));
        let gate2_carrier = match p.gate2_source {
            Gate2Source::ModOsc => true,
            Gate2Source::Gate1 => gate1_open,
        };
        gate1_audible || (p.mix2 > 0.0 && gate2_open && gate2_carrier)
    }
    fn current_audio_path_live(&self, p: &Params, routing: &Routing) -> bool {
        Self::gate_drives_reach_audio(
            p,
            p.gate1_level + self.graph.sum(CvDestination::Gate1, routing),
            p.gate2_level + self.graph.sum(CvDestination::Gate2, routing),
        )
    }
    fn sum_with_source_value(
        &self,
        routing: &Routing,
        destination: CvDestination,
        variable: CvSource,
        value: f32,
    ) -> f32 {
        // Keep the affine sum unclipped so its zero crossing stays exact. For legal panel gate
        // levels, the sum's own [-4, 4] bound cannot change which side of zero the drive ends on.
        //
        // **Live routes only** (`plans/plan-mxm-mono-08-modulation.md` §7): an absent pair
        // contributes nothing whatever its amount holds, so a predicate that walked every source
        // would be answering for a patch the audio path does not render.
        self.graph
            .live(destination)
            .iter()
            .map(|&index| {
                let source = CvSource::ALL[index];
                let source_value = if source == variable {
                    value
                } else if source == CvSource::Inverter {
                    // **The inverter is a destination now** (D2), so asking what it would read
                    // means summing its own routes under the same substitution. One level deep and
                    // no deeper: `INVERTER_IS_NOT_ITS_OWN_SOURCE` keeps it out of its own live
                    // list, so this cannot recur again.
                    complement(self.sum_with_source_value(
                        routing,
                        CvDestination::InverterInput,
                        variable,
                        value,
                    ))
                } else {
                    self.graph.read(source)
                };
                source_value * routing.amounts[destination.index()][index].clamp(-1.0, 1.0)
            })
            .sum()
    }
    fn positive_affine_interval(at_low: f32, at_high: f32) -> Option<(f32, f32)> {
        match (at_low > 0.0, at_high > 0.0) {
            (false, false) => None,
            (true, true) => Some((0.0, 1.0)),
            (false, true) => {
                let crossing = (-at_low / (at_high - at_low)).clamp(0.0, 1.0);
                Some((crossing, 1.0))
            }
            (true, false) => {
                let crossing = (-at_low / (at_high - at_low)).clamp(0.0, 1.0);
                Some((0.0, crossing))
            }
        }
    }
    /// **Presence first, then depth** (§7): a route that does not exist controls nothing, however
    /// deep the amount stored against it happens to be.
    /// Whether a live route carries this source into the inverter's input at any depth.
    ///
    /// **D2's replacement for `p.inverter_input.source() == source`.** The inverter reads a sum
    /// now, so a source reaches it through a present pair rather than by being the one selected —
    /// and several sources can reach it at once, which the selector could never express.
    fn source_drives_inverter(routing: &Routing, source: CvSource) -> bool {
        let destination = CvDestination::InverterInput.index();
        routing.present[destination][source.index()]
            && routing.amounts[destination][source.index()].abs() > f32::EPSILON
    }
    fn source_controls_gate(routing: &Routing, source: CvSource) -> bool {
        [CvDestination::Gate1, CvDestination::Gate2]
            .into_iter()
            .any(|destination| {
                let present = &routing.present[destination.index()];
                let amounts = &routing.amounts[destination.index()];
                let reaches =
                    |s: CvSource| present[s.index()] && amounts[s.index()].abs() > f32::EPSILON;
                reaches(source)
                    || (Self::source_drives_inverter(routing, source)
                        && reaches(CvSource::Inverter))
            })
    }
    fn source_value_reaches_audio(
        &self,
        p: &Params,
        routing: &Routing,
        source: CvSource,
        value: f32,
    ) -> bool {
        Self::source_controls_gate(routing, source)
            && Self::gate_drives_reach_audio(
                p,
                p.gate1_level
                    + self.sum_with_source_value(routing, CvDestination::Gate1, source, value),
                p.gate2_level
                    + self.sum_with_source_value(routing, CvDestination::Gate2, source, value),
            )
    }
    const fn random_index(source: CvSource) -> Option<usize> {
        match source {
            CvSource::Random1 => Some(0),
            CvSource::Random2 => Some(1),
            CvSource::Random3 => Some(2),
            CvSource::Random4 => Some(3),
            _ => None,
        }
    }
    fn sum_with_random_values(
        &self,
        routing: &Routing,
        destination: CvDestination,
        values: [f32; 4],
    ) -> f32 {
        self.graph
            .live(destination)
            .iter()
            .map(|&index| {
                let source = CvSource::ALL[index];
                let source_value = if let Some(slot) = Self::random_index(source) {
                    values[slot]
                } else if source == CvSource::Inverter {
                    // **Wider than the branch it replaces.** The selector named one source, so this
                    // substituted at most a single random slot; the inverter's input is a sum now,
                    // and every random route into it substitutes at once.
                    complement(self.sum_with_random_values(
                        routing,
                        CvDestination::InverterInput,
                        values,
                    ))
                } else {
                    self.graph.read(source)
                };
                source_value * routing.amounts[destination.index()][index].clamp(-1.0, 1.0)
            })
            .sum()
    }
    fn random_values_reach_audio(&self, p: &Params, routing: &Routing) -> bool {
        let random_sources = [
            CvSource::Random1,
            CvSource::Random2,
            CvSource::Random3,
            CvSource::Random4,
        ];
        if !random_sources
            .into_iter()
            .any(|source| Self::source_controls_gate(routing, source))
        {
            return false;
        }
        let drives = |values| {
            (
                p.gate1_level + self.sum_with_random_values(routing, CvDestination::Gate1, values),
                p.gate2_level + self.sum_with_random_values(routing, CvDestination::Gate2, values),
            )
        };
        let reaches = |values| {
            let (gate1, gate2) = drives(values);
            Self::gate_drives_reach_audio(p, gate1, gate2)
        };

        // A single affine gate reaches its maximum at a box vertex. For two serial gates the
        // maximum of their minimum may instead occur where the two drives cross on an edge.
        for mask in 0..16 {
            let values = std::array::from_fn(|index| ((mask >> index) & 1) as f32);
            if reaches(values) {
                return true;
            }
        }
        if p.mix2 <= 0.0 || p.gate2_source != Gate2Source::Gate1 {
            return false;
        }
        for free in 0..4 {
            for mask in 0..8 {
                let mut low = [0.0; 4];
                let mut bit = 0;
                for (index, value) in low.iter_mut().enumerate() {
                    if index != free {
                        *value = ((mask >> bit) & 1) as f32;
                        bit += 1;
                    }
                }
                let mut high = low;
                high[free] = 1.0;
                let (g1_low, g2_low) = drives(low);
                let (g1_high, g2_high) = drives(high);
                let difference_low = g1_low - g2_low;
                let difference_change = (g1_high - g2_high) - difference_low;
                if difference_change.abs() > f32::EPSILON {
                    let crossing = -difference_low / difference_change;
                    if crossing > 0.0 && crossing < 1.0 {
                        low[free] = crossing;
                        if reaches(low) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
    fn source_interval_reaches_audio(
        &self,
        p: &Params,
        routing: &Routing,
        source: CvSource,
        low: f32,
        high: f32,
    ) -> bool {
        if !Self::source_controls_gate(routing, source) {
            return false;
        }
        let gate_interval = |destination, base| {
            Self::positive_affine_interval(
                base + self.sum_with_source_value(routing, destination, source, low),
                base + self.sum_with_source_value(routing, destination, source, high),
            )
        };
        let gate1 = gate_interval(CvDestination::Gate1, p.gate1_level);
        let gate2 = gate_interval(CvDestination::Gate2, p.gate2_level);
        let gate1_audible = p.mix1 > 0.0 && gate1.is_some();
        let gate2_carried = match p.gate2_source {
            Gate2Source::ModOsc => gate2.is_some(),
            Gate2Source::Gate1 => match (gate1, gate2) {
                (Some((g1_low, g1_high)), Some((g2_low, g2_high))) => {
                    g1_low.max(g2_low) < g1_high.min(g2_high)
                }
                _ => false,
            },
        };
        gate1_audible || (p.mix2 > 0.0 && gate2_carried)
    }
    fn autonomous_modulation_reaches_audio(&self, p: &Params, routing: &Routing) -> bool {
        self.source_interval_reaches_audio(p, routing, CvSource::ModOsc, 0.0, 1.0)
    }
    fn refresh_parameter_owned_control_values(&mut self, p: &Params, routing: &Routing) {
        self.sequencer.set(p.sequence_levels, p.sequence_pulses);
        self.graph
            .write(CvSource::Sequencer, self.sequencer.value());
        let inverter = self.graph.inverter(routing);
        self.graph.write(CvSource::Inverter, inverter);
    }
    fn audible_gate_memory(&self, p: &Params) -> bool {
        let gate1_open = self.gate1.optical_level() > 0.0;
        let gate1_audible = gate1_open
            && (p.mix1 > 0.0
                || (p.mix2 > 0.0
                    && p.gate2_source == Gate2Source::Gate1
                    && self.gate2.optical_level() > 0.0));
        let gate2_carrier = match p.gate2_source {
            Gate2Source::ModOsc => true,
            Gate2Source::Gate1 => gate1_open,
        };
        gate1_audible || (p.mix2 > 0.0 && self.gate2.optical_level() > 0.0 && gate2_carrier)
    }
    fn pulser_reaches_audio(&self, p: &Params, routing: &Routing) -> bool {
        if self.source_interval_reaches_audio(p, routing, CvSource::Pulser, 0.0, 1.0) {
            return true;
        }
        let end = PulseSource::PulserEnd.index();
        let stage = PulseSource::SequenceStage.index();
        let sequence = routing.pulses[PulseDestination::Sequencer.index()][end];
        let length_cv = self.graph.sum(CvDestination::SequenceLength, routing);
        let length = ((p.sequence_length.clamp(2, 5) as f32 + length_cv * 3.0).round() as isize)
            .clamp(2, 5) as usize;
        let stage_can_pulse = p.sequence_pulses[..length].iter().any(|enabled| *enabled);
        let envelope = routing.pulses[PulseDestination::Envelope.index()][end]
            || (sequence
                && stage_can_pulse
                && routing.pulses[PulseDestination::Envelope.index()][stage]);
        let random = routing.pulses[PulseDestination::Random.index()][end]
            || (sequence
                && stage_can_pulse
                && routing.pulses[PulseDestination::Random.index()][stage]);
        (envelope && self.source_interval_reaches_audio(p, routing, CvSource::Envelope, 0.0, 1.0))
            || (random && self.random_values_reach_audio(p, routing))
            || (sequence
                && p.sequence_levels[..length].iter().any(|&value| {
                    self.source_value_reaches_audio(p, routing, CvSource::Sequencer, value)
                }))
    }
    #[inline]
    pub fn process(&mut self, p: &Params, routing: &Routing) -> f32 {
        let fs = self.fs;
        let held = self.sounding_index();
        if let Some(i) = held {
            self.owner = self.presses[i].id;
            if let Some(poly) = self.presses[i].poly_pressure {
                self.pressure = poly;
            }
        }
        if p.master <= 0. {
            self.activity = Activity::Inert;
            self.last_out = 0.;
            return 0.;
        }
        self.refresh_parameter_owned_control_values(p, routing);
        let envelope_live = self.envelope.active()
            && self.source_interval_reaches_audio(p, routing, CvSource::Envelope, 0.0, 1.0);
        let local_live = self.current_audio_path_live(p, routing)
            || self.autonomous_modulation_reaches_audio(p, routing)
            || envelope_live;
        let pulser_path_live = self.pulser_reaches_audio(p, routing);
        let pulser_live =
            (self.pulser.running() || self.pulser_end_pending || self.once_pending > 0)
                && pulser_path_live;
        let event = self.touch_pending
            || self.once_pending > 0
            || (self.pulser_end_pending && pulser_path_live);
        if self.activity == Activity::Inert && !event && !pulser_live && !local_live {
            return 0.0;
        }
        if event || local_live {
            self.activity = Activity::Live;
            self.silent = 0;
        }
        self.graph.begin_sample();
        // The performance sources travel with pressure, before the first destination reads: they
        // are held values the plugin supplies, not signals this voice generates — each through the
        // collection's standard, zero at its rest, Velocity as `v − 1`.
        self.graph
            .write(CvSource::Pressure, standard::pressure(self.pressure));
        self.graph
            .write(CvSource::Velocity, standard::velocity(p.velocity));
        self.graph.write(CvSource::Wheel, standard::wheel(p.wheel));
        self.graph.write(CvSource::Bend, standard::bend(p.bend));
        let target = self.owner.key as f64
            + self.owner_tuning() as f64
            + p.bend_semitones.clamp(-48., 48.) as f64;
        let pressure_speed = self
            .graph
            .sum(CvDestination::PortamentoSpeed, routing)
            .max(0.) as f64;
        let glide = p.portamento_s.clamp(0., 10.) as f64;
        if glide <= 0. {
            self.key = target
        } else {
            let step = (12.0 / (glide * fs as f64)) * (1.0 + 4.0 * pressure_speed);
            self.key += (target - self.key).clamp(-step, step);
        }
        // The standard Key, after glide: `(note − 60) / 60`, bipolar about middle C.
        self.graph.write(
            CvSource::Key,
            standard::key(self.key as f32, KEY_UNIT_SEMITONES),
        );

        let mut pulses = [false; PULSE_SOURCES];
        pulses[PulseSource::Keyboard.index()] = self.touch_pending;
        pulses[PulseSource::PulserEnd.index()] = self.pulser_end_pending;
        self.touch_pending = false;
        self.pulser_end_pending = false;
        let seq_cv = self.graph.sum(CvDestination::SequenceLength, routing);
        let length = ((p.sequence_length.clamp(2, 5) as f32 + seq_cv * 3.).round() as isize)
            .clamp(2, 5) as usize;
        let advance = Self::routed(routing, PulseDestination::Sequencer, pulses);
        self.last_stage_pulse = if advance {
            self.sequencer.advance(length)
        } else {
            false
        };
        pulses[PulseSource::SequenceStage.index()] = self.last_stage_pulse;
        if Self::routed(routing, PulseDestination::Random, pulses) {
            self.random.trigger();
        }
        let env_trigger = Self::routed(routing, PulseDestination::Envelope, pulses);
        let pulser_trigger = Self::routed(routing, PulseDestination::Pulser, pulses)
            || (p.pulser_self && pulses[PulseSource::PulserEnd.index()])
            || self.once_pending > 0;
        if self.once_pending > 0 {
            self.once_pending -= 1;
        }
        if env_trigger {
            self.envelope.trigger();
        }
        if pulser_trigger {
            self.pulser.trigger();
        }
        // The keyboard pulse route also holds the envelope's gate while a key is down. It reads a
        // presence, which has no depth (D1), so this is the route existing and nothing more.
        let keyboard_sustain = routing.pulses[PulseDestination::Envelope.index()]
            [PulseSource::Keyboard.index()]
            && held.is_some();
        self.envelope.set_gate(keyboard_sustain || env_trigger);
        self.graph
            .write(CvSource::Sequencer, self.sequencer.value());
        for (i, x) in self.random.values().into_iter().enumerate() {
            self.graph.write(
                [
                    CvSource::Random1,
                    CvSource::Random2,
                    CvSource::Random3,
                    CvSource::Random4,
                ][i],
                x,
            );
        }
        // In octaves: the network's pairs scaled once by `MOD_CV_OCTAVES`, as they always were.
        let mod_pitch = self.graph.pitch_octaves(CvDestination::ModPitch, routing);
        let key_oct = if p.mod_keyboard {
            (self.key as f32 - 60.) / 12.
        } else {
            0.
        };
        let mod_oct = (key_oct + mod_pitch).clamp(-16., 16.);
        let range = if p.mod_high { MOD_HIGH_RATIO } else { 1.0 };
        let mod_hz = p.mod_hz.clamp(0.16, 50.0)
            * range
            * self
                .mod_hz_exp
                .get([mod_oct as f64], || mod_oct.exp2() as f64) as f32;
        let mod_cv = self.mod_osc.process_cv(mod_hz, p.mod_wave, fs);
        self.last_mod_cv = mod_cv;
        self.graph.write(CvSource::ModOsc, mod_cv);
        let env = self
            .envelope
            .process(p.attack_s, p.duration_s, p.decay_s, p.envelope_mode, fs);
        self.graph.write(CvSource::Envelope, env);
        let period_cv = self.graph.sum(CvDestination::PulserPeriod, routing).max(0.);
        let period = p.pulser_period_s.clamp(0.002, 10.) * (1. - 0.95 * period_cv.clamp(0., 1.));
        self.last_period = period;
        let (ramp, end) = self.pulser.process(period, fs);
        self.graph.write(CvSource::Pulser, ramp);
        self.pulser_end_pending = end;
        let inv = self.graph.inverter(routing);
        self.graph.write(CvSource::Inverter, inv);
        let timbre = (p.timbre + self.graph.sum(CvDestination::Timbre, routing)).clamp(0., 1.);
        let index =
            (p.modulation_index + self.graph.sum(CvDestination::ModIndex, routing)).clamp(0., 1.);
        // In octaves: the network's pairs scaled once by `CV_OCTAVES`, as they always were.
        let pitch_cv = self
            .graph
            .pitch_octaves(CvDestination::ComplexPitch, routing);
        let pitch_oct = if p.complex_keyboard {
            (self.key as f32 - 69.) / 12.
        } else {
            0.
        };
        let complex_oct = (pitch_oct + pitch_cv).clamp(-16., 16.);
        let hz = p.complex_hz.clamp(0.01, 20_000.)
            * self
                .complex_hz_exp
                .get([complex_oct as f64], || complex_oct.exp2() as f64) as f32;
        let bipolar = mod_cv * 2. - 1.;
        let mut complex = self.complex.process(
            ComplexInput {
                hz,
                endpoint: p.complex_endpoint,
                wave_mix: p.wave_mix,
                timbre,
                fm: if p.modulation_mode == ModulationMode::Fm {
                    bipolar
                } else {
                    0.
                },
                mod_index: index,
            },
            fs,
        );
        if p.modulation_mode == ModulationMode::Am {
            complex = am_crossfade(complex, mod_cv, index);
        }
        self.last_complex = complex;
        // **Published last, so every route from it is one sample late** — including back into the
        // complex pitch and timbre that make it, which is what makes this instrument's FM a
        // first-class route rather than a special case. It needs no scale: `easel_measure` finds a
        // peak of exactly one across the keyboard, with nothing above it.
        self.graph.write(CvSource::ComplexAudio, complex);
        let monitored = self.mod_osc.monitored_audio(mod_cv, fs);
        let g1_drive =
            (p.gate1_level + self.graph.sum(CvDestination::Gate1, routing)).clamp(0., 1.);
        let a = self.gate1.process(complex, g1_drive, p.gate1_mode, fs);
        let source = match p.gate2_source {
            Gate2Source::ModOsc => monitored,
            Gate2Source::Gate1 => a,
        };
        let g2_drive =
            (p.gate2_level + self.graph.sum(CvDestination::Gate2, routing)).clamp(0., 1.);
        let b = self.gate2.process(source, g2_drive, p.gate2_mode, fs);
        self.last_gate = [self.gate1.optical_level(), self.gate2.optical_level()];
        let mixed = a * p.mix1.clamp(0., 1.) - b * p.mix2.clamp(0., 1.);
        // **The collection's standard Amplitude**, a factor on the mix before the spring and before
        // the mix's own bound: it cannot open a closed gate, so nothing about activity changes.
        let mixed = if self.graph.is_empty(CvDestination::Amplitude) {
            mixed
        } else {
            mixed * standard::amplitude_factor(self.graph.sum(CvDestination::Amplitude, routing))
        };
        let dry = mixed.clamp(-4., 4.);
        let out = flush(
            (self.spring.process(dry, p.reverb, fs) * p.master.clamp(0., 1.))
                .clamp(-OUTPUT_BOUND, OUTPUT_BOUND),
        );
        self.last_out = out;
        self.last_cv_sources = self.graph.values();
        self.stage_pulse_latch |= self.last_stage_pulse;
        for (latched, fired) in self.pulse_latch.iter_mut().zip(pulses) {
            *latched |= fired;
        }
        let future_live = ((self.pulser.running()
            || self.pulser_end_pending
            || self.once_pending > 0)
            && self.pulser_reaches_audio(p, routing))
            || self.current_audio_path_live(p, routing)
            || self.autonomous_modulation_reaches_audio(p, routing)
            || (self.envelope.active()
                && self.source_interval_reaches_audio(p, routing, CvSource::Envelope, 0.0, 1.0));
        if future_live {
            self.activity = Activity::Live;
            self.silent = 0;
        } else if out.abs() > SILENCE_FLOOR {
            self.activity = Activity::Tailing;
            self.silent = 0;
        } else {
            self.activity = Activity::Tailing;
            self.silent = self.silent.saturating_add(1);
            if self.silent >= (POST_TAIL_S * fs) as u32
                && !self.spring.active()
                && !self.audible_gate_memory(p)
            {
                self.activity = Activity::Inert;
                self.complex.reset();
                self.mod_osc.reset();
                self.last_out = 0.;
                return 0.;
            }
        }
        out
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    const FS: f32 = 48_000.;
    fn id(key: u8) -> NoteId {
        NoteId {
            voice_id: None,
            channel: 0,
            key,
        }
    }
    /// A patch with nothing routed, and **an empty routing beside it** — which is what makes the
    /// parking tests below mean what they say.
    fn silent_patch() -> (Params, Routing) {
        let mut p = Params::default();
        p.gate1_level = 0.0;
        p.gate2_level = 0.0;
        p.mix1 = 1.0;
        p.mix2 = 0.0;
        p.reverb = 0.0;
        (p, Routing::new())
    }
    /// Wire a route: **present, at that depth.** A test that set only an amount would be asserting
    /// against a pair the audio path never reads, and would pass for the wrong reason.
    fn wire(routing: &mut Routing, d: CvDestination, s: CvSource, amount: f32) {
        routing.present[d.index()][s.index()] = true;
        routing.amounts[d.index()][s.index()] = amount;
    }
    fn pulse(routing: &mut Routing, d: PulseDestination, s: PulseSource) {
        routing.pulses[d.index()][s.index()] = true;
    }
    /// A voice whose live lists already match `routing`, as the shell compacts once per interval.
    fn voice(routing: &Routing) -> Voice {
        let mut v = Voice::new();
        v.set_topology(routing);
        v
    }
    fn peak(x: &[f32]) -> f32 {
        x.iter().fold(0., |m, x| m.max(x.abs()))
    }
    fn render(v: &mut Voice, p: &Params, routing: &Routing, s: f32) -> Vec<f32> {
        (0..(FS * s) as usize)
            .map(|_| v.process(p, routing))
            .collect()
    }
    #[test]
    fn init_is_keyed_and_reaches_exact_idle_silence() {
        let routing = Routing::init();
        let mut v = voice(&routing);
        let p = Params::default();
        assert_eq!(v.process(&p, &routing), 0.);
        v.note_on(id(69));
        let x = render(&mut v, &p, &routing, 0.5);
        assert!(peak(&x) > 0.01);
        v.note_off(None, 0, 69);
        let x = render(&mut v, &p, &routing, 8.);
        assert_eq!(*x.last().unwrap(), 0.);
        assert_eq!(v.activity(), Activity::Inert);
        for _ in 0..1000 {
            assert_eq!(v.process(&p, &routing), 0.);
        }
    }
    #[test]
    fn reset_and_panic_leave_no_tail_or_pending_pulse() {
        let routing = Routing::init();
        let mut v = voice(&routing);
        let mut p = Params::default();
        p.reverb = 1.;
        v.note_on(id(60));
        render(&mut v, &p, &routing, 0.2);
        v.once();
        v.all_sound_off();
        assert_eq!(v.process(&p, &routing), 0.);
        assert_eq!(v.activity(), Activity::Inert);
        v.note_on(id(60));
        render(&mut v, &p, &routing, 0.05);
        v.reset();
        assert_eq!(v.process(&p, &routing), 0.);
    }
    #[test]
    fn equal_gate_paths_cancel_in_the_opposite_polarity_mixer() {
        let mut p = Params::default();
        p.gate1_level = 1.;
        p.gate2_level = 1.;
        p.mix1 = 1.;
        p.mix2 = 1.;
        p.gate1_mode = GateMode::Vca;
        p.gate2_mode = GateMode::Vca;
        p.gate2_source = Gate2Source::Gate1;
        // Both gates sit open at their panel levels with nothing routed into either.
        let routing = Routing::new();
        let mut v = voice(&routing);
        let x = render(&mut v, &p, &routing, 0.5);
        assert!(
            peak(&x[(FS * 0.25) as usize..]) < 0.02,
            "paths did not cancel: {}",
            peak(&x)
        );
        p.mix2 = 0.;
        let mut v = voice(&routing);
        assert!(peak(&render(&mut v, &p, &routing, 0.5)) > 0.1);
    }
    #[test]
    fn a_non_self_pulser_end_can_advance_the_sequencer() {
        let mut p = Params::default();
        p.pulser_period_s = 0.01;
        let mut routing = Routing::init();
        pulse(
            &mut routing,
            PulseDestination::Sequencer,
            PulseSource::PulserEnd,
        );
        let mut v = voice(&routing);
        v.once();
        for _ in 0..1_000 {
            v.process(&p, &routing);
            if v.sequencer.stage() == 1 {
                return;
            }
        }
        panic!("a pulser end is available to routing even when self mode is off");
    }

    #[test]
    fn feedback_rhythm_changes_period_without_a_block_delay() {
        let mut p = Params::default();
        p.pulser_self = true;
        p.sequence_levels = [0., 1., 0., 1., 0.];
        p.pulser_period_s = 0.02;
        // The declared order, over routes rather than a dense grid: the sequencer's new voltage
        // reaches the pulser's period in the same sample it is produced.
        let mut routing = Routing::init();
        pulse(
            &mut routing,
            PulseDestination::Sequencer,
            PulseSource::PulserEnd,
        );
        wire(
            &mut routing,
            CvDestination::PulserPeriod,
            CvSource::Sequencer,
            1.,
        );
        let mut v = voice(&routing);
        v.once();
        let mut stages = Vec::new();
        let mut old = 0;
        for i in 0..5000 {
            v.process(&p, &routing);
            if v.stage() != old {
                stages.push((i, v.stage(), v.pulser_period()));
                old = v.stage();
                if stages.len() == 3 {
                    break;
                }
            }
        }
        assert!(stages.len() >= 2);
        assert!(
            stages[0].2 < 0.003,
            "new stage CV did not shorten current period: {:?}",
            stages
        );
        assert!(stages[1].2 > 0.018);
    }
    #[test]
    fn pulse_disabled_stage_still_changes_voice_sequence_value() {
        let mut p = Params::default();
        p.sequence_levels = [0.1, 0.7, 0.3, 0.4, 0.5];
        p.sequence_pulses = [true, false, true, true, true];
        let mut routing = Routing::init();
        pulse(
            &mut routing,
            PulseDestination::Sequencer,
            PulseSource::Keyboard,
        );
        let mut v = voice(&routing);
        v.process(&p, &routing);
        v.note_on(id(60));
        v.process(&p, &routing);
        assert_eq!(v.stage(), 1);
        assert_eq!(v.sequence_value(), 0.7);
        assert!(!v.stage_pulsed());
    }
    #[test]
    fn only_an_audibly_reachable_slow_control_loop_stays_live() {
        fn assert_parks(p: &Params, routing: &Routing, why: &str) -> Voice {
            let mut v = voice(routing);
            assert_eq!(v.process(p, routing), 0.);
            v.once();
            for _ in 0..(FS * 2.0) as usize {
                assert_eq!(v.process(p, routing), 0.0, "{why}");
            }
            assert_eq!(v.activity(), Activity::Inert, "{why}");
            v
        }

        let mut disconnected = Params::default();
        disconnected.pulser_self = true;
        disconnected.pulser_period_s = 10.;
        // Not one presence anywhere, which is what "disconnected" now means.
        let bare = Routing::new();
        let mut parked = assert_parks(&disconnected, &bare, "all routes disconnected");
        let phases = parked.complex.phases();
        for _ in 0..10_000 {
            assert_eq!(parked.process(&disconnected, &bare), 0.0);
        }
        assert_eq!(
            parked.complex.phases(),
            phases,
            "an inaudible loop did core work"
        );

        let mut closed_gate = disconnected.clone();
        closed_gate.mix1 = 1.0;
        let mut closed_gate_routing = Routing::new();
        wire(
            &mut closed_gate_routing,
            CvDestination::ComplexPitch,
            CvSource::Pulser,
            1.0,
        );
        assert_parks(
            &closed_gate,
            &closed_gate_routing,
            "pitch modulation cannot pass a closed Gate 1",
        );

        let mut disabled_stages = disconnected.clone();
        disabled_stages.pulser_period_s = 0.05;
        disabled_stages.sequence_pulses = [false; 5];
        let mut staged_routing = Routing::new();
        pulse(
            &mut staged_routing,
            PulseDestination::Sequencer,
            PulseSource::PulserEnd,
        );
        pulse(
            &mut staged_routing,
            PulseDestination::Envelope,
            PulseSource::SequenceStage,
        );
        wire(
            &mut staged_routing,
            CvDestination::Gate1,
            CvSource::Envelope,
            1.0,
        );
        assert_parks(
            &disabled_stages,
            &staged_routing,
            "disabled sequence stages cannot conduct a pulse loop",
        );

        let mut outside_length = disabled_stages.clone();
        outside_length.sequence_length = 2;
        outside_length.sequence_pulses = [false, false, true, true, true];
        assert_parks(
            &outside_length,
            &staged_routing,
            "enabled stages outside the active length cannot conduct a pulse loop",
        );

        let mut enabled_stage = disabled_stages;
        enabled_stage.sequence_pulses = [true; 5];
        let mut staged = voice(&staged_routing);
        staged.once();
        let mut staged_peak = 0.0f32;
        for _ in 0..(FS * 2.0) as usize {
            staged_peak = staged_peak.max(staged.process(&enabled_stage, &staged_routing).abs());
        }
        assert!(staged_peak > 1e-4, "the enabled stage pulse did not sound");
        assert_eq!(staged.activity(), Activity::Live);

        let reachable = disconnected.clone();
        let mut reachable_routing = Routing::new();
        wire(
            &mut reachable_routing,
            CvDestination::Gate1,
            CvSource::Pulser,
            1.0,
        );
        let mut live = voice(&reachable_routing);
        live.once();
        for _ in 0..(FS * 2.) as usize {
            live.process(&reachable, &reachable_routing);
        }
        assert_eq!(live.activity(), Activity::Live);

        let through_inverter = disconnected.clone();
        let mut inverter_routing = Routing::new();
        wire(
            &mut inverter_routing,
            CvDestination::InverterInput,
            CvSource::Pulser,
            1.0,
        );
        wire(
            &mut inverter_routing,
            CvDestination::Gate1,
            CvSource::Inverter,
            1.0,
        );
        let mut inverted = voice(&inverter_routing);
        inverted.once();
        let mut inverted_peak = 0.0f32;
        for _ in 0..(FS * 2.) as usize {
            inverted_peak =
                inverted_peak.max(inverted.process(&through_inverter, &inverter_routing).abs());
        }
        assert!(
            inverted_peak > 1e-4,
            "the selected inverter route was not audible"
        );
        assert_eq!(
            inverted.activity(),
            Activity::Live,
            "the pulser dependency through the selected inverter was lost"
        );

        let autonomous_mod = disconnected;
        let mut autonomous_routing = Routing::new();
        wire(
            &mut autonomous_routing,
            CvDestination::Gate1,
            CvSource::ModOsc,
            1.0,
        );
        let mut modulated = voice(&autonomous_routing);
        let mut modulated_peak = 0.0f32;
        for _ in 0..(FS * 2.) as usize {
            modulated_peak = modulated_peak.max(
                modulated
                    .process(&autonomous_mod, &autonomous_routing)
                    .abs(),
            );
        }
        assert!(
            modulated_peak > 1e-4,
            "the autonomous modulation oscillator did not open Gate 1"
        );
        assert_eq!(
            modulated.activity(),
            Activity::Live,
            "an audible autonomous modulation route was parked"
        );
    }
    #[test]
    fn signed_cv_cancellation_parks_pulser_and_pulse_derived_routes() {
        fn assert_parks(patch: &Params, routing: &Routing, why: &str) {
            let mut v = voice(routing);
            v.once();
            for _ in 0..(FS * 2.0) as usize {
                assert_eq!(v.process(patch, routing), 0.0, "{why}");
            }
            assert_eq!(v.activity(), Activity::Inert, "{why}");
        }

        let (mut direct, mut direct_routing) = silent_patch();
        direct.pulser_self = true;
        direct.pulser_period_s = 0.05;
        direct.sequence_levels = [1.0; 5];
        wire(
            &mut direct_routing,
            CvDestination::Gate1,
            CvSource::Pulser,
            1.0,
        );
        wire(
            &mut direct_routing,
            CvDestination::Gate1,
            CvSource::Sequencer,
            -1.0,
        );
        assert_parks(
            &direct,
            &direct_routing,
            "held CV did not cancel the full pulser range",
        );

        let (mut derived, mut derived_routing) = silent_patch();
        derived.pulser_self = true;
        derived.pulser_period_s = 0.05;
        derived.sequence_levels = [1.0; 5];
        pulse(
            &mut derived_routing,
            PulseDestination::Envelope,
            PulseSource::PulserEnd,
        );
        wire(
            &mut derived_routing,
            CvDestination::Gate1,
            CvSource::Envelope,
            1.0,
        );
        wire(
            &mut derived_routing,
            CvDestination::Gate1,
            CvSource::Sequencer,
            -1.0,
        );
        assert_parks(
            &derived,
            &derived_routing,
            "held CV did not cancel the pulse-derived envelope range",
        );
    }
    #[test]
    fn joint_random_box_keeps_a_serial_gate_path_live_until_trigger() {
        let (mut patch, mut routing) = silent_patch();
        patch.mix1 = 0.0;
        patch.mix2 = 1.0;
        patch.gate2_source = Gate2Source::Gate1;
        patch.pulser_self = true;
        patch.pulser_period_s = 1.0;
        patch.sequence_levels = [1.0; 5];
        pulse(
            &mut routing,
            PulseDestination::Random,
            PulseSource::PulserEnd,
        );
        for destination in [CvDestination::Gate1, CvDestination::Gate2] {
            wire(&mut routing, destination, CvSource::Sequencer, -1.0);
            for source in [
                CvSource::Random1,
                CvSource::Random2,
                CvSource::Random3,
                CvSource::Random4,
            ] {
                wire(&mut routing, destination, source, 1.0);
            }
        }

        let mut v = voice(&routing);
        v.once();
        for _ in 0..(FS * 0.5) as usize {
            assert_eq!(v.process(&patch, &routing), 0.0);
        }
        assert_eq!(
            v.activity(),
            Activity::Live,
            "the voice parked before the first joint random update"
        );

        let mut peak = 0.0f32;
        for _ in 0..FS as usize {
            peak = peak.max(v.process(&patch, &routing).abs());
        }
        assert!(
            peak > 1e-6,
            "the joint random update never opened both gates"
        );
    }
    #[test]
    fn held_control_voltage_is_live_only_when_it_conducts_to_audio() {
        fn assert_held_cv_live(
            patch: (Params, Routing),
            configure: impl FnOnce(&mut Params, &mut Routing),
            why: &str,
        ) {
            let (mut p, mut routing) = patch;
            configure(&mut p, &mut routing);
            let mut v = voice(&routing);
            v.note_on(id(60));
            v.process(&p, &routing);
            v.note_off(None, 0, 60);
            let mut output_peak = 0.0f32;
            for _ in 0..(FS * 2.0) as usize {
                output_peak = output_peak.max(v.process(&p, &routing).abs());
            }
            assert!(output_peak > 1e-4, "{why}: route was not audible");
            assert_eq!(v.activity(), Activity::Live, "{why}");
        }

        let (disconnected, bare) = silent_patch();
        let mut inaudible = voice(&bare);
        inaudible.note_on(id(60));
        for _ in 0..(FS * 2.0) as usize {
            assert_eq!(inaudible.process(&disconnected, &bare), 0.0);
        }
        assert_eq!(
            inaudible.activity(),
            Activity::Inert,
            "a held but disconnected note prevented parking"
        );

        assert_held_cv_live(
            silent_patch(),
            |p, routing| {
                p.sequence_levels = [0.75; 5];
                pulse(routing, PulseDestination::Sequencer, PulseSource::Keyboard);
                wire(routing, CvDestination::Gate1, CvSource::Sequencer, 1.0);
            },
            "held sequencer CV was reported as a finite tail",
        );
        assert_held_cv_live(
            silent_patch(),
            |_, routing| {
                pulse(routing, PulseDestination::Random, PulseSource::Keyboard);
                for source in [
                    CvSource::Random1,
                    CvSource::Random2,
                    CvSource::Random3,
                    CvSource::Random4,
                ] {
                    wire(routing, CvDestination::Gate1, source, 1.0);
                }
            },
            "held random CV was reported as a finite tail",
        );
        assert_held_cv_live(
            silent_patch(),
            |_p, routing| {
                wire(
                    routing,
                    CvDestination::InverterInput,
                    CvSource::Pressure,
                    1.0,
                );
                wire(routing, CvDestination::Gate1, CvSource::Inverter, 1.0);
            },
            "held inverter CV was reported as a finite tail",
        );

        let (mut parked_patch, mut parked_routing) = silent_patch();
        parked_patch.sequence_levels = [0.75; 5];
        pulse(
            &mut parked_routing,
            PulseDestination::Sequencer,
            PulseSource::Keyboard,
        );
        let mut parked_sequence = voice(&parked_routing);
        parked_sequence.note_on(id(60));
        parked_sequence.process(&parked_patch, &parked_routing);
        parked_sequence.note_off(None, 0, 60);
        for _ in 0..(FS * 2.0) as usize {
            assert_eq!(parked_sequence.process(&parked_patch, &parked_routing), 0.0);
        }
        assert_eq!(parked_sequence.activity(), Activity::Inert);
        // The route arrives while the voice is inert: a new presence, compacted as the shell would.
        wire(
            &mut parked_routing,
            CvDestination::Gate1,
            CvSource::Sequencer,
            1.0,
        );
        parked_sequence.set_topology(&parked_routing);
        assert!(
            parked_sequence
                .process(&parked_patch, &parked_routing)
                .abs()
                > 1e-6,
            "a newly connected held sequencer CV did not wake the inert voice"
        );
        assert_eq!(parked_sequence.activity(), Activity::Live);
    }
    #[test]
    fn editing_the_active_stage_wakes_its_inert_gate_route() {
        let (mut patch, mut routing) = silent_patch();
        wire(&mut routing, CvDestination::Gate1, CvSource::Sequencer, 1.0);
        patch.sequence_levels = [0.0; 5];
        let mut v = voice(&routing);
        assert_eq!(v.process(&patch, &routing), 0.0);
        assert_eq!(v.activity(), Activity::Inert);

        patch.sequence_levels[0] = 0.75;
        let mut peak = 0.0f32;
        for _ in 0..256 {
            peak = peak.max(v.process(&patch, &routing).abs());
        }
        assert!(
            peak > 1e-6,
            "an active-stage value edit was hidden behind the inert shortcut"
        );
        assert_eq!(v.activity(), Activity::Live);
    }
    #[test]
    fn signed_cv_cancellation_parks_an_autonomous_gate_route() {
        let (mut patch, mut routing) = silent_patch();
        patch.sequence_levels = [1.0; 5];
        wire(
            &mut routing,
            CvDestination::Gate1,
            CvSource::Sequencer,
            -1.0,
        );
        wire(&mut routing, CvDestination::Gate1, CvSource::ModOsc, 1.0);
        let mut v = voice(&routing);
        let phases = v.complex.phases();
        for _ in 0..10_000 {
            assert_eq!(v.process(&patch, &routing), 0.0);
        }
        assert_eq!(v.activity(), Activity::Inert);
        assert_eq!(
            v.complex.phases(),
            phases,
            "a fully cancelled autonomous route advanced the audio core"
        );
    }
    #[test]
    fn serial_gates_find_an_interior_autonomous_overlap() {
        let (mut patch, mut routing) = silent_patch();
        patch.mix1 = 0.0;
        patch.mix2 = 1.0;
        patch.gate2_source = Gate2Source::Gate1;
        wire(
            &mut routing,
            CvDestination::InverterInput,
            CvSource::ModOsc,
            1.0,
        );
        patch.sequence_levels = [0.25; 5];
        for destination in [CvDestination::Gate1, CvDestination::Gate2] {
            wire(&mut routing, destination, CvSource::Sequencer, -1.0);
        }
        wire(&mut routing, CvDestination::Gate1, CvSource::ModOsc, 1.0);
        wire(&mut routing, CvDestination::Gate2, CvSource::Inverter, 1.0);

        let mut v = voice(&routing);
        v.refresh_parameter_owned_control_values(&patch, &routing);
        let reaches_at = |value| {
            Voice::gate_drives_reach_audio(
                &patch,
                patch.gate1_level
                    + v.sum_with_source_value(
                        &routing,
                        CvDestination::Gate1,
                        CvSource::ModOsc,
                        value,
                    ),
                patch.gate2_level
                    + v.sum_with_source_value(
                        &routing,
                        CvDestination::Gate2,
                        CvSource::ModOsc,
                        value,
                    ),
            )
        };
        assert!(!reaches_at(0.0));
        assert!(reaches_at(0.5));
        assert!(!reaches_at(1.0));

        let mut peak = 0.0f32;
        for _ in 0..10_000 {
            peak = peak.max(v.process(&patch, &routing).abs());
        }
        assert!(
            peak > 1e-6,
            "the serial gates never opened in their interior overlap"
        );
        assert_eq!(v.activity(), Activity::Live);
    }
    #[test]
    fn every_modulation_mode_and_gate_2_input_stays_finite() {
        for mode in [ModulationMode::Am, ModulationMode::Fm] {
            for source in [Gate2Source::ModOsc, Gate2Source::Gate1] {
                let mut p = Params::default();
                p.modulation_mode = mode;
                p.gate2_source = source;
                p.gate2_level = 1.0;
                p.mix2 = 1.0;
                p.modulation_index = 1.0;
                let routing = Routing::init();
                let mut v = voice(&routing);
                v.note_on(id(64));
                for _ in 0..512 {
                    let y = v.process(&p, &routing);
                    assert!(y.is_finite(), "{mode:?}/{source:?}");
                }
            }
        }
    }
    #[test]
    fn routed_cv_extends_the_modulation_oscillator_beyond_its_panel_range() {
        let mut p = Params::default();
        p.mod_hz = 50.0;
        p.gate2_source = Gate2Source::ModOsc;
        p.gate2_level = 1.0;
        p.mix2 = 1.0;
        let mut routing = Routing::init();
        wire(&mut routing, CvDestination::ModPitch, CvSource::Key, 1.0);
        let mut v = voice(&routing);
        // Three octaves above middle C: the standard Key is 0.6, eight octaves per unit lifts the
        // 50 Hz panel ceiling by 4.8 of them.
        v.note_on(id(96));
        let mut crossings = 0;
        let mut previous = v.modulation_cv();
        for _ in 0..(FS * 0.05) as usize {
            v.process(&p, &routing);
            let current = v.modulation_cv();
            crossings += usize::from(previous < 0.5 && current >= 0.5);
            previous = current;
        }
        assert!(
            crossings > 20,
            "routed extension remained low-rate: {crossings}"
        );
    }
    #[test]
    fn the_high_range_runs_the_modulation_oscillator_at_the_ratio_times_its_panel_rate() {
        fn cycles_in_one_second(high: bool) -> usize {
            let mut p = Params::default();
            p.mod_hz = 5.0;
            p.mod_high = high;
            p.gate2_source = Gate2Source::ModOsc;
            p.gate2_level = 1.0;
            p.mix2 = 1.0;
            let routing = Routing::init();
            let mut v = voice(&routing);
            let mut crossings = 0;
            let mut previous = v.modulation_cv();
            for _ in 0..FS as usize {
                v.process(&p, &routing);
                let current = v.modulation_cv();
                crossings += usize::from(previous < 0.5 && current >= 0.5);
                previous = current;
            }
            crossings
        }
        let low = cycles_in_one_second(false);
        let high = cycles_in_one_second(true);
        assert!(low.abs_diff(5) <= 1, "low range ran {low} cycles, not 5");
        let expected = (5.0 * MOD_HIGH_RATIO) as usize;
        assert!(
            high.abs_diff(expected) <= 1,
            "high range ran {high} cycles, not {expected}"
        );
    }
    #[test]
    fn one_sample_pulses_latch_until_the_shell_publishes_them() {
        let p = Params::default();
        let mut routing = Routing::init();
        pulse(
            &mut routing,
            PulseDestination::Sequencer,
            PulseSource::Keyboard,
        );
        let mut v = voice(&routing);
        v.note_on(id(60));
        v.process(&p, &routing);
        for _ in 0..256 {
            v.process(&p, &routing);
        }
        let (stage, sources) = v.take_pulse_telemetry();
        assert!(sources[PulseSource::Keyboard.index()]);
        assert!(sources[PulseSource::SequenceStage.index()]);
        assert!(stage);
        assert_eq!(
            v.take_pulse_telemetry(),
            (false, [false; PULSE_SOURCES]),
            "a published pulse must not stick"
        );
    }

    #[test]
    fn a_new_stage_pulse_reaches_the_envelope_in_the_same_sample() {
        let p = Params::default();
        // The keyboard pulse drives the sequencer only; the envelope is reached through the stage.
        let mut routing = Routing::new();
        pulse(
            &mut routing,
            PulseDestination::Sequencer,
            PulseSource::Keyboard,
        );
        pulse(
            &mut routing,
            PulseDestination::Envelope,
            PulseSource::SequenceStage,
        );
        let mut v = voice(&routing);
        v.note_on(id(60));
        v.process(&p, &routing);
        assert_eq!(v.stage(), 1);
        assert!(v.stage_pulsed());
        assert!(
            v.envelope_level() > 0.,
            "the forward stage pulse was delayed"
        );
    }
    #[test]
    fn pressure_is_continuous_and_velocity_is_not_in_the_dsp_contract() {
        let p = Params::default();
        let mut routing = Routing::new();
        wire(&mut routing, CvDestination::Gate1, CvSource::Pressure, 1.);
        let mut v = voice(&routing);
        v.note_on(id(60));
        render(&mut v, &p, &routing, 0.1);
        let a = v.gate_levels()[0];
        assert!(v.set_pressure(None, 0, 60, 1.));
        render(&mut v, &p, &routing, 0.1);
        assert!(v.gate_levels()[0] > a + 0.2);
    }
    #[test]
    fn choke_is_immediate_but_falls_back_to_another_held_touch() {
        let p = Params::default();
        let routing = Routing::init();
        let mut v = voice(&routing);
        v.note_on(id(48));
        v.note_on(id(60));
        v.choke(None, 0, 60);
        assert_eq!(v.owner(), id(48));
        render(&mut v, &p, &routing, 0.02);
        v.choke(None, 0, 48);
        assert_eq!(v.process(&p, &routing), 0.0);
        assert_eq!(v.gate_levels(), [0.0; 2]);
    }
    #[test]
    fn per_note_tuning_holds_through_release_and_a_new_press_clears_it() {
        let mut v = Voice::new();
        v.note_on(id(60));
        assert!(v.set_tuning(None, 0, 60, 3.25));
        assert_eq!(v.owner_tuning(), 3.25);
        v.note_off(None, 0, 60);
        assert_eq!(
            v.owner_tuning(),
            3.25,
            "release must not snap a tuned tail back to the key centre"
        );
        v.note_on(id(64));
        assert_eq!(v.owner_tuning(), 0.0, "a new press starts untuned");
    }
    #[test]
    fn last_touch_priority_and_linear_portamento_are_deterministic() {
        let mut p = Params::default();
        p.portamento_s = 1.;
        let routing = Routing::init();
        let mut v = voice(&routing);
        v.note_on(id(48));
        v.process(&p, &routing);
        v.note_on(id(72));
        for _ in 0..4800 {
            v.process(&p, &routing);
        }
        assert!(v.key > 61. && v.key < 62.);
        v.note_off(None, 0, 72);
        for _ in 0..4800 {
            v.process(&p, &routing);
        }
        assert!(v.key < 61.);
    }
    #[test]
    fn master_zero_is_immediately_inert_and_exactly_silent() {
        let mut p = Params::default();
        p.reverb = 1.;
        let routing = Routing::init();
        let mut v = voice(&routing);
        v.note_on(id(60));
        render(&mut v, &p, &routing, 0.2);
        p.master = 0.;
        assert_eq!(v.process(&p, &routing), 0.);
        assert_eq!(v.activity(), Activity::Inert);
        p.master = 1.;
        assert_ne!(
            v.process(&p, &routing),
            0.,
            "a held press wakes after the level returns"
        );
    }
    #[test]
    fn extreme_legal_sweep_is_finite_bounded_at_validator_rates() {
        for fs in [
            1_000., 1_234.57, 44_100., 48_000., 96_000., 192_000., 384_000., 768_000.,
        ] {
            let mut p = Params::default();
            p.reverb = 1.;
            p.gate1_level = 1.;
            p.gate2_level = 1.;
            p.mix2 = 1.;
            p.modulation_index = 1.;
            p.timbre = 1.;
            p.wave_mix = 1.;
            // Every pair present at full depth, so the bound is exercised rather than a sparse
            // corner of it — fifteen sources into ten destinations, the new sources included. The
            // inverter's own input pair is wired here too and cleared by `set_topology`, which is
            // the refusal doing its job rather than the test dodging it.
            let mut routing = Routing::init();
            for destination in CvDestination::ALL {
                for source in CvSource::ALL {
                    wire(&mut routing, destination, source, 1.);
                }
            }
            let mut v = voice(&routing);
            v.set_sample_rate(fs);
            v.note_on(id(127));
            for _ in 0..fs.min(20_000.) as usize {
                let y = v.process(&p, &routing);
                assert!(y.is_finite() && y.abs() <= OUTPUT_BOUND, "{fs}: {y}");
            }
        }
    }
    #[test]
    fn fresh_instances_render_bit_identically() {
        let p = Params::default();
        let routing = Routing::init();
        let mut a = voice(&routing);
        let mut b = voice(&routing);
        a.note_on(id(64));
        b.note_on(id(64));
        for _ in 0..20_000 {
            assert_eq!(a.process(&p, &routing), b.process(&p, &routing));
        }
    }
}
