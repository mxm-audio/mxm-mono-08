//! mxm-mono-08's routing as the collection's modulation standard checks it
//! (`mxm_modulation::conformance`; `plans/plan-modulation-standard.md`).
//!
//! Behind the `conformance` feature, which only `[dev-dependencies]` enable — this crate's own
//! tests, and the plugin's, whose route readings are held to [`Declared::deliver`] — so no shipped
//! graph carries it. [`Declared`] answers every question through [`crate::routing`]'s own tables and
//! a real [`Graph`], never a copy of them.

use mxm_modulation::conformance::{Declaration, Kind};
use mxm_modulation::standard::{self, Law, Offer, Performance, Sign};

use crate::routing::{
    self, CV_DESTINATIONS, CV_SOURCES, CvDestination, CvSource, DESTINATION_NAMES, Graph,
    INVERTER_IS_NOT_ITS_OWN_SOURCE, KEY_UNIT_SEMITONES, Routing, SOURCE_NAMES, complement,
};

/// What each destination is, for the standard. Pitch and the two `0…1` controls are the standard's
/// kinds; the LPG levels are the machine's CV amplifiers; the clock period, glide speed and the
/// inverter's input throw a negative sum away in a unit of their own; the sequence length adds
/// steps; Amplitude is the standard factor.
const KINDS: [Kind; CV_DESTINATIONS] = [
    Kind::Pitch,
    Kind::Pitch,
    Kind::Control,
    Kind::Control,
    Kind::Machine(Law::MachineAmplifier),
    Kind::Machine(Law::MachineAmplifier),
    Kind::Machine(Law::OneSided(Sign::Negative)),
    Kind::Machine(Law::OneSided(Sign::Negative)),
    Kind::Machine(Law::Sum),
    Kind::Machine(Law::OneSided(Sign::Negative)),
    Kind::Amplitude,
];

/// mxm-mono-08's routing declaration.
#[derive(Debug, Clone, Copy, Default)]
pub struct Declared;

/// Exactly one route, at `amount`.
fn one_route(destination: usize, source: usize, amount: f32) -> Routing {
    let mut routing = Routing::new();
    routing.present[destination][source] = true;
    routing.amounts[destination][source] = amount;
    routing.compact();
    routing
}

impl Declaration for Declared {
    fn sources(&self) -> usize {
        CV_SOURCES
    }

    fn targets(&self) -> usize {
        CV_DESTINATIONS
    }

    fn performance(&self, source: usize) -> Option<Performance> {
        routing::PERFORMANCE[source]
    }

    fn kind(&self, target: usize) -> Kind {
        KINDS[target]
    }

    fn machine(&self, target: usize, source: usize) -> bool {
        routing::machine(target, source)
    }

    fn offered(&self, target: usize, source: usize) -> Offer {
        routing::offer(target, source)
    }

    fn key_unit(&self) -> f32 {
        KEY_UNIT_SEMITONES
    }

    /// One route alone through a real [`Graph`], in what the destination's law makes of its sum:
    /// semitones of pitch, a fraction of a `0…1` control or gate drive, the fraction of the clock's
    /// period taken away, the glide speed gained, the steps added, the change in the inverter's
    /// output, and Amplitude's factor less one.
    fn deliver(&self, target: usize, source: usize, amount: f32, raw: f32) -> f32 {
        let routing = one_route(target, source, amount);
        let mut graph = Graph::new();
        graph.set_topology(&routing);
        graph.begin_sample();
        graph.write(CvSource::ALL[source], raw);
        let destination = CvDestination::ALL[target];
        let sum = graph.sum(destination, &routing);
        match destination {
            CvDestination::ComplexPitch | CvDestination::ModPitch => {
                graph.pitch_octaves(destination, &routing) * 12.0
            }
            CvDestination::Timbre
            | CvDestination::ModIndex
            | CvDestination::Gate1
            | CvDestination::Gate2 => sum,
            CvDestination::PulserPeriod => -0.95 * sum.clamp(0.0, 1.0),
            CvDestination::PortamentoSpeed => 4.0 * sum.max(0.0),
            CvDestination::SequenceLength => 3.0 * sum,
            CvDestination::InverterInput => graph.inverter(&routing) - complement(0.0),
            CvDestination::Amplitude => standard::amplitude_factor(sum) - 1.0,
        }
    }

    fn name(&self, target: usize, source: usize) -> String {
        format!(
            "{} from {}",
            DESTINATION_NAMES[target], SOURCE_NAMES[source]
        )
    }

    /// The inverter is not its own input — this instrument's own refusal, not the standard's.
    fn ruled_out(&self, target: usize, source: usize) -> bool {
        (target, source) == INVERTER_IS_NOT_ITS_OWN_SOURCE
    }
}

#[cfg(test)]
mod tests {
    use mxm_modulation::conformance::{self, Case, Input};

    use super::*;
    use crate::voice::{Activity, NoteId, Params, Voice};

    fn report(result: Result<(), Vec<String>>) {
        if let Err(failures) = result {
            panic!("{} failure(s):\n{}", failures.len(), failures.join("\n"));
        }
    }

    fn id(key: u8) -> NoteId {
        NoteId {
            voice_id: None,
            channel: 0,
            key,
        }
    }

    /// **Every pair means what the standard says**: offered as `standard::offer` says — the LPG
    /// levels keep Velocity's closing half, the clock period, glide speed and inverter input a
    /// one-signed source's live half — nothing at a source's rest, a meaningful move at full, and
    /// the standard reach for every performance pair the 208 did not have.
    ///
    /// Falsified before trusted: with every pair left on the network's reach, it names both halves
    /// of Velocity, the wheel and the lever at both pitch destinations.
    #[test]
    fn every_pair_means_what_the_standard_says() {
        report(conformance::check_declaration(&Declared));
    }

    /// **A voice publishes what the standard says**, in raw units: Key from middle C over sixty
    /// semitones, Velocity as `v − 1`, the wheel, pressure and lever as they arrive.
    ///
    /// Falsified before trusted: publishing Key as `note / 127` fails at every note.
    #[test]
    fn a_voice_publishes_what_the_standard_says() {
        report(conformance::check_publishers(&Declared, |from, input| {
            let mut voice = Voice::new();
            voice.set_topology(&Routing::init());
            let mut p = Params::default();
            let key = match input {
                Input::Note(n) => n,
                _ => 60,
            };
            match input {
                Input::Normalised(value) if from == CvSource::Velocity.index() => {
                    p.velocity = value;
                }
                Input::Normalised(value) if from == CvSource::Wheel.index() => p.wheel = value,
                Input::Normalised(value) if from == CvSource::Pressure.index() => {
                    voice.set_channel_pressure(value);
                }
                Input::Lever(value) => p.bend = value,
                _ => {}
            }
            voice.note_on(id(key));
            voice.process(&p, &Routing::init());
            voice.published_for_test(CvSource::ALL[from])
        }));
    }

    /// **The Amplitude destination is the standard factor on the mix**: at −100 % from a wheel at
    /// full it silences the voice exactly, and at +100 % doubles it.
    ///
    /// Falsified before trusted: with the voice's factor removed, the doubled render equals the
    /// plain one.
    #[test]
    fn amplitude_is_the_standard_factor_on_the_mix() {
        let render = |amount: Option<f32>| {
            let mut routing = Routing::init();
            if let Some(amount) = amount {
                let destination = CvDestination::Amplitude.index();
                routing.present[destination][CvSource::Wheel.index()] = true;
                routing.amounts[destination][CvSource::Wheel.index()] = amount;
                routing.compact();
            }
            let mut voice = Voice::new();
            voice.set_topology(&routing);
            let p = Params {
                wheel: 1.0,
                ..Params::default()
            };
            voice.note_on(id(60));
            (0..2_400)
                .map(|_| voice.process(&p, &routing))
                .collect::<Vec<_>>()
        };
        let plain = render(None);
        assert!(
            plain.iter().any(|s| s.abs() > 1e-4),
            "the premise: it sounds"
        );
        assert!(render(Some(-1.0)).iter().all(|&s| s == 0.0), "silence");
        for (doubled, plain) in render(Some(1.0)).iter().zip(&plain) {
            assert_eq!(*doubled, plain * 2.0);
        }
    }

    /// **After a release, no performance route holds a note open** — every offered pair, on its
    /// offered half, at the softest and hardest notes and the keyboard's ends, gestures held at full
    /// through the note and let go at the release. LPG level ← Key is the 208's own keyboard voltage
    /// holding its gate, which is what that patch does; both are declared drones.
    ///
    /// **Run at the lowest rate the voice supports**, `MIN_SAMPLE_RATE`: a closed optical gate takes
    /// seconds to reach exact zero (`voice::tests::init_is_keyed_and_reaches_exact_idle_silence`
    /// waits eight), and every time in this voice is in seconds, so at a kilohertz four hundred cases
    /// can each wait for exact silence and `Inert` — a tail that never parks fails, not only a gate
    /// held open.
    ///
    /// Falsified before trusted: without the drones declared, it names LPG 1 level ← Key (LPG 2 is
    /// not in the default mix); publishing the raw velocity, it names LPG 1 level ← Velocity; with a
    /// tail that never parks, it names every case.
    #[test]
    fn after_a_release_no_performance_route_holds_a_note_open() {
        let drones = [
            (CvDestination::Gate1.index(), CvSource::Key.index()),
            (CvDestination::Gate2.index(), CvSource::Key.index()),
        ];
        report(conformance::check_release_silence(
            &Declared,
            &drones,
            |case: Case| {
                let mut routing = Routing::init();
                routing.present[case.target][case.source] = true;
                routing.amounts[case.target][case.source] = case.amount;
                routing.compact();
                let mut voice = Voice::new();
                voice.set_sample_rate(crate::MIN_SAMPLE_RATE);
                voice.set_topology(&routing);
                let mut p = Params {
                    decay_s: 0.02,
                    velocity: case.velocity,
                    wheel: 1.0,
                    bend: 1.0,
                    ..Params::default()
                };
                voice.set_channel_pressure(1.0);
                voice.note_on(id(case.key));
                // A tenth of a second held, then twenty to reach exact silence and `Inert`.
                for _ in 0..100 {
                    voice.process(&p, &routing);
                }
                voice.note_off(None, 0, case.key);
                p.wheel = 0.0;
                p.bend = 0.0;
                voice.set_channel_pressure(0.0);
                (0..20_000).any(|_| {
                    voice.process(&p, &routing) == 0.0 && voice.activity() == Activity::Inert
                })
            },
        ));
    }
}
