//! Automated proof for the shipped editor surface. Geometry comes from `flow::drawn`, interaction
//! from AccessKit, and host gestures from a setter that applies and records every callback.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use egui::{Rect, ThemePreference, vec2};
use kittest::Queryable;
use nice_plug::params::internals::ParamPtr;
use nice_plug::prelude::{ParamSetter, PluginApi, PluginState};

use crate::telemetry::VoiceSnapshot;
use mxm_mono_08_dsp::routing::{CV_SOURCES, PULSE_SOURCES};
use mxm_mono_08_dsp::voice::Activity;

use super::*;

#[derive(Default)]
struct ApplyingHost(Mutex<Vec<(&'static str, String)>>);

impl ApplyingHost {
    fn record(&self, action: &'static str, param: ParamPtr) {
        self.0
            .lock()
            .unwrap()
            .push((action, unsafe { param.name() }.to_owned()));
    }
}

fn set_normalised(params: &MxmMono08Params, id: &str, value: f32) {
    let (_, parameter, _) = params
        .param_map()
        .into_iter()
        .find(|(candidate, _, _)| candidate == id)
        .unwrap();
    unsafe {
        parameter._internal_set_normalized_value(value);
    }
}

fn snapshot(cv_sources: [f32; CV_SOURCES], pulse_sources: [bool; PULSE_SOURCES]) -> VoiceSnapshot {
    VoiceSnapshot {
        activity: Activity::Live,
        modulation_cv: cv_sources[2],
        envelope: cv_sources[3],
        gates: [0.2, 0.4],
        pulser_period: 0.5,
        stage: 1,
        sequence_value: 0.2,
        pulse: true,
        cv_sources,
        pulse_sources,
    }
}

impl nice_plug::context::gui::GuiContextInner for ApplyingHost {
    // A test double has no host to ask for a restart (nice-plug 0.4).
    fn request_restart(&self) {}
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }

    unsafe fn raw_begin_set_parameter(&self, param: ParamPtr) {
        self.record("begin", param);
    }

    unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, value: f32) {
        self.record("set", param);
        unsafe {
            param._internal_set_normalized_value(value);
        }
    }

    unsafe fn raw_end_set_parameter(&self, param: ParamPtr) {
        self.record("end", param);
    }

    fn get_state(&self) -> PluginState {
        PluginState {
            version: String::new(),
            params: Default::default(),
            fields: Default::default(),
        }
    }

    fn set_state(&self, _: PluginState) {}
}

use mxm_plugin_test::{opening_size, paging_checks};

struct Layout {
    synth: Vec<Rect>,
    seq: Vec<Rect>,
    modulation: Vec<Rect>,
}

// Whole-surface component geometry; physical fit is checked per derived page, not here.
fn render_layout(width: f32, height: f32) -> Layout {
    render_layout_at(width, height, false)
}

/// The same geometry with **every route revealed**, which the init patch cannot reach: one route is
/// present in the whole instrument by default, so almost no route row is drawn.
fn render_layout_revealed(width: f32, height: f32) -> Layout {
    render_layout_at(width, height, true)
}

fn render_layout_at(width: f32, _height: f32, revealed: bool) -> Layout {
    let params = MxmMono08Params::default();
    let telemetry = Telemetry::default();
    let actions = TransientActions::default();
    let host = ApplyingHost::default();
    let setter = ParamSetter::new(&host);
    if revealed {
        // This host applies its writes, so `add` actually moves the presences.
        for (destination, group) in params.routes.ordinary() {
            for route in &group.routes(destination) {
                mxm_modulation_params::add(route, &setter);
            }
        }
        for route in &params.routes.inverter_input.routes() {
            mxm_modulation_params::add(route, &setter);
        }
    }
    let mut view = 0;
    let mut entries = HashMap::new();
    let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
    let mut nav = mxm_ui::navigation::State::default();
    let theme = ThemePreference::Light;
    let ctx = egui::Context::default();
    mxm_ui::typography::apply(&ctx);
    ctx.set_theme(theme);
    mxm_ui::theme::apply(&ctx);
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(width, 20000.0))),
        ..Default::default()
    };

    let mut draw = |selected| {
        view = selected;
        for _ in 0..3 {
            let mut output = ctx.run_ui(input.clone(), |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &actions,
                    &|| {},
                    &setter,
                    &mut view,
                    &mut entries,
                    &mut presets,
                    &[],
                    &mut nav,
                );
            });
            output.textures_delta.clear();
        }
    };

    draw(0);
    let synth = paging_checks::all_rects(&ctx, CARDS)[..6].to_vec();
    draw(1);
    let modulation = paging_checks::all_rects(&ctx, CARDS)[6..9].to_vec();
    draw(2);
    let seq = paging_checks::all_rects(&ctx, CARDS)[9..11].to_vec();

    Layout {
        synth,
        seq,
        modulation,
    }
}

fn rows(rects: &[Rect]) -> Vec<Vec<(usize, Rect)>> {
    let mut ordered: Vec<_> = rects.iter().copied().enumerate().collect();
    ordered.sort_by(|(_, a), (_, b)| {
        a.top()
            .total_cmp(&b.top())
            .then(a.left().total_cmp(&b.left()))
    });
    let mut rows: Vec<Vec<(usize, Rect)>> = Vec::new();
    for item in ordered {
        match rows.last_mut() {
            Some(row)
                if row
                    .iter()
                    .any(|(_, rect)| rect.bottom() > item.1.top() + 1.0) =>
            {
                row.push(item)
            }
            _ => rows.push(vec![item]),
        }
    }
    rows
}

/// Every card is drawn at exactly its floor: the floor is also its ceiling (the owner, 2026-09-24).
fn assert_geometry(rects: &[Rect], floors: &[f32]) {
    assert_eq!(rects.len(), floors.len());
    for (index, (rect, floor)) in rects.iter().zip(floors).enumerate() {
        let ceiling = *floor;
        assert!(
            rect.width() >= floor - 0.75,
            "card {index} is {:.1} wide below its {floor:.1} floor",
            rect.width()
        );
        assert!(
            rect.width() <= ceiling + 0.75,
            "card {index} stretched to {:.1} beyond its {ceiling:.1} ceiling",
            rect.width()
        );
    }
    for (index, a) in rects.iter().enumerate() {
        for (other, b) in rects.iter().enumerate().skip(index + 1) {
            let overlap = a.intersect(*b);
            assert!(
                overlap.width() <= 0.75 || overlap.height() <= 0.75,
                "cards {index} and {other} overlap: {a:?}, {b:?}"
            );
        }
    }
    for row in rows(rects) {
        if row.len() > 1 {
            let low = row
                .iter()
                .map(|(_, rect)| rect.bottom())
                .fold(f32::INFINITY, f32::min);
            let high = row
                .iter()
                .map(|(_, rect)| rect.bottom())
                .fold(f32::NEG_INFINITY, f32::max);
            assert!(
                high - low < 1.0,
                "row bottoms differ by {:.1}: {row:?}",
                high - low
            );
        }
    }
    let visual_order: Vec<_> = rows(rects)
        .into_iter()
        .flatten()
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        visual_order,
        (0..rects.len()).collect::<Vec<_>>(),
        "reflow changed card sequence"
    );
}

#[test]
fn narrow_default_and_wide_reflow_keep_floors_order_alignment_and_separation() {
    for width in [MINIMUM.0 as f32, REFERENCE.0 as f32, 2200.0] {
        let layout = render_layout(width, 2600.0);
        let floors = test_floors();
        assert_geometry(&layout.synth, &floors[..6]);
        assert_geometry(&layout.modulation, &floors[6..9]);
        assert_geometry(&layout.seq, &floors[9..]);
    }
}

/// And they still fit **with every route revealed**, which the init patch cannot reach.
///
/// The check above draws the parameter defaults, where one route is present in the whole instrument
/// and so almost no route row is drawn at all — the easy case. A route row is the widest thing
/// these cards draw: a slider that lays its value out right-aligned across whatever width it is
/// handed, with a remove beside it. `mxm-mono-01` grew this test because a live row ran its remove
/// straight through the card's right border while the default-patch check passed happily.
///
/// **Height is not the subject.** A card may grow past the window as routes are added and the
/// player resizes (the owner's ruling, 2026-09-16) — but a card cannot page its way out of being
/// too narrow, so width still has to hold.
#[test]
fn cards_keep_their_floors_with_every_route_revealed() {
    for width in [MINIMUM.0 as f32, REFERENCE.0 as f32, 2200.0] {
        let layout = render_layout_revealed(width, 6000.0);
        let floors = test_floors();
        assert_geometry(&layout.synth, &floors[..6]);
        assert_geometry(&layout.modulation, &floors[6..9]);
        assert_geometry(&layout.seq, &floors[9..]);
    }
}

/// Every card, in every state that changes what it holds, passes the layout tree's checks
/// (plans/plan-layout-tree.md §4.3, `tree_checks::card`): its computed floor holds its content with
/// nothing painted outside the card, the content floor is exact, the height its tree states is the
/// height it draws, and every leaf stays in the room it was given.
///
/// The states are this editor's structural-state matrix: the init patch; every route revealed at
/// full negative depth — where a reading carries its sign and every digit, the widest text a row
/// can show; the Once queue full, which adds the Clock card's warning line; the Mod oscillator's
/// high range, whose frequency reads in another unit; and the Clock card's telemetry at its
/// longest — a slow period and every counter at its largest.
#[test]
fn every_card_passes_the_tree_checks_in_every_state() {
    let floors = test_floors();
    for state in [
        "init",
        "every route revealed",
        "the Once queue full",
        "the high range",
        "telemetry at its longest",
        "the clock synced, no tempo",
        "the clock synced to a tempo",
    ] {
        let params = MxmMono08Params::default();
        let host = ApplyingHost::default();
        let setter = ParamSetter::new(&host);
        if state == "every route revealed" {
            reveal_every_route(&params);
            for (destination, group) in params.routes.ordinary() {
                for route in &group.routes(destination) {
                    route.amount.set(&setter, 0.0);
                }
            }
            for route in &params.routes.inverter_input.routes() {
                route.amount.set(&setter, 0.0);
            }
        }
        if state == "the high range" {
            set_normalised(&params, "modhigh", 1.0);
            assert!(params.mod_high.value(), "the high range is on");
        }
        let telemetry = Telemetry::default();
        if state.starts_with("the clock synced") {
            set_normalised(&params, "pulsersync", 1.0);
        }
        if state == "the clock synced to a tempo" {
            telemetry.tempo.publish(Some(120.0));
        }
        if state == "telemetry at its longest" {
            telemetry.publish_once(u64::MAX, u64::MAX, u64::MAX);
            telemetry.publish_voice(VoiceSnapshot {
                pulser_period: 1000.0,
                ..snapshot([-1.0; CV_SOURCES], [true; PULSE_SOURCES])
            });
        }
        let actions = TransientActions::default();
        let full = state == "the Once queue full";
        let setup = move |ctx: &egui::Context| {
            mxm_ui::pilot::enable(ctx);
            if full {
                ctx.data_mut(|d| d.insert_temp(egui::Id::new(sections::ONCE_FULL), true));
            }
        };
        for (index, floor) in floors.iter().enumerate() {
            let mut entries = HashMap::new();
            let mut live = sections::Live {
                params: &params,
                telemetry: &telemetry,
                actions: &actions,
                wake: &|| {},
                setter: &setter,
                entries: &mut entries,
                waveform: &[],
                pulses: [false; PULSE_SOURCES],
                stage: (0, false),
            };
            tree_checks::card(
                &setup,
                state,
                card_title(index),
                *floor,
                &|ui| sections::card(ui, index, &params, &telemetry),
                &mut |ui, leaf, rect| sections::paint(ui, &mxm_ui::LIGHT, leaf, rect, &mut live),
            );
        }
    }
}

#[test]
fn editing_cards_fit_the_quarter_4k_content_budget_at_one_times_scale() {
    let params = MxmMono08Params::default();
    let telemetry = Telemetry::default();
    let actions = TransientActions::default();
    let host = ApplyingHost::default();
    let mut view = 0;
    let mut entries = HashMap::new();
    let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
    let mut nav = mxm_ui::navigation::State::default();
    paging_checks::verify(
        &test_items(),
        &[
            vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
            vec2(1880.0, 1040.0),
            vec2(MINIMUM.0 as f32, MINIMUM.1 as f32),
        ],
        |ui| {
            panel(
                ui,
                &params,
                &telemetry,
                &actions,
                &|| {},
                &ParamSetter::new(&host),
                &mut view,
                &mut entries,
                &mut presets,
                &[],
                &mut nav,
            )
        },
    );
    assert!(
        host.0.lock().unwrap().is_empty(),
        "measurement/navigation emitted host edits"
    );
}

/// **At a width that can hold them** — which is what the name says, and no longer the opening
/// width. The opening size is now the smallest window showing as many modules as the quarter-4K
/// budget allows (owner, 2026-09-09), and at 1160 the five-stage run wraps. §4.3 permits that:
/// grouping is a preference the packing applies while it can, never a floor on the window. What
/// this test owns is that the preference *is* applied wherever the row can hold the group, so it
/// measures at the budget width rather than at whatever the window happens to open at.
#[test]
fn parallel_groups_stay_together_when_the_row_can_hold_them() {
    let layout = render_layout(1920.0, 2600.0);
    for (a, b, name) in [(1, 2, "oscillators"), (3, 4, "gates")] {
        assert!(
            (layout.synth[a].top() - layout.synth[b].top()).abs() < 1.0,
            "the parallel {name} were split despite fitting"
        );
    }
    // The five steps were five cards; they are one. What remains beside them is the card holding
    // length and the triggers that advance it, and the two must not be separated by a wrap — the split is
    // what lets the faders be tall, and a wrap between them would put the run on its own page.
    assert!(
        (layout.seq[0].top() - layout.seq[1].top()).abs() < 1.0,
        "the sequence controls were split from the run they drive"
    );
}

#[test]
fn a_lone_card_never_stretches_and_the_narrow_flow_stays_inside_its_workspace() {
    let floors = test_floors();
    let layout = render_layout(MINIMUM.0 as f32, 2600.0);
    for (rects, floors) in [
        (&layout.synth, &floors[..6]),
        (&layout.modulation, &floors[6..9]),
        (&layout.seq, &floors[9..]),
    ] {
        for row in rows(rects) {
            if row.len() == 1 {
                let floor = floors[row[0].0];
                assert!(
                    row[0].1.width() <= floor + 0.75,
                    "a lone card stretched past its {floor:.1} floor: {row:?}"
                );
            }
        }
        for rect in rects {
            assert!(
                rect.left() >= SPACE_4 - 0.75,
                "card left the workspace: {rect:?}"
            );
            assert!(
                rect.right() <= MINIMUM.0 as f32 - SPACE_4 + 0.75,
                "card clipped at the narrow edge: {rect:?}"
            );
        }
    }
}

#[test]
fn essential_controls_do_not_clip_their_cards_at_the_one_card_minimum() {
    let params = MxmMono08Params::default();
    let telemetry = Telemetry::default();
    let actions = TransientActions::default();
    let host = ApplyingHost::default();
    let mut view = 0;
    let mut entries = HashMap::new();
    let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
    let mut nav = mxm_ui::navigation::State::default();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(vec2(MINIMUM.0 as f32, 2600.0))
        .build_ui(|ui| {
            mxm_ui::typography::apply(ui.ctx());
            mxm_ui::theme::apply(ui.ctx());
            panel(
                ui,
                &params,
                &telemetry,
                &actions,
                &|| {},
                &ParamSetter::new(&host),
                &mut view,
                &mut entries,
                &mut presets,
                &[],
                &mut nav,
            );
        });
    harness.run_steps(3);
    for (card, label) in [
        (0, "Glide time"),
        (1, "Mod frequency"),
        (1, "Mod key tracking"),
        (2, "Complex waveform"),
        (2, "Complex key tracking"),
        (3, "LPG 1 response"),
        (4, "LPG 2 input: LPG 1"),
        (5, "LPG 1 mix"),
        (5, "Reverb"),
    ] {
        mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(card));
        harness.run_steps(3);
        let card = mxm_ui::paging::editor::report(&harness.ctx)
            .unwrap()
            .visible
            .into_iter()
            .find(|(key, _)| key.0 == card)
            .unwrap()
            .1;
        let control = harness.get_by_label(label).rect();
        assert!(
            card.contains_rect(control),
            "{label} clips its card: {control:?} outside {card:?}"
        );
    }
    // Output is not on a card: it is in the app bar, which must still hold it at this width.
    output_in_the_app_bar(&harness, MINIMUM.0 as f32);
}

/// The app bar's Output, which must be drawn **once**, above every card, and inside the window.
///
/// Above the paging viewport rather than inside a named rectangle, because the bar has no report of
/// its own: every card is drawn inside that viewport, so a control wholly above it is not on one.
fn output_in_the_app_bar(harness: &egui_kittest::Harness<'_>, width: f32) -> Rect {
    let nodes: Vec<_> = harness.query_all_by_label("Output").collect();
    assert_eq!(nodes.len(), 1, "Output must be drawn exactly once");
    let output = nodes[0].rect();
    let viewport = mxm_ui::paging::editor::report(&harness.ctx)
        .unwrap()
        .viewport;
    assert!(
        output.bottom() <= viewport.top(),
        "Output {output:?} is not in the app bar above the cards {viewport:?}"
    );
    assert!(
        output.left() >= 0.0 && output.right() <= width,
        "Output {output:?} leaves the {width}-point window"
    );
    output
}

#[test]
fn mixer_and_spring_share_the_final_card_and_output_sits_in_the_app_bar() {
    for theme_choice in [ThemePreference::Light, ThemePreference::Dark] {
        for width in [MINIMUM.0 as f32, REFERENCE.0 as f32] {
            let params = MxmMono08Params::default();
            let telemetry = Telemetry::default();
            let actions = TransientActions::default();
            let host = ApplyingHost::default();
            let mut view = 0;
            let mut entries = HashMap::new();
            let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
            let mut nav = mxm_ui::navigation::State::default();
            // Tall canvas observes all narrow rows; this is not a physical-window fit claim.
            let mut harness = egui_kittest::Harness::builder()
                .with_size(vec2(width, 4000.0))
                .build_ui(|ui| {
                    mxm_ui::typography::apply(ui.ctx());
                    ui.ctx().set_theme(theme_choice);
                    mxm_ui::theme::apply(ui.ctx());
                    panel(
                        ui,
                        &params,
                        &telemetry,
                        &actions,
                        &|| {},
                        &ParamSetter::new(&host),
                        &mut view,
                        &mut entries,
                        &mut presets,
                        &[],
                        &mut nav,
                    );
                });
            harness.run_steps(3);
            mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(5));
            harness.run_steps(3);
            let mixer = mxm_ui::paging::editor::report(&harness.ctx)
                .unwrap()
                .visible
                .into_iter()
                .find(|(key, _)| key.0 == 5)
                .unwrap()
                .1;
            for label in ["LPG 1 mix", "LPG 2 mix", "Reverb"] {
                assert!(
                    mixer.contains_rect(harness.get_by_label(label).rect()),
                    "{label} must be on the final, combined card"
                );
            }
            // The instrument's output level is the app bar's (design system §3.1), never the
            // card's, and it is drawn once.
            let output = output_in_the_app_bar(&harness, width);
            assert!(
                !mixer.intersects(output),
                "Output {output:?} is drawn over the mixer card {mixer:?}"
            );
            let painted = painted_strings(harness.output());
            assert!(painted.iter().any(|s| s == "Mixer and reverb"));
            assert!(
                !painted
                    .iter()
                    .any(|s| s == "Mixer and output" || s == "Reverb and output" || s == "Mixer")
            );

            // The walk that used to follow — clicking every source through each destination's
            // picker and checking the picker sat on the amount's name line — retired with the
            // picker itself, and its empty-array scaffolding retired with D2, which removed the
            // last selector in the instrument. A destination lists the routes it carries, so there
            // is no selection to step through and no name line to share. What replaced it as a
            // reachability claim is the two-frame keyboard coverage check below.
            assert!(
                host.0.lock().unwrap().is_empty(),
                "laying the panel out is not a parameter edit"
            );
        }
    }
}

#[test]
fn live_route_and_pulse_painters_use_shape_reinforcement_in_both_themes() {
    fn colors(theme: ThemePreference, positive: bool, firing: bool) -> Vec<egui::Color32> {
        let ctx = egui::Context::default();
        ctx.set_theme(theme);
        mxm_ui::theme::apply(&ctx);
        let tokens = *mxm_ui::theme::tokens(&ctx);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(200.0, 80.0))),
                ..Default::default()
            },
            |ui| {
                // The route meter retired with the picker (the owner's ruling, 2026-09-16): a row
                // shows its own depth and `source_levels` shows every source, so the flash is the
                // only painted indicator left that carries meaning by colour alone.
                let _ = positive;
                visuals::pulse_route(ui, &tokens, "Clock", true, firing);
            },
        );
        output.textures_delta.clear();
        ctx.tessellate(output.shapes, output.pixels_per_point)
            .into_iter()
            .flat_map(|primitive| match primitive.primitive {
                egui::epaint::Primitive::Mesh(mesh) => mesh
                    .vertices
                    .into_iter()
                    .map(|vertex| vertex.color)
                    .collect(),
                egui::epaint::Primitive::Callback(_) => Vec::new(),
            })
            .collect()
    }

    for theme in [ThemePreference::Light, ThemePreference::Dark] {
        let ctx = egui::Context::default();
        ctx.set_theme(theme);
        mxm_ui::theme::apply(&ctx);
        let tokens = *mxm_ui::theme::tokens(&ctx);
        let armed = colors(theme, true, false);
        let pulse = colors(theme, true, true);
        assert!(pulse.contains(&tokens.warning), "a firing pulse is filled");
        assert!(
            !armed.contains(&tokens.warning),
            "armed and firing must differ by more than brightness"
        );
    }
}

#[test]
fn the_accessibility_tree_names_each_view_and_its_non_text_visuals() {
    let params = MxmMono08Params::default();
    let telemetry = Arc::new(Telemetry::default());
    let actions = TransientActions::default();
    let host = ApplyingHost::default();
    let mut view = 0;
    let mut entries = HashMap::new();
    let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
    let mut nav = mxm_ui::navigation::State::default();
    let shared = Arc::clone(&telemetry);
    let mut harness = egui_kittest::Harness::builder()
        .with_size(vec2(REFERENCE.0 as f32, REFERENCE.1 as f32))
        .build_ui(|ui| {
            mxm_ui::typography::apply(ui.ctx());
            mxm_ui::theme::apply(ui.ctx());
            panel(
                ui,
                &params,
                &shared,
                &actions,
                &|| {},
                &ParamSetter::new(&host),
                &mut view,
                &mut entries,
                &mut presets,
                &[],
                &mut nav,
            );
        });
    harness.run_steps(3);
    for (card, label) in [
        (2, "Complex waveform"),
        // Each CV destination's add menu, which is what a destination with nothing routed shows.
        // It is named for what its card paints, which drops the card's own prefix, so *Pitch* is
        // on both oscillators and each is asked of its own card.
        (0, "Glide speed: modulate"),
        (1, "Pitch: modulate"),
        (1, "Depth: modulate"),
        (2, "Pitch: modulate"),
        (2, "Timbre: modulate"),
        (4, "Level: modulate"),
        (3, "LPG 1 response"),
        (5, "Reverb"),
    ] {
        mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(card));
        harness.run_steps(3);
        on_card(&harness, card, label);
    }

    telemetry.request_view(1);
    harness.run_steps(3);
    for (card, label) in [
        // `Fire once` is the Clock's, not the Envelope's. It rode along on card 6 while the two
        // always shared a page; with fewer cards a page can hold one of them alone, so each label
        // is now asked of the card that actually draws it.
        (7, "Fire once"),
        (6, "Envelope attack"),
        // A trigger destination shows its three enables directly. Each paints *Trigger from Key*
        // on its own card and is named in full, which is what the accessibility tree carries.
        (6, "Envelope trigger from Key"),
        (7, "Period: modulate"),
        (7, "Clock trigger from Key"),
        // Random and the inverter share card 8. The inverter's label is the stack's
        // own affordance, because at the init patch the only route present is Random 1 and a
        // stack draws nothing else.
        (8, "Random trigger from Key"),
        (8, "Inverter input: modulate"),
    ] {
        mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(card));
        harness.run_steps(3);
        on_card(&harness, card, label);
    }

    telemetry.request_view(2);
    harness.run_steps(3);
    for (card, label) in [
        // The steps' switches paint only `Trigger`, and their canonical name is what the
        // accessibility tree carries.
        (9, "Sequence length: 5 steps"),
        (9, "Length: modulate"),
        (9, "Sequencer trigger from Key"),
        (10, "Step 1, active, value 0 percent"),
        (10, "Step 5 trigger"),
    ] {
        mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(card));
        harness.run_steps(3);
        harness.get_by_label(label);
    }

    telemetry.request_view(mxm_ui::paging::PARAMETERS as u8);
    harness.run_steps(3);
    for (id, parameter) in params.all_parameters() {
        // The app bar draws Output on every view, this one included, so the complete list is its
        // second copy here and only here.
        let expected = if id == "master" { 2 } else { 1 };
        assert_eq!(
            harness.query_all_by_label(parameter.name()).count(),
            expected,
            "{id}"
        );
    }
}

#[test]
fn a_semantic_toggle_is_one_complete_host_gesture() {
    let params = Arc::new(MxmMono08Params::default());
    let telemetry = Telemetry::default();
    let actions = TransientActions::default();
    let host = Arc::new(ApplyingHost::default());
    let mut view = 0;
    let mut entries = HashMap::new();
    let mut presets = PresetUi::at(crate::preset::Library::at(None), params.as_ref());
    let mut nav = mxm_ui::navigation::State::default();
    let shared_params = Arc::clone(&params);
    let shared_host = Arc::clone(&host);
    let mut harness = egui_kittest::Harness::builder()
        .with_size(vec2(REFERENCE.0 as f32, REFERENCE.1 as f32))
        .build_ui(|ui| {
            mxm_ui::typography::apply(ui.ctx());
            mxm_ui::theme::apply(ui.ctx());
            panel(
                ui,
                &shared_params,
                &telemetry,
                &actions,
                &|| {},
                &ParamSetter::new(shared_host.as_ref()),
                &mut view,
                &mut entries,
                &mut presets,
                &[],
                &mut nav,
            );
        });
    mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(2));
    harness.run_steps(3);
    harness.get_by_label("Complex key tracking").click();
    harness.run_steps(3);
    assert!(!params.complex_keyboard.value());
    assert_eq!(
        *host.0.lock().unwrap(),
        ["begin", "set", "end"].map(|action| (action, "Complex key tracking".to_owned()))
    );
}

/// The node labelled `label` on card `card`.
///
/// A route stack's add menu is named for what its card paints, which drops the card's own prefix,
/// so *Pitch: modulate* is on both oscillators and *Level: modulate* on both gates. Each is
/// unambiguous only inside its card, which is where a person reads it, so that is where it is
/// asked for.
fn on_card<'h>(
    harness: &'h egui_kittest::Harness<'_>,
    card: u64,
    label: &'h str,
) -> egui_kittest::Node<'h> {
    let rect = mxm_ui::paging::editor::report(&harness.ctx)
        .unwrap()
        .visible
        .into_iter()
        .find(|(key, _)| key.0 == card)
        .unwrap_or_else(|| panic!("card {card} is not drawn"))
        .1;
    harness
        .get_all_by_label(label)
        .find(|node| rect.contains(node.rect().center()))
        .unwrap_or_else(|| panic!("no {label:?} on card {card}"))
}

fn assert_gesture(host: &ApplyingHost, parameter: &str) {
    assert_eq!(
        *host.0.lock().unwrap(),
        ["begin", "set", "end"].map(|action| (action, parameter.to_owned()))
    );
    host.0.lock().unwrap().clear();
}

#[test]
fn shipped_panel_semantics_cover_control_kinds_views_updates_resets_and_cancellation() {
    for theme_choice in [ThemePreference::Light, ThemePreference::Dark] {
        let params = MxmMono08Params::default();
        let telemetry = Telemetry::default();
        let actions = TransientActions::default();
        let host = ApplyingHost::default();
        let mut view = 0;
        let mut entries = HashMap::new();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        let mut harness = egui_kittest::Harness::builder()
            .with_step_dt(1.0 / 60.0)
            .with_size(vec2(REFERENCE.0 as f32, 2600.0))
            .build_ui(|ui| {
                mxm_ui::typography::apply(ui.ctx());
                ui.ctx().set_theme(theme_choice);
                mxm_ui::theme::apply(ui.ctx());
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &actions,
                    &|| {},
                    &ParamSetter::new(&host),
                    &mut view,
                    &mut entries,
                    &mut presets,
                    &[],
                    &mut nav,
                );
            });
        mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(1));
        harness.run_steps(3);

        harness.get_by_label("Mod type: FM").click();
        harness.run_steps(2);
        assert_gesture(&host, "Mod type");
        harness.get_by_label("Mod key tracking").click();
        harness.run_steps(2);
        assert_gesture(&host, "Mod key tracking");
        let knob = harness.get_by_label("Mod depth");
        knob.focus();
        value_up(&mut harness);
        harness.run_steps(2);
        assert_gesture(&host, "Mod depth");

        mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(2));
        harness.run_steps(3);
        // **Adding a route is now an edit, and that is the point.** The picker this replaced chose
        // which of twelve always-active edges to show, so selecting was deliberately not a host
        // gesture. A source is added by writing its presence, so choosing one must reach the host
        // as one complete gesture — otherwise a preset or an automation lane could not carry it.
        on_card(&harness, 2, "Pitch: modulate").click();
        harness.run_steps(2);
        harness
            .get_by_role_and_label(egui::accesskit::Role::Button, "Pressure")
            .click();
        harness.run_steps(2);
        assert_gesture(&host, "Complex pitch from Pressure on");
        let route = harness.get_by_label("Complex pitch from Pressure");
        route.focus();
        value_up(&mut harness);
        harness.run_steps(2);
        assert_gesture(&host, "Complex pitch from Pressure");

        // The inverter is on card 8, and since D2 it holds a route stack rather than a selector.
        // Init's one inverter route is Random 1, so that row is the one drawn there.
        mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(8));
        harness.run_steps(3);
        let inverter = harness.get_by_label("Inverter input from Random 1");
        inverter.focus();
        value_up(&mut harness);
        harness.run_steps(2);
        assert_gesture(&host, "Inverter input from Random 1");

        telemetry.request_view(2);
        harness.set_size(vec2(MINIMUM.0 as f32, 2600.0));
        harness.run_steps(3);
        harness.get_by_label("Sequence length: 3 steps").click();
        harness.run_steps(2);
        assert_gesture(&host, "Sequence length");
        mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(9));
        harness.run_steps(3);
        let step = harness.get_by_label("Step 2 level");
        step.focus();
        value_up(&mut harness);
        harness.run_steps(2);
        assert_gesture(&host, "Step 2 level");
        mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(9));
        harness.run_steps(3);
        harness.get_by_label("Step 5 trigger").click();
        harness.run_steps(2);
        assert_gesture(&host, "Step 5 trigger");

        // Output is in the app bar, on every view, and an edit there is one gesture like any other.
        let output = harness.get_by_label("Output");
        output.focus();
        value_up(&mut harness);
        harness.run_steps(2);
        assert_gesture(&host, "Output");

        telemetry.request_view(mxm_ui::paging::PARAMETERS as u8);
        harness.run_steps(3);
        set_normalised(&params, "master", 0.35);
        harness.run_steps(2);
        assert_eq!(sections::bound(&params, "master").param.normalised(), 0.35);
        assert!(
            host.0.lock().unwrap().is_empty(),
            "external update emitted a gesture"
        );

        telemetry.request_view(3);
        mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(1));
        harness.run_steps(60);
        // FM, as the first click in this test left it: the second of `modtype`'s two options.
        assert_eq!(sections::bound(&params, "modtype").param.normalised(), 1.0);
        harness.get_by_label("Mod type: FM").click();
        harness.run_steps(1);
        harness.get_by_label("Mod type: FM").click();
        harness.run_steps(3);
        assert_gesture(&host, "Mod type");
        assert_eq!(sections::bound(&params, "modtype").param.normalised(), 0.0);
    }
}

/// On the developer list, and on a musician page, where Output's only control is the app bar's.
#[test]
fn cancelling_an_open_text_entry_emits_no_host_edit() {
    for starting_view in [mxm_ui::paging::PARAMETERS, 0] {
        let params = MxmMono08Params::default();
        let telemetry = Telemetry::default();
        let actions = TransientActions::default();
        let host = ApplyingHost::default();
        let setter = ParamSetter::new(&host);
        let mut view = starting_view;
        let mut entries = HashMap::from([("master", Some("cancel me".to_owned()))]);
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                vec2(REFERENCE.0 as f32, 2600.0),
            )),
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            panel(
                ui,
                &params,
                &telemetry,
                &actions,
                &|| {},
                &setter,
                &mut view,
                &mut entries,
                &mut presets,
                &[],
                &mut nav,
            );
        });
        output.textures_delta.clear();
        assert_eq!(entries.get("master"), Some(&None), "view {starting_view}");
        assert!(host.0.lock().unwrap().is_empty(), "view {starting_view}");
    }
}

// Two tests retired with the picker they examined. One looked for the per-route live meter
// ("Pressure route live +"), which the owner's ruling of 2026-09-16 removed; the other
// asserted that no destination painted an "Also from" caption, which was the picker's way of
// naming the edges it was hiding. A destination now lists every route it carries, so there is
// nothing hidden to caption and nothing selected to feed a meter.

/// Dragging a route amount: nothing beneath the pointer moves, and the host sees one complete
/// begin/set/end gesture.
///
/// **The defect this guards outlived its original subject.** It was written when a live bar was
/// inserted above the slider only once the amount left zero: the first drag frame pushed the
/// slider down and, the bar being an un-id'd allocation, renumbered the slider's id, so egui's
/// held drag no longer matched and the gesture never ended. The owner's ruling of 2026-09-16
/// removed that bar — but a route row now ends in a remove, and removing a row shifts every row
/// below it under the pointer, so a stack has a sharper version of the same hazard. The subject
/// moves to the stack; the claim does not change.
#[test]
fn dragging_a_route_amount_from_zero_moves_nothing_and_ends_its_gesture() {
    let params = MxmMono08Params::default();
    // The row has to exist before it can be dragged: Timbre carries no route at the init patch.
    reveal_every_route(&params);
    let telemetry = Telemetry::default();
    let actions = TransientActions::default();
    let host = ApplyingHost::default();
    let mut view = 0;
    let mut entries = HashMap::new();
    let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
    let mut nav = mxm_ui::navigation::State::default();
    let mut harness = egui_kittest::Harness::builder()
        .with_step_dt(1.0 / 60.0)
        .with_size(vec2(REFERENCE.0 as f32, 2600.0))
        .build_ui(|ui| {
            mxm_ui::typography::apply(ui.ctx());
            mxm_ui::theme::apply(ui.ctx());
            panel(
                ui,
                &params,
                &telemetry,
                &actions,
                &|| {},
                &ParamSetter::new(&host),
                &mut view,
                &mut entries,
                &mut presets,
                &[],
                &mut nav,
            );
        });
    harness.run_steps(3);
    // A route row exists only while its card is drawn: Timbre lives on the Complex oscillator card,
    // so revealing the route is necessary but not sufficient.
    mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(2));
    harness.run_steps(3);

    let slider = |harness: &egui_kittest::Harness<'_>| {
        harness
            .get_by_role_and_label(egui::accesskit::Role::Slider, "Complex timbre from Key")
            .rect()
    };
    // The row's remove, which sits on the slider's own track: if the row moved, so would this.
    let remove = |harness: &egui_kittest::Harness<'_>| {
        harness
            .get_by_label("Remove Key from Complex timbre")
            .rect()
    };
    let before = (slider(&harness), remove(&harness));

    let start = before.0.center();
    harness.hover_at(start);
    harness.event(egui::Event::PointerButton {
        pos: start,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    for step in 1..=4 {
        let pos = start + vec2(12.0 * step as f32, 0.0);
        harness.event(egui::Event::PointerMoved(pos));
        harness.step();
        assert_eq!(
            (slider(&harness), remove(&harness)),
            before,
            "the route row moved under the pointer on drag step {step}"
        );
    }
    let end = start + vec2(48.0, 0.0);
    harness.event(egui::Event::PointerButton {
        pos: end,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(3);

    assert_eq!((slider(&harness), remove(&harness)), before);
    assert!(
        (params.routes.timbre.key.unmodulated_normalized_value() - 0.5).abs() > 0.01,
        "the drag did not move the amount"
    );
    let recorded = host.0.lock().unwrap().clone();
    assert!(
        recorded
            .iter()
            .all(|(_, name)| name == "Complex timbre from Key"),
        "{recorded:?}"
    );
    assert_eq!(recorded.first().map(|(action, _)| *action), Some("begin"));
    assert_eq!(
        recorded.last().map(|(action, _)| *action),
        Some("end"),
        "the drag's gesture never reached the host's end: {recorded:?}"
    );
    assert!(recorded.iter().any(|(action, _)| *action == "set"));
}

/// A routing group is tighter inside than the space around it.
///
/// This is the owner's report, made mechanical. On the Modulation oscillator card every gap was
/// egui's default 3 points or the 12 I had put between groups, so the card's own caption sat
/// closer to the toggle above it than a source picker sat to the slider it belongs to, and the
/// column read as one undifferentiated list. Design system §4.1 now states the rule; this holds
/// it: a group's internal spacing must be strictly smaller than the space separating it from
/// what is around it.
#[test]
fn a_routing_group_is_tighter_inside_than_the_space_around_it() {
    let params = MxmMono08Params::default();
    // A group encloses the routes a destination carries, and the init patch carries one in the
    // whole instrument — so without this there is no group on this card to measure at all.
    //
    // **Only this card's routes.** Revealing all 159 pushes the card's controls off a canvas this
    // test measures against, and the control above the routes is one of its three landmarks. Two
    // routes into Mod pitch and one into Mod depth are what the claim actually needs: a group with
    // two members, and a second group after it.
    reveal_route(&params, 1, 0);
    reveal_route(&params, 1, 1);
    reveal_route(&params, 3, 0);
    let telemetry = Telemetry::default();
    let actions = TransientActions::default();
    let host = ApplyingHost::default();
    let mut view = 0;
    let mut entries = HashMap::new();
    let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
    let mut nav = mxm_ui::navigation::State::default();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(vec2(REFERENCE.0 as f32, 2600.0))
        .build_ui(|ui| {
            mxm_ui::typography::apply(ui.ctx());
            mxm_ui::theme::apply(ui.ctx());
            panel(
                ui,
                &params,
                &telemetry,
                &actions,
                &|| {},
                &ParamSetter::new(&host),
                &mut view,
                &mut entries,
                &mut presets,
                &[],
                &mut nav,
            );
        });
    harness.run_steps(3);

    // The Modulation oscillator card, top to bottom: its Key tracking switch, then two groups.
    // Two rows of one destination's group, then the first row of the next destination's: the gap
    // within a group must read as tighter than the gap across the boundary between them.
    let first = harness
        .get_by_role_and_label(egui::accesskit::Role::Slider, "Mod pitch from Key")
        .rect();
    // The complex oscillator paints a Key tracking of its own, so the landmark is the one in this
    // column, straight above the routes.
    let above = painted_text(harness.output())
        .into_iter()
        .filter(|(text, rect)| {
            text == "Key tracking"
                && rect.bottom() <= first.top()
                && rect.left() < first.right()
                && rect.right() > first.left()
        })
        .map(|(_, rect)| rect)
        .max_by(|a, b| a.bottom().total_cmp(&b.bottom()))
        .expect("the card's Key tracking switch is painted above its routes");
    let sibling = harness
        .get_by_role_and_label(egui::accesskit::Role::Slider, "Mod pitch from Pressure")
        .rect();
    let second = harness
        .get_by_role_and_label(egui::accesskit::Role::Slider, "Mod depth from Key")
        .rect();

    let inside = sibling.top() - first.bottom();
    let between = second.top() - sibling.bottom();
    let after_control = first.top() - above.bottom();

    assert!(
        inside < between,
        "a group's own gap is {inside}, the gap to the next group only {between}"
    );
    assert!(
        between - inside >= mxm_ui::space::SPACE_3,
        "the ladder must be a step, not a hair: {inside} inside against {between} between"
    );
    assert!(
        after_control > inside,
        "the card's last control sits {after_control} above the first group but the group's own \
         members are {inside} apart, so the control reads as part of the routing"
    );

    // Spacing alone is not the fix the owner asked for. One tinted hairline panel must actually
    // enclose the routes this destination carries, and must not swallow the next destination's.
    let tokens = mxm_ui::theme::tokens(&harness.ctx);
    let panels = painted_panels(harness.output(), tokens.border);
    assert!(
        panels.iter().any(|panel| {
            panel.contains_rect(first)
                && panel.contains_rect(sibling)
                && !panel.contains_rect(second)
        }),
        "no panel encloses Mod pitch's routes alone; found {panels:?}"
    );
}

/// The unfilled hairline panels `mxm_ui::shell::group` paints, in draw order.
fn painted_panels(output: &egui::FullOutput, border: egui::Color32) -> Vec<Rect> {
    fn collect(shape: &egui::Shape, border: egui::Color32, found: &mut Vec<Rect>) {
        match shape {
            egui::Shape::Rect(rect) => {
                if rect.fill.a() == 0
                    && rect.stroke.color == border
                    && (rect.stroke.width - mxm_ui::space::HAIRLINE).abs() < 0.01
                {
                    found.push(rect.rect);
                }
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, border, found);
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in &output.shapes {
        collect(&clipped.shape, border, &mut found);
    }
    found
}

/// The painted text runs, with the rectangle each was drawn into.
fn painted_text(output: &egui::FullOutput) -> Vec<(String, Rect)> {
    fn collect(shape: &egui::Shape, runs: &mut Vec<(String, Rect)>) {
        match shape {
            egui::Shape::Text(text) => {
                runs.push((text.galley.text().to_owned(), text.visual_bounding_rect()));
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, runs);
                }
            }
            _ => {}
        }
    }
    let mut runs = Vec::new();
    for clipped in &output.shapes {
        collect(&clipped.shape, &mut runs);
    }
    runs
}

fn painted_strings(output: &egui::FullOutput) -> Vec<String> {
    fn collect(shape: &egui::Shape, strings: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) => strings.push(text.galley.job.text.clone()),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, strings);
                }
            }
            _ => {}
        }
    }
    let mut strings = Vec::new();
    for shape in &output.shapes {
        collect(&shape.shape, &mut strings);
    }
    strings
}

#[test]
fn each_stage_visual_uses_its_own_level_with_independent_active_and_pulse_state() {
    let params = MxmMono08Params::default();
    for (id, value) in [
        ("seq1level", 0.0),
        ("seq2level", 0.25),
        ("seq3level", 0.5),
        ("seq4level", 0.75),
        ("seq5level", 1.0),
    ] {
        set_normalised(&params, id, value);
    }
    let telemetry = Telemetry::default();
    telemetry.set_editor_open(true);
    telemetry.publish_voice(snapshot([0.0; CV_SOURCES], [false; PULSE_SOURCES]));
    let actions = TransientActions::default();
    let host = ApplyingHost::default();
    let mut view = 2;
    let mut entries = HashMap::new();
    let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
    let mut nav = mxm_ui::navigation::State::default();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(vec2(REFERENCE.0 as f32, REFERENCE.1 as f32))
        .build_ui(|ui| {
            mxm_ui::typography::apply(ui.ctx());
            ui.ctx().set_theme(ThemePreference::Dark);
            mxm_ui::theme::apply(ui.ctx());
            panel(
                ui,
                &params,
                &telemetry,
                &actions,
                &|| {},
                &ParamSetter::new(&host),
                &mut view,
                &mut entries,
                &mut presets,
                &[],
                &mut nav,
            );
        });
    mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(9));
    harness.run_steps(3);
    telemetry.publish_voice(snapshot([0.0; CV_SOURCES], [false; PULSE_SOURCES]));
    harness.step();
    for label in [
        "Step 1, value 0 percent",
        "Step 2, active, trigger, value 25 percent",
        "Step 3, value 50 percent",
        "Step 4, value 75 percent",
        "Step 5, value 100 percent",
    ] {
        harness.get_by_label(label);
    }
}

#[test]
fn fire_once_acceptance_also_schedules_one_non_parameter_wake() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let params = MxmMono08Params::default();
    let telemetry = Telemetry::default();
    let actions = TransientActions::default();
    let host = ApplyingHost::default();
    let wakes = AtomicUsize::new(0);
    let wake = || {
        wakes.fetch_add(1, Ordering::Relaxed);
    };
    let mut view = 1;
    let mut entries = HashMap::new();
    let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
    let mut nav = mxm_ui::navigation::State::default();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(vec2(REFERENCE.0 as f32, REFERENCE.1 as f32))
        .build_ui(|ui| {
            panel(
                ui,
                &params,
                &telemetry,
                &actions,
                &wake,
                &ParamSetter::new(&host),
                &mut view,
                &mut entries,
                &mut presets,
                &[],
                &mut nav,
            );
        });
    harness.run_steps(3);
    // **Ask for the Clock card rather than assuming it opens on the first page.** The opening
    // size is the smallest window showing as many modules as the budget allows, and which of the
    // ten those are is `the_opening_size_is_the_budget_hugged`'s business, not this test's:
    // the subject here is that Once is a transient command and never a parameter gesture.
    mxm_ui::paging::editor::request_card(&harness.ctx, mxm_ui::paging::Key(7));
    harness.run_steps(3);
    harness.get_by_label("Fire once").click();
    harness.run_steps(3);
    assert_eq!(actions.pending(), 1);
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    assert!(
        host.0.lock().unwrap().is_empty(),
        "wake became a parameter gesture"
    );
}

#[test]
fn private_state_starts_clean_and_developer_requests_change_only_editor_state() {
    let params = Arc::new(MxmMono08Params::default());
    let telemetry = Telemetry::shared();
    let actions = TransientActions::shared();
    let app = MxmMono08App::new(
        Arc::clone(&params),
        Arc::clone(&telemetry),
        Arc::clone(&actions),
    );
    assert_eq!(app.view, 0);
    assert!(app.gui_context.is_none());
    assert!(app.text_entry.is_empty());
    assert_eq!(app.waveform.capacity(), WAVEFORM_LEN);
    assert!(!telemetry.editor_open());
    assert_eq!(actions.pending(), 0);
    // 160 before the routing conversion, 340 after it, 369 after D2, 370 with the modulation high
    // range, 349 without the external input: 149 routes with an amount and a presence each, the
    // twelve trigger enables, and the thirty-nine panel controls. 350 with the clock period's tempo
    // sync; 380 with the standard Amplitude's fifteen routes.
    assert_eq!(params.all_parameters().len(), 380);
}

use mxm_plugin_test::keyboard_checks;
use mxm_plugin_test::keyboard_checks::{OUT, VALUE, key_of};

/// What this editor keeps behind a disclosure, opened so the reachability check sees it.
/// Nothing here: this brief has no disclosure, which is why `panel` discards the gated request.
const REVEAL: fn(&egui::Context) = |_| {};

/// Every route present, as the `‹ modulate ›` menu would add them one at a time.
///
/// **An absent route draws nothing at all**, so a check at the init patch reaches one route of a
/// hundred and forty-four. That is why this runs in two frames rather than one.
/// One route made present, as the `‹ modulate ›` menu would add it.
fn reveal_route(params: &MxmMono08Params, destination: usize, source: usize) {
    let host = ApplyingHost::default();
    let setter = ParamSetter::new(&host);
    mxm_modulation_params::add(
        &params.routes.cv(destination).routes(destination)[source],
        &setter,
    );
}

/// Through [`ApplyingHost`], because `add` reports the write to a host and a host that only counts
/// them swallows it: with the coverage check's own recorder this would reveal nothing at all and
/// the test would quietly measure the init patch twice.
fn reveal_every_route(params: &MxmMono08Params) {
    let host = ApplyingHost::default();
    let setter = ParamSetter::new(&host);
    for (destination, group) in params.routes.ordinary() {
        for route in &group.routes(destination) {
            mxm_modulation_params::add(route, &setter);
        }
    }
    for route in &params.routes.inverter_input.routes() {
        mxm_modulation_params::add(route, &setter);
    }
}

/// Every parameter the panel should draw: the cards' own controls, and a presence and an amount for
/// every route that is **currently present**.
///
/// The declared surface is 349, but 298 of those belong to routes, and a route that is not present
/// is not drawn. So the expected set is built from the patch rather than from the parameter list:
/// the panel's own ids, plus the two ids of each live pair. `master` is among the panel's own: the
/// app bar draws it inside its own bar card, so the cursor reaches it on every page.
fn drawn_ids(params: &MxmMono08Params) -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = params
        .all_parameters()
        .into_iter()
        .map(|(id, _)| id)
        .filter(|id| !id.starts_with("cv_"))
        .collect();
    for (destination, group) in params.routes.ordinary() {
        for (source, present) in group.presences(destination).into_iter().enumerate() {
            if present {
                let (amount, presence) = crate::routes::ROUTE_IDS[destination][source];
                ids.push(amount);
                ids.push(presence);
            }
        }
    }
    // The tenth destination, whose fifteen routes are indexed in its own order rather than the
    // frame's — so its ids come from its own row of the table.
    for (index, route) in params.routes.inverter_input.routes().iter().enumerate() {
        if route.is_present() {
            let (amount, presence) = crate::routes::ROUTE_IDS[crate::routes::INVERTER][index];
            ids.push(amount);
            ids.push(presence);
        }
    }
    ids
}

/// VALUE + ↑, kept with OUT: the keyboard language's edit.
fn value_up<State>(harness: &mut egui_kittest::Harness<'_, State>) {
    for key in [key_of(VALUE), egui::Key::ArrowUp, key_of(OUT)] {
        harness.key_press(key);
    }
}

/// The rollout's own failure mode: a control whose `navigation::at` scope was forgotten paints
/// exactly as before and is simply unreachable from the keyboard. Nothing else would say so.
///
/// **Two frames** (`plan-modulation-routing.md` §8a): the init patch, and every route present.
#[test]
fn the_keyboard_cursor_reaches_and_operates_every_parameter() {
    reach_and_operate(false);
}

#[test]
fn the_keyboard_cursor_reaches_and_operates_every_route_revealed() {
    reach_and_operate(true);
}

fn reach_and_operate(revealed: bool) {
    let params = MxmMono08Params::default();
    if revealed {
        reveal_every_route(&params);
    }
    let telemetry = Telemetry::default();
    let actions = TransientActions::default();
    let host = keyboard_checks::Recorder::default();
    let setter = ParamSetter::new(&host);
    let mut view = 0usize;
    let mut entries = HashMap::new();
    let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
    let mut nav = mxm_ui::navigation::State::default();
    let ids = drawn_ids(&params);
    keyboard_checks::the_cursor_reaches_and_operates(
        vec2(1880.0, 1040.0),
        &test_items(),
        keyboard_checks::Coverage::Exactly(&ids),
        &REVEAL,
        &host,
        &mut |ui| {
            panel(
                ui,
                &params,
                &telemetry,
                &actions,
                &|| {},
                &setter,
                &mut view,
                &mut entries,
                &mut presets,
                &[],
                &mut nav,
            );
        },
    );
}

/// **The editor opens at the quarter-4K budget, hugged** — the owner's rule, 2026-09-09. The budget
/// is the most room an editor may ask for, so laying the panel out there shows as many modules as
/// it ever will; taking the slack away is the whole of the size.
#[test]
fn the_opening_size_is_the_budget_hugged() {
    let params = MxmMono08Params::default();
    let telemetry = Telemetry::default();
    let actions = TransientActions::default();
    let host = ApplyingHost::default();
    let setter = ParamSetter::new(&host);
    let mut view = 0usize;
    let mut entries = HashMap::new();
    let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
    let mut nav = mxm_ui::navigation::State::default();
    opening_size::is_the_budget_hugged(
        vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
        &REVEAL,
        &mut |ui| {
            panel(
                ui,
                &params,
                &telemetry,
                &actions,
                &|| {},
                &setter,
                &mut view,
                &mut entries,
                &mut presets,
                &[],
                &mut nav,
            );
        },
    );
}

/// **The app bar holds in the narrowest window**: its `…` menu whole and nothing drawn over
/// anything else, from `MINIMUM` up (`opening_size::bar_holds_from_the_minimum`).
#[test]
fn the_app_bar_holds_in_the_minimum_window() {
    let params = MxmMono08Params::default();
    let telemetry = Telemetry::default();
    let actions = TransientActions::default();
    let host = ApplyingHost::default();
    let setter = ParamSetter::new(&host);
    let mut view = 0usize;
    let mut entries = HashMap::new();
    let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
    let mut nav = mxm_ui::navigation::State::default();
    opening_size::bar_holds_from_the_minimum(vec2(MINIMUM.0 as f32, MINIMUM.1 as f32), &mut |ui| {
        panel(
            ui,
            &params,
            &telemetry,
            &actions,
            &|| {},
            &setter,
            &mut view,
            &mut entries,
            &mut presets,
            &[],
            &mut nav,
        );
    });
}

use mxm_plugin_test::tree_checks;

/// Every page at the opening size, light and dark, for the owner's review of the layout-tree
/// conversion (plans/plan-layout-tree.md §4.3): `target/layout-tree/mxm-mono-08/<tag>/`, where
/// `MXM_PICTURES` names the tag — `before` on the unconverted editor, `after` on the tree.
///
/// `MXM_PICTURES=after cargo test -p mxm-mono-08 --lib tree_pictures -- --ignored`
#[test]
#[ignore = "renders through wgpu; run by hand"]
fn tree_pictures() {
    let tag = std::env::var("MXM_PICTURES").unwrap_or_else(|_| "after".to_owned());
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/layout-tree/mxm-mono-08")
        .join(tag);
    let params = MxmMono08Params::default();
    let telemetry = Telemetry::default();
    let actions = TransientActions::default();
    let host = ApplyingHost::default();
    let setter = ParamSetter::new(&host);
    let mut view = 0;
    let mut entries = HashMap::new();
    let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
    let mut nav = mxm_ui::navigation::State::default();
    tree_checks::pictures(
        &|_| {},
        vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
        &dir,
        &mut |ui| {
            panel(
                ui,
                &params,
                &telemetry,
                &actions,
                &|| {},
                &setter,
                &mut view,
                &mut entries,
                &mut presets,
                &[],
                &mut nav,
            );
        },
    );
}

/// **No help text on the panel** (the owner, 2026-09-27; design system §7.6): the only captions a
/// card may carry are live readings — the clock's period and the Once counts. A sentence explaining
/// a control is its tooltip.
///
/// Falsified before trusted: restoring the Random card's caption fails it.
#[test]
fn no_card_prints_help_text() {
    let params = MxmMono08Params::default();
    let telemetry = Telemetry::default();
    let mut harness = egui_kittest::Harness::builder().build_ui(|ui| {
        let cards =
            sections::SYNTH_TITLES.len() + sections::MOD_TITLES.len() + SEQUENCER_TITLES.len();
        for index in 0..cards {
            for key in sections::card(ui, index, &params, &telemetry).keys() {
                if let sections::Leaf::Caption(text) = key {
                    assert!(
                        text.starts_with("Live period: ") || text.starts_with("Fired "),
                        "{} prints {text:?}",
                        card_title(index)
                    );
                }
            }
        }
    });
    harness.run_steps(1);
}
