//! Production editor for the approved `mxm-mono-08` §14 brief.
//!
//! This is a software-native panel: four task views, semantic collection controls, named routing
//! amounts instead of patch cables, and card order derived from signal flow rather than hardware
//! geography. Audio sees parameters and the bounded Once command path only; it never reads this
//! editor's view, zoom, theme, text-entry, preset, or layout state.

mod binding;
mod sections;
mod visuals;

use std::collections::HashMap;
use std::sync::Arc;

use egui::Ui;
use mxm_ui::space::SPACE_4;
use mxm_ui::theme::Tokens;
use nice_plug::context::gui::GuiContext;
use nice_plug::prelude::*;
use nice_plug_egui::{EguiEditorState, NiceEguiApp, create_egui_editor};

use crate::actions::TransientActions;
use crate::params::MxmMono08Params;
use crate::telemetry::{Telemetry, WAVEFORM_LEN};

const REFERENCE: (u32, u32) = (1680, 999);
const MINIMUM: (u32, u32) = (540, 420);
/// The keyboard cursor's card for the app bar's Output, outside the paging keys 0…16.
const MASTER_CARD: u64 = 64;
// Every card is drawn at its floor and no wider: the floor is also its ceiling (the owner,
// 2026-09-24: *"so everything pack tight"*). **Each floor is computed from its card's tree**
// (plans/plan-layout-tree.md) — the tree's narrowest plus the card's chrome — so nothing here is a
// number: what sets a floor is what cannot shrink, a route slider's minimum track with its source and
// widest reading, a selector's cells, a row of knobs. `page_items` computes them every frame.
/// The two Sequencers cards: what drives the run, and the run's five steps.
const SEQUENCER_TITLES: [&str; 2] = ["Sequencer", "Steps"];
/// Every paging item this editor declares. `page_items` is the truth; this is what the tests count
/// against so a card added or removed without them fails loudly rather than silently.
#[cfg(test)]
const CARDS: usize = 11;

pub type MxmMono08Editor = nice_plug_egui::EguiEditor<MxmMono08App>;

#[cfg(debug_assertions)]
fn arm_real_host_once_probe(
    actions: Arc<TransientActions>,
    executor: AsyncExecutor<crate::MxmMono08>,
) {
    const DIRECTORY: &str = "MXM_MONO_08_TEST_ONCE_DIR";
    let Some(directory) = std::env::var_os(DIRECTORY).map(std::path::PathBuf::from) else {
        return;
    };
    if !directory.is_dir() {
        return;
    }
    // Bundle integration tests cannot reach Rust internals. The parent test gives a child process
    // one private directory through immutable startup configuration. Claiming `ready` arms at most
    // one debug instance; an atomically created `fire` file is the thread-safe signal. No release
    // instance or ordinary debug host creates this worker, and its lifetime is bounded below.
    if std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("ready"))
        .is_err()
    {
        return;
    }
    std::thread::spawn(move || {
        let fire = directory.join("fire");
        for _ in 0..500 {
            match std::fs::remove_file(&fire) {
                Ok(()) => {
                    let wake = || executor.execute_gui(crate::EditorTask::WakeAudio);
                    let _ = actions.submit_once_from_editor(&wake);
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return,
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
}

pub fn create(
    params: Arc<MxmMono08Params>,
    telemetry: Arc<Telemetry>,
    actions: Arc<TransientActions>,
    executor: AsyncExecutor<crate::MxmMono08>,
) -> Option<MxmMono08Editor> {
    #[cfg(debug_assertions)]
    arm_real_host_once_probe(Arc::clone(&actions), executor.clone());
    let state = EguiEditorState::from_size(
        nice_plug::editor::dpi::LogicalSize::new(REFERENCE.0, REFERENCE.1),
        1.0,
    );
    create_egui_editor(
        state,
        nice_plug_egui::RepaintNotifier::new(),
        nice_plug_egui::EguiNiceSettings {
            title: crate::NAME.to_owned(),
            resize_hint: ResizeHint {
                size_constraints: nice_plug::editor::SizeConstraints::min_logical_size(
                    nice_plug::editor::dpi::LogicalSize::new(MINIMUM.0 as f32, MINIMUM.1 as f32),
                ),
                ..ResizeHint::RESIZABLE
            },
            ..Default::default()
        },
        MxmMono08App::with_executor(params, telemetry, actions, executor),
    )
}

pub use mxm_preset::PresetUi;

pub struct MxmMono08App {
    params: Arc<MxmMono08Params>,
    telemetry: Arc<Telemetry>,
    actions: Arc<TransientActions>,
    executor: Option<AsyncExecutor<crate::MxmMono08>>,
    gui_context: Option<GuiContext>,
    view: usize,
    text_entry: HashMap<&'static str, Option<String>>,
    presets: PresetUi,
    waveform: Vec<f32>,
    /// Where the keyboard is: a card, and a parameter inside it. Transient, like the text
    /// buffers — it is not a parameter and nothing durable reads it.
    nav: mxm_ui::navigation::State,
}

impl MxmMono08App {
    pub fn new(
        params: Arc<MxmMono08Params>,
        telemetry: Arc<Telemetry>,
        actions: Arc<TransientActions>,
    ) -> Self {
        let presets = PresetUi::new(params.as_ref());
        Self {
            params,
            telemetry,
            actions,
            executor: None,
            gui_context: None,
            view: 0,
            text_entry: HashMap::new(),
            presets,
            waveform: Vec::with_capacity(WAVEFORM_LEN),
            nav: mxm_ui::navigation::State::default(),
        }
    }

    fn with_executor(
        params: Arc<MxmMono08Params>,
        telemetry: Arc<Telemetry>,
        actions: Arc<TransientActions>,
        executor: AsyncExecutor<crate::MxmMono08>,
    ) -> Self {
        let mut app = Self::new(params, telemetry, actions);
        app.executor = Some(executor);
        app
    }
}

impl NiceEguiApp for MxmMono08App {
    fn build(
        &mut self,
        egui_ctx: egui::Context,
        nice_gui_ctx: GuiContext,
        _frame: &mut nice_plug_egui::Frame,
    ) -> Result<(), nice_plug_egui::baseview::HandlerError> {
        mxm_ui::theme::apply(&egui_ctx);
        mxm_ui::typography::apply(&egui_ctx);
        // Light by default, overridable with `MXM_EDITOR_THEME`; the reasoning, and why the
        // default is not `System`, lives on `mxm_ui::theme::preference`.
        egui_ctx.set_theme(mxm_ui::theme::preference());
        self.gui_context = Some(nice_gui_ctx);
        self.telemetry.set_editor_open(true);
        Ok(())
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut nice_plug_egui::Frame) {
        let Some(context) = self.gui_context.clone() else {
            return;
        };
        self.telemetry.waveform_snapshot(&mut self.waveform);
        let wake = || {
            if let Some(executor) = &self.executor {
                executor.execute_gui(crate::EditorTask::WakeAudio);
            }
        };
        panel(
            ui,
            &self.params,
            &self.telemetry,
            &self.actions,
            &wake,
            &context.param_setter(),
            &mut self.view,
            &mut self.text_entry,
            &mut self.presets,
            &self.waveform,
            &mut self.nav,
        );
    }

    fn editor_closed(&mut self) {
        self.telemetry.set_editor_open(false);
        self.gui_context = None;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn panel(
    ui: &mut Ui,
    params: &MxmMono08Params,
    telemetry: &Telemetry,
    actions: &TransientActions,
    wake: &dyn Fn(),
    setter: &ParamSetter<'_>,
    view: &mut usize,
    text_entry: &mut HashMap<&'static str, Option<String>>,
    presets: &mut PresetUi,
    waveform: &[f32],
    nav: &mut mxm_ui::navigation::State,
) {
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(50));
    // mxm-mono-08 is where the collection's next rules are tried first (`mxm_ui::pilot`), and
    // nothing is piloted now. On before any card is built, because a piloted rule may change a
    // card's size.
    mxm_ui::pilot::enable(ui.ctx());
    // One question, and both layers suspend on it: the paging renderer's `hold` and the cursor's
    // `inert` both ask whether another surface owns this frame's keyboard.
    let busy = presets.holds_the_keyboard() || text_entry.values().any(Option::is_some);
    mxm_ui::paging::editor::hold(ui.ctx(), busy);
    mxm_ui::paging::editor::developer_request(ui.ctx(), view, telemetry.take_view_request());

    // **The keyboard cursor moves before anything is drawn**, so a navigation arrow is consumed
    // here rather than also walking egui's own focus ring. It reads the registry and the exact
    // card rectangles the previous frame built, and it resolves the developer-view request first,
    // because which surface this frame is deciding who owns its keyboard.
    if *view == mxm_ui::paging::PARAMETERS {
        // This surface has no cards. Stop rather than merely hiding the outline, or its controls
        // lose their legacy bare-arrow editing to an invisible stale musician cursor.
        mxm_ui::navigation::stop(ui.ctx());
    } else {
        mxm_ui::navigation::paged_with_bar(ui.ctx(), nav, busy, &[MASTER_CARD]);
    }
    if let Some(open) = telemetry.take_browser_request() {
        presets.set_browser_open(open);
    }
    // Applied and never stored: a capture run must not rewrite the choice made in the control.
    if let Some(preference) = telemetry
        .take_theme_request()
        .and_then(mxm_ui::theme::from_index)
    {
        ui.ctx().set_theme(preference);
    }
    let _ = telemetry.take_disclosure_request(); // This brief has no disclosure; consume the gated request.

    let tokens = tokens_for(ui);
    let peak = telemetry.take_peak();
    let clipped = telemetry.clipped();
    mxm_ui::AppBar::new(crate::NAME).show_with(
        ui,
        &tokens,
        |ui| mxm_preset::ui::preset_row(ui, &tokens, params, setter, presets),
        |ui| {
            if mxm_ui::shell::level_meter(ui, &tokens, peak, clipped) {
                telemetry.clear_clip();
            }
            // Design system §3.1 slot 6: the instrument's output level sits beside its meter, not
            // on a card.
            mxm_ui::navigation::bar_card(ui, MASTER_CARD, |ui| {
                ui.scope(|ui| {
                    sections::bound(params, "master")
                        .slider_inline(ui, &tokens, setter, text_entry, 96.0);
                })
                .response
                .rect
            });
            mxm_ui::shell::zoom_control(ui);
            mxm_ui::shell::editor_theme_control(ui);
        },
    );
    mxm_preset::ui::overlays(ui, &tokens, params, setter, presets);

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(tokens.canvas)
                .inner_margin(egui::Margin::same(SPACE_4 as i8)),
        )
        .show(ui, |ui| {
            if *view == mxm_ui::paging::PARAMETERS {
                parameters_view(ui, &tokens, params, setter, text_entry);
            } else {
                paged_view(
                    ui, &tokens, params, telemetry, actions, wake, setter, text_entry, waveform,
                );
            }
        });
}

/// Card `index`'s title, in paging order.
fn card_title(index: usize) -> &'static str {
    match index {
        0..=5 => sections::SYNTH_TITLES[index],
        6..=8 => sections::MOD_TITLES[index - 6],
        _ => SEQUENCER_TITLES[index - 9],
    }
}

/// Every paging item, each floor computed from its card's tree in `ui`'s fonts — and, the floor being
/// the ceiling, each card exactly as wide as its content.
pub fn page_items(
    ui: &Ui,
    params: &MxmMono08Params,
    telemetry: &Telemetry,
) -> Vec<mxm_ui::paging::Item<'static>> {
    use mxm_ui::{
        flow::Card,
        paging::{Category as C, Item, Key},
    };
    let floor = |index: usize| {
        mxm_ui::tree::card_floor(
            ui,
            card_title(index),
            &sections::card(ui, index, params, telemetry),
        )
    };
    let mut items: Vec<_> = sections::SYNTH_TITLES
        .iter()
        .enumerate()
        .map(|(i, title)| Item {
            key: Key(i as u64),
            card: Card::new(title, floor(i)).capped(floor(i)),
            category: [
                C::Performance,
                C::Generators,
                C::Generators,
                C::Tone,
                C::Tone,
                C::Tone,
            ][i],
            kind: [
                "Voice",
                "Oscillators",
                "Oscillators",
                "Low-pass gates",
                "Low-pass gates",
                "Mixer and reverb",
            ][i],
        })
        .collect();
    items.extend(
        sections::MOD_TITLES
            .iter()
            .enumerate()
            .map(|(i, title)| Item {
                key: Key(6 + i as u64),
                card: Card::new(title, floor(6 + i)).capped(floor(6 + i)),
                category: C::Modulators,
                kind: title,
            }),
    );
    items.extend(
        SEQUENCER_TITLES
            .into_iter()
            .enumerate()
            .map(|(i, title)| Item {
                key: Key(9 + i as u64),
                card: Card::new(title, floor(9 + i)).capped(floor(9 + i)),
                category: C::Sequencers,
                kind: "Sequencer",
            }),
    );
    items
}

#[allow(clippy::too_many_arguments)]
fn paged_view(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmMono08Params,
    telemetry: &Telemetry,
    actions: &TransientActions,
    wake: &dyn Fn(),
    setter: &ParamSetter<'_>,
    entries: &mut HashMap<&'static str, Option<String>>,
    waveform: &[f32],
) {
    use mxm_ui::paging::Key;
    let items = page_items(ui, params, telemetry);
    let text_editing = entries.values().any(Option::is_some);
    // Destructive telemetry is read once, before anything is drawn.
    let mut live = sections::Live {
        params,
        telemetry,
        actions,
        wake,
        setter,
        entries,
        waveform,
        pulses: telemetry.take_pulse_sources(),
        stage: (telemetry.sequence().0, telemetry.take_sequence_pulse()),
    };
    mxm_ui::paging::editor::show(
        ui,
        tokens,
        &items,
        // Random and the inverter were a group of cards and are one card; so were the five steps.
        // The pairs that remain are the parallel branches §3.4 asks to keep on one row.
        &[
            &[Key(1), Key(2)],
            &[Key(3), Key(4)],
            &[Key(6), Key(7)],
            // What drives the run, and the run. Separating them is what lets the faders be tall;
            // wrapping between them would undo the reason they were split.
            &[Key(9), Key(10)],
        ],
        text_editing,
        &mut |ui, index| sections::card(ui, index, params, telemetry),
        &mut |ui, _, leaf, rect| sections::paint(ui, tokens, leaf, rect, &mut live),
    );
}

/// The paging items as the editor computes them, from a context set up as an editor's is — the
/// pilot on, three passes in so the weighted font cuts are bound — for tests, which have no editor
/// `Ui` to hand.
#[cfg(test)]
pub(crate) fn test_items() -> Vec<mxm_ui::paging::Item<'static>> {
    let ctx = egui::Context::default();
    mxm_ui::typography::apply(&ctx);
    mxm_ui::theme::apply(&ctx);
    mxm_ui::pilot::enable(&ctx);
    let params = MxmMono08Params::default();
    let telemetry = Telemetry::default();
    let mut items = Vec::new();
    for _ in 0..3 {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            items = page_items(ui, &params, &telemetry);
        });
        output.textures_delta.clear();
    }
    items
}

/// The cards' floors in paging order, as [`test_items`] computes them.
#[cfg(test)]
pub(crate) fn test_floors() -> Vec<f32> {
    test_items().iter().map(|item| item.card.floor).collect()
}

#[cfg(test)]
fn cards(titles: &[&'static str], floors: &[f32]) -> Vec<mxm_ui::flow::Card<'static>> {
    titles
        .iter()
        .zip(floors)
        .map(|(title, floor)| mxm_ui::flow::Card::new(title, *floor))
        .collect()
}

fn parameters_view(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmMono08Params,
    setter: &ParamSetter<'_>,
    entries: &mut HashMap<&'static str, Option<String>>,
) {
    mxm_ui::shell::scroll_list(ui).show(ui, |ui| {
        let columns = if ui.available_width() >= 1050.0 {
            3
        } else if ui.available_width() >= 620.0 {
            2
        } else {
            1
        };
        let all = params.all_parameters();
        let per_column = all.len().div_ceil(columns);
        ui.columns(columns, |uis| {
            for (column, chunk) in uis.iter_mut().zip(all.chunks(per_column)) {
                for (id, _) in chunk {
                    // A flat list with no card to shorten a name: the full one.
                    sections::bound(params, id)
                        .unlabelled()
                        .slider(column, tokens, setter, entries);
                }
            }
        });
    });
}

fn tokens_for(ui: &Ui) -> Tokens {
    if ui.visuals().dark_mode {
        mxm_ui::DARK
    } else {
        mxm_ui::LIGHT
    }
}

#[cfg(test)]
mod proof;

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::params::internals::ParamPtr;
    use nice_plug::prelude::{PluginApi, PluginState};

    struct NoHost;
    impl nice_plug::context::gui::GuiContextInner for NoHost {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, _param: ParamPtr, _normalized: f32) {}
        unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {}
        fn get_state(&self) -> PluginState {
            PluginState {
                version: String::new(),
                params: Default::default(),
                fields: Default::default(),
            }
        }
        fn set_state(&self, _state: PluginState) {}
    }

    #[test]
    fn dynamic_pages_reach_every_card_and_fit_the_reference_budget() {
        for preference in [egui::ThemePreference::Light, egui::ThemePreference::Dark] {
            for size in [
                egui::vec2(1440.0, 920.0),
                egui::vec2(1880.0, 1040.0),
                egui::vec2(540.0, 420.0),
            ] {
                let ctx = egui::Context::default();
                mxm_ui::typography::apply(&ctx);
                mxm_ui::theme::apply(&ctx);
                ctx.set_theme(preference);
                let params = MxmMono08Params::default();
                let telemetry = Telemetry::default();
                let actions = TransientActions::default();
                let mut view = 0;
                let mut entries = HashMap::new();
                let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
                let mut nav = mxm_ui::navigation::State::default();
                for item in test_items() {
                    mxm_ui::paging::editor::request_card(&ctx, item.key);
                    for _ in 0..3 {
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    size,
                                )),
                                ..Default::default()
                            },
                            |ui| {
                                panel(
                                    ui,
                                    &params,
                                    &telemetry,
                                    &actions,
                                    &|| {},
                                    &ParamSetter::new(&NoHost),
                                    &mut view,
                                    &mut entries,
                                    &mut presets,
                                    &[],
                                    &mut nav,
                                );
                            },
                        );
                        output.textures_delta.clear();
                    }
                    let report = mxm_ui::paging::editor::report(&ctx).unwrap();
                    assert!(
                        report.visible.iter().any(|(key, _)| *key == item.key),
                        "{}: {report:?}",
                        item.card.title
                    );
                    if size.x > 1000.0 {
                        assert!(
                            !report.scrolling,
                            "{} at {size:?}: {report:?}",
                            item.card.title
                        );
                        for (_, rect) in &report.visible {
                            assert!(
                                report.viewport.expand(1.0).contains_rect(*rect),
                                "{rect:?} outside {:?}",
                                report.viewport
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn category_requests_and_diagnostics_have_separate_addresses() {
        assert_eq!(
            mxm_ui::paging::Category::from_request(2),
            Some(mxm_ui::paging::Category::Sequencers)
        );
        assert_eq!(mxm_ui::paging::PARAMETERS, 127);
    }

    #[test]
    fn every_parameter_has_an_editor_binding_and_description() {
        let params = MxmMono08Params::default();
        for (id, _) in params.all_parameters() {
            let bound = sections::bound(&params, id);
            assert!(
                bound.description.ends_with('.'),
                "{id}: {}",
                bound.description
            );
        }
        // 160 before the routing conversion, 340 after it, 369 after D2, 370 with the modulation
        // high range, 349 without the external input: 149 routes with an amount and a presence
        // each, the twelve trigger enables, and the thirty-nine panel controls. 350 with the clock
        // period's tempo sync; 380 with the standard Amplitude's fifteen routes.
        assert_eq!(params.all_parameters().len(), 380);
    }

    #[test]
    fn the_one_card_minimum_holds_the_widest_declared_floor_plus_gutters() {
        let floors = test_floors();
        let synth = cards(sections::SYNTH_TITLES, &floors[..6]);
        let modulation = cards(sections::MOD_TITLES, &floors[6..9]);
        // The sequencer belongs here too. It was five narrow stage cards and this check skipped
        // them; one card holding a five-column step row is exactly the case that can outgrow the
        // one-card window, so it is the one that most needs asserting.
        let sequence = cards(&SEQUENCER_TITLES, &floors[9..]);
        let widest = mxm_ui::flow::minimum_width(&synth)
            .max(mxm_ui::flow::minimum_width(&modulation))
            .max(mxm_ui::flow::minimum_width(&sequence));
        assert!(widest + 2.0 * SPACE_4 <= MINIMUM.0 as f32);
    }

    #[test]
    fn every_view_lays_out_at_reference_and_one_card_width_in_both_themes() {
        for preference in [egui::ThemePreference::Light, egui::ThemePreference::Dark] {
            for zoom in [1.0, 2.0] {
                for size in [
                    egui::vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
                    egui::vec2(MINIMUM.0 as f32, MINIMUM.1 as f32),
                ] {
                    for requested_view in [0, 1, 2, 3, 4, 5, 127] {
                        let ctx = egui::Context::default();
                        mxm_ui::typography::apply(&ctx);
                        ctx.set_theme(preference);
                        ctx.set_zoom_factor(zoom);
                        mxm_ui::theme::apply(&ctx);
                        let params = MxmMono08Params::default();
                        let telemetry = Telemetry::default();
                        telemetry.request_view(requested_view as u8);
                        let actions = TransientActions::default();
                        let setter = ParamSetter::new(&NoHost);
                        let mut view = requested_view;
                        let mut entries = HashMap::new();
                        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
                        let mut nav = mxm_ui::navigation::State::default();
                        let input = egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                            ..Default::default()
                        };
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
                                )
                            });
                            output.textures_delta.clear();
                        }
                        assert!(ctx.globally_used_rect().height() > 0.0);
                    }
                }
            }
        }
    }
}
