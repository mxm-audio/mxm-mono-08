//! What mxm-mono-08 can route, and with what.
//!
//! `plans/plan-mxm-mono-08-modulation.md` §2–§5, under `plans/plan-modulation-routing.md` §9 M5. The
//! shared machinery is [`mxm_modulation`]; this module is the instrument's own declaration — its
//! **source list**, its **destination list**, the **pulse law**, and which routes the init patch
//! holds. Sources and destinations are finite lists, not a generic patchbay.
//!
//! # The frame's unit is one, and the instrument owns its own clamps
//!
//! [`SourceFrame`] bounds everything it holds to unit magnitude, which is what stops a cycle
//! growing. It does **not** impose this instrument's unipolar convention: the eleven original
//! sources are `0…1` control voltages and clamp themselves **at their write sites**, where that
//! statement about the 208's voltages belongs (governing plan §4.1). Bend is signed, and the complex
//! oscillator's audio is signed and already fits the unit bound exactly — `easel_measure` sweeps
//! every endpoint, pitch, mix, timbre and modulation setting a player can reach and finds a peak of
//! exactly 1.0 with nothing above it, so that source is published as it stands, with no scale.
//!
//! # Evaluation order
//!
//! Unchanged, and still the crate's AGENTS.md policy: a destination reads whatever the frame holds
//! at its point in the fixed order, so a source produced earlier in the sample is read forward and
//! one produced later is read from the preceding sample. The complex oscillator's audio is the
//! extreme case — it is produced last, so **every** route from it is backward by one sample,
//! including into the complex pitch and timbre that make it.
//!
//! # Presence, not depth
//!
//! An absent route contributes nothing whatever its amount holds, and presence changes only on a
//! parameter event, so [`Graph::set_topology`] compacts once per processing interval and the
//! per-sample sum runs over the live list. Pulses have no depth at all: they are presence and an OR
//! (D1, the one declared exception to decision 1.6).
//!
//! # The inverter is a destination, not a selector
//!
//! D2. What the affine inverter reads is [`CvDestination::InverterInput`], summed like any other
//! destination and then complemented ([`Graph::inverter`]) rather than scaled. It replaces an
//! eleven-way source selector, and a single route present at `+1.0` reproduces any setting that
//! selector could hold, bit for bit.
//!
//! It is the one destination that does not offer every source: [`INVERTER_IS_NOT_ITS_OWN_SOURCE`]
//! is refused, so it carries fourteen routes where the others carry fifteen.
//!
//! # The collection's standard
//!
//! Key, Velocity, Wheel, Pressure and Bend mean what they mean on every instrument, and a route the
//! machine never had reaches what it reaches on every instrument ([`mxm_modulation::standard`];
//! `plans/plan-modulation-standard.md`). **Key is `(glided note − 60) / 60`**
//! ([`KEY_UNIT_SEMITONES`]), bipolar about middle C — it was `note / 127`, a unipolar voltage whose
//! route into complex pitch tracked 0.47 octave per octave; five octaves of complex pitch per unit
//! now track exactly one. The machine's own pairs — the eleven original sources, and the complex
//! oscillator's audio, a generator whose depth is this network's FM — keep each destination's
//! network reach; a performance source the 208 did not have (Velocity, the wheel, the lever) takes
//! the standard reach, which differs only at the two pitch destinations: twelve semitones rather
//! than five and eight octaves, through `mxm_modulation::sum_split` so the network's instruction
//! sequence is kept to the bit while no such pair is live. **Amplitude** is a new destination, the
//! standard factor on the mix before the spring. Clock period, glide speed and the inverter's input
//! discard a negative sum, so they offer a one-signed source only its live half, and the LPG levels
//! are the machine's CV amplifiers ([`offer`]).

use mxm_modulation::standard::{self, Law, Offer, Performance, Sign, reach as standard_reach};
use mxm_modulation::{Compacted, SourceFrame};

use crate::voice::{CV_OCTAVES, MOD_CV_OCTAVES};

/// How many sources the instrument declares.
pub const CV_SOURCES: usize = 15;
/// How many CV destinations it declares.
pub const CV_DESTINATIONS: usize = 11;
/// How many pulse sources it declares.
pub const PULSE_SOURCES: usize = 3;
/// How many pulse destinations it declares.
pub const PULSE_DESTINATIONS: usize = 4;

/// Every source a route can read.
///
/// **Index is identity** within a build: the eleven original sources come first and the four the
/// conversion added follow. Nothing persisted keys on the number — every route's permanent id is a
/// string — which is what let the envelope detector leave from the middle of the list when the
/// external input was dropped (the owner's ruling, 2026-09-23). Evaluation order is declared
/// separately, in the voice, and does not follow this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum CvSource {
    /// The keyboard voltage after glide — the collection's standard Key, `(note − 60) / 60`,
    /// bipolar about middle C. The frame's unit bound holds it at one above C9.
    Key,
    Pressure,
    ModOsc,
    Envelope,
    Pulser,
    Sequencer,
    Random1,
    Random2,
    Random3,
    Random4,
    Inverter,
    /// The last press's note-on velocity, held through release, published as the standard's
    /// `v − 1`: zero at the hardest note. New with the conversion.
    Velocity,
    /// CC 1 on the sounding press's channel. It writes no parameter, which is what the control
    /// map's CC 1 reservation allows.
    Wheel,
    /// The bender's normalised position, **signed** — not its reach in semitones. The hard-wired
    /// bend into complex pitch stays beside it.
    Bend,
    /// The complex oscillator's audio, **signed**, and the instrument's FM source. Produced last in
    /// the sample, so every route from it is one sample late.
    ComplexAudio,
}

impl CvSource {
    pub const ALL: [Self; CV_SOURCES] = [
        Self::Key,
        Self::Pressure,
        Self::ModOsc,
        Self::Envelope,
        Self::Pulser,
        Self::Sequencer,
        Self::Random1,
        Self::Random2,
        Self::Random3,
        Self::Random4,
        Self::Inverter,
        Self::Velocity,
        Self::Wheel,
        Self::Bend,
        Self::ComplexAudio,
    ];
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Their names, in source order, for the interface and for accessibility.
pub const SOURCE_NAMES: [&str; CV_SOURCES] = [
    "Key",
    "Pressure",
    "Mod oscillator",
    "Envelope",
    "Clock ramp",
    "Sequencer",
    "Random 1",
    "Random 2",
    "Random 3",
    "Random 4",
    "Inverter",
    "Velocity",
    "Wheel",
    "Bend",
    "Complex oscillator",
];

/// Which sources are **unipolar control voltages**, clamped to `0…1` where they are written.
///
/// The original sources are, **except Key**, which is the standard's bipolar keyboard voltage; the
/// four the conversion added, from `Velocity` on, are not: velocity and the wheel are held to their
/// standard ranges by their publishers, while bend and the complex oscillator's audio are signed and
/// would be destroyed by a clamp.
#[must_use]
pub const fn is_unipolar_cv(source: CvSource) -> bool {
    !matches!(source, CvSource::Key) && source.index() < CvSource::Velocity.index()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum CvDestination {
    ComplexPitch,
    ModPitch,
    Timbre,
    ModIndex,
    Gate1,
    Gate2,
    PulserPeriod,
    PortamentoSpeed,
    SequenceLength,
    /// **What the affine inverter reads** (plan D2). Its sum is complemented rather than scaled,
    /// and it is the one destination the inverter itself cannot reach — see
    /// [`INVERTER_IS_NOT_ITS_OWN_SOURCE`].
    InverterInput,
    /// **The collection's standard Amplitude** (`standard::amplitude_factor`), on the mix before
    /// the spring. New with the modulation standard; nothing routes to it at Init.
    Amplitude,
}

impl CvDestination {
    pub const ALL: [Self; CV_DESTINATIONS] = [
        Self::ComplexPitch,
        Self::ModPitch,
        Self::Timbre,
        Self::ModIndex,
        Self::Gate1,
        Self::Gate2,
        Self::PulserPeriod,
        Self::PortamentoSpeed,
        Self::SequenceLength,
        Self::InverterInput,
        Self::Amplitude,
    ];
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Their names, in destination order: each is the name of the parameter it moves, so a route
/// reads *LPG 1 level from Envelope* and the knob it adds to is *LPG 1 level*. What the host's
/// automation list, the tooltips and the accessibility tree read.
pub const DESTINATION_NAMES: [&str; CV_DESTINATIONS] = [
    "Complex pitch",
    "Mod pitch",
    "Complex timbre",
    "Mod depth",
    "LPG 1 level",
    "LPG 2 level",
    "Clock period",
    "Glide speed",
    "Sequence length",
    "Inverter input",
    "Amplitude",
];

/// Their **painted** names: the same destinations with the module prefix dropped, because each is
/// drawn only on its own module's card — design system §7.1, and `mxm-mono-00`'s
/// `TARGET_PANEL_NAMES`. *Level* under a card titled *LPG 1* is unambiguous there, so
/// the two gates and the two oscillators may repeat one.
///
/// **It is also what keeps a knob and its own stack apart.** The knob beside a stack is the
/// parameter the stack moves and carries the full name — *Complex timbre* over *Timbre* — so no
/// card paints one label twice. *Glide speed* keeps its word because the Voice card is not a Glide
/// card, and *Inverter input* keeps its because its card holds three modules.
pub const DESTINATION_PANEL_NAMES: [&str; CV_DESTINATIONS] = [
    "Pitch",
    "Pitch",
    "Timbre",
    "Depth",
    "Level",
    "Level",
    "Period",
    "Glide speed",
    "Length",
    "Inverter input",
    "Amplitude",
];

/// The bound on every destination's sum, in frame units.
///
/// **The caller's, not the shared crate's** — four units of control voltage, which is what this
/// network summed into before it had routing, and each destination applies its own scale after.
pub const SUM_BOUND: f32 = 4.0;

/// What **one unit of summed CV delivers** at each destination, in that destination's own unit —
/// for the machine's pairs; a performance source the machine did not have reaches
/// [`ADDED_PITCH_SEMITONES`] at the two pitch destinations instead ([`reach`]).
///
/// Read straight out of the per-sample path, where each law is applied after the sum: complex and
/// modulation pitch exponentiate `cv × octaves`, read here in semitones; timbre, the modulation index
/// and both gate levels add the sum to a `0…1` control; the pulser's period is multiplied by
/// `1 − 0.95·cv`, so a unit of CV takes 95 % of it away; the portamento step is multiplied by
/// `1 + 4·cv`; the sequence length adds `3·cv` stages before rounding; and Amplitude is the
/// standard factor, a unit of it the whole ±100 %.
///
/// **This is the reading, not the evaluation** — nothing in the voice multiplies by this table. It
/// exists so a route's amount reads as what its pair actually moves, which is the one defect a
/// player meets on the first knob they turn and no audio assertion can see.
pub const DESTINATION_REACH: [f32; CV_DESTINATIONS] = [
    CV_OCTAVES * 12.0,
    MOD_CV_OCTAVES * 12.0,
    100.0,
    100.0,
    100.0,
    100.0,
    -95.0,
    4.0,
    3.0,
    // The inverter's input is a `0…1` control like timbre and the two gate levels, so one unit of
    // CV is its whole range. The complement is applied to the sum, after it, and does not belong
    // in a reading of what one route delivers.
    100.0,
    100.0 * standard_reach::AMPLITUDE,
];

/// The unit each reading is quoted in, in destination order.
pub const DESTINATION_UNIT: [&str; CV_DESTINATIONS] = [
    " st",
    " st",
    " %",
    " %",
    " %",
    " %",
    " % period",
    " x speed",
    " steps",
    " %",
    " %",
];

/// How many decimals each destination's reading carries.
pub const DESTINATION_PLACES: [usize; CV_DESTINATIONS] = [2, 2, 0, 0, 0, 0, 0, 2, 2, 0, 0];

/// Semitones per unit of Key: the standard's middle-C-centred keyboard over sixty semitones, so the
/// network's five octaves of complex pitch per unit track one octave per octave.
pub const KEY_UNIT_SEMITONES: f32 = 60.0;

/// Which sources are the collection's performance sources. Random 1–4 keep this machine's own
/// meaning — four held voltages drawn on a trigger, not one per note — and are not.
pub const PERFORMANCE: [Option<Performance>; CV_SOURCES] = {
    let mut table = [None; CV_SOURCES];
    table[CvSource::Key.index()] = Some(Performance::Key);
    table[CvSource::Pressure.index()] = Some(Performance::Pressure);
    table[CvSource::Velocity.index()] = Some(Performance::Velocity);
    table[CvSource::Wheel.index()] = Some(Performance::Wheel);
    table[CvSource::Bend.index()] = Some(Performance::Bend);
    table
};

/// What each destination does with its sum, for the standard's offer. The LPG levels are the
/// machine's CV amplifiers; the clock period and glide speed read `max(Σ, 0)` and the inverter's
/// input clamps to `0…1`, so all three throw a negative sum away.
pub const LAW: [Law; CV_DESTINATIONS] = [
    Law::Sum,
    Law::Sum,
    Law::Sum,
    Law::Sum,
    Law::MachineAmplifier,
    Law::MachineAmplifier,
    Law::OneSided(Sign::Negative),
    Law::OneSided(Sign::Negative),
    Law::Sum,
    Law::OneSided(Sign::Negative),
    Law::Factor,
];

/// Whether a pair is one **the 208 had**: an original source into an original destination. The
/// conversion's four sources and the Amplitude destination are not.
#[must_use]
pub const fn machine(destination: usize, source: usize) -> bool {
    destination != CvDestination::Amplitude.index() && source < CvSource::Velocity.index()
}

/// Whether a pair takes **the standard reach** rather than the destination's network reach: a
/// performance source the machine did not have. The complex oscillator's audio, the other source
/// the conversion added, is a generator — its depth is this network's FM, and *Feedback sheen* is
/// built on two and a half octaves of it — so it keeps the network's reach.
#[must_use]
pub const fn takes_standard_reach(destination: usize, source: usize) -> bool {
    PERFORMANCE[source].is_some() && !machine(destination, source)
}

/// Whether a pair is offered, and on which half: the standard's criterion
/// ([`standard::offer`]) — except the one pair this instrument refuses on its own account,
/// [`INVERTER_IS_NOT_ITS_OWN_SOURCE`].
#[must_use]
pub const fn offer(destination: usize, source: usize) -> Offer {
    if destination == INVERTER_IS_NOT_ITS_OWN_SOURCE.0 && source == INVERTER_IS_NOT_ITS_OWN_SOURCE.1
    {
        return Offer::Refused;
    }
    standard::offer(
        LAW[destination],
        PERFORMANCE[source],
        machine(destination, source),
    )
}

/// What a performance source the machine did not have reaches at either pitch destination, per
/// unit: the standard's twelve semitones.
pub const ADDED_PITCH_SEMITONES: f32 = standard_reach::PITCH_SEMITONES;

/// The added half of a pitch destination's split sum, in octaves per unit.
const ADDED_PITCH_OCTAVES: [f32; CV_SOURCES] = [ADDED_PITCH_SEMITONES / 12.0; CV_SOURCES];

/// A pitch destination's network scale, in octaves per unit — `None` for every other destination.
#[must_use]
pub const fn network_octaves(destination: usize) -> Option<f32> {
    if destination == CvDestination::ComplexPitch.index() {
        Some(CV_OCTAVES)
    } else if destination == CvDestination::ModPitch.index() {
        Some(MOD_CV_OCTAVES)
    } else {
        None
    }
}

/// **What each source actually reaches, in frame units** — for reading an amount, never for
/// evaluating one.
///
/// Every source fills the unit here, which is worth stating rather than leaving implicit: the eleven
/// control voltages are clamped to `0…1` at their write sites and reach one at the top; the bender
/// reaches one either way; velocity and the wheel reach one; and the complex oscillator's audio
/// peaks at exactly one, measured across the keyboard by `easel_measure`.
pub const SOURCE_PEAK: [f32; CV_SOURCES] = [1.0; CV_SOURCES];

/// What a route delivers **at this amount with its source at one unit**, in the destination's unit:
/// the network's reach, or the standard's at a pitch destination for a pair that takes it.
#[inline]
#[must_use]
pub fn reach(destination: usize, source: usize, amount: f32) -> f32 {
    let per_unit =
        if network_octaves(destination).is_some() && takes_standard_reach(destination, source) {
            ADDED_PITCH_SEMITONES
        } else {
            DESTINATION_REACH[destination]
        };
    amount * per_unit * SOURCE_PEAK[source]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum PulseSource {
    Keyboard,
    PulserEnd,
    SequenceStage,
}
impl PulseSource {
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum PulseDestination {
    Envelope,
    Pulser,
    Sequencer,
    Random,
}
impl PulseDestination {
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// The one pair this instrument refuses: **the inverter cannot be its own input.**
///
/// Before D2 that was structural — the input was an eleven-way selector that simply did not offer
/// the inverter. A destination offers every source by construction, so the refusal has to be stated
/// somewhere, and it is stated twice on purpose: `plugins/mxm-mono-08/src/routes.rs` mints no
/// parameters for the pair, so a player cannot reach it, and [`Graph::set_topology`] clears it, so
/// no caller can wire it by accident either.
///
/// Routed into itself the inverter would read its own previous sample through `complement`, which
/// is a one-sample feedback oscillator sitting inside a control source — a sound the instrument has
/// never made and that nothing else in the network could damp.
pub const INVERTER_IS_NOT_ITS_OWN_SOURCE: (usize, usize) = (
    CvDestination::InverterInput.index(),
    CvSource::Inverter.index(),
);

/// `full_scale - input`, not a sign inversion (`research:modulation/buchla-208-control-sources.md` §6).
#[inline]
#[must_use]
pub fn complement(x: f32) -> f32 {
    1.0 - x.clamp(0.0, 1.0)
}

/// Which routes exist, and how deep each one is.
///
/// **It travels beside the patch, never inside it.** `voice::Params` is rebuilt every sample, and a
/// 165-float grid carried there is a memcpy per sample for values that change only on a parameter
/// event (mxm-kit's `docs/code-review-notes.md` §7).
#[derive(Debug, Clone, Copy)]
pub struct Routing {
    /// Per destination, per source: whether that route exists.
    pub present: [[bool; CV_SOURCES]; CV_DESTINATIONS],
    /// Per destination, per source: how much, signed. Only a live route's amount is read.
    pub amounts: [[f32; CV_SOURCES]; CV_DESTINATIONS],
    /// The live pairs, `(destination, source)`, built once per interval by [`Routing::compact`].
    ///
    /// **The whole point of the conversion is here.** A per-sample walk over every grid position
    /// testing presence costs one branch per position to advance one smoother — 144 of them when
    /// that was measured at B2, 165 now that D2 and the standard Amplitude added a tenth and an
    /// eleventh destination and the envelope detector left with the external input — which is
    /// worse than the
    /// dense grid it replaced; walking this list costs what the patch actually holds.
    live: [(u8, u8); CV_DESTINATIONS * CV_SOURCES],
    live_len: usize,
    /// Per pulse destination, per pulse source: whether that pulse route exists. **No depth** — a
    /// pulse is a one-sample event with no level to scale, and the keyboard pulse route also holds
    /// the envelope's gate while a key is down, where an amount would mean nothing (D1).
    pub pulses: [[bool; PULSE_SOURCES]; PULSE_DESTINATIONS],
}

impl Default for Routing {
    fn default() -> Self {
        Self::new()
    }
}

impl Routing {
    /// Nothing routed anywhere.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            present: [[false; CV_SOURCES]; CV_DESTINATIONS],
            amounts: [[0.0; CV_SOURCES]; CV_DESTINATIONS],
            live: [(0, 0); CV_DESTINATIONS * CV_SOURCES],
            live_len: 0,
            pulses: [[false; PULSE_SOURCES]; PULSE_DESTINATIONS],
        }
    }

    /// Rebuilds the live list from [`Routing::present`]. **Once per interval, never per sample**,
    /// and every caller that changes a presence owes it before the next render.
    pub fn compact(&mut self) {
        self.live_len = 0;
        for (destination, row) in self.present.iter().enumerate() {
            for (source, &on) in row.iter().enumerate() {
                if on {
                    self.live[self.live_len] = (destination as u8, source as u8);
                    self.live_len += 1;
                }
            }
        }
    }

    /// The live pairs, `(destination, source)`, in declared order.
    #[inline]
    #[must_use]
    pub fn live(&self) -> &[(u8, u8)] {
        &self.live[..self.live_len]
    }

    /// The init patch: **exactly today's two non-default routes and nothing else.**
    ///
    /// Gate 1 from the envelope, present at the owner-approved `0.72`, and the envelope from the
    /// keyboard pulse. Every other pair is absent, every new source is absent, and a fresh instance
    /// renders what it rendered before the conversion.
    #[must_use]
    pub const fn init() -> Self {
        let mut routing = Self::new();
        routing.present[CvDestination::Gate1.index()][CvSource::Envelope.index()] = true;
        routing.amounts[CvDestination::Gate1.index()][CvSource::Envelope.index()] = 0.72;
        // **The retired selector's default, as a route** (D2, §6). `inverterinput` shipped set to
        // Random 1, so that is what the inverter read on a fresh instance; leaving this pair absent
        // would make the inverter a constant `complement(0.0)` and silently change every patch that
        // routes it. It is a translation of existing wiring, not a route added to Init.
        routing.present[CvDestination::InverterInput.index()][CvSource::Random1.index()] = true;
        routing.amounts[CvDestination::InverterInput.index()][CvSource::Random1.index()] = 1.0;
        routing.pulses[PulseDestination::Envelope.index()][PulseSource::Keyboard.index()] = true;
        // The live pairs, compacted here so a caller that never calls `compact` still renders them,
        // in the ascending order `compact` itself produces.
        routing.live[0] = (
            CvDestination::Gate1.index() as u8,
            CvSource::Envelope.index() as u8,
        );
        routing.live[1] = (
            CvDestination::InverterInput.index() as u8,
            CvSource::Random1.index() as u8,
        );
        routing.live_len = 2;
        routing
    }

    /// Whether a pulse destination is driven by any of the pulses that fired this sample — the OR
    /// law, which lives here rather than in the shared crate because one consumer is not a reason to
    /// give that crate a boolean law.
    #[inline]
    #[must_use]
    pub fn pulsed(&self, destination: PulseDestination, fired: [bool; PULSE_SOURCES]) -> bool {
        (0..PULSE_SOURCES).any(|source| self.pulses[destination.index()][source] && fired[source])
    }
}

/// The voice's routing state: one frame, one compacted live list per destination, and each pitch
/// destination's live list split into the network's pairs and the standard's.
#[derive(Debug, Clone)]
pub struct Graph {
    frame: SourceFrame<CV_SOURCES>,
    live: [Compacted<CV_SOURCES>; CV_DESTINATIONS],
    pitch_network: [Compacted<CV_SOURCES>; 2],
    pitch_added: [Compacted<CV_SOURCES>; 2],
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

impl Graph {
    /// An empty graph.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            frame: SourceFrame::new(),
            live: [const { Compacted::new() }; CV_DESTINATIONS],
            pitch_network: [const { Compacted::new() }; 2],
            pitch_added: [const { Compacted::new() }; 2],
        }
    }

    /// Rebuilds which routes are live. **Once per processing interval, never per sample.**
    ///
    /// No source is owed a clear here, because this instrument publishes every source
    /// unconditionally (D8): the editor's live bars read all of them every sample and the generators
    /// run for the network anyway, so no slot can hold a value from a phrase that a newly added
    /// route would read. Gating publication later would owe that clear, and its test.
    pub fn set_topology(&mut self, routing: &Routing) {
        // The one pair this instrument refuses, enforced here so no caller can wire it by accident
        // (see [`INVERTER_IS_NOT_ITS_OWN_SOURCE`]).
        let mut present = routing.present;
        let (destination, source) = INVERTER_IS_NOT_ITS_OWN_SOURCE;
        present[destination][source] = false;
        for (live, present) in self.live.iter_mut().zip(present.iter()) {
            live.build(present);
        }
        for (slot, destination) in [CvDestination::ComplexPitch, CvDestination::ModPitch]
            .into_iter()
            .enumerate()
        {
            let row = &present[destination.index()];
            let split = |added: bool| {
                std::array::from_fn(|source| {
                    row[source] && takes_standard_reach(destination.index(), source) == added
                })
            };
            self.pitch_network[slot].build(&split(false));
            self.pitch_added[slot].build(&split(true));
        }
    }

    /// Opens a sample.
    #[inline]
    pub fn begin_sample(&mut self) {
        self.frame.begin_sample();
    }

    /// Publishes a source's value for this sample.
    ///
    /// A unipolar control voltage is clamped to `0…1` here, at the write site, because that is a
    /// statement about the 208's control voltages rather than about frames.
    #[inline]
    pub fn write(&mut self, source: CvSource, value: f32) {
        let value = if is_unipolar_cv(source) {
            value.clamp(0.0, 1.0)
        } else {
            value
        };
        self.frame.write(source.index(), value);
    }

    /// What the frame holds for a source: this sample's value if it has been published, otherwise
    /// the preceding sample's.
    #[inline]
    #[must_use]
    pub fn read(&self, source: CvSource) -> f32 {
        self.frame.read(source.index())
    }

    /// Every source's current reading, in source order — for telemetry, never for evaluation.
    #[must_use]
    pub fn values(&self) -> [f32; CV_SOURCES] {
        std::array::from_fn(|index| self.frame.read(index))
    }

    /// This destination's summed modulation, in frame units, bounded by [`SUM_BOUND`].
    ///
    /// **Each destination applies its own scale afterwards**, which is why this is the shared
    /// crate's unit-scale `sum` and not `sum_scaled`: two multiplies per route, in declared source
    /// order, exactly the sequence this network executed before it had routing.
    #[inline]
    #[must_use]
    pub fn sum(&self, destination: CvDestination, routing: &Routing) -> f32 {
        mxm_modulation::sum(
            &self.frame,
            &self.live[destination.index()],
            &routing.amounts[destination.index()],
            SUM_BOUND,
        )
    }

    /// A pitch destination's modulation **in octaves**: the network's pairs summed and scaled once
    /// by the destination's octaves per unit — the instruction sequence this network always ran —
    /// and a pair that takes the standard reach at its own twelve semitones
    /// (`mxm_modulation::sum_split`, the network's sum to the bit while none is live).
    ///
    /// # Panics
    ///
    /// For a destination that is not a pitch.
    #[inline]
    #[must_use]
    pub fn pitch_octaves(&self, destination: CvDestination, routing: &Routing) -> f32 {
        let (slot, octaves) = match destination {
            CvDestination::ComplexPitch => (0, CV_OCTAVES),
            CvDestination::ModPitch => (1, MOD_CV_OCTAVES),
            _ => panic!("not a pitch destination"),
        };
        mxm_modulation::sum_split(
            &self.frame,
            &self.pitch_network[slot],
            &routing.amounts[destination.index()],
            octaves,
            &self.pitch_added[slot],
            &ADDED_PITCH_OCTAVES,
            SUM_BOUND,
        )
    }

    /// **The affine inverter's value** (plan §3, D2): the sum of its live routes, complemented.
    ///
    /// [`complement`] clamps its own input, so the `0…1` clamp the law names needs no second
    /// statement here. With exactly one route present at `+1.0` this is bit for bit the
    /// `complement(read(source))` the retired `inverterinput` selector computed: the shared sum is
    /// `0.0 + x * 1.0`, and both of those operations are exact.
    #[inline]
    #[must_use]
    pub fn inverter(&self, routing: &Routing) -> f32 {
        complement(self.sum(CvDestination::InverterInput, routing))
    }

    /// The sources with a live route into this destination, in source order.
    ///
    /// For the activity predicate, which re-sums the gates with one source held at a trial value
    /// and must walk what exists rather than what the grid could hold.
    #[inline]
    #[must_use]
    pub fn live(&self, destination: CvDestination) -> &[usize] {
        self.live[destination.index()].sources()
    }

    /// Whether any route is live into that destination.
    #[inline]
    #[must_use]
    pub fn is_empty(&self, destination: CvDestination) -> bool {
        self.live[destination.index()].is_empty()
    }

    /// Clears both halves of the frame, leaving no tail between renders.
    pub fn reset(&mut self) {
        self.frame.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every pair the instrument *can* hold present — which is not every grid position, because
    /// [`INVERTER_IS_NOT_ITS_OWN_SOURCE`] is refused. Setting it here would assert against a pair
    /// `Graph::set_topology` clears, so the test would be measuring a patch the audio path cannot
    /// render.
    fn all_present() -> Routing {
        let mut routing = Routing::new();
        routing.present = [[true; CV_SOURCES]; CV_DESTINATIONS];
        let (destination, source) = INVERTER_IS_NOT_ITS_OWN_SOURCE;
        routing.present[destination][source] = false;
        routing
    }

    /// A route's name is `‹destination› from ‹source›` and the plugin finds a source's peak by its
    /// name, so two sources or two destinations sharing one would make two routes indistinguishable
    /// to a host and to a screen reader. The painted names are exempt — each is read inside its own
    /// card — but none may be empty.
    #[test]
    fn every_source_and_destination_has_its_own_name() {
        for names in [&SOURCE_NAMES[..], &DESTINATION_NAMES[..]] {
            for (i, a) in names.iter().enumerate() {
                assert!(!a.is_empty());
                for b in &names[i + 1..] {
                    assert_ne!(a, b, "two share a name");
                }
            }
        }
        for name in DESTINATION_PANEL_NAMES {
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn forward_reads_current_and_backward_reads_previous() {
        let mut graph = Graph::new();
        graph.begin_sample();
        graph.write(CvSource::Envelope, 0.25);
        graph.write(CvSource::Sequencer, 0.75);
        graph.begin_sample();
        graph.write(CvSource::Sequencer, 0.5);
        assert_eq!(graph.read(CvSource::Sequencer), 0.5);
        assert_eq!(
            graph.read(CvSource::Envelope),
            0.25,
            "a source not yet produced this sample reads the preceding one"
        );
    }

    #[test]
    fn the_inverter_is_a_unipolar_complement() {
        assert_eq!(complement(0.0), 1.0);
        assert_eq!(complement(0.1), 0.9);
        assert_eq!(complement(0.6), 0.39999998);
        assert_eq!(complement(1.0), 0.0);
    }

    /// **The one pair this instrument refuses**, refused in the engine and not only left unminted:
    /// a caller that wires it by hand still renders as though it were absent. Before D2 the
    /// selector could not name the inverter, so this was structural; a destination offers every
    /// source by construction, so now it has to be enforced.
    #[test]
    fn the_inverter_cannot_be_its_own_input() {
        let mut routing = Routing::new();
        let (destination, source) = INVERTER_IS_NOT_ITS_OWN_SOURCE;
        routing.present[destination][source] = true;
        routing.amounts[destination][source] = 1.0;
        let mut graph = Graph::new();
        graph.set_topology(&routing);
        graph.begin_sample();
        graph.write(CvSource::Inverter, 1.0);
        assert!(
            graph.live(CvDestination::InverterInput).is_empty(),
            "the refused pair reached the live list"
        );
        assert_eq!(graph.sum(CvDestination::InverterInput, &routing), 0.0);
        assert_eq!(graph.inverter(&routing), 1.0);
    }

    /// **D2's arithmetic claim, at this layer.** One route present at `+1.0` makes the inverter
    /// exactly the `complement(read(source))` the retired `inverterinput` selector computed — for
    /// every source that selector could name and that still exists, bit for bit, which is what lets
    /// the recorded settings in `plugins/mxm-mono-08/BASELINE-M0.md` survive the conversion. Its
    /// *Envelope follower* setting left with the external input.
    #[test]
    fn one_route_at_full_depth_is_exactly_the_retired_selectors_complement() {
        for chosen in CvSource::ALL {
            if chosen == CvSource::Inverter {
                continue;
            }
            let mut routing = Routing::new();
            routing.present[CvDestination::InverterInput.index()][chosen.index()] = true;
            routing.amounts[CvDestination::InverterInput.index()][chosen.index()] = 1.0;
            let mut graph = Graph::new();
            graph.set_topology(&routing);
            graph.begin_sample();
            let mut value = 0.0f32;
            for source in CvSource::ALL {
                value += 0.0137;
                graph.write(source, value.fract());
            }
            assert_eq!(
                graph.inverter(&routing),
                complement(graph.read(chosen)),
                "{} lost the selector's bits",
                SOURCE_NAMES[chosen.index()]
            );
        }
    }

    /// **The conversion's arithmetic claim, at this layer.** With every route present the compacted
    /// sum must be bit-identical to the dense one the instrument ran before, because `Compacted`
    /// keeps source order, `amount × source` is the same product as `source × amount`, and the
    /// shared sum's unit scale is an exact multiply by one.
    #[test]
    fn a_full_grid_sums_exactly_as_the_dense_grid_did() {
        let mut routing = all_present();
        let mut graph = Graph::new();
        let mut value = 0.0f32;
        for (destination, amounts) in routing.amounts.iter_mut().enumerate() {
            for (source, amount) in amounts.iter_mut().enumerate() {
                *amount = ((destination * CV_SOURCES + source) as f32 * 0.017).sin();
            }
        }
        graph.set_topology(&routing);
        graph.begin_sample();
        for source in CvSource::ALL {
            value += 0.0137;
            graph.write(source, value.fract());
        }
        for destination in CvDestination::ALL {
            let dense: f32 = CvSource::ALL
                .iter()
                // The refused pair is no part of the dense grid either: `all_present` leaves it
                // absent and `set_topology` would clear it regardless, so summing it here would be
                // comparing the engine against a patch it cannot render.
                .filter(|&&source| routing.present[destination.index()][source.index()])
                .map(|&source| {
                    graph.read(source) * routing.amounts[destination.index()][source.index()]
                })
                .sum();
            assert_eq!(
                graph.sum(destination, &routing),
                dense.clamp(-SUM_BOUND, SUM_BOUND),
                "{} lost bit-identity",
                DESTINATION_NAMES[destination.index()]
            );
        }
    }

    #[test]
    fn an_absent_route_contributes_nothing_whatever_its_amount_holds() {
        let mut routing = Routing::new();
        routing.amounts[CvDestination::Gate1.index()][CvSource::Envelope.index()] = 1.0;
        let mut graph = Graph::new();
        graph.set_topology(&routing);
        graph.begin_sample();
        graph.write(CvSource::Envelope, 1.0);
        assert_eq!(graph.sum(CvDestination::Gate1, &routing), 0.0);

        routing.present[CvDestination::Gate1.index()][CvSource::Envelope.index()] = true;
        graph.set_topology(&routing);
        assert_eq!(graph.sum(CvDestination::Gate1, &routing), 1.0);
    }

    #[test]
    fn a_unipolar_source_clamps_at_its_write_site_and_a_signed_one_does_not() {
        let mut graph = Graph::new();
        graph.begin_sample();
        graph.write(CvSource::Pressure, 1.5);
        graph.write(CvSource::Sequencer, -0.25);
        graph.write(CvSource::Key, -0.5);
        graph.write(CvSource::Bend, -0.75);
        graph.write(CvSource::ComplexAudio, -1.0);
        assert_eq!(graph.read(CvSource::Pressure), 1.0);
        assert_eq!(graph.read(CvSource::Sequencer), 0.0);
        assert_eq!(
            graph.read(CvSource::Key),
            -0.5,
            "Key is the standard's, bipolar"
        );
        assert_eq!(graph.read(CvSource::Bend), -0.75);
        assert_eq!(graph.read(CvSource::ComplexAudio), -1.0);
    }

    /// **A pitch destination's split sum is the network's sum to the bit while no pair that takes
    /// the standard reach is live** — every original source and the complex oscillator's audio at
    /// once — and a performance pair the 208 did not have adds its own twelve semitones.
    #[test]
    fn a_pitch_sum_is_the_networks_until_an_added_pair_is_live() {
        for (destination, octaves) in [
            (CvDestination::ComplexPitch, CV_OCTAVES),
            (CvDestination::ModPitch, MOD_CV_OCTAVES),
        ] {
            let mut routing = all_present();
            for source in [CvSource::Velocity, CvSource::Wheel, CvSource::Bend] {
                routing.present[destination.index()][source.index()] = false;
            }
            for (source, amount) in routing.amounts[destination.index()].iter_mut().enumerate() {
                *amount = (source as f32 * 0.37).sin();
            }
            let mut graph = Graph::new();
            graph.set_topology(&routing);
            graph.begin_sample();
            let mut value = 0.0f32;
            for source in CvSource::ALL {
                value += 0.0137;
                graph.write(source, value.fract());
            }
            assert_eq!(
                graph.pitch_octaves(destination, &routing),
                graph.sum(destination, &routing) * octaves,
                "{} lost bit-identity",
                DESTINATION_NAMES[destination.index()]
            );

            let mut alone = Routing::new();
            alone.present[destination.index()][CvSource::Wheel.index()] = true;
            alone.amounts[destination.index()][CvSource::Wheel.index()] = 1.0;
            graph.set_topology(&alone);
            graph.write(CvSource::Wheel, 1.0);
            assert_eq!(
                graph.pitch_octaves(destination, &alone),
                1.0,
                "twelve semitones"
            );
        }
    }

    /// The standard's offers, where they are not simply both halves: the LPG levels keep only
    /// Velocity's closing half, and the three destinations that discard a negative sum keep a
    /// one-signed source's live half. Every pair the 208 had is offered whole.
    #[test]
    fn the_one_sided_offers_are_the_standards() {
        use CvDestination as D;
        use CvSource as S;
        for d in CvDestination::ALL {
            for s in CvSource::ALL {
                let expected = match (d, s) {
                    (D::InverterInput, S::Inverter) => Offer::Refused,
                    (D::Gate1 | D::Gate2, S::Velocity) => Offer::PositiveOnly,
                    (D::PulserPeriod | D::PortamentoSpeed | D::InverterInput, S::Velocity) => {
                        Offer::NegativeOnly
                    }
                    (D::PulserPeriod | D::PortamentoSpeed | D::InverterInput, S::Wheel) => {
                        Offer::PositiveOnly
                    }
                    _ => Offer::Both,
                };
                assert_eq!(
                    offer(d.index(), s.index()),
                    expected,
                    "{} from {}",
                    DESTINATION_NAMES[d.index()],
                    SOURCE_NAMES[s.index()]
                );
            }
        }
    }

    #[test]
    fn init_holds_exactly_the_two_routes_the_machine_wires() {
        let routing = Routing::init();
        let mut present: Vec<(usize, usize)> = Vec::new();
        for destination in CvDestination::ALL {
            for source in CvSource::ALL {
                if routing.present[destination.index()][source.index()] {
                    present.push((destination.index(), source.index()));
                }
            }
        }
        // **Both pairs, named rather than counted.** A count alone would still pass if a route
        // moved to another destination, which is the way this could go wrong without anything
        // sounding obviously broken.
        assert_eq!(
            present,
            vec![
                (CvDestination::Gate1.index(), CvSource::Envelope.index()),
                (
                    CvDestination::InverterInput.index(),
                    CvSource::Random1.index()
                ),
            ],
            "Init wires the machine's own Gate 1 envelope and D2's translated inverter input"
        );
        assert_eq!(
            routing.amounts[CvDestination::Gate1.index()][CvSource::Envelope.index()],
            0.72
        );
        // The retired `inverterinput` selector shipped set to Random 1, so that is what the
        // inverter read on a fresh instance; absent, it would read a constant instead.
        assert_eq!(
            routing.amounts[CvDestination::InverterInput.index()][CvSource::Random1.index()],
            1.0
        );
        assert!(routing.pulsed(PulseDestination::Envelope, [true, false, false]));
        assert!(!routing.pulsed(PulseDestination::Pulser, [true, true, true]));
    }
}
