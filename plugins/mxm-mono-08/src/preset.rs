//! Factory content and the instrument-specific side of the shared preset system.

use crate::params::MxmMono08Params;
pub use mxm_preset::{
    Category, Entry, INIT_NAME, Library, Loaded, Origin, Preset, PresetIdentity, Refused, Value,
    factory, loaded, mark_loaded, mark_none, read_favourites, snapshot, write_favourites,
};
use std::sync::RwLock;

/// **The tempo syncs this plugin gained on 2026-09-25** (`plans/plan-tempo-sync-controls.md`). A
/// preset file written before them was written unsynced, so each loads off rather than keeping the
/// instance's sync, and without reporting a missing control.
pub(crate) const TEMPO_SYNC_IDS: &[&str] = &["pulsersync"];

impl mxm_preset::Instrument for MxmMono08Params {
    fn clap_id(&self) -> &'static str {
        crate::CLAP_ID
    }
    fn parameters(&self) -> Vec<(&'static str, &dyn mxm_preset::ErasedParam)> {
        self.all_parameters()
    }
    fn identity(&self) -> &RwLock<PresetIdentity> {
        &self.preset
    }
    fn factory_files(&self) -> &'static [(&'static str, &'static str)] {
        FACTORY_FILES
    }
    /// The modulation oscillator's high range came after preset files existed (2026-09-23). A file
    /// that does not name it was written for the one range there was, so it loads in the low range
    /// rather than keeping whichever range the instance was in.
    ///
    /// The clock period's tempo sync is the same case ([`TEMPO_SYNC_IDS`]).
    fn default_missing_legacy_parameter(&self, id: &str) -> bool {
        id == "modhigh" || TEMPO_SYNC_IDS.contains(&id)
    }
}

/// The shipped bank, in the order `FACTORY_DESIGN` states it.
///
/// Generated — never hand-edited. `write_the_factory_presets` writes the files and
/// `shipped_files_are_exactly_the_readable_design` proves this list and those files still
/// say what the design says.
pub const FACTORY_FILES: &[(&str, &str)] = &[
    ("Struck bell", include_str!("../presets/struck-bell.json")),
    ("Wood block", include_str!("../presets/wood-block.json")),
    ("Metal ring", include_str!("../presets/metal-ring.json")),
    ("Gong wash", include_str!("../presets/gong-wash.json")),
    ("Tin cascade", include_str!("../presets/tin-cascade.json")),
    ("Iron pulse", include_str!("../presets/iron-pulse.json")),
    ("Hollow key", include_str!("../presets/hollow-key.json")),
    ("Glass keys", include_str!("../presets/glass-keys.json")),
    ("Soft strike", include_str!("../presets/soft-strike.json")),
    ("Reed key", include_str!("../presets/reed-key.json")),
    ("Clear pluck", include_str!("../presets/clear-pluck.json")),
    ("Wire pluck", include_str!("../presets/wire-pluck.json")),
    ("Gut pluck", include_str!("../presets/gut-pluck.json")),
    ("Harp ping", include_str!("../presets/harp-ping.json")),
    ("Muted pick", include_str!("../presets/muted-pick.json")),
    ("Round bass", include_str!("../presets/round-bass.json")),
    ("Tight bass", include_str!("../presets/tight-bass.json")),
    ("Folded bass", include_str!("../presets/folded-bass.json")),
    ("Growl bass", include_str!("../presets/growl-bass.json")),
    (
        "Pressure bloom",
        include_str!("../presets/pressure-bloom.json"),
    ),
    ("Long spring", include_str!("../presets/long-spring.json")),
    (
        "Breathing air",
        include_str!("../presets/breathing-air.json"),
    ),
    ("Wide swell", include_str!("../presets/wide-swell.json")),
    ("Mod motion", include_str!("../presets/mod-motion.json")),
    ("Narrow lead", include_str!("../presets/narrow-lead.json")),
    ("Singing lead", include_str!("../presets/singing-lead.json")),
    ("Pulser drone", include_str!("../presets/pulser-drone.json")),
    (
        "Mod oscillator drone",
        include_str!("../presets/mod-oscillator-drone.json"),
    ),
    (
        "Low pulse drone",
        include_str!("../presets/low-pulse-drone.json"),
    ),
    ("Random hold", include_str!("../presets/random-hold.json")),
    (
        "Inverted motion",
        include_str!("../presets/inverted-motion.json"),
    ),
    ("Beating pair", include_str!("../presets/beating-pair.json")),
    ("Rain on tin", include_str!("../presets/rain-on-tin.json")),
    (
        "Two step climb",
        include_str!("../presets/two-step-climb.json"),
    ),
    (
        "Three step fall",
        include_str!("../presets/three-step-fall.json"),
    ),
    (
        "Four step arch",
        include_str!("../presets/four-step-arch.json"),
    ),
    (
        "Five step rise",
        include_str!("../presets/five-step-rise.json"),
    ),
    ("Sparse steps", include_str!("../presets/sparse-steps.json")),
    (
        "Pulse staircase",
        include_str!("../presets/pulse-staircase.json"),
    ),
    ("Stage tempo", include_str!("../presets/stage-tempo.json")),
    (
        "Random sequence",
        include_str!("../presets/random-sequence.json"),
    ),
    (
        "Gliding sequence",
        include_str!("../presets/gliding-sequence.json"),
    ),
    ("Stage timbre", include_str!("../presets/stage-timbre.json")),
    (
        "Pressure bend",
        include_str!("../presets/pressure-bend.json"),
    ),
    (
        "Wheel opening",
        include_str!("../presets/wheel-opening.json"),
    ),
    (
        "Feedback sheen",
        include_str!("../presets/feedback-sheen.json"),
    ),
    ("Ring clang", include_str!("../presets/ring-clang.json")),
    (
        "Random bleeps",
        include_str!("../presets/random-bleeps.json"),
    ),
    (
        "Inverter duck",
        include_str!("../presets/inverter-duck.json"),
    ),
    (
        "Patch laboratory",
        include_str!("../presets/patch-laboratory.json"),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// **A project saved before the tempo syncs restores them Off** (`mxm_preset::add_switches_off`),
    /// whatever this instance had.
    #[test]
    fn an_older_state_restores_the_tempo_syncs_off() {
        use nice_plug::prelude::Plugin as _;
        let mut state = nice_plug::prelude::PluginState {
            version: String::new(),
            params: Default::default(),
            fields: Default::default(),
        };
        crate::MxmMono08::filter_state(&mut state);
        for id in TEMPO_SYNC_IDS {
            assert!(
                matches!(
                    state.params.get(*id),
                    Some(nice_plug::plugin::ParamValue::Bool(false))
                ),
                "{{id}} was not restored off"
            );
        }
    }

    /// **A preset saved before the tempo syncs loads them off, and cleanly** ([`TEMPO_SYNC_IDS`]).
    #[test]
    fn a_preset_from_before_the_tempo_syncs_loads_them_off() {
        let params = crate::params::MxmMono08Params::default();
        let mut old = mxm_preset::Preset::init(&params);
        for id in TEMPO_SYNC_IDS {
            old.params.remove(*id);
        }
        let (writes, problems) = old.resolve(&params);
        assert!(problems.is_empty(), "{{problems:?}}");
        for id in TEMPO_SYNC_IDS {
            assert!(
                writes.iter().any(|(w, _, v)| w == id && *v == 0.0),
                "{{id}} was not written off"
            );
        }
    }

    use mxm_preset::user_root;

    /// **The fifty sounds, as a readable design.** Each entry states its whole voice.
    ///
    /// The bank this replaced stated three to five overrides on Init, so fifty names shared one
    /// patch: `master` and `mix1` identical in all fifty, `complexfreq` in forty-six, and fifteen
    /// presets rendering to the same fingerprint to within a rounding error. A design here sets its
    /// oscillator, its endpoint, both gates, its envelope, its mixer and its routes, because a
    /// sound is all of those and a dent in Init is none of them.
    ///
    /// **Values are normalised.** `the_mapping_table` prints what each one means in Hz, ms and mode
    /// names; a route amount is signed and reads zero at `0.5`. Its fader is linear, so `0.75` is
    /// +50 % and `0.25` is −50 % — except a network pitch pair's, which is square-law, so there
    /// `0.75` is +25 %. (The modulation standard turned every other fader linear; each amount here
    /// was re-expressed then for the same plain value, bit for bit.) Naming a `cv_…` amount mints its
    /// presence; naming a `cv_…on` at `0.0` clears one.
    ///
    /// `bank_quality` in `lib.rs` holds the set to fifty *different* sounds. Designs are chosen to
    /// spread its axes deliberately: pitch across the range rather than one note, attacks from a
    /// 2 ms strike to a 2 s swell, and movement from a still drone to a pulsed sequence.
    type Design = (&'static str, Category, &'static [(&'static str, f32)]);
    const FACTORY_DESIGN: &[Design] = &[
        // ── Struck and ringing: the complex oscillator through a combination gate, which is what
        //    couples brightness to loudness and makes a decay sound struck rather than faded.
        (
            "Struck bell",
            Category::Percussion,
            &[
                ("complexfreq", 0.72),
                ("complexendpoint", 1.0),
                ("wavemix", 0.28),
                ("timbre", 0.85),
                ("gate1mode", 1.0),
                ("gate1level", 0.0),
                ("mix1", 0.75),
                ("mix2", 0.0),
                ("reverb", 0.34),
                ("attack", 0.06),
                ("duration", 0.08),
                ("decay", 0.62),
                ("envmode", 0.0),
                ("modtype", 1.0),
                ("modindex", 0.16),
                ("modfreq", 0.86),
                ("modkeyboard", 1.0),
                // The run detunes the FM partner, so each strike rings on a different partial.
                ("pulserperiod", 0.46),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                ("seqlength", 0.34),
                ("seq1level", 0.2),
                ("seq2level", 0.65),
                ("seq3level", 0.4),
                ("cv_modpitch_sequencer", 0.72),
                ("cv_gate1_envelope", 0.905),
                ("cv_timbre_envelope", 0.6152),
            ],
        ),
        (
            "Wood block",
            Category::Percussion,
            &[
                ("complexfreq", 0.56),
                ("complexendpoint", 0.0),
                ("wavemix", 0.72),
                ("timbre", 0.38),
                ("gate1mode", 1.0),
                ("mix1", 0.92),
                ("reverb", 0.1),
                ("attack", 0.05),
                ("duration", 0.05),
                ("decay", 0.19),
                ("envmode", 0.0),
                ("cv_gate1_envelope", 0.81999993),
            ],
        ),
        (
            "Metal ring",
            Category::Percussion,
            &[
                ("complexfreq", 0.8),
                ("complexendpoint", 1.0),
                ("wavemix", 0.4),
                ("timbre", 0.6),
                ("modtype", 0.0),
                ("modindex", 1.0),
                ("modfreq", 0.97),
                ("modkeyboard", 1.0),
                ("gate1mode", 1.0),
                ("mix1", 0.7),
                ("reverb", 0.6),
                ("attack", 0.05),
                ("duration", 0.1),
                ("decay", 0.66),
                ("cv_gate1_envelope", 0.8528),
            ],
        ),
        (
            "Gong wash",
            Category::Percussion,
            &[
                ("complexfreq", 0.34),
                ("complexendpoint", 1.0),
                ("wavemix", 0.5),
                ("timbre", 0.95),
                ("gate1mode", 1.0),
                ("mix1", 0.66),
                ("reverb", 0.95),
                ("attack", 0.14),
                ("duration", 0.18),
                ("decay", 0.8),
                ("envmode", 0.0),
                ("cv_gate1_envelope", 0.7888),
                ("cv_timbre_envelope", 0.7048),
            ],
        ),
        (
            "Tin cascade",
            Category::Percussion,
            &[
                ("complexfreq", 0.8),
                ("complexendpoint", 0.5),
                ("wavemix", 0.55),
                ("timbre", 0.45),
                ("gate1mode", 1.0),
                ("mix1", 0.72),
                ("reverb", 0.26),
                ("attack", 0.05),
                ("duration", 0.06),
                ("decay", 0.3),
                ("pulserperiod", 0.3),
                ("pulse_pulser_keyboard", 1.0),
                ("pulserself", 1.0),
                ("pulse_envelope_pulserend", 1.0),
                ("pulse_random_pulserend", 1.0),
                ("cv_gate1_envelope", 0.81999993),
                ("cv_complexpitch_random1", 0.62),
                // Five stages of brightness under the random pitch, so the cascade is not uniform.
                ("pulse_sequencer_pulserend", 1.0),
                ("seqlength", 1.0),
                ("seq1level", 0.9),
                ("seq2level", 0.2),
                ("seq3level", 0.6),
                ("seq4level", 0.35),
                ("seq5level", 0.75),
                ("cv_timbre_sequencer", 0.68),
            ],
        ),
        (
            "Iron pulse",
            Category::Percussion,
            &[
                ("complexfreq", 0.62),
                ("complexendpoint", 0.0),
                ("wavemix", 0.85),
                ("timbre", 0.7),
                ("gate1mode", 1.0),
                ("gate1level", 0.05),
                ("mix1", 0.6),
                ("reverb", 0.3),
                ("attack", 0.05),
                ("duration", 0.07),
                ("decay", 0.35),
                ("pulserperiod", 0.36),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_envelope_pulserend", 1.0),
                // The sequencer paces the pulser that advances it, so the strikes fall unevenly.
                ("pulse_sequencer_pulserend", 1.0),
                ("pulse_pulser_stage", 1.0),
                ("seqlength", 0.0),
                ("seq1level", 0.3),
                ("seq2level", 0.9),
                ("cv_pulserperiod_sequencer", 0.68),
                ("cv_gate1_envelope", 0.8872),
            ],
        ),
        // ── Keys: played, sustained where the machine sustains, and none of them the same gate.
        (
            "Hollow key",
            Category::Keys,
            &[
                ("complexfreq", 0.7),
                ("complexendpoint", 0.5),
                ("wavemix", 0.82),
                ("timbre", 0.12),
                ("gate1mode", 1.0),
                ("gate1level", 0.18),
                ("mix1", 0.5),
                ("attack", 0.12),
                ("duration", 0.42),
                ("decay", 0.44),
                ("envmode", 0.0),
                ("cv_gate1_envelope", 0.6568),
            ],
        ),
        (
            "Glass keys",
            Category::Keys,
            &[
                ("complexfreq", 0.75),
                ("complexendpoint", 1.0),
                ("wavemix", 0.9),
                ("timbre", 0.75),
                ("gate1mode", 1.0),
                ("mix1", 0.68),
                ("reverb", 0.22),
                ("attack", 0.05),
                ("duration", 0.12),
                ("decay", 0.44),
                ("cv_gate1_envelope", 0.7888),
                ("cv_timbre_pressure", 0.6568),
            ],
        ),
        (
            "Soft strike",
            Category::Keys,
            &[
                ("complexfreq", 0.42),
                ("complexendpoint", 1.0),
                ("wavemix", 0.16),
                ("timbre", 0.08),
                ("gate1mode", 1.0),
                ("gate1level", 0.08),
                ("mix1", 0.88),
                ("attack", 0.32),
                ("duration", 0.1),
                ("decay", 0.52),
                ("cv_gate1_envelope", 0.7048),
            ],
        ),
        (
            "Reed key",
            Category::Keys,
            &[
                ("complexfreq", 0.74),
                ("complexendpoint", 0.5),
                ("wavemix", 0.45),
                ("timbre", 0.74),
                ("gate1mode", 1.0),
                ("gate1level", 0.2),
                ("mix1", 0.62),
                ("attack", 0.4),
                ("duration", 0.4),
                ("decay", 0.42),
                ("envmode", 1.0),
                ("modtype", 1.0),
                ("modindex", 0.26),
                ("modfreq", 0.66),
                ("cv_gate1_envelope", 0.68),
                ("cv_timbre_pressure", 0.58),
            ],
        ),
        // ── Plucked: the short end of the envelope, and where the spike endpoint earns its keep.
        (
            "Clear pluck",
            Category::Pluck,
            &[
                ("complexfreq", 0.6),
                ("complexendpoint", 1.0),
                ("wavemix", 0.3),
                ("timbre", 0.25),
                ("gate1mode", 1.0),
                ("mix1", 0.86),
                ("attack", 0.05),
                ("duration", 0.09),
                ("decay", 0.26),
                ("cv_gate1_envelope", 0.81999993),
            ],
        ),
        (
            "Wire pluck",
            Category::Pluck,
            &[
                ("complexfreq", 0.84),
                ("complexendpoint", 0.0),
                ("wavemix", 0.6),
                ("timbre", 0.77040005),
                ("gate1mode", 1.0),
                ("mix1", 0.78),
                ("reverb", 0.34),
                ("attack", 0.05),
                ("duration", 0.06),
                ("decay", 0.32),
                ("cv_gate1_envelope", 0.8698),
                ("cv_timbre_velocity", 0.6352),
            ],
        ),
        (
            "Gut pluck",
            Category::Pluck,
            &[
                ("complexfreq", 0.46),
                ("complexendpoint", 1.0),
                ("wavemix", 0.12),
                ("timbre", 0.18),
                ("gate1mode", 1.0),
                ("gate1level", 0.05),
                ("mix1", 0.8),
                ("attack", 0.07),
                ("duration", 0.11),
                ("decay", 0.31),
                ("cv_gate1_envelope", 0.745),
            ],
        ),
        (
            "Harp ping",
            Category::Pluck,
            &[
                ("complexfreq", 0.88),
                ("complexendpoint", 1.0),
                ("wavemix", 0.5),
                ("timbre", 0.4),
                ("gate1mode", 1.0),
                ("mix1", 0.55),
                ("reverb", 0.44),
                ("attack", 0.05),
                ("duration", 0.05),
                ("decay", 0.4),
                ("cv_gate1_envelope", 0.92319995),
            ],
        ),
        (
            "Muted pick",
            Category::Pluck,
            &[
                ("complexfreq", 0.54),
                ("complexendpoint", 0.5),
                ("wavemix", 0.25),
                ("timbre", 0.6),
                ("gate1mode", 0.0),
                ("gate1level", 0.1),
                ("mix1", 0.9),
                ("attack", 0.05),
                ("duration", 0.07),
                ("decay", 0.17),
                ("cv_gate1_envelope", 0.7888),
            ],
        ),
        // ── Low: the complex oscillator under 140 Hz, where the endpoint choice changes everything.
        (
            "Round bass",
            Category::Bass,
            &[
                ("complexfreq", 0.34),
                ("complexendpoint", 1.0),
                ("wavemix", 0.1),
                ("timbre", 0.1),
                ("gate1mode", 1.0),
                ("gate1level", 0.12),
                ("mix1", 0.9),
                ("attack", 0.08),
                ("duration", 0.2),
                ("decay", 0.4),
                ("envmode", 1.0),
                ("cv_gate1_envelope", 0.7312),
            ],
        ),
        (
            "Tight bass",
            Category::Bass,
            &[
                ("complexfreq", 0.3),
                ("complexendpoint", 0.5),
                ("wavemix", 0.4),
                ("timbre", 0.35),
                ("gate1mode", 1.0),
                ("mix1", 0.95),
                ("attack", 0.05),
                ("duration", 0.08),
                ("decay", 0.22),
                ("cv_gate1_envelope", 0.8872),
            ],
        ),
        (
            "Folded bass",
            Category::Bass,
            &[
                ("complexfreq", 0.28),
                ("complexendpoint", 0.0),
                ("wavemix", 0.92),
                ("timbre", 0.72),
                ("gate1mode", 1.0),
                ("mix1", 0.82),
                ("attack", 0.06),
                ("duration", 0.14),
                ("decay", 0.38),
                ("cv_gate1_envelope", 0.81999993),
                ("cv_timbre_envelope", 0.5648),
            ],
        ),
        (
            "Growl bass",
            Category::Bass,
            &[
                ("complexfreq", 0.32),
                ("complexendpoint", 0.5),
                ("wavemix", 0.7),
                ("timbre", 0.5),
                ("modtype", 1.0),
                ("modindex", 0.22),
                ("modfreq", 0.72),
                ("modkeyboard", 1.0),
                ("gate1mode", 1.0),
                ("mix1", 0.86),
                ("attack", 0.05),
                ("duration", 0.22),
                ("decay", 0.36),
                ("envmode", 1.0),
                ("cv_gate1_envelope", 0.8528),
                // A stepped bass: the run opens Gate 1 rather than moving pitch.
                ("pulserperiod", 0.38),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                ("seqlength", 0.67),
                ("seq1level", 0.0),
                ("seq2level", 0.55),
                ("seq3level", 0.25),
                ("seq4level", 0.8),
                ("cv_gate1_sequencer", 0.5968),
            ],
        ),
        // ── Sustained and slow: where the 220 ms optical fall and a long envelope meet.
        (
            "Pressure bloom",
            Category::Pad,
            &[
                ("complexfreq", 0.58),
                ("complexendpoint", 1.0),
                ("wavemix", 0.55),
                ("timbre", 0.3),
                ("gate1mode", 1.0),
                ("mix1", 0.7),
                ("reverb", 0.3),
                ("attack", 0.55),
                ("duration", 0.4),
                ("decay", 0.6),
                ("envmode", 1.0),
                ("cv_gate1_envelope", 0.68),
                ("cv_timbre_pressure", 0.7592),
                ("cv_gate1_pressure", 0.58),
            ],
        ),
        (
            "Long spring",
            Category::Pad,
            &[
                ("complexfreq", 0.48),
                ("complexendpoint", 1.0),
                ("wavemix", 0.35),
                ("timbre", 0.4),
                ("gate1mode", 1.0),
                ("mix1", 0.55),
                ("reverb", 0.95),
                ("attack", 0.34),
                ("duration", 0.45),
                ("decay", 0.66),
                ("envmode", 1.0),
                ("cv_gate1_envelope", 0.7048),
            ],
        ),
        (
            "Breathing air",
            Category::Pad,
            &[
                ("complexfreq", 0.66),
                ("complexendpoint", 0.5),
                ("wavemix", 0.3),
                ("timbre", 0.25),
                ("gate1mode", 0.5),
                ("gate1level", 0.12),
                ("mix1", 0.6),
                ("reverb", 0.4),
                ("attack", 0.42),
                ("duration", 0.5),
                ("decay", 0.55),
                ("envmode", 1.0),
                ("modfreq", 0.36),
                ("modwave", 0.0),
                ("cv_gate1_modosc", 0.81999993),
                ("cv_timbre_modosc", 0.55120003),
                ("cv_gate1_envelope", 0.6352),
            ],
        ),
        (
            "Wide swell",
            Category::Pad,
            &[
                ("complexfreq", 0.54),
                ("complexendpoint", 1.0),
                ("wavemix", 0.75),
                ("timbre", 0.55),
                ("gate1mode", 1.0),
                ("gate1level", 0.25),
                ("mix1", 0.64),
                ("reverb", 0.6),
                ("attack", 0.62),
                ("duration", 0.55),
                ("decay", 0.7),
                ("envmode", 1.0),
                ("cv_gate1_envelope", 0.6568),
                ("cv_timbre_wheel", 0.7312),
                // The run drives the inverter and the complement opens the timbre, so brightness falls as the stage voltage climbs.
                ("pulserperiod", 0.56),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                ("seqlength", 1.0),
                ("seq1level", 0.15),
                ("seq2level", 0.5),
                ("seq3level", 0.85),
                ("seq4level", 0.4),
                ("seq5level", 0.7),
                ("cv_inverter_random1on", 0.0),
                ("cv_inverter_sequencer", 0.81999993),
                ("cv_timbre_inverter", 0.68),
            ],
        ),
        // ── Played leads: one voice each, and modulation that a hand can reach.
        (
            "Mod motion",
            Category::Lead,
            &[
                ("complexfreq", 0.8),
                ("complexendpoint", 0.5),
                ("wavemix", 0.5),
                ("timbre", 0.45),
                ("modtype", 1.0),
                ("modindex", 0.4),
                ("modfreq", 0.8),
                ("modkeyboard", 1.0),
                ("gate1mode", 1.0),
                ("mix1", 0.72),
                ("attack", 0.1),
                ("duration", 0.3),
                ("decay", 0.3),
                ("envmode", 1.0),
                ("cv_gate1_envelope", 0.745),
                ("cv_modindex_wheel", 0.68),
            ],
        ),
        (
            "Narrow lead",
            Category::Lead,
            &[
                ("complexfreq", 0.76),
                ("complexendpoint", 0.0),
                ("wavemix", 0.88),
                ("timbre", 0.2),
                ("gate1mode", 1.0),
                ("mix1", 0.66),
                ("attack", 0.08),
                ("duration", 0.26),
                ("decay", 0.3),
                ("envmode", 1.0),
                ("bendrange", 0.25),
                ("cv_gate1_envelope", 0.7592),
                ("cv_complexpitch_bend", 0.64400005),
            ],
        ),
        (
            "Singing lead",
            Category::Lead,
            &[
                ("complexfreq", 0.71),
                ("complexendpoint", 1.0),
                ("wavemix", 0.48),
                ("timbre", 0.52),
                ("gate1mode", 1.0),
                ("mix1", 0.6),
                ("reverb", 0.28),
                ("attack", 0.22),
                ("duration", 0.35),
                ("decay", 0.45),
                ("envmode", 1.0),
                ("portamento", 0.28),
                ("modfreq", 0.52),
                ("modtype", 1.0),
                ("modindex", 0.08),
                ("cv_gate1_envelope", 0.68),
                ("cv_complexpitch_modosc", 0.56),
            ],
        ),
        // ── Self-running: the pulser, the sequencer and the loop between them. These sound with a
        //    key down and keep sounding; `pulserself` cannot start itself, so each arms the pulser
        //    from the keyboard (`self_mode_does_not_start_itself`).
        (
            "Pulser drone",
            Category::Drone,
            &[
                ("complexfreq", 0.5),
                ("complexendpoint", 1.0),
                ("wavemix", 0.42),
                ("timbre", 0.35),
                ("gate1mode", 1.0),
                ("gate1level", 0.3),
                ("mix1", 0.68),
                ("reverb", 0.52),
                ("pulserperiod", 0.6),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("attack", 0.1),
                ("duration", 0.25),
                ("decay", 0.45),
                ("envmode", 1.0),
                ("cv_gate1_envelope", 0.68),
                ("cv_gate1_pulser", 0.8528),
            ],
        ),
        (
            "Mod oscillator drone",
            Category::Drone,
            &[
                ("complexfreq", 0.46),
                ("complexendpoint", 0.5),
                ("wavemix", 0.6),
                ("timbre", 0.3),
                ("gate2input", 0.0),
                ("gate2mode", 1.0),
                ("gate2level", 0.8),
                ("modfreq", 0.64),
                ("modwave", 0.5),
                ("modkeyboard", 1.0),
                ("mix1", 0.5),
                ("mix2", 0.7),
                ("reverb", 0.25),
                ("attack", 0.2),
                ("duration", 0.4),
                ("decay", 0.5),
                ("envmode", 1.0),
                ("cv_gate1_envelope", 0.68),
                ("gate1level", 0.2),
            ],
        ),
        (
            "Low pulse drone",
            Category::Drone,
            &[
                ("complexfreq", 0.28),
                ("complexendpoint", 1.0),
                ("wavemix", 0.08),
                ("timbre", 0.04),
                ("gate1mode", 1.0),
                ("gate1level", 0.12),
                ("cv_gate1_pulser", 0.7888),
                ("mix1", 0.8),
                ("reverb", 0.5),
                ("pulserperiod", 0.72),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("attack", 0.15),
                ("duration", 0.35),
                ("decay", 0.68),
                ("envmode", 1.0),
                ("cv_gate1_envelope", 0.6152),
                ("cv_timbre_pulser", 0.68),
                // Stages work the gate, so the drone breathes in three unequal steps.
                ("pulse_sequencer_pulserend", 1.0),
                ("seqlength", 0.34),
                ("seq1level", 0.1),
                ("seq2level", 0.7),
                ("seq3level", 0.35),
                ("cv_gate1_sequencer", 0.68),
            ],
        ),
        (
            "Random hold",
            Category::Drone,
            &[
                ("complexfreq", 0.6),
                ("complexendpoint", 1.0),
                ("wavemix", 0.45),
                ("timbre", 0.3),
                ("gate1mode", 1.0),
                ("gate1level", 0.35),
                ("mix1", 0.7),
                ("reverb", 0.35),
                ("pulserperiod", 0.36),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_random_pulserend", 1.0),
                ("attack", 0.12),
                ("duration", 0.3),
                ("decay", 0.5),
                ("envmode", 1.0),
                ("cv_gate1_envelope", 0.6568),
                ("cv_complexpitch_random2", 0.66),
                ("cv_timbre_random4", 0.5968),
            ],
        ),
        (
            "Inverted motion",
            Category::Drone,
            &[
                ("complexfreq", 0.5),
                ("complexendpoint", 0.5),
                ("wavemix", 0.22),
                ("timbre", 0.06),
                ("gate1mode", 1.0),
                ("gate1level", 0.08),
                ("mix1", 0.72),
                ("reverb", 0.36),
                ("cv_inverter_random1on", 0.0),
                ("cv_inverter_envelope", 1.0),
                ("cv_timbre_inverter", 0.92319995),
                ("cv_gate1_envelope", 0.68),
                ("attack", 0.1),
                ("duration", 0.3),
                ("decay", 0.7),
                ("envmode", 0.0),
            ],
        ),
        (
            "Beating pair",
            Category::Drone,
            &[
                ("complexfreq", 0.62),
                ("complexendpoint", 1.0),
                ("wavemix", 0.65),
                ("timbre", 0.62),
                ("modtype", 0.0),
                ("modindex", 0.45),
                ("modfreq", 0.42),
                // Two stages step the ring-modulation depth, so the beating changes character.
                ("pulserperiod", 0.5),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                ("seqlength", 0.0),
                ("seq1level", 0.25),
                ("seq2level", 0.85),
                ("cv_modindex_sequencer", 0.7048),
                ("gate1mode", 1.0),
                ("gate1level", 0.45),
                ("mix1", 0.66),
                ("reverb", 0.2),
                ("attack", 0.18),
                ("duration", 0.4),
                ("decay", 0.5),
                ("envmode", 1.0),
                ("cv_gate1_envelope", 0.6352),
            ],
        ),
        (
            "Rain on tin",
            Category::Drone,
            &[
                ("complexfreq", 0.78),
                ("complexendpoint", 0.0),
                ("wavemix", 0.8),
                ("timbre", 0.55),
                ("gate1mode", 1.0),
                ("mix1", 0.6),
                ("reverb", 0.55),
                ("pulserperiod", 0.27),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_envelope_pulserend", 1.0),
                ("pulse_random_pulserend", 1.0),
                ("attack", 0.05),
                ("duration", 0.05),
                ("decay", 0.22),
                ("cv_gate1_envelope", 0.81999993),
                ("cv_complexpitch_random3", 0.68),
            ],
        ),
        // ── The five-stage sequencer. Stage voltage and stage pulse are independent, which is what
        //    lets a run articulate the envelope on some steps and only change pitch on others.
        (
            "Two step climb",
            Category::Sequence,
            &[
                ("seqlength", 0.0),
                ("seq1level", 0.15),
                ("seq2level", 0.7),
                ("complexfreq", 0.66),
                ("complexendpoint", 1.0),
                ("wavemix", 0.35),
                ("timbre", 0.5),
                ("gate1mode", 1.0),
                ("mix1", 0.8),
                ("reverb", 0.36),
                ("pulserperiod", 0.34),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                // Advanced by the key as well as the clock, so playing steps the run.
                ("pulse_sequencer_keyboard", 1.0),
                ("pulse_envelope_stage", 1.0),
                ("attack", 0.05),
                ("duration", 0.08),
                ("decay", 0.3),
                ("cv_gate1_envelope", 0.81999993),
                ("cv_complexpitch_sequencer", 0.88),
            ],
        ),
        (
            "Three step fall",
            Category::Sequence,
            &[
                ("seqlength", 0.34),
                ("seq1level", 0.9),
                ("seq2level", 0.5),
                ("seq3level", 0.1),
                ("complexfreq", 0.64),
                ("complexendpoint", 0.5),
                ("wavemix", 0.55),
                ("timbre", 0.4),
                ("gate1mode", 1.0),
                ("mix1", 0.74),
                ("reverb", 0.18),
                ("pulserperiod", 0.33),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                ("pulse_envelope_stage", 1.0),
                ("attack", 0.05),
                ("duration", 0.06),
                ("decay", 0.26),
                ("cv_gate1_envelope", 0.8528),
                ("cv_complexpitch_sequencer", 0.76),
                // The run works Gate 1 as well as pitch: each step lands at its own loudness.
                ("cv_gate1_sequencer", 0.6152),
            ],
        ),
        (
            "Four step arch",
            Category::Sequence,
            &[
                ("seqlength", 0.67),
                ("seq1level", 0.1),
                ("seq2level", 0.75),
                ("seq3level", 0.75),
                ("seq4level", 0.1),
                ("complexfreq", 0.34),
                ("complexendpoint", 1.0),
                ("wavemix", 0.28),
                ("timbre", 0.1),
                ("gate1mode", 1.0),
                ("gate1level", 0.15),
                ("mix1", 0.78),
                ("reverb", 0.44),
                ("pulserperiod", 0.48),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                ("pulse_envelope_stage", 1.0),
                // Stages redraw the four random voltages, so each pass through the arch differs.
                ("pulse_random_stage", 1.0),
                ("cv_modindex_random2", 0.5648),
                ("attack", 0.08),
                ("duration", 0.1),
                ("decay", 0.34),
                ("cv_gate1_envelope", 0.7592),
                ("cv_timbre_sequencer", 0.7312),
            ],
        ),
        (
            "Five step rise",
            Category::Sequence,
            &[
                ("seqlength", 1.0),
                ("seq1level", 0.05),
                ("seq2level", 0.28),
                ("seq3level", 0.5),
                ("seq4level", 0.72),
                ("seq5level", 0.95),
                ("complexfreq", 0.58),
                ("complexendpoint", 1.0),
                ("wavemix", 0.25),
                ("timbre", 0.25),
                ("gate1mode", 1.0),
                ("mix1", 0.82),
                ("reverb", 0.48),
                ("pulserperiod", 0.38),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                // A ratchet: a pulsing stage advances the run again, so the rise is uneven.
                ("pulse_sequencer_stage", 1.0),
                ("pulse_envelope_stage", 1.0),
                ("attack", 0.05),
                ("duration", 0.07),
                ("decay", 0.28),
                ("cv_gate1_envelope", 0.81999993),
                ("cv_complexpitch_sequencer", 0.8),
            ],
        ),
        (
            "Sparse steps",
            Category::Sequence,
            &[
                // The sequencer articulates this one, not the key: holding one note plays the
                // figure, and the two stages whose pulse is off pass silently.
                ("pulse_envelope_keyboard", 0.0),
                // Only two stages pulse the envelope, so three of the five change pitch silently
                // and the run is heard as a gapped figure rather than five even notes.
                ("seqlength", 1.0),
                ("seq1level", 0.2),
                ("seq2level", 0.55),
                ("seq3level", 0.35),
                ("seq4level", 0.85),
                ("seq5level", 0.45),
                ("seq1pulse", 1.0),
                ("seq2pulse", 0.0),
                ("seq3pulse", 0.0),
                ("seq4pulse", 1.0),
                ("seq5pulse", 0.0),
                ("complexfreq", 0.46),
                ("complexendpoint", 1.0),
                ("wavemix", 0.45),
                ("timbre", 0.35),
                ("gate1mode", 1.0),
                ("mix1", 0.8),
                ("reverb", 0.58),
                ("pulserperiod", 0.34),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                ("pulse_envelope_stage", 1.0),
                ("attack", 0.05),
                ("duration", 0.09),
                ("decay", 0.36),
                ("cv_gate1_envelope", 0.81999993),
                ("cv_complexpitch_sequencer", 0.74),
            ],
        ),
        (
            "Pulse staircase",
            Category::Sequence,
            &[
                // The keyboard does not pulse the envelope here: the sequencer articulates it, so
                // holding one key plays the figure.
                ("pulse_envelope_keyboard", 0.0),
                ("pulse_envelope_stage", 1.0),
                ("seqlength", 1.0),
                ("seq1level", 0.0),
                ("seq2level", 0.25),
                ("seq3level", 0.5),
                ("seq4level", 0.75),
                ("seq5level", 1.0),
                ("complexfreq", 0.5),
                ("complexendpoint", 0.5),
                ("wavemix", 0.7),
                ("timbre", 0.45),
                ("gate1mode", 1.0),
                ("mix1", 0.76),
                ("pulserperiod", 0.32),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                ("attack", 0.05),
                ("duration", 0.06),
                ("decay", 0.24),
                ("cv_gate1_envelope", 0.8872),
                ("cv_complexpitch_sequencer", 0.82),
                // The step also sets how fast the glide to it is.
                ("portamento", 0.3),
                ("cv_portamentospeed_sequencer", 0.6568),
            ],
        ),
        (
            "Stage tempo",
            Category::Sequence,
            &[
                // The documented timing loop: the stage voltage shortens the pulser period that
                // advances the sequencer, in the same sample.
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                ("pulse_envelope_stage", 1.0),
                // The stage pulse re-fires the pulser it is clocked by, tightening the loop.
                ("pulse_pulser_stage", 1.0),
                ("cv_pulserperiod_sequencer", 0.92319995),
                ("seqlength", 1.0),
                ("seq1level", 0.1),
                ("seq2level", 0.4),
                ("seq3level", 0.2),
                ("seq4level", 0.9),
                ("seq5level", 0.6),
                ("complexfreq", 0.68),
                ("complexendpoint", 1.0),
                ("wavemix", 0.4),
                ("timbre", 0.5),
                ("gate1mode", 1.0),
                ("mix1", 0.72),
                ("reverb", 0.22),
                ("pulserperiod", 0.34),
                ("attack", 0.05),
                ("duration", 0.06),
                ("decay", 0.2),
                ("cv_gate1_envelope", 0.8528),
            ],
        ),
        (
            "Random sequence",
            Category::Sequence,
            &[
                ("pulse_random_keyboard", 1.0),
                ("pulse_random_pulserend", 1.0),
                ("pulse_random_stage", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                ("pulse_envelope_stage", 1.0),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulserperiod", 0.36),
                ("seqlength", 0.67),
                ("seq1level", 0.3),
                ("seq2level", 0.6),
                ("seq3level", 0.15),
                ("seq4level", 0.8),
                ("complexfreq", 0.56),
                ("complexendpoint", 0.0),
                ("wavemix", 0.55),
                ("timbre", 0.3),
                ("gate1mode", 1.0),
                ("mix1", 0.74),
                ("reverb", 0.26),
                ("attack", 0.05),
                ("duration", 0.07),
                ("decay", 0.3),
                ("cv_gate1_envelope", 0.81999993),
                ("cv_timbre_random1", 0.6568),
                ("cv_complexpitch_sequencer", 0.68),
            ],
        ),
        (
            "Gliding sequence",
            Category::Sequence,
            &[
                ("portamento", 0.62),
                ("seqlength", 0.67),
                ("seq1level", 0.1),
                ("seq2level", 0.9),
                ("seq3level", 0.4),
                ("seq4level", 0.65),
                ("complexfreq", 0.62),
                ("complexendpoint", 1.0),
                ("wavemix", 0.3),
                ("timbre", 0.2),
                ("gate1mode", 1.0),
                ("gate1level", 0.1),
                ("mix1", 0.78),
                ("reverb", 0.24),
                ("pulserperiod", 0.72),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                ("attack", 0.2),
                ("duration", 0.4),
                ("decay", 0.5),
                ("envmode", 1.0),
                ("cv_gate1_envelope", 0.6568),
                ("cv_complexpitch_sequencer", 0.95),
                // The run detunes the modulation oscillator with it.
                ("cv_modpitch_sequencer", 0.7),
                ("modindex", 0.24),
                ("modtype", 1.0),
            ],
        ),
        (
            "Stage timbre",
            Category::Sequence,
            &[
                // The run moves brightness, not pitch: the same note five ways.
                ("seqlength", 1.0),
                ("seq1level", 0.0),
                ("seq2level", 0.3),
                ("seq3level", 0.6),
                ("seq4level", 0.45),
                ("seq5level", 0.9),
                ("complexfreq", 0.44),
                ("complexendpoint", 1.0),
                ("wavemix", 0.85),
                ("timbre", 0.1),
                ("gate1mode", 1.0),
                ("mix1", 0.76),
                ("reverb", 0.34),
                ("pulserperiod", 0.56),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                ("pulse_envelope_stage", 1.0),
                ("attack", 0.06),
                ("duration", 0.1),
                ("decay", 0.34),
                ("cv_gate1_envelope", 0.7888),
                ("cv_timbre_sequencer", 0.905),
                ("cv_modindex_sequencer", 0.6568),
                ("modtype", 1.0),
                ("modfreq", 0.7),
            ],
        ),
        // ── Pressure, wheel, velocity and bend: the four sources the old bank never routed.
        (
            "Pressure bend",
            Category::Lead,
            &[
                ("complexfreq", 0.76),
                ("complexendpoint", 1.0),
                ("wavemix", 0.5),
                ("timbre", 0.71000004),
                ("gate1mode", 1.0),
                ("mix1", 0.7),
                ("reverb", 0.42),
                ("attack", 0.12),
                ("duration", 0.3),
                ("decay", 0.3),
                ("envmode", 1.0),
                ("bendrange", 0.5),
                ("cv_gate1_envelope", 0.7312),
                ("cv_complexpitch_pressure", 0.84),
                ("cv_timbre_velocity", 0.68),
            ],
        ),
        (
            "Wheel opening",
            Category::Fx,
            &[
                ("complexfreq", 0.62),
                ("complexendpoint", 0.5),
                ("wavemix", 0.6),
                ("timbre", 0.15),
                ("gate1mode", 1.0),
                ("gate1level", 0.1),
                ("mix1", 0.72),
                ("reverb", 0.34),
                ("attack", 0.3),
                ("duration", 0.45),
                ("decay", 0.55),
                ("envmode", 1.0),
                ("cv_gate1_envelope", 0.6352),
                ("cv_gate1_wheel", 0.7888),
                ("cv_timbre_wheel", 0.7048),
            ],
        ),
        (
            "Feedback sheen",
            Category::Fx,
            &[
                // The complex oscillator's own output as a control source — the instrument's FM
                // feedback path, and the one source nothing in the old bank ever routed.
                ("complexfreq", 0.7),
                ("complexendpoint", 1.0),
                ("wavemix", 0.6),
                ("timbre", 0.4),
                ("modtype", 1.0),
                ("modindex", 0.42),
                ("modfreq", 0.8),
                ("gate1mode", 1.0),
                ("mix1", 0.64),
                ("reverb", 0.4),
                ("attack", 0.08),
                ("duration", 0.15),
                ("decay", 0.45),
                ("cv_gate1_envelope", 0.7592),
                ("cv_timbre_complexosc", 0.8528),
                ("cv_modpitch_complexosc", 0.78),
            ],
        ),
        // **The three sounds that replaced the external input's** (the owner's ruling, 2026-09-23):
        // *Balanced edge*, *External follower* and *External spring* needed audio from a port the
        // instrument no longer has. Each replacement is built on a mechanism the bank used least.
        (
            "Ring clang",
            Category::Fx,
            &[
                // AM at full depth is complete ring modulation (the 1974 manual), and the high
                // range puts the modulator at audio rate, so the partials are sums and differences
                // rather than a tremolo. The envelope sweeps the modulator down as the strike decays.
                ("complexfreq", 0.66),
                ("complexendpoint", 1.0),
                ("wavemix", 0.25),
                ("timbre", 0.35),
                ("modtype", 0.0),
                ("modindex", 1.0),
                ("modhigh", 1.0),
                ("modfreq", 0.66),
                ("modkeyboard", 1.0),
                ("gate1mode", 1.0),
                ("mix1", 0.75),
                ("reverb", 0.45),
                ("attack", 0.08),
                ("duration", 0.1),
                ("decay", 0.62),
                ("cv_gate1_envelope", 0.81999993),
                ("cv_modpitch_envelope", 0.62),
            ],
        ),
        (
            "Random bleeps",
            Category::Fx,
            &[
                // A fast free-running clock strikes the envelope and redraws the random levels on
                // every tick, so each bleep lands on a new pitch, FM ratio and depth.
                ("complexfreq", 0.58),
                ("complexendpoint", 0.5),
                ("wavemix", 0.5),
                ("timbre", 0.25),
                ("modtype", 1.0),
                ("modindex", 0.55),
                ("modhigh", 1.0),
                ("modfreq", 0.6),
                ("modkeyboard", 1.0),
                ("gate1mode", 1.0),
                ("mix1", 0.75),
                ("reverb", 0.3),
                ("pulserperiod", 0.33),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_random_pulserend", 1.0),
                ("pulse_envelope_pulserend", 1.0),
                ("attack", 0.03),
                ("duration", 0.05),
                ("decay", 0.3),
                ("cv_gate1_envelope", 0.81999993),
                ("cv_complexpitch_random1", 0.7),
                ("cv_modpitch_random2", 0.75),
                ("cv_modindex_random3", 0.58),
            ],
        ),
        (
            "Inverter duck",
            Category::Fx,
            &[
                // The mod oscillator at audio rate is a drone through Gate 2, held open by the
                // inverter; each strike raises the envelope, the inverter's full-scale-minus-input
                // falls, and the drone ducks under the note.
                ("complexfreq", 0.55),
                ("complexendpoint", 1.0),
                ("wavemix", 0.3),
                ("timbre", 0.3),
                ("gate2input", 0.0),
                ("gate2mode", 1.0),
                ("gate2level", 0.0),
                ("modhigh", 1.0),
                ("modfreq", 0.52),
                ("modwave", 0.5),
                ("modkeyboard", 1.0),
                ("mix1", 0.75),
                ("mix2", 0.6),
                ("reverb", 0.3),
                ("cv_inverter_random1on", 0.0),
                ("cv_inverter_envelope", 1.0),
                ("cv_gate2_inverter", 0.905),
                ("attack", 0.2),
                ("duration", 0.25),
                ("decay", 0.55),
                ("envmode", 0.0),
                ("cv_gate1_envelope", 0.745),
            ],
        ),
        (
            "Patch laboratory",
            Category::Template,
            &[
                // The deliberate starting point: one of everything, at a depth that can be heard
                // and then moved.
                ("complexfreq", 0.58),
                ("complexendpoint", 1.0),
                ("wavemix", 0.4),
                ("timbre", 0.3),
                ("gate1mode", 1.0),
                ("gate1level", 0.2),
                ("gate2input", 1.0),
                ("gate2mode", 1.0),
                ("gate2level", 0.3),
                ("mix1", 0.7),
                ("mix2", 0.25),
                ("reverb", 0.2),
                ("modtype", 1.0),
                ("modindex", 0.18),
                ("modfreq", 0.6),
                ("pulserperiod", 0.38),
                ("pulserself", 1.0),
                ("pulse_pulser_keyboard", 1.0),
                ("pulse_sequencer_pulserend", 1.0),
                ("attack", 0.1),
                ("duration", 0.2),
                ("decay", 0.38),
                ("cv_gate1_envelope", 0.7312),
                ("cv_timbre_pressure", 0.58),
                ("cv_gate2_sequencer", 0.5968),
            ],
        ),
    ];

    fn params() -> MxmMono08Params {
        MxmMono08Params::default()
    }
    fn generated(
        params: &MxmMono08Params,
        name: &str,
        category: Category,
        overrides: &[(&str, f32)],
    ) -> Preset {
        let bindings = params.all_parameters();
        let mut preset = Preset::init(params);
        preset.name = name.to_owned();
        preset.category = category;
        let mut write = |id: &str, v: f32| {
            let bound = bindings
                .iter()
                .find(|(key, _)| key == &id)
                .unwrap_or_else(|| panic!("{name:?} names `{id}`"));
            preset.params.insert(
                id.to_owned(),
                Value {
                    v,
                    text: bound.1.format(v),
                },
            );
        };
        for (id, v) in overrides {
            write(id, *v);
            // **A design names a depth; the route that carries it has to exist**
            // (`plans/plan-mxm-mono-08-modulation.md` §6). Init wires Gate 1 from the envelope and
            // nothing else, so without this every routed factory sound would load with its amount
            // stored against an absent pair and render as if the design had said nothing. The
            // presence id is the amount id plus `on`, so it needs no table to find.
            // Values here are **normalised**, as `Preset::init` stores them, so a signed route
            // amount reads 0.5 at zero depth. §6's rule is *every pair a design overrides to a
            // non-zero amount*, so an override that lands exactly on zero stays absent.
            // **Amounts only.** A design may also name a presence directly — `Inverted motion`
            // clears Init's inverter pair that way, because the selector it replaces could name
            // exactly one source — and appending `on` to a presence id would name a parameter that
            // does not exist, which `write` panics on rather than storing quietly.
            if id.starts_with("cv_") && !id.ends_with("on") {
                write(&format!("{id}on"), if *v == 0.5 { 0.0 } else { 1.0 });
            }
        }
        preset
    }

    /// **What a normalised value means, printed.** mxm-kit's `docs/adding-an-instrument.md` says
    /// every instrument has this and mono-08 did not, which is most of why `FACTORY_DESIGN` reads
    /// like fifty guesses: a design names `("modfreq", 0.42)` and nothing anywhere says that is 12
    /// Hz until the file has been generated and its `text` field read back.
    ///
    /// It uses `ErasedParam::format`, the same call the generator writes into `text`, so the table
    /// and the shipped files cannot disagree.
    #[test]
    #[ignore = "a printed table, not an assertion"]
    fn the_mapping_table() {
        let params = params();
        let grid = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        println!();
        for (id, param) in params.all_parameters() {
            // The 159 route pairs share two shapes — a signed amount and a presence — so printing
            // each of them 11 times buries the panel controls the designs actually reach for.
            if id.starts_with("cv_") || id.starts_with("pulse_") {
                continue;
            }
            print!("{id:<18}");
            for v in grid {
                print!("{:>12}", param.format(v));
            }
            println!("   [default {}]", param.format(param.default_normalised()));
        }
        println!(
            "\nRoute amounts are signed and read zero at 0.5: {} at 0.0, {} at 0.5, {} at 1.0.",
            params
                .all_parameters()
                .iter()
                .find(|(id, _)| *id == "cv_timbre_sequencer")
                .map(|(_, p)| p.format(0.0))
                .unwrap_or_default(),
            params
                .all_parameters()
                .iter()
                .find(|(id, _)| *id == "cv_timbre_sequencer")
                .map(|(_, p)| p.format(0.5))
                .unwrap_or_default(),
            params
                .all_parameters()
                .iter()
                .find(|(id, _)| *id == "cv_timbre_sequencer")
                .map(|(_, p)| p.format(1.0))
                .unwrap_or_default(),
        );
    }

    #[test]
    #[ignore = "writes the generated factory preset files"]
    fn write_the_factory_presets() {
        let p = params();
        for (name, category, overrides) in FACTORY_DESIGN {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("presets")
                .join(format!("{}.json", name.to_lowercase().replace(' ', "-")));
            std::fs::write(path, generated(&p, name, *category, overrides).to_json()).unwrap();
        }
    }
    #[test]
    fn exactly_fifty_designed_factory_sounds() {
        assert_eq!(FACTORY_DESIGN.len(), 50);
        assert_eq!(FACTORY_FILES.len(), 50);
    }
    #[test]
    fn the_set_covers_the_instruments_distinct_mechanisms() {
        let names: Vec<_> = FACTORY_DESIGN.iter().map(|(name, _, _)| *name).collect();
        for required in [
            "Pressure bloom",
            "Metal ring",
            "Mod motion",
            "Inverter duck",
            "Two step climb",
            "Three step fall",
            "Four step arch",
            "Five step rise",
            "Sparse steps",
            "Stage tempo",
            "Pulser drone",
            "Long spring",
        ] {
            assert!(
                names.contains(&required),
                "missing mechanism patch {required}"
            );
        }
    }
    #[test]
    fn routed_factory_sources_are_actually_animated() {
        let design = |wanted: &str| {
            FACTORY_DESIGN
                .iter()
                .find(|(name, _, _)| *name == wanted)
                .unwrap()
                .2
        };
        let value = |values: &[(&str, f32)], id: &str| {
            values
                .iter()
                .find_map(|(key, value)| (*key == id).then_some(*value))
        };

        for (name, _, values) in FACTORY_DESIGN {
            // **Presence and depth, not spelling.** A design can name a random pair to *clear* it —
            // `Inverted motion` clears Init's inverter pair that way — and an id that merely
            // mentions a random source is not a sound routing one. An amount sits at normalised 0.5
            // when it is zero, and a presence is a switch.
            let routes_random = values.iter().any(|(id, value)| {
                id.starts_with("cv_")
                    && id.contains("_random")
                    && if id.ends_with("on") {
                        *value >= 0.5
                    } else {
                        *value != 0.5
                    }
            });
            if routes_random {
                assert!(
                    values
                        .iter()
                        .any(|(id, value)| id.starts_with("pulse_random_") && *value >= 0.5),
                    "{name} routes held random CV but never draws a value"
                );
            }
        }

        for name in ["Pulse staircase", "Sparse steps"] {
            let values = design(name);
            assert_eq!(value(values, "pulse_envelope_keyboard"), Some(0.0));
            assert_eq!(value(values, "pulse_envelope_stage"), Some(1.0));
        }
        let tempo = design("Stage tempo");
        for id in [
            "pulserself",
            "pulse_pulser_keyboard",
            "pulse_sequencer_pulserend",
            "pulse_envelope_stage",
        ] {
            assert_eq!(
                value(tempo, id),
                Some(1.0),
                "Stage tempo needs `{id}` for a repeating audible loop"
            );
        }
        let duck = design("Inverter duck");
        assert_eq!(
            value(duck, "gate2input"),
            Some(0.0),
            "the drone is the mod oscillator"
        );
        assert!(
            value(duck, "cv_gate2_inverter").unwrap_or(0.5) > 0.5,
            "the inverter must hold Gate 2 open in its named sound"
        );
    }
    #[test]
    fn every_design_names_real_normalized_parameters() {
        let p = params();
        let ids = p.all_parameters();
        for (name, _, values) in FACTORY_DESIGN {
            for (id, v) in *values {
                assert!(ids.iter().any(|(key, _)| key == id), "{name}: {id}");
                assert!((0.0..=1.0).contains(v), "{name}: {id}={v}");
            }
        }
    }
    #[test]
    fn shipped_files_are_exactly_the_readable_design() {
        let p = params();
        for (name, category, values) in FACTORY_DESIGN {
            let text = FACTORY_FILES.iter().find(|(n, _)| n == name).unwrap().1;
            assert_eq!(
                Preset::parse(text, crate::CLAP_ID).unwrap(),
                generated(&p, name, *category, values),
                "regenerate {name}"
            );
        }
    }
    #[test]
    fn every_file_is_complete_distinct_categorised_and_owned() {
        let p = params();
        let mut seen = Vec::new();
        for (name, text) in FACTORY_FILES {
            let preset =
                Preset::parse(text, crate::CLAP_ID).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(&preset.name, name);
            assert_ne!(preset.category, Category::Uncategorised);
            assert!(preset.resolve(&p).1.is_empty(), "{name} is incomplete");
            assert!(
                !seen.iter().any(|old| old == &preset.params),
                "duplicate {name}"
            );
            seen.push(preset.params);
        }
    }
    #[test]
    fn every_sound_keeps_an_audible_output_path() {
        for (name, text) in FACTORY_FILES {
            let p = Preset::parse(text, crate::CLAP_ID).unwrap();
            let v = |id: &str| p.params[id].v;
            assert!(v("mix1") > 0.05 || v("mix2") > 0.05, "{name}");
            assert!(v("master") > 0.05, "{name}");
            assert!(
                v("gate1level") > 0.05 || v("cv_gate1_envelope") > 0.55,
                "{name}"
            );
        }
    }
    #[test]
    fn the_init_preset_is_every_parameter_default() {
        let p = params();
        let init = Preset::init(&p);
        assert_eq!(init.params.len(), p.all_parameters().len());
        for (id, param) in p.all_parameters() {
            assert_eq!(
                init.params[id].v,
                param.default_normalised(),
                "Init's {id} diverged from the host reset value"
            );
        }
    }
    #[test]
    fn factory_begins_with_generated_init_and_master_is_not_designed() {
        let p = params();
        assert_eq!(factory(&p).len(), 51);
        assert_eq!(factory(&p)[0].name, INIT_NAME);
        assert!(
            FACTORY_DESIGN
                .iter()
                .all(|(_, _, v)| !v.iter().any(|(id, _)| *id == "master"))
        );
    }
    #[test]
    fn user_files_are_namespaced_to_the_permanent_id() {
        if let Some(root) = user_root(crate::CLAP_ID) {
            assert!(root.to_string_lossy().contains(crate::CLAP_ID));
        }
    }
}
