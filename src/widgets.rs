//! Small custom buttons for the menus and the style panel.

use eframe::egui::{self, Align2, Color32, Pos2, Rect, Stroke, StrokeKind, Vec2, epaint::PathShape};

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
    color_swatch_sized(ui, selected, name, color, Vec2::splat(22.0), 14.0).clicked()
}

/// A color swatch of `swatch` points on a selectable button of at least `button`.
pub fn color_swatch_sized(
    ui: &mut egui::Ui,
    selected: bool,
    name: &str,
    color: Option<Color32>,
    button: Vec2,
    swatch: f32,
) -> egui::Response {
    let button = egui::Button::selectable(selected, "").min_size(button);
    let response = ui.add(button).on_hover_text(name);
    let swatch = Rect::from_center_size(response.rect.center(), Vec2::splat(swatch));
    match color {
        Some(color) => _ = ui.painter().rect_filled(swatch, 2.0, color),
        None => paint_checkerboard(&ui.painter().with_clip_rect(swatch), swatch, 3.5, 150),
    }
    ui.painter().rect_stroke(swatch, 2.0, Stroke::new(1.0, Color32::GRAY), StrokeKind::Outside);
    response
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

/// A button icon, traced from the Deeraw icon set.
#[derive(Clone, Copy)]
pub enum Icon {
    Select,
    Pencil,
    Arrow,
    Line,
    Text,
    Crop,
    StrokeWidth,
}

/// The icon set's stroke width, on its 32×32 grid.
const W: f32 = 2.5;

impl Icon {
    /// Its strokes on a 32×32 grid: points, whether the stroke is closed, and its width.
    fn strokes(self) -> &'static [(&'static [[f32; 2]], bool, f32)] {
        match self {
            Icon::Select => &[(&[[6.0, 6.0], [27.0, 14.0], [18.0, 18.0], [14.0, 27.0]], true, W)],
            Icon::Pencil => &[
                (&[[24.0, 4.0], [28.0, 8.0], [10.0, 26.0], [4.0, 28.0], [6.0, 22.0]], true, W),
                (&[[18.0, 10.0], [22.0, 14.0]], false, W),
            ],
            Icon::Arrow => &[(&[[6.0, 26.0], [26.0, 6.0]], false, W), (&[[14.0, 6.0], [26.0, 6.0], [26.0, 18.0]], false, W)],
            Icon::Line => &[(&[[4.0, 16.0], [28.0, 16.0]], false, W)],
            Icon::Text => &[
                (&[[7.0, 10.0], [7.0, 6.0], [25.0, 6.0], [25.0, 10.0]], false, W),
                (&[[16.0, 6.0], [16.0, 26.0]], false, W),
                (&[[12.0, 26.0], [20.0, 26.0]], false, W),
            ],
            Icon::Crop => &[
                (&[[8.0, 3.0], [8.0, 24.0], [29.0, 24.0]], false, W),
                (&[[24.0, 29.0], [24.0, 8.0], [3.0, 8.0]], false, W),
                (&[[8.0, 24.0], [27.0, 5.0]], false, W),
            ],
            Icon::StrokeWidth => &[
                (&[[5.0, 9.0], [27.0, 9.0]], false, 3.5),
                (&[[5.0, 16.0], [27.0, 16.0]], false, 3.5),
                (&[[5.0, 23.0], [27.0, 23.0]], false, 3.5),
            ],
        }
    }
}

/// A selectable button showing `icon` at `size` points; true when clicked.
pub fn icon_button(ui: &mut egui::Ui, selected: bool, icon: Icon, size: f32, hint: &str) -> bool {
    let button = egui::Button::selectable(selected, "").min_size(Vec2::splat(size) + Vec2::new(12.0, 8.0));
    let response = ui.add(button).on_hover_text(hint);
    let origin = response.rect.center() - Vec2::splat(size / 2.0);
    let scale = size / 32.0;
    let color = if selected { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
    for &(points, closed, width) in icon.strokes() {
        let points = points.iter().map(|&[x, y]| origin + Vec2::new(x, y) * scale).collect();
        let stroke = Stroke::new(width * scale, color);
        let path = if closed { PathShape::closed_line(points, stroke) } else { PathShape::line(points, stroke) };
        ui.painter().add(path);
    }
    response.clicked()
}
