//! mxm-mono-08's routing parameters: one presence and one amount per *(destination, source)* pair.
//!
//! `plans/plan-mxm-mono-08-modulation.md` §5, under `plans/plan-modulation-routing.md` §4.3. The
//! derive needs concrete fields and this instrument's source list is its own, so the struct is
//! declared here rather than generated at runtime — the shape `mxm-mono-01`, `mxm-mono-03` and
//! `mxm-poly-06` use. What is shared is everything around these fields: [`mxm_modulation_params`]
//! reads them and [`mxm_mono_08_dsp::routing`] evaluates them.
//!
//! # Permanent ids
//!
//! One `#[nested(id_prefix = …)]` per destination, so a pair's ids are `cv_<destination>_<source>`
//! and `cv_<destination>_<source>on`. **The 108 amount ids that shipped before the conversion keep
//! their exact spelling**, which is why the source suffixes are the ones the dense grid already
//! used; the 36 amounts for the four new sources and all 144 presences are new, and D2 adds a
//! further 30 for the tenth destination. The envelope detector's twenty (`cv_*_follower` and
//! `cv_*_followeron`) were **retired** with the external input (the owner's ruling, 2026-09-23),
//! leaving 149 routes; the standard Amplitude added fifteen (`cv_amplitude_*`), so 164.
//! `the_id_table_is_what_the_derive_actually_produces` holds the written-out table to what
//! nice-plug actually emits.
//!
//! # Ten destinations are uniform and one is not
//!
//! *Inverter input* (D2) carries fourteen sources where the others carry fifteen, because the
//! inverter is not offered to itself. So [`ROUTE_IDS`] is a slice per destination rather than a
//! fixed-width table, [`InverterRoutes`] is its own `Params` struct beside [`CvRoutes`], and
//! [`Routes::ordinary`] returns only the ten that share a shape, each with its index. This is the arrangement
//! `mxm-mono-pr1` already carries for its summing module, which offers one fewer source for its
//! own reasons.
//!
//! # Pulses have no depth
//!
//! The twelve pulse routes are presence and an OR, the one declared exception to decision 1.6
//! (plan D1): a pulse is a one-sample event with no level to scale, and the keyboard pulse route
//! also holds the envelope's gate while a key is down, where an amount would mean nothing.
//!
//! # The collection's standard
//!
//! Readings are the shared `mxm_modulation_params::reading`: pitch in semitones (it read octaves),
//! the `0…1` controls, the gates, the inverter and Amplitude in percent, Key per octave; the clock,
//! glide and sequence keep their own units. **Faders are linear**, except a network pitch pair's —
//! five or eight octaves a unit, where a vibrato's cents need the square law's first tenth — and a
//! one-sided offer has only its live half (`mxm_mono_08_dsp::routing::offer`). The eleventh
//! destination, **Amplitude** (`cv_amplitude_*`, thirty new ids), is the standard factor on the
//! mix.

use mxm_modulation_params::Route;
use mxm_modulation_params::reading::{self, Fader, Reach, Unit};
use mxm_mono_08_dsp::routing::{
    CV_DESTINATIONS, CV_SOURCES, CvDestination, CvSource, DESTINATION_NAMES, KEY_UNIT_SEMITONES,
    PULSE_DESTINATIONS, PULSE_SOURCES, Routing, SOURCE_NAMES, network_octaves, offer,
    takes_standard_reach,
};
use nice_plug::prelude::*;

/// Every routing pair's two permanent ids, `(amount, presence)`, in `[destination][source]` order.
///
/// **Written out rather than derived at runtime**, because a preset's parameter list is
/// `&'static str` and because these are permanent: they belong in the source where they can be
/// read, grepped and diffed.
///
/// **A slice per destination, not a fixed-width row** — the shape `mxm-mono-pr1` carries for the
/// same reason. Ten destinations offer all fifteen sources; *Inverter input* offers fourteen,
/// because [`INVERTER_IS_NOT_ITS_OWN_SOURCE`](mxm_mono_08_dsp::routing::INVERTER_IS_NOT_ITS_OWN_SOURCE)
/// is refused and no parameter is minted for it. A rectangular table could only express that
/// refusal by minting a pair and hiding it, which would leave it automatable from a host.
pub const ROUTE_IDS: [&[(&str, &str)]; CV_DESTINATIONS] = [
    &[
        ("cv_complexpitch_key", "cv_complexpitch_keyon"),
        ("cv_complexpitch_pressure", "cv_complexpitch_pressureon"),
        ("cv_complexpitch_modosc", "cv_complexpitch_modoscon"),
        ("cv_complexpitch_envelope", "cv_complexpitch_envelopeon"),
        ("cv_complexpitch_pulser", "cv_complexpitch_pulseron"),
        ("cv_complexpitch_sequencer", "cv_complexpitch_sequenceron"),
        ("cv_complexpitch_random1", "cv_complexpitch_random1on"),
        ("cv_complexpitch_random2", "cv_complexpitch_random2on"),
        ("cv_complexpitch_random3", "cv_complexpitch_random3on"),
        ("cv_complexpitch_random4", "cv_complexpitch_random4on"),
        ("cv_complexpitch_inverter", "cv_complexpitch_inverteron"),
        ("cv_complexpitch_velocity", "cv_complexpitch_velocityon"),
        ("cv_complexpitch_wheel", "cv_complexpitch_wheelon"),
        ("cv_complexpitch_bend", "cv_complexpitch_bendon"),
        ("cv_complexpitch_complexosc", "cv_complexpitch_complexoscon"),
    ],
    &[
        ("cv_modpitch_key", "cv_modpitch_keyon"),
        ("cv_modpitch_pressure", "cv_modpitch_pressureon"),
        ("cv_modpitch_modosc", "cv_modpitch_modoscon"),
        ("cv_modpitch_envelope", "cv_modpitch_envelopeon"),
        ("cv_modpitch_pulser", "cv_modpitch_pulseron"),
        ("cv_modpitch_sequencer", "cv_modpitch_sequenceron"),
        ("cv_modpitch_random1", "cv_modpitch_random1on"),
        ("cv_modpitch_random2", "cv_modpitch_random2on"),
        ("cv_modpitch_random3", "cv_modpitch_random3on"),
        ("cv_modpitch_random4", "cv_modpitch_random4on"),
        ("cv_modpitch_inverter", "cv_modpitch_inverteron"),
        ("cv_modpitch_velocity", "cv_modpitch_velocityon"),
        ("cv_modpitch_wheel", "cv_modpitch_wheelon"),
        ("cv_modpitch_bend", "cv_modpitch_bendon"),
        ("cv_modpitch_complexosc", "cv_modpitch_complexoscon"),
    ],
    &[
        ("cv_timbre_key", "cv_timbre_keyon"),
        ("cv_timbre_pressure", "cv_timbre_pressureon"),
        ("cv_timbre_modosc", "cv_timbre_modoscon"),
        ("cv_timbre_envelope", "cv_timbre_envelopeon"),
        ("cv_timbre_pulser", "cv_timbre_pulseron"),
        ("cv_timbre_sequencer", "cv_timbre_sequenceron"),
        ("cv_timbre_random1", "cv_timbre_random1on"),
        ("cv_timbre_random2", "cv_timbre_random2on"),
        ("cv_timbre_random3", "cv_timbre_random3on"),
        ("cv_timbre_random4", "cv_timbre_random4on"),
        ("cv_timbre_inverter", "cv_timbre_inverteron"),
        ("cv_timbre_velocity", "cv_timbre_velocityon"),
        ("cv_timbre_wheel", "cv_timbre_wheelon"),
        ("cv_timbre_bend", "cv_timbre_bendon"),
        ("cv_timbre_complexosc", "cv_timbre_complexoscon"),
    ],
    &[
        ("cv_modindex_key", "cv_modindex_keyon"),
        ("cv_modindex_pressure", "cv_modindex_pressureon"),
        ("cv_modindex_modosc", "cv_modindex_modoscon"),
        ("cv_modindex_envelope", "cv_modindex_envelopeon"),
        ("cv_modindex_pulser", "cv_modindex_pulseron"),
        ("cv_modindex_sequencer", "cv_modindex_sequenceron"),
        ("cv_modindex_random1", "cv_modindex_random1on"),
        ("cv_modindex_random2", "cv_modindex_random2on"),
        ("cv_modindex_random3", "cv_modindex_random3on"),
        ("cv_modindex_random4", "cv_modindex_random4on"),
        ("cv_modindex_inverter", "cv_modindex_inverteron"),
        ("cv_modindex_velocity", "cv_modindex_velocityon"),
        ("cv_modindex_wheel", "cv_modindex_wheelon"),
        ("cv_modindex_bend", "cv_modindex_bendon"),
        ("cv_modindex_complexosc", "cv_modindex_complexoscon"),
    ],
    &[
        ("cv_gate1_key", "cv_gate1_keyon"),
        ("cv_gate1_pressure", "cv_gate1_pressureon"),
        ("cv_gate1_modosc", "cv_gate1_modoscon"),
        ("cv_gate1_envelope", "cv_gate1_envelopeon"),
        ("cv_gate1_pulser", "cv_gate1_pulseron"),
        ("cv_gate1_sequencer", "cv_gate1_sequenceron"),
        ("cv_gate1_random1", "cv_gate1_random1on"),
        ("cv_gate1_random2", "cv_gate1_random2on"),
        ("cv_gate1_random3", "cv_gate1_random3on"),
        ("cv_gate1_random4", "cv_gate1_random4on"),
        ("cv_gate1_inverter", "cv_gate1_inverteron"),
        ("cv_gate1_velocity", "cv_gate1_velocityon"),
        ("cv_gate1_wheel", "cv_gate1_wheelon"),
        ("cv_gate1_bend", "cv_gate1_bendon"),
        ("cv_gate1_complexosc", "cv_gate1_complexoscon"),
    ],
    &[
        ("cv_gate2_key", "cv_gate2_keyon"),
        ("cv_gate2_pressure", "cv_gate2_pressureon"),
        ("cv_gate2_modosc", "cv_gate2_modoscon"),
        ("cv_gate2_envelope", "cv_gate2_envelopeon"),
        ("cv_gate2_pulser", "cv_gate2_pulseron"),
        ("cv_gate2_sequencer", "cv_gate2_sequenceron"),
        ("cv_gate2_random1", "cv_gate2_random1on"),
        ("cv_gate2_random2", "cv_gate2_random2on"),
        ("cv_gate2_random3", "cv_gate2_random3on"),
        ("cv_gate2_random4", "cv_gate2_random4on"),
        ("cv_gate2_inverter", "cv_gate2_inverteron"),
        ("cv_gate2_velocity", "cv_gate2_velocityon"),
        ("cv_gate2_wheel", "cv_gate2_wheelon"),
        ("cv_gate2_bend", "cv_gate2_bendon"),
        ("cv_gate2_complexosc", "cv_gate2_complexoscon"),
    ],
    &[
        ("cv_pulserperiod_key", "cv_pulserperiod_keyon"),
        ("cv_pulserperiod_pressure", "cv_pulserperiod_pressureon"),
        ("cv_pulserperiod_modosc", "cv_pulserperiod_modoscon"),
        ("cv_pulserperiod_envelope", "cv_pulserperiod_envelopeon"),
        ("cv_pulserperiod_pulser", "cv_pulserperiod_pulseron"),
        ("cv_pulserperiod_sequencer", "cv_pulserperiod_sequenceron"),
        ("cv_pulserperiod_random1", "cv_pulserperiod_random1on"),
        ("cv_pulserperiod_random2", "cv_pulserperiod_random2on"),
        ("cv_pulserperiod_random3", "cv_pulserperiod_random3on"),
        ("cv_pulserperiod_random4", "cv_pulserperiod_random4on"),
        ("cv_pulserperiod_inverter", "cv_pulserperiod_inverteron"),
        ("cv_pulserperiod_velocity", "cv_pulserperiod_velocityon"),
        ("cv_pulserperiod_wheel", "cv_pulserperiod_wheelon"),
        ("cv_pulserperiod_bend", "cv_pulserperiod_bendon"),
        ("cv_pulserperiod_complexosc", "cv_pulserperiod_complexoscon"),
    ],
    &[
        ("cv_portamentospeed_key", "cv_portamentospeed_keyon"),
        (
            "cv_portamentospeed_pressure",
            "cv_portamentospeed_pressureon",
        ),
        ("cv_portamentospeed_modosc", "cv_portamentospeed_modoscon"),
        (
            "cv_portamentospeed_envelope",
            "cv_portamentospeed_envelopeon",
        ),
        ("cv_portamentospeed_pulser", "cv_portamentospeed_pulseron"),
        (
            "cv_portamentospeed_sequencer",
            "cv_portamentospeed_sequenceron",
        ),
        ("cv_portamentospeed_random1", "cv_portamentospeed_random1on"),
        ("cv_portamentospeed_random2", "cv_portamentospeed_random2on"),
        ("cv_portamentospeed_random3", "cv_portamentospeed_random3on"),
        ("cv_portamentospeed_random4", "cv_portamentospeed_random4on"),
        (
            "cv_portamentospeed_inverter",
            "cv_portamentospeed_inverteron",
        ),
        (
            "cv_portamentospeed_velocity",
            "cv_portamentospeed_velocityon",
        ),
        ("cv_portamentospeed_wheel", "cv_portamentospeed_wheelon"),
        ("cv_portamentospeed_bend", "cv_portamentospeed_bendon"),
        (
            "cv_portamentospeed_complexosc",
            "cv_portamentospeed_complexoscon",
        ),
    ],
    &[
        ("cv_sequencelength_key", "cv_sequencelength_keyon"),
        ("cv_sequencelength_pressure", "cv_sequencelength_pressureon"),
        ("cv_sequencelength_modosc", "cv_sequencelength_modoscon"),
        ("cv_sequencelength_envelope", "cv_sequencelength_envelopeon"),
        ("cv_sequencelength_pulser", "cv_sequencelength_pulseron"),
        (
            "cv_sequencelength_sequencer",
            "cv_sequencelength_sequenceron",
        ),
        ("cv_sequencelength_random1", "cv_sequencelength_random1on"),
        ("cv_sequencelength_random2", "cv_sequencelength_random2on"),
        ("cv_sequencelength_random3", "cv_sequencelength_random3on"),
        ("cv_sequencelength_random4", "cv_sequencelength_random4on"),
        ("cv_sequencelength_inverter", "cv_sequencelength_inverteron"),
        ("cv_sequencelength_velocity", "cv_sequencelength_velocityon"),
        ("cv_sequencelength_wheel", "cv_sequencelength_wheelon"),
        ("cv_sequencelength_bend", "cv_sequencelength_bendon"),
        (
            "cv_sequencelength_complexosc",
            "cv_sequencelength_complexoscon",
        ),
    ],
    // *Inverter input*: **fourteen, not fifteen.** The inverter is not offered to itself.
    &[
        ("cv_inverter_key", "cv_inverter_keyon"),
        ("cv_inverter_pressure", "cv_inverter_pressureon"),
        ("cv_inverter_modosc", "cv_inverter_modoscon"),
        ("cv_inverter_envelope", "cv_inverter_envelopeon"),
        ("cv_inverter_pulser", "cv_inverter_pulseron"),
        ("cv_inverter_sequencer", "cv_inverter_sequenceron"),
        ("cv_inverter_random1", "cv_inverter_random1on"),
        ("cv_inverter_random2", "cv_inverter_random2on"),
        ("cv_inverter_random3", "cv_inverter_random3on"),
        ("cv_inverter_random4", "cv_inverter_random4on"),
        ("cv_inverter_velocity", "cv_inverter_velocityon"),
        ("cv_inverter_wheel", "cv_inverter_wheelon"),
        ("cv_inverter_bend", "cv_inverter_bendon"),
        ("cv_inverter_complexosc", "cv_inverter_complexoscon"),
    ],
    &[
        ("cv_amplitude_key", "cv_amplitude_keyon"),
        ("cv_amplitude_pressure", "cv_amplitude_pressureon"),
        ("cv_amplitude_modosc", "cv_amplitude_modoscon"),
        ("cv_amplitude_envelope", "cv_amplitude_envelopeon"),
        ("cv_amplitude_pulser", "cv_amplitude_pulseron"),
        ("cv_amplitude_sequencer", "cv_amplitude_sequenceron"),
        ("cv_amplitude_random1", "cv_amplitude_random1on"),
        ("cv_amplitude_random2", "cv_amplitude_random2on"),
        ("cv_amplitude_random3", "cv_amplitude_random3on"),
        ("cv_amplitude_random4", "cv_amplitude_random4on"),
        ("cv_amplitude_inverter", "cv_amplitude_inverteron"),
        ("cv_amplitude_velocity", "cv_amplitude_velocityon"),
        ("cv_amplitude_wheel", "cv_amplitude_wheelon"),
        ("cv_amplitude_bend", "cv_amplitude_bendon"),
        ("cv_amplitude_complexosc", "cv_amplitude_complexoscon"),
    ],
];

/// The twelve pulse routes' permanent ids, in `[destination][source]` order. **Presence only.**
pub const PULSE_IDS: [[&str; PULSE_SOURCES]; PULSE_DESTINATIONS] = [
    [
        "pulse_envelope_keyboard",
        "pulse_envelope_pulserend",
        "pulse_envelope_stage",
    ],
    [
        "pulse_pulser_keyboard",
        "pulse_pulser_pulserend",
        "pulse_pulser_stage",
    ],
    [
        "pulse_sequencer_keyboard",
        "pulse_sequencer_pulserend",
        "pulse_sequencer_stage",
    ],
    [
        "pulse_random_keyboard",
        "pulse_random_pulserend",
        "pulse_random_stage",
    ],
];

/// One destination's routes: a presence and a signed amount for every source the instrument
/// declares.
///
/// **Presence is the enable and the amount is the depth**, and nothing else: no selector, because
/// a pair *is* its source; no polarity switch, because the amount is signed. Declared in source
/// order, and the derive emits them in that order, which `all_parameters` must match exactly —
/// that test compares ordered lists, not sets.
#[derive(Params)]
pub struct CvRoutes {
    #[id = "keyon"]
    pub key_on: BoolParam,
    #[id = "key"]
    pub key: FloatParam,
    #[id = "pressureon"]
    pub pressure_on: BoolParam,
    #[id = "pressure"]
    pub pressure: FloatParam,
    #[id = "modoscon"]
    pub mod_osc_on: BoolParam,
    #[id = "modosc"]
    pub mod_osc: FloatParam,
    #[id = "envelopeon"]
    pub envelope_on: BoolParam,
    #[id = "envelope"]
    pub envelope: FloatParam,
    #[id = "pulseron"]
    pub pulser_on: BoolParam,
    #[id = "pulser"]
    pub pulser: FloatParam,
    #[id = "sequenceron"]
    pub sequencer_on: BoolParam,
    #[id = "sequencer"]
    pub sequencer: FloatParam,
    #[id = "random1on"]
    pub random1_on: BoolParam,
    #[id = "random1"]
    pub random1: FloatParam,
    #[id = "random2on"]
    pub random2_on: BoolParam,
    #[id = "random2"]
    pub random2: FloatParam,
    #[id = "random3on"]
    pub random3_on: BoolParam,
    #[id = "random3"]
    pub random3: FloatParam,
    #[id = "random4on"]
    pub random4_on: BoolParam,
    #[id = "random4"]
    pub random4: FloatParam,
    #[id = "inverteron"]
    pub inverter_on: BoolParam,
    #[id = "inverter"]
    pub inverter: FloatParam,
    #[id = "velocityon"]
    pub velocity_on: BoolParam,
    #[id = "velocity"]
    pub velocity: FloatParam,
    #[id = "wheelon"]
    pub wheel_on: BoolParam,
    #[id = "wheel"]
    pub wheel: FloatParam,
    #[id = "bendon"]
    pub bend_on: BoolParam,
    #[id = "bend"]
    pub bend: FloatParam,
    #[id = "complexoscon"]
    pub complex_on: BoolParam,
    #[id = "complexosc"]
    pub complex: FloatParam,
}

/// One pulse destination's three enables.
#[derive(Params)]
pub struct PulseRoutes {
    #[id = "keyboard"]
    pub keyboard: BoolParam,
    #[id = "pulserend"]
    pub pulser_end: BoolParam,
    #[id = "stage"]
    pub stage: BoolParam,
}

/// The *Amplitude* destination's index.
pub const AMPLITUDE: usize = CvDestination::Amplitude.index();
/// The *Inverter input* destination's index, and which frame slots its routes carry.
///
/// Every source but the inverter itself, in frame order, so the inverter's slot is absent: see
/// [`INVERTER_IS_NOT_ITS_OWN_SOURCE`](mxm_mono_08_dsp::routing::INVERTER_IS_NOT_ITS_OWN_SOURCE).
pub const INVERTER: usize = CvDestination::InverterInput.index();
/// Which frame slots [`InverterRoutes`] carries, in its own declared order.
pub const INVERTER_SOURCES: [usize; CV_SOURCES - 1] = {
    let mut out = [0; CV_SOURCES - 1];
    let (mut source, mut slot) = (0, 0);
    while source < CV_SOURCES {
        if source != CvSource::Inverter.index() {
            out[slot] = source;
            slot += 1;
        }
        source += 1;
    }
    out
};

/// The amount a pair starts at. **Two routes are wired at Init**, and both are existing wiring
/// rather than sound added to the init patch:
///
/// - Gate 1 from the envelope at the owner-approved 0.72, this instrument's one recorded init
///   deviation; and
/// - *Inverter input* from Random 1 at full depth, which is the retired `inverterinput` selector's
///   own default expressed as a route (D2, plan §6). Leaving it absent would make a fresh
///   instance's inverter a constant `complement(0.0)` and silently change every patch that routes
///   the inverter.
///
/// Every other pair starts at zero and absent.
#[must_use]
pub const fn init_amount(destination: usize, source: usize) -> f32 {
    if destination == 4 && source == 3 {
        0.72
    } else if destination == INVERTER && source == 6 {
        1.0
    } else {
        0.0
    }
}

/// Whether a pair is wired at Init. See [`init_amount`] for why there are two.
#[must_use]
pub const fn init_present(destination: usize, source: usize) -> bool {
    (destination == 4 && source == 3) || (destination == INVERTER && source == 6)
}

/// A route amount, smoothed at 10 ms — **the smoothing the 108 shipped amounts already had** (§5) —
/// on its offer's fader, reading what the pair delivers.
fn amount(destination: usize, source: usize) -> FloatParam {
    reading::amount_param_at(
        format!(
            "{} from {}",
            DESTINATION_NAMES[destination], SOURCE_NAMES[source]
        ),
        init_amount(destination, source),
        reach(destination, source),
        fader(destination, source),
        10.0,
    )
}

/// Each destination's reading unit: the shared ones where the standard has one, and the clock's,
/// the glide's and the sequence's own.
const UNITS: [Unit; CV_DESTINATIONS] = [
    reading::SEMITONES,
    reading::SEMITONES,
    reading::PERCENT,
    reading::PERCENT,
    reading::PERCENT,
    reading::PERCENT,
    Unit {
        suffix: " % period",
        places: 0,
        scale: 100.0,
    },
    Unit {
        suffix: " x speed",
        places: 2,
        scale: 1.0,
    },
    Unit {
        suffix: " steps",
        places: 2,
        scale: 1.0,
    },
    reading::PERCENT,
    reading::PERCENT,
];

/// What a pair delivers at an amount of one, in its destination's unit — per octave of keyboard for
/// Key. From the DSP's own [`mxm_mono_08_dsp::routing::reach`], never a copy of its table: a
/// network pitch pair's five or eight octaves, a pair that takes the standard its twelve semitones.
pub fn reach(destination: usize, source: usize) -> Reach {
    let unit = UNITS[destination];
    let full = mxm_mono_08_dsp::routing::reach(destination, source, 1.0) / unit.scale;
    if source == CvSource::Key.index() {
        Reach::per_octave(full * 12.0 / KEY_UNIT_SEMITONES, unit)
    } else {
        Reach::new(full, unit)
    }
}

/// A pair's fader: its offer's halves, square-law only for a network pitch pair.
fn fader(destination: usize, source: usize) -> Fader {
    let square_law =
        network_octaves(destination).is_some() && !takes_standard_reach(destination, source);
    Fader::for_offer(offer(destination, source), square_law)
}

/// Whether a route exists. **Configuration, not an amount**, so its default is the machine's own
/// wiring: Gate 1 from the envelope, and nothing else.
fn present(destination: usize, source: usize) -> BoolParam {
    BoolParam::new(
        format!(
            "{} from {} on",
            DESTINATION_NAMES[destination], SOURCE_NAMES[source]
        ),
        init_present(destination, source),
    )
}

impl CvRoutes {
    /// Every pair for one destination, at the init patch.
    pub fn new(destination: usize) -> Self {
        Self {
            key_on: present(destination, CvSource::Key.index()),
            key: amount(destination, CvSource::Key.index()),
            pressure_on: present(destination, CvSource::Pressure.index()),
            pressure: amount(destination, CvSource::Pressure.index()),
            mod_osc_on: present(destination, CvSource::ModOsc.index()),
            mod_osc: amount(destination, CvSource::ModOsc.index()),
            envelope_on: present(destination, CvSource::Envelope.index()),
            envelope: amount(destination, CvSource::Envelope.index()),
            pulser_on: present(destination, CvSource::Pulser.index()),
            pulser: amount(destination, CvSource::Pulser.index()),
            sequencer_on: present(destination, CvSource::Sequencer.index()),
            sequencer: amount(destination, CvSource::Sequencer.index()),
            random1_on: present(destination, CvSource::Random1.index()),
            random1: amount(destination, CvSource::Random1.index()),
            random2_on: present(destination, CvSource::Random2.index()),
            random2: amount(destination, CvSource::Random2.index()),
            random3_on: present(destination, CvSource::Random3.index()),
            random3: amount(destination, CvSource::Random3.index()),
            random4_on: present(destination, CvSource::Random4.index()),
            random4: amount(destination, CvSource::Random4.index()),
            inverter_on: present(destination, CvSource::Inverter.index()),
            inverter: amount(destination, CvSource::Inverter.index()),
            velocity_on: present(destination, CvSource::Velocity.index()),
            velocity: amount(destination, CvSource::Velocity.index()),
            wheel_on: present(destination, CvSource::Wheel.index()),
            wheel: amount(destination, CvSource::Wheel.index()),
            bend_on: present(destination, CvSource::Bend.index()),
            bend: amount(destination, CvSource::Bend.index()),
            complex_on: present(destination, CvSource::ComplexAudio.index()),
            complex: amount(destination, CvSource::ComplexAudio.index()),
        }
    }

    /// This destination's routes in **declared source order**.
    pub fn routes(&self, destination: usize) -> [Route<'_>; CV_SOURCES] {
        let ids = ROUTE_IDS[destination];
        let pairs: [(&BoolParam, &FloatParam); CV_SOURCES] = [
            (&self.key_on, &self.key),
            (&self.pressure_on, &self.pressure),
            (&self.mod_osc_on, &self.mod_osc),
            (&self.envelope_on, &self.envelope),
            (&self.pulser_on, &self.pulser),
            (&self.sequencer_on, &self.sequencer),
            (&self.random1_on, &self.random1),
            (&self.random2_on, &self.random2),
            (&self.random3_on, &self.random3),
            (&self.random4_on, &self.random4),
            (&self.inverter_on, &self.inverter),
            (&self.velocity_on, &self.velocity),
            (&self.wheel_on, &self.wheel),
            (&self.bend_on, &self.bend),
            (&self.complex_on, &self.complex),
        ];
        std::array::from_fn(|s| Route {
            source: SOURCE_NAMES[s],
            present: pairs[s].0,
            amount: pairs[s].1,
            present_id: ids[s].1,
            amount_id: ids[s].0,
        })
    }

    /// Whether each of this destination's routes exists. Read **once per interval**, never per
    /// sample.
    pub fn presences(&self, destination: usize) -> [bool; CV_SOURCES] {
        mxm_modulation_params::presences(&self.routes(destination))
    }

    /// One source's amount parameter, by index, in declared source order — a `match` rather than
    /// an array of references, so a source nothing reads costs a branch and no pointer stores.
    #[inline]
    pub fn amount_param(&self, source: usize) -> &FloatParam {
        // Over the enum rather than the number, and without a catch-all: a source added or removed
        // is a compile error here instead of a silent map onto the wrong parameter.
        match CvSource::ALL[source] {
            CvSource::Key => &self.key,
            CvSource::Pressure => &self.pressure,
            CvSource::ModOsc => &self.mod_osc,
            CvSource::Envelope => &self.envelope,
            CvSource::Pulser => &self.pulser,
            CvSource::Sequencer => &self.sequencer,
            CvSource::Random1 => &self.random1,
            CvSource::Random2 => &self.random2,
            CvSource::Random3 => &self.random3,
            CvSource::Random4 => &self.random4,
            CvSource::Inverter => &self.inverter,
            CvSource::Velocity => &self.velocity,
            CvSource::Wheel => &self.wheel,
            CvSource::Bend => &self.bend,
            CvSource::ComplexAudio => &self.complex,
        }
    }

    /// Snaps a newly present route's smoother to its stored value.
    ///
    /// **An absent route's smoother is not advanced, so it must not be resumed either.** While the
    /// pair was absent nothing called `next()`, but the parameter stayed editable: a host
    /// automating it, or a preset load, moves the target and leaves the smoother wherever the last
    /// live sample left it, so resuming from there ramps the route in from a stale number over a
    /// span the host's buffers decide (mxm-kit's `docs/code-review-notes.md` §7).
    pub fn arm(&self, newly_present: &[bool; CV_SOURCES]) {
        for (source, &now) in newly_present.iter().enumerate() {
            if now {
                let param = self.amount_param(source);
                param.smoothed.reset(param.value());
            }
        }
    }
}

/// The *Inverter input* destination's routes: **every source except the inverter itself.**
///
/// It is the one destination that does not offer all fifteen. Routed into itself the inverter would
/// read its own previous sample through `complement` — a one-sample feedback oscillator inside a
/// control source, which the instrument has never made and nothing in the network would damp. Before
/// D2 that was structural, because the eleven-way selector it replaces simply did not list the
/// inverter; a destination offers every source by construction, so the refusal is now stated, here
/// and again in `Graph::set_topology`.
///
/// `cv_inverter_inverter` and `cv_inverter_inverteron` are therefore never minted: the pair is
/// unreachable rather than merely discouraged, and no host can automate what does not exist.
#[derive(Params)]
pub struct InverterRoutes {
    #[id = "keyon"]
    pub key_on: BoolParam,
    #[id = "key"]
    pub key: FloatParam,
    #[id = "pressureon"]
    pub pressure_on: BoolParam,
    #[id = "pressure"]
    pub pressure: FloatParam,
    #[id = "modoscon"]
    pub mod_osc_on: BoolParam,
    #[id = "modosc"]
    pub mod_osc: FloatParam,
    #[id = "envelopeon"]
    pub envelope_on: BoolParam,
    #[id = "envelope"]
    pub envelope: FloatParam,
    #[id = "pulseron"]
    pub pulser_on: BoolParam,
    #[id = "pulser"]
    pub pulser: FloatParam,
    #[id = "sequenceron"]
    pub sequencer_on: BoolParam,
    #[id = "sequencer"]
    pub sequencer: FloatParam,
    #[id = "random1on"]
    pub random1_on: BoolParam,
    #[id = "random1"]
    pub random1: FloatParam,
    #[id = "random2on"]
    pub random2_on: BoolParam,
    #[id = "random2"]
    pub random2: FloatParam,
    #[id = "random3on"]
    pub random3_on: BoolParam,
    #[id = "random3"]
    pub random3: FloatParam,
    #[id = "random4on"]
    pub random4_on: BoolParam,
    #[id = "random4"]
    pub random4: FloatParam,
    #[id = "velocityon"]
    pub velocity_on: BoolParam,
    #[id = "velocity"]
    pub velocity: FloatParam,
    #[id = "wheelon"]
    pub wheel_on: BoolParam,
    #[id = "wheel"]
    pub wheel: FloatParam,
    #[id = "bendon"]
    pub bend_on: BoolParam,
    #[id = "bend"]
    pub bend: FloatParam,
    #[id = "complexoscon"]
    pub complex_on: BoolParam,
    #[id = "complexosc"]
    pub complex: FloatParam,
}

impl InverterRoutes {
    /// Its fourteen pairs at the init patch — Random 1 present at full depth, the rest absent.
    pub fn new() -> Self {
        Self {
            key_on: present(INVERTER, CvSource::Key.index()),
            key: amount(INVERTER, CvSource::Key.index()),
            pressure_on: present(INVERTER, CvSource::Pressure.index()),
            pressure: amount(INVERTER, CvSource::Pressure.index()),
            mod_osc_on: present(INVERTER, CvSource::ModOsc.index()),
            mod_osc: amount(INVERTER, CvSource::ModOsc.index()),
            envelope_on: present(INVERTER, CvSource::Envelope.index()),
            envelope: amount(INVERTER, CvSource::Envelope.index()),
            pulser_on: present(INVERTER, CvSource::Pulser.index()),
            pulser: amount(INVERTER, CvSource::Pulser.index()),
            sequencer_on: present(INVERTER, CvSource::Sequencer.index()),
            sequencer: amount(INVERTER, CvSource::Sequencer.index()),
            random1_on: present(INVERTER, CvSource::Random1.index()),
            random1: amount(INVERTER, CvSource::Random1.index()),
            random2_on: present(INVERTER, CvSource::Random2.index()),
            random2: amount(INVERTER, CvSource::Random2.index()),
            random3_on: present(INVERTER, CvSource::Random3.index()),
            random3: amount(INVERTER, CvSource::Random3.index()),
            random4_on: present(INVERTER, CvSource::Random4.index()),
            random4: amount(INVERTER, CvSource::Random4.index()),
            velocity_on: present(INVERTER, CvSource::Velocity.index()),
            velocity: amount(INVERTER, CvSource::Velocity.index()),
            wheel_on: present(INVERTER, CvSource::Wheel.index()),
            wheel: amount(INVERTER, CvSource::Wheel.index()),
            bend_on: present(INVERTER, CvSource::Bend.index()),
            bend: amount(INVERTER, CvSource::Bend.index()),
            complex_on: present(INVERTER, CvSource::ComplexAudio.index()),
            complex: amount(INVERTER, CvSource::ComplexAudio.index()),
        }
    }

    /// The pairs it does have, in declared source order. **Fourteen, not fifteen.**
    pub fn routes(&self) -> [Route<'_>; CV_SOURCES - 1] {
        let ids = ROUTE_IDS[INVERTER];
        let pairs: [(&BoolParam, &FloatParam); CV_SOURCES - 1] = [
            (&self.key_on, &self.key),
            (&self.pressure_on, &self.pressure),
            (&self.mod_osc_on, &self.mod_osc),
            (&self.envelope_on, &self.envelope),
            (&self.pulser_on, &self.pulser),
            (&self.sequencer_on, &self.sequencer),
            (&self.random1_on, &self.random1),
            (&self.random2_on, &self.random2),
            (&self.random3_on, &self.random3),
            (&self.random4_on, &self.random4),
            (&self.velocity_on, &self.velocity),
            (&self.wheel_on, &self.wheel),
            (&self.bend_on, &self.bend),
            (&self.complex_on, &self.complex),
        ];
        std::array::from_fn(|s| Route {
            source: SOURCE_NAMES[INVERTER_SOURCES[s]],
            present: pairs[s].0,
            amount: pairs[s].1,
            present_id: ids[s].1,
            amount_id: ids[s].0,
        })
    }

    /// Its presences, widened into the frame-shaped slot the routing carries. **The inverter's own
    /// slot is always false**, which is what makes the pair structurally impossible rather than
    /// merely undrawn.
    pub fn presences(&self) -> [bool; CV_SOURCES] {
        let mut out = [false; CV_SOURCES];
        for (route, slot) in self.routes().iter().zip(INVERTER_SOURCES) {
            out[slot] = route.is_present();
        }
        out
    }

    /// One source's amount parameter, by **frame** index. `None` for the inverter, which has none.
    #[inline]
    pub(crate) fn amount_param(&self, source: usize) -> Option<&FloatParam> {
        Some(match CvSource::ALL[source] {
            CvSource::Key => &self.key,
            CvSource::Pressure => &self.pressure,
            CvSource::ModOsc => &self.mod_osc,
            CvSource::Envelope => &self.envelope,
            CvSource::Pulser => &self.pulser,
            CvSource::Sequencer => &self.sequencer,
            CvSource::Random1 => &self.random1,
            CvSource::Random2 => &self.random2,
            CvSource::Random3 => &self.random3,
            CvSource::Random4 => &self.random4,
            CvSource::Velocity => &self.velocity,
            CvSource::Wheel => &self.wheel,
            CvSource::Bend => &self.bend,
            CvSource::ComplexAudio => &self.complex,
            CvSource::Inverter => return None,
        })
    }

    /// See [`CvRoutes::arm`].
    pub fn arm(&self, newly_present: &[bool; CV_SOURCES]) {
        for (source, &now) in newly_present.iter().enumerate() {
            if now && let Some(param) = self.amount_param(source) {
                param.smoothed.reset(param.value());
            }
        }
    }
}

impl Default for InverterRoutes {
    fn default() -> Self {
        Self::new()
    }
}

impl PulseRoutes {
    /// One pulse destination at the init patch: the envelope is triggered by the keyboard pulse,
    /// and nothing else is wired.
    ///
    /// Named *"‹Module› trigger from ‹source›"* — *Random trigger from Key* — so a trigger route
    /// cannot be read as a modulation route, whose name is *"‹Destination› from ‹Source›"*.
    /// *Random from keyboard* read as a random voltage coming from the keyboard.
    ///
    /// **No `on` suffix.** A CV presence carries one to tell it apart from the amount it enables
    /// (§5); a trigger route has no amount, so the enable *is* the route.
    pub fn new(destination: usize) -> Self {
        let on = |source: usize| {
            BoolParam::new(
                format!(
                    "{} trigger from {}",
                    PULSE_DESTINATION_NAMES[destination], PULSE_SOURCE_NAMES[source]
                ),
                destination == 0 && source == 0,
            )
        };
        Self {
            keyboard: on(0),
            pulser_end: on(1),
            stage: on(2),
        }
    }

    /// Which pulse sources reach this destination.
    pub fn values(&self) -> [bool; PULSE_SOURCES] {
        [
            self.keyboard.value(),
            self.pulser_end.value(),
            self.stage.value(),
        ]
    }

    /// The three enables, in declared source order, for the preset layer.
    pub fn each(&self) -> [&BoolParam; PULSE_SOURCES] {
        [&self.keyboard, &self.pulser_end, &self.stage]
    }
}

/// The trigger destinations' names, in destination order: the module each one fires. The code
/// calls the clock the *pulser*, after the hardware.
pub const PULSE_DESTINATION_NAMES: [&str; PULSE_DESTINATIONS] =
    ["Envelope", "Clock", "Sequencer", "Random"];

/// The trigger sources' names, in source order: a key going down, the clock's ramp reaching its
/// end, and a step whose trigger is enabled becoming active.
pub const PULSE_SOURCE_NAMES: [&str; PULSE_SOURCES] = ["Key", "Clock", "Step"];

/// Every destination's routes, and every pulse destination's enables.
#[derive(Params)]
pub struct Routes {
    #[nested(id_prefix = "cv_complexpitch", group = "Modulation - Complex pitch")]
    pub complex_pitch: CvRoutes,
    #[nested(id_prefix = "cv_modpitch", group = "Modulation - Mod pitch")]
    pub mod_pitch: CvRoutes,
    #[nested(id_prefix = "cv_timbre", group = "Modulation - Complex timbre")]
    pub timbre: CvRoutes,
    #[nested(id_prefix = "cv_modindex", group = "Modulation - Mod depth")]
    pub mod_index: CvRoutes,
    #[nested(id_prefix = "cv_gate1", group = "Modulation - LPG 1 level")]
    pub gate1: CvRoutes,
    #[nested(id_prefix = "cv_gate2", group = "Modulation - LPG 2 level")]
    pub gate2: CvRoutes,
    #[nested(id_prefix = "cv_pulserperiod", group = "Modulation - Clock period")]
    pub pulser_period: CvRoutes,
    #[nested(id_prefix = "cv_portamentospeed", group = "Modulation - Glide speed")]
    pub portamento_speed: CvRoutes,
    #[nested(
        id_prefix = "cv_sequencelength",
        group = "Modulation - Sequence length"
    )]
    pub sequence_length: CvRoutes,
    /// The tenth destination, and the only one that is not a [`CvRoutes`]: it carries fourteen
    /// sources rather than fifteen (D2).
    #[nested(id_prefix = "cv_inverter", group = "Modulation - Inverter input")]
    pub inverter_input: InverterRoutes,
    /// The eleventh, the collection's standard Amplitude on the mix (the modulation standard).
    #[nested(id_prefix = "cv_amplitude", group = "Modulation - Amplitude")]
    pub amplitude: CvRoutes,

    #[nested(id_prefix = "pulse_envelope", group = "Triggers - Envelope")]
    pub envelope: PulseRoutes,
    #[nested(id_prefix = "pulse_pulser", group = "Triggers - Clock")]
    pub pulser: PulseRoutes,
    #[nested(id_prefix = "pulse_sequencer", group = "Triggers - Sequencer")]
    pub sequencer: PulseRoutes,
    #[nested(id_prefix = "pulse_random", group = "Triggers - Random")]
    pub random: PulseRoutes,
}

impl Default for Routes {
    fn default() -> Self {
        Self::new()
    }
}

impl Routes {
    /// The init patch: Gate 1 from the envelope at 0.72, the envelope from the keyboard pulse, and
    /// nothing else — exactly the two non-default routes this instrument shipped with.
    pub fn new() -> Self {
        Self {
            complex_pitch: CvRoutes::new(0),
            mod_pitch: CvRoutes::new(1),
            timbre: CvRoutes::new(2),
            mod_index: CvRoutes::new(3),
            gate1: CvRoutes::new(4),
            gate2: CvRoutes::new(5),
            pulser_period: CvRoutes::new(6),
            portamento_speed: CvRoutes::new(7),
            sequence_length: CvRoutes::new(8),
            inverter_input: InverterRoutes::new(),
            amplitude: CvRoutes::new(AMPLITUDE),
            envelope: PulseRoutes::new(0),
            pulser: PulseRoutes::new(1),
            sequencer: PulseRoutes::new(2),
            random: PulseRoutes::new(3),
        }
    }

    /// The **ten uniform** CV destinations, each **with its destination index**, in declared order.
    ///
    /// *Inverter input* is not one of them — it carries one fewer source, so it is handled beside
    /// this rather than through it, as `mxm-mono-pr1` handles its summing module — and it sits
    /// between the ninth and Amplitude, so a position here is not a destination: every entry
    /// carries its own. The name says so on purpose: a silent change to `each`'s length would have
    /// compiled anywhere the caller only iterated.
    pub fn ordinary(&self) -> [(usize, &CvRoutes); CV_DESTINATIONS - 1] {
        [
            (0, &self.complex_pitch),
            (1, &self.mod_pitch),
            (2, &self.timbre),
            (3, &self.mod_index),
            (4, &self.gate1),
            (5, &self.gate2),
            (6, &self.pulser_period),
            (7, &self.portamento_speed),
            (8, &self.sequence_length),
            (AMPLITUDE, &self.amplitude),
        ]
    }

    /// One uniform destination's routes, by destination index.
    ///
    /// # Panics
    ///
    /// For *Inverter input*, which is not a [`CvRoutes`].
    #[inline]
    pub fn cv(&self, destination: usize) -> &CvRoutes {
        // A `match`, not a search: `advance` calls this for every live route every sample.
        match CvDestination::ALL[destination] {
            CvDestination::ComplexPitch => &self.complex_pitch,
            CvDestination::ModPitch => &self.mod_pitch,
            CvDestination::Timbre => &self.timbre,
            CvDestination::ModIndex => &self.mod_index,
            CvDestination::Gate1 => &self.gate1,
            CvDestination::Gate2 => &self.gate2,
            CvDestination::PulserPeriod => &self.pulser_period,
            CvDestination::PortamentoSpeed => &self.portamento_speed,
            CvDestination::SequenceLength => &self.sequence_length,
            CvDestination::Amplitude => &self.amplitude,
            CvDestination::InverterInput => panic!("Inverter input is not a CvRoutes"),
        }
    }

    /// The four pulse destinations, in declared order.
    pub fn pulses(&self) -> [&PulseRoutes; PULSE_DESTINATIONS] {
        [&self.envelope, &self.pulser, &self.sequencer, &self.random]
    }

    /// Which routes are live, for the whole instrument. **Once per interval.**
    pub fn topology(&self) -> Routing {
        let mut routing = Routing::new();
        for (index, group) in self.ordinary() {
            routing.present[index] = group.presences(index);
        }
        routing.present[INVERTER] = self.inverter_input.presences();
        for (slot, group) in routing.pulses.iter_mut().zip(self.pulses()) {
            *slot = group.values();
        }
        // Built once here, so the per-sample path walks what the patch holds rather than testing
        // every grid position — 165 of them, of which 164 can legally be present — which
        // is the whole cost argument for the conversion.
        routing.compact();
        routing
    }

    /// The topology for this interval, with every **newly present** route's smoother snapped to
    /// its stored value. `previous` is the topology the last interval ran, which the caller keeps.
    pub fn topology_from(&self, previous: &Routing) -> Routing {
        let routing = self.topology();
        let newly = |index: usize| {
            let mut out = [false; CV_SOURCES];
            for (slot, (&now, &before)) in out.iter_mut().zip(
                routing.present[index]
                    .iter()
                    .zip(previous.present[index].iter()),
            ) {
                *slot = now && !before;
            }
            out
        };
        for (index, group) in self.ordinary() {
            group.arm(&newly(index));
        }
        self.inverter_input.arm(&newly(INVERTER));
        routing
    }

    /// Fills this sample's amounts into an already-topologised [`Routing`]: **each live route's
    /// smoother advanced once**, and an absent route's left exactly where the player put it.
    ///
    /// This is where the conversion's cost argument lives. The dense grid advanced all 108
    /// smoothers every sample whatever the patch held; this advances only what exists.
    #[inline]
    pub fn advance(&self, routing: &mut Routing) {
        for index in 0..routing.live().len() {
            let (destination, source) = routing.live()[index];
            let (destination, source) = (destination as usize, source as usize);
            // The tenth destination has no parameter for the inverter's own slot, and the engine
            // clears that pair anyway, so a `None` here is the refusal and not an error.
            let next = if destination == INVERTER {
                self.inverter_input
                    .amount_param(source)
                    .map(|p| p.smoothed.next())
            } else {
                Some(self.cv(destination).amount_param(source).smoothed.next())
            };
            if let Some(value) = next {
                routing.amounts[destination][source] = value;
            }
        }
    }

    /// Every routing parameter, named by its permanent id, **in the order the derive emits them**
    /// — presence then amount per pair, destination by destination, then the pulse enables.
    /// `all_parameters` compares ordered lists against the host's, so this order is load-bearing.
    ///
    /// **Presets carry routing**, presences included, or a sound would load with somebody else's
    /// routes still in it.
    pub fn parameters(&self) -> Vec<(&'static str, &dyn mxm_preset::ErasedParam)> {
        let mut out: Vec<(&'static str, &dyn mxm_preset::ErasedParam)> = Vec::with_capacity(
            ((CV_DESTINATIONS - 1) * CV_SOURCES + CV_SOURCES - 1) * 2
                + PULSE_DESTINATIONS * PULSE_SOURCES,
        );
        // In the derive's order: the nine, the inverter's input, then Amplitude.
        for (index, group) in self.ordinary() {
            if index == AMPLITUDE {
                for (route, &(amount, presence)) in self
                    .inverter_input
                    .routes()
                    .into_iter()
                    .zip(ROUTE_IDS[INVERTER])
                {
                    out.push((presence, route.present));
                    out.push((amount, route.amount));
                }
            }
            for (route, &(amount, presence)) in
                group.routes(index).into_iter().zip(ROUTE_IDS[index])
            {
                out.push((presence, route.present));
                out.push((amount, route.amount));
            }
        }
        for (group, ids) in self.pulses().into_iter().zip(PULSE_IDS) {
            for (param, id) in group.each().into_iter().zip(ids) {
                out.push((id, param));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use mxm_plugin_test::routing_checks;

    /// **Every route parameter says what the DSP does** — the modulation standard's plugin half:
    /// a pair has a parameter exactly where it is offered (the inverter's own pair nowhere), its
    /// travel is its offer's, its reading carries its destination's unit and states what
    /// `mxm_mono_08_dsp::conformance` measures the graph delivering, and every reading survives the
    /// host's round trip.
    ///
    /// Falsified before trusted: with the performance pitch pairs read at the network's reach, it
    /// names all six.
    #[test]
    fn every_route_parameter_says_what_the_dsp_does() {
        let routes = Routes::new();
        if let Err(failures) = routing_checks::amounts(
            &mxm_mono_08_dsp::conformance::Declared,
            |destination, source| {
                if destination == INVERTER {
                    routes.inverter_input.amount_param(source)
                } else {
                    Some(routes.cv(destination).amount_param(source))
                }
            },
        ) {
            panic!("{} failure(s):\n{}", failures.len(), failures.join("\n"));
        }
    }

    /// **A one-sided offer has only its live half**, and the design's plain amounts survive the
    /// faders turning linear: a network pitch pair keeps the square law, every other pair is linear.
    #[test]
    fn faders_follow_the_offer_and_the_network_pitch_law() {
        let routes = Routes::new();
        let velocity = CvSource::Velocity.index();
        let wheel = CvSource::Wheel.index();
        let bounds = |p: &FloatParam| (p.preview_plain(0.0), p.preview_plain(1.0));
        assert_eq!(bounds(routes.gate1.amount_param(velocity)), (0.0, 1.0));
        assert_eq!(
            bounds(routes.pulser_period.amount_param(velocity)),
            (-1.0, 0.0)
        );
        assert_eq!(
            bounds(routes.portamento_speed.amount_param(wheel)),
            (0.0, 1.0)
        );
        assert_eq!(
            bounds(routes.inverter_input.amount_param(velocity).unwrap()),
            (-1.0, 0.0)
        );
        let key = CvSource::Key.index();
        assert_eq!(
            routes.complex_pitch.amount_param(key).preview_plain(0.75),
            0.25
        );
        assert_eq!(
            routes.complex_pitch.amount_param(wheel).preview_plain(0.75),
            0.5
        );
        assert_eq!(routes.timbre.amount_param(key).preview_plain(0.75), 0.5);
    }

    /// **The table and the derive cannot be allowed to disagree.** `ROUTE_IDS` and `PULSE_IDS` are
    /// written out by hand, because presets and the editor name pairs by permanent id; the derive
    /// is what the host actually sees. This test is the only thing holding the two together — and
    /// this module's header claimed it existed for some time before it did, which D2 fixes along
    /// with the raggedness that makes it matter more.
    #[test]
    fn the_id_table_is_what_the_derive_actually_produces() {
        let routes = Routes::new();
        let derived: Vec<String> = routes
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        let tabled: Vec<String> = routes
            .parameters()
            .into_iter()
            .map(|(id, _)| id.to_owned())
            .collect();
        assert_eq!(derived, tabled);
    }

    /// The refused pair is minted nowhere: not reachable, rather than merely not drawn.
    #[test]
    fn the_inverter_destination_mints_no_pair_for_itself() {
        assert_eq!(ROUTE_IDS[INVERTER].len(), CV_SOURCES - 1);
        for (destination, row) in ROUTE_IDS.iter().enumerate() {
            if destination != INVERTER {
                assert_eq!(row.len(), CV_SOURCES);
            }
        }
        let ids: Vec<&str> = Routes::new()
            .parameters()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert!(!ids.contains(&"cv_inverter_inverter"));
        assert!(!ids.contains(&"cv_inverter_inverteron"));
    }
}
