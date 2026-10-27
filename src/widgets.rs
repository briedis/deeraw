//! Small custom buttons for the menus and the style panel.

use eframe::egui::{self, Align, Align2, Color32, Pos2, Rect, Stroke, StrokeKind, Vec2, epaint::PathShape};

use crate::object::{ArrowHeads, BlurStrength, Head, Outline};
use crate::paint::{paint_arrow, paint_checkerboard};
use crate::stroke::paint_hand_drawn;
use crate::style::{FONT_SIZES, PALETTE, SizeKind};

/// Padding around a dialog's content (its title bar keeps egui's default).
pub const DIALOG_MARGIN: f32 = 16.0;

/// A fixed-size window in the middle of the screen, laid out alike for every dialog (About, Changelog,
/// Preferences, updates): the same margins, row spacing and button size. Closed by its × (`open`). Kept on
/// top of the toolbar and style panel (`Order::Foreground` areas), even after they're clicked.
pub fn dialog(ctx: &egui::Context, title: &str, open: &mut bool, content: impl FnOnce(&mut egui::Ui)) {
    let style = ctx.global_style();
    let shown = egui::Window::new(title)
        .order(egui::Order::Foreground)
        .open(open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .title_frame(egui::Frame::window(&style))
        .frame(egui::Frame::window(&style).inner_margin(DIALOG_MARGIN))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.spacing_mut().button_padding = Vec2::new(10.0, 5.0);
            content(ui);
        });
    if let Some(shown) = shown {
        ctx.move_to_top(shown.response.layer_id);
    }
}

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
    let response = color_swatch_sized(ui, selected, name, color, Vec2::splat(22.0), 14.0);
    pressed(ui, &response)
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
    let response = response.on_hover_text(hover);
    pressed(ui, &response)
}

/// A selectable struck-through circle, for no border (shapes) or no outline (text, arrows and lines).
pub fn no_border_button(ui: &mut egui::Ui, selected: bool, name: &str) -> bool {
    let button = egui::Button::selectable(selected, "").min_size(Vec2::new(30.0, 22.0));
    let response = ui.add(button);
    let center = response.rect.center();
    let stroke = Stroke::new(1.5, ui.visuals().text_color());
    let radius = 6.0;
    ui.painter().circle_stroke(center, radius, stroke);
    let d = Vec2::splat(radius * std::f32::consts::FRAC_1_SQRT_2);
    ui.painter().line_segment([center - Vec2::new(-d.x, d.y), center + Vec2::new(-d.x, d.y)], stroke);
    let response = response.on_hover_text(name);
    pressed(ui, &response)
}

/// The outline section's button: a capsule outline, filled with the outline's color while it has one
/// (gray when automatic).
pub fn outline_button(ui: &mut egui::Ui, open: bool, outline: Outline) -> bool {
    let button = egui::Button::selectable(open, "").min_size(Vec2::new(30.0, 22.0));
    let name = match (outline.on, outline.color) {
        (false, _) => "No outline".to_owned(),
        (true, None) => format!("Outline: automatic color, {:.0}px", outline.width),
        (true, Some(color)) => {
            let name = PALETTE.iter().find(|(c, _)| *c == color).map_or("Color", |(_, n)| n);
            format!("Outline: {name}, {:.0}px", outline.width)
        }
    };
    let response = ui.add(button).on_hover_text(name);
    let color = if open { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
    let capsule = Rect::from_center_size(response.rect.center(), Vec2::new(16.0, 8.0));
    if outline.on {
        ui.painter().rect_filled(capsule, 4.0, outline.color.unwrap_or(Color32::GRAY));
    }
    ui.painter().rect_stroke(capsule, 4.0, Stroke::new(1.5, color), StrokeKind::Middle);
    pressed(ui, &response)
}

/// The automatic outline color option: a swatch half black, half white.
pub fn auto_color_button(ui: &mut egui::Ui, selected: bool) -> bool {
    let button = egui::Button::selectable(selected, "").min_size(Vec2::splat(22.0));
    let response = ui.add(button).on_hover_text("Automatic: black or white, whichever stands out");
    let swatch = Rect::from_center_size(response.rect.center(), Vec2::splat(14.0));
    let (left, right) = swatch.split_left_right_at_fraction(0.5);
    ui.painter().rect_filled(left, egui::CornerRadius { nw: 2, sw: 2, ..Default::default() }, Color32::BLACK);
    ui.painter().rect_filled(right, egui::CornerRadius { ne: 2, se: 2, ..Default::default() }, Color32::WHITE);
    ui.painter().rect_stroke(swatch, 2.0, Stroke::new(1.0, Color32::GRAY), StrokeKind::Outside);
    pressed(ui, &response)
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
    let response = response.on_hover_text(name);
    pressed(ui, &response)
}

/// A selectable button showing a short arrow with `head` at its `start` (pointing left) or
/// its end (pointing right).
pub fn arrow_head_button(ui: &mut egui::Ui, selected: bool, head: Head, start: bool, hint: &str) -> bool {
    let button = egui::Button::selectable(selected, "").min_size(Vec2::new(36.0, 22.0));
    let response = ui.add(button).on_hover_text(hint);
    let r = response.rect.shrink2(Vec2::new(7.0, 0.0));
    let color = if selected { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
    let heads = if start { ArrowHeads { start: head, end: Head::None } } else { ArrowHeads { start: Head::None, end: head } };
    // Sized to look like a 2px arrow at 100%.
    paint_arrow(ui.painter(), r.left_center(), r.right_center(), Stroke::new(1.5, color), 0.45, heads, None);
    pressed(ui, &response)
}

/// A selectable blur strength button: a checkered square, 4×4 (light), 3×3 or 2×2 (strong).
pub fn blur_button(ui: &mut egui::Ui, selected: bool, strength: BlurStrength) -> bool {
    let button = egui::Button::selectable(selected, "").min_size(Vec2::splat(22.0));
    let response = ui.add(button).on_hover_text(format!("Blur: {}", strength.name()));
    let cells = match strength {
        BlurStrength::Light => 4,
        BlurStrength::Medium => 3,
        BlurStrength::Strong => 2,
    };
    // Whole physical pixels per cell, on the pixel grid: fractional cells blur together.
    let ppp = ui.ctx().pixels_per_point();
    let cell = (16.0 * ppp / cells as f32).floor() / ppp;
    let size = Vec2::splat(cell * cells as f32);
    let min = ((response.rect.center() - size / 2.0).to_vec2() * ppp).round().to_pos2() / ppp;
    let square = Rect::from_min_size(min, size);
    // Like the icons: solid cells in the icon color (white on the blue selected button), the
    // others left empty.
    let color = if selected { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
    for y in 0..cells {
        for x in (0..cells).filter(|x| (x + y) % 2 == 0) {
            let min = square.min + Vec2::new(x as f32, y as f32) * cell;
            ui.painter().rect_filled(Rect::from_min_size(min, Vec2::splat(cell)), 0.0, color);
        }
    }
    pressed(ui, &response)
}

/// E.g. "top left", "center".
pub fn align_name(align: Align2) -> String {
    let v = ["top", "middle", "bottom"][align.y() as usize];
    let h = ["left", "center", "right"][align.x() as usize];
    match v {
        "middle" => h.to_owned(),
        _ => format!("{v} {h}"),
    }
}

/// A selectable horizontal alignment button (lines of text, like a word processor's); true when clicked.
pub fn align_x_button(ui: &mut egui::Ui, selected: bool, x: Align) -> bool {
    let (icon, name) = match x {
        Align::Min => (Icon::AlignLeft, "left"),
        Align::Center => (Icon::AlignCenter, "center"),
        Align::Max => (Icon::AlignRight, "right"),
    };
    icon_button(ui, selected, icon, 14.0, &format!("Align {name}"))
}

/// A selectable vertical alignment button (lines at the top, middle or bottom of a box); true when clicked.
pub fn align_y_button(ui: &mut egui::Ui, selected: bool, y: Align) -> bool {
    let (icon, name) = match y {
        Align::Min => (Icon::AlignTop, "top"),
        Align::Center => (Icon::AlignMiddle, "middle"),
        Align::Max => (Icon::AlignBottom, "bottom"),
    };
    icon_button(ui, selected, icon, 14.0, &format!("Align {name}"))
}

/// A button icon, traced from the Deeraw icon set.
#[derive(Clone, Copy)]
pub enum Icon {
    Select,
    Pencil,
    Arrow,
    Line,
    Text,
    Rectangle,
    /// Painted as a circle, not from strokes.
    Ellipse,
    Diamond,
    /// A droplet.
    Blur,
    Crop,
    StrokeWidth,
    /// Corner brackets around a crosshair.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Screenshot,
    /// A U-turn arrow pointing left.
    Undo,
    /// A U-turn arrow pointing right.
    Redo,
    /// Lines of text aligned left, centered or right.
    AlignLeft,
    AlignCenter,
    AlignRight,
    /// Two lines at the top, middle or bottom of a box.
    AlignTop,
    AlignMiddle,
    AlignBottom,
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
            Icon::Rectangle => &[(&[[5.0, 7.0], [27.0, 7.0], [27.0, 25.0], [5.0, 25.0]], true, W)],
            Icon::Ellipse => &[],
            Icon::Diamond => &[(&[[16.0, 4.0], [28.0, 16.0], [16.0, 28.0], [4.0, 16.0]], true, W)],
            Icon::Blur => &[
                (
                    &[
                        [16.0, 3.5], [23.2, 15.0], [24.2, 17.4], [24.5, 19.9], [24.0, 22.4], [22.7, 24.7], [20.9, 26.5], [18.5, 27.6],
                        [16.0, 28.0], [13.5, 27.6], [11.1, 26.5], [9.3, 24.7], [8.0, 22.4], [7.5, 19.9], [7.8, 17.4], [8.8, 15.0],
                    ],
                    true,
                    W,
                ),
                (&[[11.5, 20.0], [12.3, 22.8], [14.5, 24.5]], false, 1.8),
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
            Icon::Screenshot => &[
                (&[[4.0, 11.0], [4.0, 4.0], [11.0, 4.0]], false, W),
                (&[[21.0, 4.0], [28.0, 4.0], [28.0, 11.0]], false, W),
                (&[[28.0, 21.0], [28.0, 28.0], [21.0, 28.0]], false, W),
                (&[[11.0, 28.0], [4.0, 28.0], [4.0, 21.0]], false, W),
                (&[[16.0, 11.0], [16.0, 21.0]], false, W),
                (&[[11.0, 16.0], [21.0, 16.0]], false, W),
            ],
            Icon::Undo => &[
                (&[[10.0, 8.0], [5.0, 13.0], [10.0, 18.0]], false, W),
                (
                    &[[5.0, 13.0], [19.0, 13.0], [22.25, 13.87], [24.63, 16.25], [25.5, 19.5], [24.63, 22.75], [22.25, 25.13], [19.0, 26.0], [12.0, 26.0]],
                    false,
                    W,
                ),
            ],
            Icon::Redo => &[
                (&[[22.0, 8.0], [27.0, 13.0], [22.0, 18.0]], false, W),
                (
                    &[[27.0, 13.0], [13.0, 13.0], [9.75, 13.87], [7.37, 16.25], [6.5, 19.5], [7.37, 22.75], [9.75, 25.13], [13.0, 26.0], [20.0, 26.0]],
                    false,
                    W,
                ),
            ],
            Icon::AlignLeft => &[
                (&[[5.0, 7.0], [27.0, 7.0]], false, W),
                (&[[5.0, 13.0], [19.0, 13.0]], false, W),
                (&[[5.0, 19.0], [27.0, 19.0]], false, W),
                (&[[5.0, 25.0], [19.0, 25.0]], false, W),
            ],
            Icon::AlignCenter => &[
                (&[[5.0, 7.0], [27.0, 7.0]], false, W),
                (&[[9.0, 13.0], [23.0, 13.0]], false, W),
                (&[[5.0, 19.0], [27.0, 19.0]], false, W),
                (&[[9.0, 25.0], [23.0, 25.0]], false, W),
            ],
            Icon::AlignRight => &[
                (&[[5.0, 7.0], [27.0, 7.0]], false, W),
                (&[[13.0, 13.0], [27.0, 13.0]], false, W),
                (&[[5.0, 19.0], [27.0, 19.0]], false, W),
                (&[[13.0, 25.0], [27.0, 25.0]], false, W),
            ],
            Icon::AlignTop => &[
                (&[[4.0, 4.0], [28.0, 4.0], [28.0, 28.0], [4.0, 28.0]], true, 2.0),
                (&[[10.0, 10.0], [22.0, 10.0]], false, W),
                (&[[10.0, 15.0], [22.0, 15.0]], false, W),
            ],
            Icon::AlignMiddle => &[
                (&[[4.0, 4.0], [28.0, 4.0], [28.0, 28.0], [4.0, 28.0]], true, 2.0),
                (&[[10.0, 13.5], [22.0, 13.5]], false, W),
                (&[[10.0, 18.5], [22.0, 18.5]], false, W),
            ],
            Icon::AlignBottom => &[
                (&[[4.0, 4.0], [28.0, 4.0], [28.0, 28.0], [4.0, 28.0]], true, 2.0),
                (&[[10.0, 17.0], [22.0, 17.0]], false, W),
                (&[[10.0, 22.0], [22.0, 22.0]], false, W),
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
    if let Icon::Ellipse = icon {
        ui.painter().circle_stroke(origin + Vec2::splat(size / 2.0), 11.0 * scale, Stroke::new(W * scale, color));
    }
    pressed(ui, &response)
}

/// Whether `response`'s button was clicked: also when the pointer moved more than 6pt or the
/// press lasted over 0.8s, as long as it was pressed and released on the button. egui's `clicked`
/// drops those, so a quick click on the way to the canvas, or a slow one, did nothing.
pub fn pressed(ui: &egui::Ui, response: &egui::Response) -> bool {
    let id = response.id.with("pressed");
    let down_on = response.is_pointer_button_down_on() || ui.data(|d| d.get_temp(id)).unwrap_or(false);
    let (down, released, pos) = ui.input(|i| (i.pointer.primary_down(), i.pointer.primary_released(), i.pointer.interact_pos()));
    ui.data_mut(|d| d.insert_temp(id, down_on && down));
    response.clicked() || (down_on && released && pos.is_some_and(|p| response.rect.contains(p)))
}
