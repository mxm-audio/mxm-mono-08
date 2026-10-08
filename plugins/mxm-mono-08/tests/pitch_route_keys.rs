//! mxm-mono-08's pitch routes and sequencer stages step musically from the keyboard.
//!
//! The owner, 2026-09-23: *"A lot of the pitch sliders in the mono 08 does not jump octaves and
//! semitones with the keys. Even though they use those as display values."* A route into Complex
//! pitch reads in semitones (octaves until the modulation standard); a press lands on the next
//! semitone (fine) and the next octave (coarse), through the shipped panel. The stage faders
//! followed the same day — *"so when this is set to 1 octave, there is a 1:1 on the pitches"* — and
//! since the whole fader is one octave through a route reading `+12.00 st`, a stage steps to the
//! next semitone (coarse) and by exactly 1 % (fine).
//!
//! Under the keyboard language a value changes with VALUE + the arrows, COARSE for the bigger step,
//! and OUT keeps it: W, S, the arrow and Tab in the default keymap. A bare arrow moves the cursor.

use std::collections::HashMap;

use egui::{Event, Key, Modifiers};
use mxm_mono_08::actions::TransientActions;
use mxm_mono_08::editor::{PresetUi, panel};
use mxm_mono_08::params::MxmMono08Params;
use mxm_mono_08::telemetry::Telemetry;
use nice_plug::prelude::*;

/// A host that applies what the editor asks for, as the real one does.
#[derive(Default)]
struct ApplyingHost;

impl nice_plug::context::gui::GuiContextInner for ApplyingHost {
    // A test double has no host to ask for a restart (nice-plug 0.4).
    fn request_restart(&self) {}
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    unsafe fn raw_begin_set_parameter(&self, _param: nice_plug::params::internals::ParamPtr) {}
    unsafe fn raw_set_parameter_normalized(
        &self,
        param: nice_plug::params::internals::ParamPtr,
        value: f32,
    ) {
        unsafe {
            param._internal_set_normalized_value(value);
        }
    }
    unsafe fn raw_end_set_parameter(&self, _param: nice_plug::params::internals::ParamPtr) {}
    fn get_state(&self) -> PluginState {
        PluginState {
            version: String::new(),
            params: Default::default(),
            fields: Default::default(),
        }
    }
    fn set_state(&self, _: PluginState) {}
}

/// The shipped panel, one frame at a time.
struct Rig {
    ctx: egui::Context,
    view: usize,
    entries: HashMap<&'static str, Option<String>>,
    presets: PresetUi,
    nav: mxm_ui::navigation::State,
}

impl Rig {
    fn new(params: &MxmMono08Params) -> Self {
        let ctx = egui::Context::default();
        mxm_ui::theme::apply(&ctx);
        mxm_ui::typography::apply(&ctx);
        Self {
            ctx,
            view: 0,
            entries: HashMap::new(),
            presets: PresetUi::at(mxm_mono_08::preset::Library::at(None), params),
            nav: mxm_ui::navigation::State::default(),
        }
    }

    fn frame(&mut self, params: &MxmMono08Params, setter: &ParamSetter<'_>, events: Vec<Event>) {
        let telemetry = Telemetry::default();
        let actions = TransientActions::default();
        let input = egui::RawInput {
            // Large enough that every card is on the one page.
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(3840.0, 2160.0),
            )),
            events,
            ..Default::default()
        };
        let (view, entries, presets, nav) = (
            &mut self.view,
            &mut self.entries,
            &mut self.presets,
            &mut self.nav,
        );
        let mut output = self.ctx.run_ui(input, |ui| {
            panel(
                ui,
                params,
                &telemetry,
                &actions,
                &|| {},
                setter,
                view,
                entries,
                presets,
                &[],
                nav,
            );
        });
        output.textures_delta.clear();
    }

    /// Settles the panel, then takes the cursor to `id` by clicking it, as a person would.
    fn take(&mut self, params: &MxmMono08Params, setter: &ParamSetter<'_>, id: &str) {
        for _ in 0..4 {
            self.frame(params, setter, Vec::new());
        }
        let at = mxm_ui::navigation::spots(&self.ctx)
            .into_iter()
            .find(|spot| spot.key == id)
            .unwrap_or_else(|| panic!("{id} is on screen"))
            .rect
            .center();
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.frame(
            params,
            setter,
            vec![Event::PointerMoved(at), button(true), button(false)],
        );
        self.frame(params, setter, Vec::new());
        self.frame(params, setter, Vec::new());
        assert_eq!(self.nav.parameter(), Some(id), "the click took the cursor");
    }

    /// Steps the parameter under the cursor as these tests read the arrows: ← and → the fine step,
    /// ↑ and ↓ the coarse one. VALUE, COARSE for ↑ and ↓, the arrow, then OUT to keep it.
    fn press(&mut self, params: &MxmMono08Params, setter: &ParamSetter<'_>, key: Key) {
        let mut keys = vec![Key::W];
        if matches!(key, Key::ArrowUp | Key::ArrowDown) {
            keys.push(Key::S);
        }
        keys.extend([key, Key::Tab]);
        for key in keys {
            self.tap(params, setter, key);
        }
    }

    /// One key down and up again.
    fn tap(&mut self, params: &MxmMono08Params, setter: &ParamSetter<'_>, key: Key) {
        let event = |pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        self.frame(params, setter, vec![event(true), event(false)]);
    }
}

#[test]
fn a_pitch_route_steps_to_the_next_semitone_and_octave() {
    const ID: &str = "cv_complexpitch_envelope";

    let params = MxmMono08Params::default();
    let host = ApplyingHost;
    let setter = ParamSetter::new(&host);
    let routes = params.routes.cv(0).routes(0);
    let route = routes
        .iter()
        .find(|route| route.amount_id == ID)
        .expect("Complex pitch from Envelope is a route");
    mxm_modulation_params::add(route, &setter);

    let mut rig = Rig::new(&params);
    rig.take(&params, &setter, ID);

    // 3.6 semitones, set as a host would: between two of them.
    let set = |text: &str| {
        let normalised = route.amount.parse(text).expect("a reading parses");
        route.amount.begin(&setter);
        route.amount.set(&setter, normalised);
        route.amount.end(&setter);
    };
    set("3.6");
    rig.frame(&params, &setter, Vec::new());
    assert_eq!(route.amount.text(), "+3.60 st");

    let mut reads = Vec::new();
    for key in [
        Key::ArrowRight, // the next semitone up: 4 st
        Key::ArrowLeft,  // back one: 3 st
        Key::ArrowUp,    // the next octave: 1 oct
        Key::ArrowUp,    // one more: 2 oct
        Key::ArrowRight, // 25 st
        Key::ArrowDown,  // the octave below 25 st: 2 oct
    ] {
        rig.press(&params, &setter, key);
        reads.push(route.amount.text());
    }
    assert_eq!(
        reads,
        [
            "+4.00 st",
            "+3.00 st",
            "+12.00 st",
            "+24.00 st",
            "+25.00 st",
            "+24.00 st"
        ]
    );
}

/// A stage steps a semitone coarse — a twelfth of its range, through a one-octave pitch route —
/// and exactly 1 % fine. A coarse press lands on the next whole semitone, so a fine detune does not
/// ride along: 2 % and then a coarse press up is one semitone, not one and 2 %.
#[test]
fn a_stage_steps_a_semitone_coarse_and_a_percent_fine() {
    const ID: &str = "seq1level";

    let params = MxmMono08Params::default();
    let host = ApplyingHost;
    let setter = ParamSetter::new(&host);

    let mut rig = Rig::new(&params);
    rig.take(&params, &setter, ID);

    // The bottom, set as a host would: the click that took the cursor also moved the fader.
    let stage = &params.sequence_1_level;
    setter.begin_set_parameter(stage);
    setter.set_parameter(stage, 0.0);
    setter.end_set_parameter(stage);
    rig.frame(&params, &setter, Vec::new());

    let semitone = 100.0 / 12.0;
    let mut percents = Vec::new();
    let mut expected = Vec::new();
    for (key, want) in [
        (Key::ArrowRight, 1.0),                 // 1 %
        (Key::ArrowRight, 2.0),                 // 2 %
        (Key::ArrowUp, semitone),               // the next semitone, not one and 2 %
        (Key::ArrowUp, 2.0 * semitone),         // one more
        (Key::ArrowLeft, 2.0 * semitone - 1.0), // 1 % below it
        (Key::ArrowDown, semitone),             // the semitone below that
        (Key::ArrowDown, 0.0),                  // the bottom
    ] {
        rig.press(&params, &setter, key);
        percents.push(stage.value() * 100.0);
        expected.push(want);
    }
    // Twelve semitones are the whole range, and the top holds.
    for _ in 0..13 {
        rig.press(&params, &setter, Key::ArrowUp);
    }
    percents.push(stage.value() * 100.0);
    expected.push(100.0);

    for (read, want) in percents.iter().zip(&expected) {
        assert!(
            (read - want).abs() < 1e-3,
            "{percents:?} is not {expected:?}"
        );
    }
}
