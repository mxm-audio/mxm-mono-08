//! Token-driven, observational displays for the complex wave, gates, modulation sources and steps.

use egui::{Pos2, Sense, Stroke, Ui, Vec2, pos2};
use mxm_ui::theme::Tokens;

pub fn waveform(ui: &mut Ui, tokens: &Tokens, samples: &[f32]) {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), WAVEFORM_HEIGHT),
        Sense::hover(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Complex waveform")
    });
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, tokens.surface_2);
    painter.line_segment(
        [
            pos2(rect.left(), rect.center().y),
            pos2(rect.right(), rect.center().y),
        ],
        Stroke::new(1.0, tokens.border),
    );
    if samples.len() > 1 {
        let peak = samples
            .iter()
            .fold(0.05_f32, |peak, sample| peak.max(sample.abs()));
        let stride = (samples.len() / rect.width().max(1.0) as usize).max(1);
        let visible: Vec<_> = samples.iter().step_by(stride).collect();
        let points: Vec<Pos2> = visible
            .iter()
            .enumerate()
            .map(|(index, sample)| {
                let x = index as f32 / (visible.len() - 1).max(1) as f32;
                pos2(
                    rect.left() + x * rect.width(),
                    rect.center().y - **sample / peak * (rect.height() * 0.42),
                )
            })
            .collect();
        painter.add(egui::Shape::line(points, Stroke::new(1.5, tokens.accent)));
    }
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, tokens.border),
        egui::StrokeKind::Inside,
    );
}

pub fn gate(ui: &mut Ui, tokens: &Tokens, name: &str, level: f32) {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), GATE_HEIGHT), Sense::hover());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ProgressIndicator, true, name));
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, tokens.surface_2);
    let value = level.clamp(0.0, 1.0);
    let y = rect.bottom() - value * rect.height();
    let points = [
        pos2(rect.left(), rect.bottom()),
        pos2(rect.left() + rect.width() * 0.15, y),
        pos2(rect.left() + rect.width() * 0.38, y),
        pos2(rect.right(), rect.bottom()),
    ];
    painter.add(egui::Shape::line(
        points.to_vec(),
        Stroke::new(2.0, tokens.mod_envelope),
    ));
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, tokens.border),
        egui::StrokeKind::Inside,
    );
}

/// The shortest a source's bar is drawn beside its name.
const LEVEL_BAR_MIN: f32 = 48.0;
/// The complex oscillator's waveform display's height; it fills its card's width.
pub const WAVEFORM_HEIGHT: f32 = 104.0;
/// A low-pass gate's response display's height; it fills its card's width.
pub const GATE_HEIGHT: f32 = 44.0;
/// The flash beside a trigger switch: a square this wide.
pub const PULSE_FLASH: f32 = 12.0;
/// The level bar's thickness in `source_levels`.
const LEVEL_BAR: f32 = 8.0;

/// What [`source_levels`] occupies for these names, without drawing it: each row the widest name,
/// the row's spacing and the shortest bar, one interact row tall, rows an item spacing apart.
pub fn source_levels_size(ui: &Ui, names: &[&str]) -> Vec2 {
    let font = mxm_ui::typography::caption_style(ui.style()).resolve(ui.style());
    let name_width = names
        .iter()
        .map(|name| {
            ui.painter()
                .layout_no_wrap((*name).to_owned(), font.clone(), egui::Color32::PLACEHOLDER)
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    let line = ui.fonts_mut(|fonts| fonts.row_height(&font));
    let row = ui.spacing().interact_size.y.max(line).max(LEVEL_BAR);
    let rows = names.len() as f32;
    Vec2::new(
        name_width + ui.spacing().item_spacing.x + LEVEL_BAR_MIN,
        row * rows + ui.spacing().item_spacing.y * (rows - 1.0).max(0.0),
    )
}

/// Each modulation source's live level: **one row per source**, its name then its bar.
///
/// They were side-by-side columns with the name over the bar, and once the card hugged its content
/// a column was narrower than *Mod oscillator*: the names broke mid-word and re-wrapped as the
/// bars moved (the owner, 2026-09-24: *"text is dancing"*). A row gives every name its own width,
/// painted unwrapped in the caption style, and the bars start on one line so their levels compare.
pub fn source_levels(ui: &mut Ui, tokens: &Tokens, values: &[(&str, f32, egui::Color32)]) {
    let font = mxm_ui::typography::caption_style(ui.style()).resolve(ui.style());
    let name_width = values
        .iter()
        .map(|(name, ..)| {
            ui.painter()
                .layout_no_wrap((*name).to_owned(), font.clone(), tokens.text_secondary)
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    let line = ui.fonts_mut(|fonts| fonts.row_height(&font));
    for &(name, value, colour) in values {
        ui.horizontal(|ui| {
            let (label, _) = ui.allocate_exact_size(Vec2::new(name_width, line), Sense::hover());
            ui.painter().text(
                label.left_center(),
                egui::Align2::LEFT_CENTER,
                name,
                font.clone(),
                tokens.text_secondary,
            );
            let value = value.clamp(0.0, 1.0);
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(ui.available_width().max(LEVEL_BAR_MIN), LEVEL_BAR),
                Sense::hover(),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::ProgressIndicator, true, name)
            });
            ui.painter().rect_filled(rect, 4.0, tokens.surface_2);
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    rect.min,
                    egui::vec2(rect.width() * value, rect.height()),
                ),
                4.0,
                colour,
            );
        });
    }
}

/// The flash beside a trigger switch: always allocated, so that enabling the switch moves nothing.
/// Off is a faint ring, armed a strong one, firing a filled warning dot.
pub fn pulse_route(ui: &mut Ui, tokens: &Tokens, source: &str, enabled: bool, firing: bool) {
    let label = format!(
        "{source} trigger route {}",
        if !enabled {
            "off"
        } else if firing {
            "firing"
        } else {
            "armed"
        }
    );
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(PULSE_FLASH), Sense::hover());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label.clone()));
    let painter = ui.painter_at(rect);
    if !enabled {
        painter.circle_stroke(rect.center(), 5.0, Stroke::new(1.0, tokens.border));
    } else if firing {
        painter.circle_filled(rect.center(), 5.0, tokens.warning);
    } else {
        painter.circle_stroke(rect.center(), 5.0, Stroke::new(1.0, tokens.border_strong));
    }
}

/// The marker above one step's fader: where the sequence is, and whether this step just sent its
/// trigger.
///
/// **Always the same size in every state**, so nothing moves under the pointer while the sequence
/// runs (design system §15) and the five columns stay aligned.
///
/// The three states it has to tell apart are idle, active, and active-and-triggering, and **none of
/// them may rest on hue** — §15 again, and the owner is red-green colour-blind. So each state adds
/// a channel rather than swapping a colour: idle is an empty outline, active is solid, and a
/// trigger puts a filled dot in the middle of the solid block. The dot is `pulse_route`'s own convention,
/// one card away, which is why it is the mark used here rather than a second accent.
///
/// It no longer paints the value: the fader beneath it shows that, and printing it twice in a
/// column this narrow spends the width the fader needs. The accessible label still carries it.
pub fn stage(ui: &mut Ui, tokens: &Tokens, index: usize, active: bool, pulsed: bool, value: f32) {
    let (rect, response) = ui.allocate_exact_size(MARKER, Sense::hover());
    let label = format!(
        "Step {}{}{}, value {:.0} percent",
        index + 1,
        if active { ", active" } else { "" },
        if pulsed { ", trigger" } else { "" },
        value.clamp(0.0, 1.0) * 100.0,
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label.clone()));
    let painter = ui.painter_at(rect);
    // Solid when the sequence is here, an empty outline when it is not. A filled block reads as a
    // position on a row of five; an outlined box beside a row of switches reads as another switch,
    // which is what an earlier draft of this looked like.
    if active {
        painter.rect_filled(rect, 3.0, tokens.accent);
    } else {
        painter.rect_filled(rect, 3.0, tokens.surface_2);
        painter.rect_stroke(
            rect,
            3.0,
            Stroke::new(1.0, tokens.border),
            egui::StrokeKind::Inside,
        );
    }
    if pulsed {
        painter.circle_filled(rect.center(), rect.height() * 0.3, tokens.warning);
    }
}

/// The step marker: a short bar over its fader, wider than it is tall so it reads as a position
/// rather than a control.
pub const MARKER: egui::Vec2 = egui::vec2(28.0, 8.0);
