//! Task-oriented cards from the approved brief. Card order follows signal flow, never source-panel
//! position. Each CV destination lists the routes it carries — one row per route, with its depth
//! and a remove — and offers the sources it does not yet carry beneath them; each trigger
//! destination shows its three enables directly. Nothing is hidden behind a picker and nothing is
//! editor-only state: the rows come from parameter values, so a preset restores the whole patch.
//! There are no jacks, cables, or matrix.

use std::collections::HashMap;

use egui::Ui;
use mxm_mono_08_dsp::routing::PULSE_SOURCES;
use mxm_preset::ErasedParam;
use mxm_ui::control::{Size, Wave};
use mxm_ui::space::{MIN_TARGET, SPACE_2, SPACE_3};
use mxm_ui::theme::Tokens;
use mxm_ui::tree::{
    self, Height, Kind, Node, Share, beside_knob, group, leaf, pad, pad_all, row_gap, share, stack,
    stack_gap,
};
use nice_plug::prelude::ParamSetter;

use super::binding::{Bound, segmented_named, segmented_waves_named};
use super::visuals;
use crate::actions::TransientActions;
use crate::params::MxmMono08Params;
use crate::telemetry::Telemetry;

pub const SYNTH_TITLES: &[&str] = &[
    "Voice",
    "Mod oscillator",
    "Complex oscillator",
    // *LPG*, the name the mixer's knobs use too (the owner, 2026-09-24).
    "LPG 1",
    "LPG 2",
    // Output is in the app bar (design system §3.1), so this card is the mixer and the reverb.
    "Mixer and reverb",
];
pub const MOD_TITLES: &[&str] = &[
    "Envelope",
    "Clock",
    // Random's three trigger switches and the Inverter input's stack, which were a card each and
    // are a nested group each here. Neither carries enough to earn a header and a border of its
    // own, so the card is titled for both.
    "Random and inverter",
];

fn param<'a>(params: &'a MxmMono08Params, id: &str) -> &'a dyn ErasedParam {
    params
        .all_parameters()
        .into_iter()
        .find(|(candidate, _)| *candidate == id)
        .map(|(_, p)| p)
        .unwrap_or_else(|| panic!("no editor parameter {id}"))
}

fn description(id: &str) -> &'static str {
    match id {
        "complexfreq" => "The complex oscillator's pitch.",
        "complexendpoint" => "The shape Wave mix moves the sine toward.",
        "wavemix" => "Moves from a sine toward the selected Complex wave.",
        "timbre" => "Adds harmonics to the complex oscillator.",
        "complexkeyboard" => "Lets the key pitch play the complex oscillator.",
        "modfreq" => "The mod oscillator's speed: slow for movement, or a tone in the high range.",
        "modhigh" => "Moves the mod oscillator up into audible pitches, for FM and AM tones.",
        "modwave" => "Selects the mod oscillator's wave.",
        "modkeyboard" => "Lets the key pitch play the mod oscillator.",
        "modtype" => {
            "Whether the mod oscillator modulates the complex oscillator's amplitude or frequency."
        }
        "modindex" => "How strongly the mod oscillator acts.",
        "gate1mode" | "gate2mode" => "How this gate opens: in volume, in tone, or both.",
        "gate1level" | "gate2level" => "Opens this gate: brighter and louder together.",
        "gate2input" => "What LPG 2 plays.",
        "mix1" => "LPG 1's level in the mixer.",
        "mix2" => "LPG 2's level in the mixer.",
        "reverb" => "How much spring reverb.",
        "master" => "The instrument's output level.",
        "attack" => "Time for the envelope to rise to full level.",
        "duration" => "How long a one-shot envelope stays at full level.",
        "decay" => "Time for the envelope to return to zero.",
        "envmode" => "Whether the envelope runs its whole shape or holds while a key is down.",
        "pulserperiod" => "Time between the clock's beats.",
        "pulsersync" => super::binding::SYNC_DESCRIPTION,
        // The same thing the route *Clock trigger from Clock* does (`voice.rs`): both exist, and
        // removing either would change what saved presets and automation address.
        "pulserself" => "Keeps the clock running by itself once it has started.",
        "seqlength" => "How many of the five steps the sequencer plays, from two to five.",
        "portamento" => "Time for the pitch to glide from one key to the next.",
        "bendrange" => "How far pitch bend reaches, in semitones.",
        id if id.starts_with("seq") && id.ends_with("level") => {
            "This step's level, held while the step is active."
        }
        id if id.starts_with("seq") && id.ends_with("pulse") => {
            "Sends a step trigger when this step becomes active. A step without one still plays its \
             level."
        }
        // The keyboard route also holds the envelope's gate while a key is down (`voice.rs`), which
        // no other trigger route does.
        "pulse_envelope_keyboard" => {
            "Fires the envelope when a key goes down; in Sustained mode it holds while the key is held."
        }
        id if id.starts_with("pulse_random_") => {
            "Fires Random 1 to 4 from the named trigger: each trigger draws four new levels, held \
             until the next."
        }
        id if id.starts_with("pulse_") => {
            "Fires this module from the named trigger; any enabled trigger fires it."
        }
        id if id.starts_with("cv_") && id.ends_with("on") => {
            "Connects this source to the named destination."
        }
        id if id.starts_with("cv_") => {
            "Sets how far this source moves the named destination; below zero inverts it."
        }
        _ => "Adjusts this instrument parameter.",
    }
}

/// What a control **paints**, where its card already says the rest (design system §7.1): *Level*
/// and *Mode* on a card titled *LPG 1*, *Frequency* on *Complex oscillator*, *Attack* on
/// *Envelope*, and *LPG 1* on the mixer's knob (the owner, 2026-09-24: *"It is inside the Complex
/// oscillator card, so it is self evident"*). `None` paints the parameter's own name. The canonical
/// name — *Complex frequency*, *LPG 1 mix* — is always what a host, a tooltip and a screen reader
/// read.
fn panel_label(id: &str) -> Option<&'static str> {
    match id {
        "complexfreq" | "modfreq" => Some("Frequency"),
        "wavemix" => Some("Wave mix"),
        "timbre" => Some("Timbre"),
        "complexendpoint" | "modwave" => Some("Wave"),
        "complexkeyboard" | "modkeyboard" => Some("Key tracking"),
        "modindex" => Some("Depth"),
        "modhigh" => Some("High range"),
        "modtype" => Some("Type"),
        "attack" => Some("Attack"),
        "duration" => Some("Hold"),
        "decay" => Some("Decay"),
        "envmode" => Some("Mode"),
        "pulserperiod" => Some("Period"),
        "pulserself" => Some("Loop"),
        "seqlength" => Some("Length"),
        "gate1level" | "gate2level" => Some("Level"),
        "gate1mode" | "gate2mode" => Some("Mode"),
        "gate2input" => Some("Input"),
        "mix1" => Some("LPG 1"),
        "mix2" => Some("LPG 2"),
        _ => None,
    }
}

pub fn bound<'a>(params: &'a MxmMono08Params, id: &'static str) -> Bound<'a> {
    Bound {
        id,
        param: param(params, id),
        panel: panel_label(id).map(std::borrow::Cow::Borrowed),
        description: description(id),
        details: details_of(id),
        stepped: None,
        // A presence is a switch, not a signed depth: `cv_…on` shares the prefix but must not be
        // drawn or read as a bipolar amount.
        bipolar: id.starts_with("cv_") && !id.ends_with("on"),
        // The keyboard steps a pitch by semitones and octaves, the owner's ruling of 2026-09-23:
        // the bend reach in whole semitones, the complex oscillator in hertz. The modulation
        // oscillator is a pitch only in its high range; in the low one it is a rate and keeps its
        // own step. The high range is a ratio of the stored value, so a semitone of it is one here.
        law: match id {
            "bendrange" => mxm_preset::StepLaw::Semitones,
            "complexfreq" => mxm_preset::StepLaw::Hertz,
            "modfreq" if params.mod_high.value() => mxm_preset::StepLaw::Hertz,
            // A stage's level is a voltage, not a pitch, but through a pitch route reading
            // `+1.00 oct` the whole fader is one octave — so a twelfth of it is a semitone. Coarse
            // goes to the next semitone and fine moves 1 %, the owner's ruling of 2026-09-23: an
            // octave step had only the two ends to land on. Only coarse lands on the grid; the
            // stored level stays continuous, and a drag or a host still reaches anything between.
            id if STAGE_LEVEL_IDS.contains(&id) => mxm_preset::StepLaw::Voltage {
                octaves_per_unit: 1.0,
                fine: 0.01,
            },
            _ => mxm_preset::StepLaw::Own,
        },
    }
}

/// A stepped parameter's cells, **labelled by the parameter itself**: each cell is its option's
/// own formatted value, so a cell always reads what the host's automation list reads, and renaming
/// an option cannot leave a stale copy here.
fn selector(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmMono08Params,
    id: &'static str,
    setter: &ParamSetter<'_>,
) {
    selector_in(ui, tokens, params, id, setter, 0.0);
}

/// The mod oscillator's shapes, in `ModWaveKind`'s order. Its saw rises.
const MOD_WAVES: &[(Wave, &str)] = &[
    (Wave::Triangle, "Triangle"),
    (Wave::Square, "Square"),
    (Wave::RampUp, "Sawtooth"),
];
/// The complex oscillator's endpoints, in `ComplexEndpointKind`'s order. *Spike* is the 208's
/// narrow dip from a high rest (`Wave::Spike`).
const COMPLEX_WAVES: &[(Wave, &str)] = &[
    (Wave::Spike, "Spike"),
    (Wave::Square, "Square"),
    (Wave::Triangle, "Triangle"),
];

/// A wave parameter's picture table.
/// What each option of a stepped control does, one sentence per cell in the parameter's own order
/// (design system §7.3; the owner, 2026-09-27: the cells of a row do not share one sentence).
/// Empty for everything drawn as a knob, slider or toggle.
fn details_of(id: &str) -> &'static [&'static str] {
    match id {
        "modwave" => &[
            "Smooth: the gentlest movement.",
            "Jumps between two values.",
            "Ramps up, then jumps back.",
        ],
        "complexendpoint" => &[
            "Wave mix moves the sine toward a sharp, bright spike.",
            "Wave mix moves the sine toward a hollow square.",
            "Wave mix moves the sine toward a soft triangle.",
        ],
        "modtype" => &[
            "Moves the complex oscillator's volume: tremolo when slow, bell-like tones when fast.",
            "Moves its pitch: vibrato when slow, bright metallic tones when fast.",
        ],
        "gate1mode" | "gate2mode" => &[
            "Opens in volume only; the tone stays as bright.",
            "Opens as a filter: darker when closed, brighter as it opens.",
            "Both at once: louder and brighter together, the plucked sound.",
        ],
        "gate2input" => &[
            "LPG 2 plays the mod oscillator.",
            "LPG 2 plays what comes out of LPG 1, for a second stage.",
        ],
        "envmode" => &[
            "Each trigger runs the whole shape, however long the key is held.",
            "Stays at full level while the key is held, then decays.",
        ],
        "seqlength" => &[
            "Plays the first two steps.",
            "Plays the first three steps.",
            "Plays the first four steps.",
            "Plays all five steps.",
        ],
        _ => &[],
    }
}

fn waves_of(id: &str) -> &'static [(Wave, &'static str)] {
    match id {
        "modwave" => MOD_WAVES,
        "complexendpoint" => COMPLEX_WAVES,
        _ => panic!("{id} has no pictures"),
    }
}

/// A waveform parameter as pictures, the collection's rule wherever a picture exists (design
/// system §7.3). The names are what the parameter's own options say, so hosts and screen readers
/// read the same words the picture replaces.
fn waves(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmMono08Params,
    id: &'static str,
    options: &[(Wave, &str)],
    setter: &ParamSetter<'_>,
) {
    segmented_waves_named(
        ui,
        tokens,
        id,
        param(params, id),
        panel_label(id),
        options,
        None,
        details_of(id),
        setter,
    );
}

/// Every option of a stepped parameter, as its own formatted text.
fn option_labels(params: &MxmMono08Params, id: &str) -> Vec<String> {
    let param = param(params, id);
    let last = param
        .steps()
        .unwrap_or_else(|| panic!("{id} is not stepped"));
    (0..=last)
        .map(|option| param.format(option as f32 / last as f32))
        .collect()
}

fn selector_in(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmMono08Params,
    id: &'static str,
    setter: &ParamSetter<'_>,
    cell: f32,
) {
    let labels = option_labels(params, id);
    let options: Vec<&str> = labels.iter().map(String::as_str).collect();
    segmented_named(
        ui,
        tokens,
        id,
        param(params, id),
        panel_label(id),
        &options,
        details_of(id),
        setter,
        cell,
    );
}

fn boolean(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmMono08Params,
    id: &'static str,
    setter: &ParamSetter<'_>,
) {
    let param = param(params, id);
    super::binding::toggle_labelled(
        ui,
        tokens,
        id,
        param,
        panel_label(id).unwrap_or(param.name()),
        description(id),
        setter,
        0.0,
    );
}

fn caption(ui: &mut Ui, tokens: &Tokens, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .color(tokens.text_secondary)
            .text_style(mxm_ui::typography::caption_style(ui.style())),
    );
}

/// One destination's routes, drawn by the shared routing widget.
///
/// Every present pair is a row — its depth, and a remove — and the affordance beneath offers the
/// sources this destination does not yet carry. **There is no picker and no editor-side state**:
/// the rows come from parameter values alone, so a preset fully determines what the panel shows,
/// and adding or removing a route is one parameter write rather than a hidden selection.
///
/// **No live bar on a row** — the owner's ruling, 2026-09-16. `visuals::source_levels` already
/// shows every source's current value on the Modulators cards and each row shows its own depth
/// moving, so a per-route meter repeated what the panel says elsewhere and spent a row of height
/// saying it. `visuals::route_activity` retired with the picker it belonged to.
///
/// What is painted is [`DESTINATION_PANEL_NAMES`](mxm_mono_08_dsp::routing::DESTINATION_PANEL_NAMES),
/// which drops the prefix the card already carries; the canonical name, which the host's
/// automation list and a screen reader use, is always the destination's own.
fn route_stack(
    ui: &mut Ui,
    tokens: &Tokens,
    destination: usize,
    params: &MxmMono08Params,
    setter: &ParamSetter<'_>,
    entries: &mut HashMap<&'static str, Option<String>>,
) {
    let group = params.routes.cv(destination);
    // One shared buffer for every row: only one control is ever being typed into.
    let entry = entries.entry("routes").or_default();
    mxm_modulation_params::ui::stack_with_law(
        ui,
        tokens,
        mxm_mono_08_dsp::routing::DESTINATION_NAMES[destination],
        mxm_mono_08_dsp::routing::DESTINATION_PANEL_NAMES[destination],
        &group.routes(destination),
        entry,
        setter,
        &|route| pitch_law(destination, route),
    );
}

/// How the keyboard steps a route's depth: by semitones and octaves where it reads as a pitch
/// interval, and by its own step everywhere else.
///
/// The owner, 2026-09-23: *"A lot of the pitch sliders in the mono 08 does not jump octaves and
/// semitones with the keys. Even though they use those as display values."* Complex pitch and Mod
/// pitch read in semitones, and the reach is the very number the reading multiplies by
/// (`routes::reach`) — a network pair's five or eight octaves, a performance pair's twelve
/// semitones, Key's per octave — so a semitone on the keyboard is a semitone on the reading.
fn pitch_law(destination: usize, route: &mxm_modulation_params::Route<'_>) -> mxm_preset::StepLaw {
    use mxm_mono_08_dsp::routing::{SOURCE_NAMES, network_octaves};
    if network_octaves(destination).is_none() {
        return mxm_preset::StepLaw::Own;
    }
    let Some(source) = SOURCE_NAMES.iter().position(|name| *name == route.source) else {
        return mxm_preset::StepLaw::Own;
    };
    mxm_preset::StepLaw::Interval {
        octaves_per_unit: f64::from(crate::routes::reach(destination, source).full / 12.0),
    }
}

/// The *Inverter input* destination's stack.
///
/// Its group is an [`InverterRoutes`](crate::routes::InverterRoutes), not a `CvRoutes`, because it
/// carries fourteen sources rather than fifteen — so it is drawn beside the ten rather than through
/// [`route_stack`]. The stack itself takes a slice and neither knows nor cares how long it is.
fn inverter_stack(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmMono08Params,
    setter: &ParamSetter<'_>,
    entries: &mut HashMap<&'static str, Option<String>>,
) {
    let entry = entries.entry("routes").or_default();
    mxm_modulation_params::ui::stack(
        ui,
        tokens,
        mxm_mono_08_dsp::routing::DESTINATION_NAMES[crate::routes::INVERTER],
        mxm_mono_08_dsp::routing::DESTINATION_PANEL_NAMES[crate::routes::INVERTER],
        &params.routes.inverter_input.routes(),
        entry,
        setter,
    );
}

/// The sequence is five steps, fixed. `Sequence length` chooses how many of them run. The code
/// calls a step a *stage*, after the hardware.
const STAGES: usize = 5;
/// The fader column's whole height — name line, track and value. `slider_vertical` spends the name
/// and value lines out of it, so the track is this less about 46 points.
///
/// **The faders are the whole of their card, and the card is sized around them** (the owner,
/// 2026-09-23). The five held voltages are a main feature of this instrument, not a detail: a short
/// track is hard to aim and makes the sequencer read as a footnote to the switches under it. That
/// is why the run is a card of its own — with the length selector and the routes in it, a fader
/// this tall pushed the card past any page it could share.
const STAGE_FADER_HEIGHT: f32 = 300.0;

const STAGE_LEVEL_IDS: [&str; STAGES] = [
    "seq1level",
    "seq2level",
    "seq3level",
    "seq4level",
    "seq5level",
];
const STAGE_PULSE_IDS: [&str; STAGES] = [
    "seq1pulse",
    "seq2pulse",
    "seq3pulse",
    "seq4pulse",
    "seq5pulse",
];

// ---------------------------------------------------------------------------------------------
// The cards, as trees (plans/plan-layout-tree.md). Each card is described once — `card` — and that
// one description is both measured (its floor and its height) and drawn, leaf by leaf, through the
// bindings above (`paint`). Nothing is typed and nothing is drawn to learn a size.
// ---------------------------------------------------------------------------------------------

/// What a leaf of this editor's cards draws. Hashed by what it names, which is also what keeps its
/// widget ids stable when a route appears above it.
#[derive(Clone, Debug, Hash)]
pub enum Leaf {
    Knob(&'static str, Size),
    Toggle(&'static str),
    /// A control's tempo sync, the quarter note beside it.
    Picture(&'static str),
    Waves(&'static str),
    Selector(&'static str),
    Caption(String),
    Warning(String),
    Routes(usize),
    InverterRoutes,
    /// A trigger destination's enable, by destination and source.
    PulseToggle(usize, usize),
    /// The flash beside it.
    PulseFlash(usize, usize),
    Waveform,
    Gate(usize),
    SourceLevels,
    FireOnce,
    StageMarker(usize),
    StageFader(usize),
    StagePulse(usize),
}

/// The knob columns every card here draws: `ui.columns` inside a width capped at
/// `Σ max(diameter + SPACE_5, 76)`, so each column is that sum's share, gaps included.
fn knobs(ui: &Ui, params: &MxmMono08Params, knobs: &[(&'static str, Size)]) -> Node<Leaf> {
    mxm_ui::tree::knob_row(
        ui,
        knobs
            .iter()
            .map(|(id, size)| (*size, knob(params, id, *size)))
            .collect(),
    )
}

fn knob(params: &MxmMono08Params, id: &'static str, size: Size) -> Node<Leaf> {
    let param = param(params, id);
    leaf(
        Leaf::Knob(id, size),
        Kind::Knob {
            name: panel_label(id).unwrap_or(param.name()).to_owned(),
            // A syncable control's column holds its free readings and its divisions.
            widest: if id == "pulserperiod" {
                super::binding::synced_widest(param, crate::params::PULSER_SYNC.span)
            } else {
                mxm_ui::control::widest_value(|n| param.format(n as f32))
            },
            size,
            column: 0.0,
        },
    )
}

fn toggle_leaf(params: &MxmMono08Params, id: &'static str) -> Node<Leaf> {
    leaf(
        Leaf::Toggle(id),
        Kind::Toggle {
            label: panel_label(id)
                .unwrap_or(param(params, id).name())
                .to_owned(),
        },
    )
}

fn waves_leaf(params: &MxmMono08Params, id: &'static str) -> Node<Leaf> {
    leaf(
        Leaf::Waves(id),
        Kind::Waves {
            label: Some(
                panel_label(id)
                    .unwrap_or(param(params, id).name())
                    .to_owned(),
            ),
            count: waves_of(id).len(),
            marks: Vec::new(),
            beside: None,
        },
    )
}

fn selector_leaf(params: &MxmMono08Params, id: &'static str) -> Node<Leaf> {
    leaf(
        Leaf::Selector(id),
        Kind::Segmented {
            label: panel_label(id)
                .unwrap_or(param(params, id).name())
                .to_owned(),
            options: option_labels(params, id),
            beside: None,
        },
    )
}

fn caption_leaf(text: &str) -> Node<Leaf> {
    tree::caption(Leaf::Caption(text.to_owned()), text)
}

/// A destination's routes, `SPACE_3` below what precedes it. A route stack is a composite with a
/// rule of its own — its floor is every route revealed — so it states its size (`stack_size`).
fn routes_leaf(ui: &Ui, params: &MxmMono08Params, destination: usize) -> Node<Leaf> {
    let size = mxm_modulation_params::ui::stack_size(
        ui,
        mxm_mono_08_dsp::routing::DESTINATION_PANEL_NAMES[destination],
        &params.routes.cv(destination).routes(destination),
    );
    pad(SPACE_3, stack_leaf(Leaf::Routes(destination), size))
}

fn inverter_leaf(ui: &Ui, params: &MxmMono08Params) -> Node<Leaf> {
    let size = mxm_modulation_params::ui::stack_size(
        ui,
        mxm_mono_08_dsp::routing::DESTINATION_PANEL_NAMES[crate::routes::INVERTER],
        &params.routes.inverter_input.routes(),
    );
    pad(SPACE_3, stack_leaf(Leaf::InverterRoutes, size))
}

fn stack_leaf(key: Leaf, size: egui::Vec2) -> Node<Leaf> {
    leaf(
        key,
        Kind::Custom {
            min_width: size.x,
            height: Height::Fixed(size.y),
            fills: true,
        },
    )
}

/// One trigger destination's three enables, each with the flash that shows it firing, in a group
/// `SPACE_3` below what precedes it. **Three switches, always visible** (plan D6): a trigger has no
/// depth, so presence is the whole of it. They share the widest label's width, and each flash is
/// centred on its switch, as the row's own alignment put it.
fn pulses_leaf(ui: &Ui, destination: usize) -> Node<Leaf> {
    let rows = (0..PULSE_SOURCES)
        .map(|source| {
            row_gap(
                ui.spacing().item_spacing.x,
                vec![
                    leaf(
                        Leaf::PulseToggle(destination, source),
                        Kind::Toggle {
                            label: pulse_label(source),
                        },
                    ),
                    pad(
                        (MIN_TARGET - visuals::PULSE_FLASH) / 2.0,
                        leaf(
                            Leaf::PulseFlash(destination, source),
                            Kind::Custom {
                                min_width: visuals::PULSE_FLASH,
                                height: Height::Fixed(visuals::PULSE_FLASH),
                                fills: false,
                            },
                        ),
                    ),
                ],
            )
        })
        .collect();
    pad(
        SPACE_3,
        group(vec![share(Share::Toggles, stack_gap(SPACE_2, rows))]),
    )
}

/// What a trigger switch paints: the card already names the module it fires (design system §7.1).
fn pulse_label(source: usize) -> String {
    format!("Trigger from {}", crate::routes::PULSE_SOURCE_NAMES[source])
}

fn display(key: Leaf, height: f32) -> Node<Leaf> {
    leaf(
        key,
        Kind::Custom {
            min_width: 0.0,
            height: Height::Fixed(height),
            fills: true,
        },
    )
}

/// Card `index`'s body, as a tree, from the parameters and the few telemetry and editor facts that
/// change what a card holds: a caption's text, and whether the Once queue has refused a request.
pub fn card(ui: &Ui, index: usize, params: &MxmMono08Params, telemetry: &Telemetry) -> Node<Leaf> {
    let gap = ui.spacing().item_spacing.x;
    match index {
        0 => stack(vec![
            knobs(
                ui,
                params,
                &[
                    ("portamento", Size::Standard),
                    ("bendrange", Size::Standard),
                ],
            ),
            routes_leaf(ui, params, 7),
        ]),
        1 => stack(vec![
            // The range switch sits in the knobs' row (owner, 2026-09-23): it changes what the
            // frequency beside it reads.
            row_gap(
                gap,
                vec![
                    knobs(
                        ui,
                        params,
                        &[("modfreq", Size::Standard), ("modindex", Size::Standard)],
                    ),
                    pad_all(
                        0.0,
                        SPACE_3,
                        0.0,
                        beside_knob(Size::Standard, toggle_leaf(params, "modhigh")),
                    ),
                ],
            ),
            pad(SPACE_3, waves_leaf(params, "modwave")),
            selector_leaf(params, "modtype"),
            toggle_leaf(params, "modkeyboard"),
            routes_leaf(ui, params, 1),
            routes_leaf(ui, params, 3),
        ]),
        2 => stack(vec![
            display(Leaf::Waveform, visuals::WAVEFORM_HEIGHT),
            pad(
                SPACE_3,
                knobs(
                    ui,
                    params,
                    &[
                        ("complexfreq", Size::Standard),
                        ("wavemix", Size::Primary),
                        ("timbre", Size::Primary),
                    ],
                ),
            ),
            pad(SPACE_3, waves_leaf(params, "complexendpoint")),
            toggle_leaf(params, "complexkeyboard"),
            routes_leaf(ui, params, 0),
            routes_leaf(ui, params, 2),
        ]),
        3 | 4 => {
            let gate = index - 3;
            let (mode, level) = if gate == 0 {
                ("gate1mode", "gate1level")
            } else {
                ("gate2mode", "gate2level")
            };
            let mut body = vec![
                display(Leaf::Gate(gate), visuals::GATE_HEIGHT),
                pad(SPACE_3, knobs(ui, params, &[(level, Size::Primary)])),
                // Each selector its own width: Mode on both cards is the same three cells, so it is
                // drawn the same size on both (the owner, 2026-09-24), rather than widened on LPG 2
                // to match the *Mod oscillator* cell of the Input below it.
                selector_leaf(params, mode),
            ];
            if gate == 1 {
                body.push(selector_leaf(params, "gate2input"));
            }
            body.push(routes_leaf(ui, params, if gate == 0 { 4 } else { 5 }));
            stack(body)
        }
        5 => stack(vec![
            knobs(
                ui,
                params,
                &[
                    ("mix1", Size::Standard),
                    ("mix2", Size::Standard),
                    ("reverb", Size::Standard),
                ],
            ),
            // The standard Amplitude: a factor on the mix, before the spring.
            routes_leaf(ui, params, crate::routes::AMPLITUDE),
        ]),
        6 => stack(vec![
            leaf(
                Leaf::SourceLevels,
                Kind::Custom {
                    min_width: visuals::source_levels_size(ui, &SOURCE_LEVEL_NAMES).x,
                    height: Height::Fixed(visuals::source_levels_size(ui, &SOURCE_LEVEL_NAMES).y),
                    fills: true,
                },
            ),
            pad(
                SPACE_3,
                knobs(
                    ui,
                    params,
                    &[
                        ("attack", Size::Standard),
                        ("duration", Size::Standard),
                        ("decay", Size::Standard),
                    ],
                ),
            ),
            selector_leaf(params, "envmode"),
            pulses_leaf(ui, 0),
        ]),
        7 => {
            let (_, _, period) = telemetry.control_levels();
            let (fired, cancelled, rejected) = telemetry.once_counts();
            let mut body = vec![
                caption_leaf(&format!("Live period: {period:.3} s")),
                // The period's tempo sync is the quarter note beside it
                // (`plans/plan-tempo-sync-controls.md`).
                mxm_ui::tree::row_gap(
                    ui.spacing().item_spacing.x,
                    vec![
                        knobs(ui, params, &[("pulserperiod", Size::Primary)]),
                        mxm_ui::tree::switch_beside_knob(
                            Size::Primary,
                            leaf(Leaf::Picture("pulsersync"), Kind::SyncToggle),
                        ),
                    ],
                ),
                toggle_leaf(params, "pulserself"),
                leaf(
                    Leaf::FireOnce,
                    Kind::Button {
                        label: FIRE_ONCE.to_owned(),
                        min: egui::Vec2::ZERO,
                        fills: false,
                    },
                ),
            ];
            if ui
                .data(|d| d.get_temp::<bool>(egui::Id::new(ONCE_FULL)))
                .unwrap_or(false)
            {
                body.push(leaf(
                    Leaf::Warning(QUEUE_FULL.to_owned()),
                    Kind::Text {
                        text: QUEUE_FULL.to_owned(),
                        font: tree::Font::Body,
                        flow: tree::Flow::Wrap,
                    },
                ));
            }
            body.push(caption_leaf(&format!(
                "Fired {fired} · cancelled {cancelled} · rejected {rejected}"
            )));
            body.push(routes_leaf(ui, params, 6));
            body.push(pulses_leaf(ui, 1));
            stack(body)
        }
        // Random and inverter: two utilities that were a card each. Each is a nested group, and
        // nothing gains a sub-heading — design system §3.3 gives a group no header, so each cluster
        // is named by its own controls.
        8 => stack(vec![pulses_leaf(ui, 3), inverter_leaf(ui, params)]),
        // What drives the run: how much of it plays, and what advances it. A card rather than a
        // header because §3.3 gives a group no header and this carries a route stack that grows.
        9 => stack(vec![
            selector_leaf(params, "seqlength"),
            routes_leaf(ui, params, 8),
            pulses_leaf(ui, 2),
        ]),
        // The run itself: five steps, side by side, and nothing else — one sequencer with five
        // steps, not five sequencers. `Columns` stretched across the card gives every step the same
        // width, which is what makes the five faders comparable at a glance. The fader says only
        // the step number and the switch is a picture of one trigger pulse (the owner, 2026-09-24);
        // both keep their canonical names — `Step 3 level`, `Step 3 trigger` — for the host.
        _ => Node::Columns {
            gap,
            column: None,
            stretch: true,
            children: (0..STAGES)
                .map(|stage| {
                    let level = param(params, STAGE_LEVEL_IDS[stage]);
                    stack(vec![
                        tree::center(leaf(
                            Leaf::StageMarker(stage),
                            Kind::Custom {
                                min_width: visuals::MARKER.x,
                                height: Height::Fixed(visuals::MARKER.y),
                                fills: false,
                            },
                        )),
                        leaf(
                            Leaf::StageFader(stage),
                            Kind::VerticalSlider {
                                label: format!("{}", stage + 1),
                                widest: mxm_ui::control::widest_value(|n| level.format(n as f32)),
                                height: STAGE_FADER_HEIGHT,
                                fills: true,
                            },
                        ),
                        tree::center(leaf(Leaf::StagePulse(stage), Kind::PictureToggle)),
                    ])
                })
                .collect(),
        },
    }
}

const FIRE_ONCE: &str = "Fire once";
const QUEUE_FULL: &str = "Request queue full";
/// Where the editor remembers that the Once queue refused a request.
pub(crate) const ONCE_FULL: &str = "mxm-mono-08-once-full";
/// The Envelope card's live sources, in the order `source_levels` draws them.
const SOURCE_LEVEL_NAMES: [&str; 3] = ["Envelope", "Mod oscillator", "Sequencer"];

/// Everything a leaf draws with: the parameters and their host, the live telemetry, and the
/// snapshots taken once before the frame (`plugins/AGENTS.md`: destructive telemetry is read once).
pub struct Live<'a, 'b> {
    pub params: &'a MxmMono08Params,
    pub telemetry: &'a Telemetry,
    pub actions: &'a TransientActions,
    pub wake: &'a dyn Fn(),
    pub setter: &'a ParamSetter<'b>,
    pub entries: &'a mut HashMap<&'static str, Option<String>>,
    pub waveform: &'a [f32],
    pub pulses: [bool; PULSE_SOURCES],
    /// The active step and whether it fired this frame.
    pub stage: (usize, bool),
}

/// Draws one leaf, in the `Ui` the tree bounded to `rect`, through the bindings above — so the
/// controls, their gestures and their names are exactly what they were.
pub fn paint(ui: &mut Ui, tokens: &Tokens, leaf: &Leaf, rect: egui::Rect, live: &mut Live<'_, '_>) {
    let params = live.params;
    match *leaf {
        // Synced to a tempo, the clock period reads its division; the host still reads its time.
        Leaf::Knob(id, size) => {
            let bound = bound(params, id);
            let division = {
                use nice_plug::prelude::Param as _;
                let synced: Option<(bool, &nice_plug::prelude::FloatParam, mxm_tempo::Ladder)> =
                    match id {
                        "pulserperiod" => Some((
                            params.pulser_sync.value(),
                            &params.pulser_period,
                            crate::params::PULSER_SYNC,
                        )),
                        _ => None,
                    };
                synced
                    .filter(|(on, _, _)| *on)
                    .and_then(|(_, param, ladder)| {
                        ladder.shown(
                            param.unmodulated_normalized_value(),
                            live.telemetry.tempo.get(),
                            f64::from(param.preview_plain(0.0)),
                            f64::from(param.preview_plain(1.0)),
                        )
                    })
            };
            match division {
                Some(division) => bound.knob_with_reading(
                    ui,
                    tokens,
                    live.setter,
                    size,
                    rect.width(),
                    live.entries,
                    division.label(),
                ),
                None => bound.knob(ui, tokens, live.setter, size, rect.width(), live.entries),
            }
        }
        Leaf::Picture(id) => {
            super::binding::sync_picture(ui, tokens, id, param(params, id), live.setter);
        }
        Leaf::Toggle(id) => boolean(ui, tokens, params, id, live.setter),
        Leaf::Waves(id) => waves(ui, tokens, params, id, waves_of(id), live.setter),
        Leaf::Selector(id) => selector(ui, tokens, params, id, live.setter),
        Leaf::Caption(ref text) => caption(ui, tokens, text),
        Leaf::Warning(ref text) => {
            ui.label(egui::RichText::new(text).color(tokens.danger));
        }
        Leaf::Routes(destination) => {
            route_stack(ui, tokens, destination, params, live.setter, live.entries);
        }
        Leaf::InverterRoutes => inverter_stack(ui, tokens, params, live.setter, live.entries),
        Leaf::PulseToggle(destination, source) => {
            let id = crate::routes::PULSE_IDS[destination][source];
            super::binding::toggle_labelled(
                ui,
                tokens,
                id,
                param(params, id),
                &pulse_label(source),
                description(id),
                live.setter,
                0.0,
            );
        }
        Leaf::PulseFlash(destination, source) => {
            let group = params.routes.pulses()[destination];
            visuals::pulse_route(
                ui,
                tokens,
                crate::routes::PULSE_SOURCE_NAMES[source],
                group.each()[source].value(),
                live.pulses[source],
            );
        }
        Leaf::Waveform => visuals::waveform(ui, tokens, live.waveform),
        Leaf::Gate(gate) => visuals::gate(
            ui,
            tokens,
            if gate == 0 {
                "LPG 1 response"
            } else {
                "LPG 2 response"
            },
            live.telemetry.gate_levels()[gate],
        ),
        Leaf::SourceLevels => {
            let (modulation, envelope, _) = live.telemetry.control_levels();
            visuals::source_levels(
                ui,
                tokens,
                &[
                    (SOURCE_LEVEL_NAMES[0], envelope, tokens.mod_envelope),
                    (SOURCE_LEVEL_NAMES[1], modulation.abs(), tokens.mod_lfo),
                    (
                        SOURCE_LEVEL_NAMES[2],
                        live.telemetry.sequence().1,
                        tokens.mod_random,
                    ),
                ],
            );
        }
        Leaf::FireOnce => {
            let button = ui.button(FIRE_ONCE).on_hover_text("Fires the clock once.");
            if button.clicked() {
                let rejected = live.actions.submit_once_from_editor(live.wake).is_err();
                ui.data_mut(|d| d.insert_temp(egui::Id::new(ONCE_FULL), rejected));
            }
        }
        Leaf::StageMarker(stage) => {
            let (active, pulsed) = live.stage;
            let value = param(params, STAGE_LEVEL_IDS[stage]).normalised();
            visuals::stage(
                ui,
                tokens,
                stage,
                active == stage,
                pulsed && active == stage,
                value,
            );
        }
        Leaf::StageFader(stage) => bound(params, STAGE_LEVEL_IDS[stage]).slider_vertical(
            ui,
            tokens,
            live.setter,
            live.entries,
            &format!("{}", stage + 1),
            rect.width(),
            STAGE_FADER_HEIGHT,
        ),
        Leaf::StagePulse(stage) => super::binding::toggle_picture(
            ui,
            tokens,
            STAGE_PULSE_IDS[stage],
            param(params, STAGE_PULSE_IDS[stage]),
            Wave::Trigger,
            description(STAGE_PULSE_IDS[stage]),
            live.setter,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wave table is positional, so it must have exactly as many pictures as its parameter has
    /// options — the check `mxm-mono-00`'s `every_switchs_option_list_matches_its_parameter` makes
    /// for its own. The names must be the parameter's own option text, so the accessible name a
    /// picture carries is the word a host shows.
    #[test]
    fn every_wave_table_matches_its_parameter() {
        let params = MxmMono08Params::default();
        for (id, table) in [("modwave", MOD_WAVES), ("complexendpoint", COMPLEX_WAVES)] {
            let labels = option_labels(&params, id);
            let names: Vec<&str> = table.iter().map(|(_, name)| *name).collect();
            assert_eq!(names, labels, "{id}'s pictures and its options disagree");
        }
    }
}
