//! Small custom buttons for the menus and the style panel.

use eframe::egui::{self, Align2, Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};

use crate::paint::paint_checkerboard;
use crate::stroke::paint_hand_drawn;
use crate::style::{FONT_SIZES, SizeKind};

/// A menu entry with its keyboard shortcut shown on the right; true when clicked.
pub fn menu_item(ui: &mut egui::Ui, label: &str, shortcut: Option<egui::KeyboardShortcut>, enabled: bool) -> bool {
    let mut button = egui::Button::new(label);
    if let Some(shortcut) = shortcut {
        button = button.shortcut_text(ui.ctx().format_shortcut(&shortcut));
    }
    ui.add_enabled(enabled, button).clicked()
}

/// A selectable color button; `None` shows a checkerboard (transparent). Returns whether it was clicked.
pub fn color_swatch(ui: &mut egui::Ui, selected: bool, name: &str, color: Option<Color32>) -> bool {
    let button = egui::Button::selectable(selected, "").min_size(Vec2::splat(22.0));
    let response = ui.add(button).on_hover_text(name);
    let swatch = Rect::from_center_size(response.rect.center(), Vec2::splat(14.0));
    match color {
        Some(color) => _ = ui.painter().rect_filled(swatch, 2.0, color),
        None => paint_checkerboard(&ui.painter().with_clip_rect(swatch), swatch, 3.5, 150),
    }
    ui.painter().rect_stroke(swatch, 2.0, Stroke::new(1.0, Color32::GRAY), StrokeKind::Outside);
    response.clicked()
}

/// A selectable size button: a line sample for line widths, S/M/L/XL for font sizes.
/// Returns whether it was clicked.
pub fn size_button(ui: &mut egui::Ui, selected: bool, kind: SizeKind, size: f32) -> bool {
    let hover = format!("{size:.0}px");
    let response = match kind {
        SizeKind::Line => {
            let button = egui::Button::selectable(selected, "").min_size(Vec2::new(30.0, 22.0));
            let response = ui.add(button);
            let r = response.rect.shrink2(Vec2::new(7.0, 0.0));
            let stroke = Stroke::new(size.min(r.height() - 6.0), ui.visuals().text_color());
            ui.painter().line_segment([r.left_center(), r.right_center()], stroke);
            response
        }
        SizeKind::Font => {
            // Resized text can have any size; show it in pixels then.
            let label = match FONT_SIZES.iter().position(|&s| s == size) {
                Some(i) => ["S", "M", "L", "XL"][i].to_owned(),
                None => hover.clone(),
            };
            let button = egui::Button::selectable(selected, label).min_size(Vec2::new(30.0, 22.0));
            ui.add(button)
        }
    };
    response.on_hover_text(hover).clicked()
}

/// A selectable pen style button with a sample stroke: plain or hand-drawn.
pub fn pen_button(ui: &mut egui::Ui, selected: bool, hand_drawn: bool) -> bool {
    let button = egui::Button::selectable(selected, "").min_size(Vec2::new(30.0, 22.0));
    let response = ui.add(button);
    let r = response.rect.shrink2(Vec2::new(7.0, 6.0));
    let points: Vec<Pos2> = (0..=16)
        .map(|i| {
            let t = i as f32 / 16.0;
            Pos2::new(r.left() + t * r.width(), r.center().y + (t * std::f32::consts::TAU).sin() * r.height() / 2.0)
        })
        .collect();
    let color = ui.visuals().text_color();
    if hand_drawn {
        // Thick at the start, thinning out: like a quick pen stroke.
        let pressures: Vec<f32> = (0..points.len()).map(|i| 1.0 - i as f32 / points.len() as f32).collect();
        paint_hand_drawn(ui.painter(), &points, &pressures, 2.5, color);
    } else {
        ui.painter().line(points, Stroke::new(1.5, color));
    }
    let name = if hand_drawn { "Hand-drawn: smoothed, faster strokes are thinner" } else { "Plain line" };
    response.on_hover_text(name).clicked()
}

/// E.g. "top left", "center".
pub fn align_name(align: Align2) -> String {
    let v = ["top", "middle", "bottom"][align.y() as usize];
    let h = ["left", "center", "right"][align.x() as usize];
    match (v, h) {
        ("middle", "center") => "center".to_owned(),
        _ => format!("{v} {h}"),
    }
}

/// A selectable alignment button: a box with a small block where the text sits.
pub fn align_button(ui: &mut egui::Ui, selected: bool, align: Align2) -> bool {
    let button = egui::Button::selectable(selected, "").min_size(Vec2::splat(22.0));
    let response = ui.add(button).on_hover_text(format!("Align {}", align_name(align)));
    let frame = Rect::from_center_size(response.rect.center(), Vec2::splat(14.0));
    let color = ui.visuals().text_color();
    ui.painter().rect_stroke(frame, 1.0, Stroke::new(1.0, color.gamma_multiply(0.5)), StrokeKind::Inside);
    let block = align.align_size_within_rect(Vec2::new(8.0, 4.0), frame.shrink(2.0));
    ui.painter().rect_filled(block, 0.0, color);
    response.clicked()
}
