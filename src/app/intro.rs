//! The intro shown on an empty canvas: the deer and the ways to get an image in.

use eframe::egui::{self, Color32, Key, KeyboardShortcut, Modifiers, Painter, Rect, Shape, Stroke, UiBuilder, Vec2, ViewportCommand};
use eframe::egui::epaint::CubicBezierShape;

use super::{DeerawApp, Tool};
use crate::style::{Background, canvas_ui_visuals};

/// Width of the intro's column, in points.
const WIDTH: f32 = 280.0;
/// Size of the deer, in points.
const LOGO_SIZE: f32 = 88.0;

/// The deer from `assets/deer.svg` (1000×1000 viewBox, before its transform), as paths of cubic
/// Bézier segments: a start point, then each segment's two control points and end point.
const DEER_PATHS: [&[[f32; 2]]; 7] = [
    // Left antler.
    &[[232., 280.], [215., 320.], [240., 350.], [300., 395.], [360., 440.], [420., 480.], [462., 515.]],
    &[[385., 200.], [355., 255.], [330., 330.], [322., 400.]],
    &[[423., 377.], [412., 410.], [412., 445.], [418., 482.]],
    // Right antler.
    &[[655., 197.], [670., 260.], [655., 330.], [632., 385.], [620., 430.], [605., 470.], [590., 515.]],
    &[[577., 292.], [590., 325.], [605., 350.], [628., 378.]],
    &[[773., 292.], [745., 340.], [700., 378.], [638., 405.]],
    // Head (closed).
    &[
        [465., 520.], [410., 525.], [345., 530.], [337., 565.], [330., 600.], [400., 610.], [440., 632.],
        [430., 690.], [412., 770.], [440., 830.], [470., 860.], [520., 800.], [575., 758.],
        [620., 728.], [680., 725.], [718., 708.], [755., 692.], [778., 665.], [760., 645.],
        [728., 625.], [668., 600.], [622., 540.], [570., 518.], [510., 518.], [465., 520.],
    ],
];
/// The deer's eye and nose: center and radius.
const DEER_DOTS: [([f32; 2], f32); 2] = [([570., 620.], 22.), ([745., 664.], 36.)];
/// The SVG's stroke width and the point it scales around (its transform's inner translate).
const DEER_STROKE: f32 = 40.0;
const DEER_CENTER: [f32; 2] = [509., 518.];

impl DeerawApp {
    /// At startup: the deer, buttons for pasting, taking a screenshot and opening an image, and
    /// a hint that drawing works right away. Only the buttons take input, so drawing or dragging
    /// over the rest works as on any empty canvas. Once something is added or another tool is
    /// picked it's gone, also when the canvas is emptied again.
    pub(super) fn intro_ui(&mut self, ui: &mut egui::Ui, canvas_rect: Rect) {
        self.intro_dismissed |= !self.objects.is_empty() || self.tool != Tool::Select || self.drawing.is_some();
        if self.intro_dismissed {
            return;
        }
        let (logo_color, text_color) = match self.background {
            Background::Black => (Color32::from_rgb(71, 85, 105), Color32::from_rgb(148, 163, 184)),
            Background::White | Background::Transparent => (Color32::from_rgb(203, 213, 225), Color32::from_rgb(100, 116, 139)),
        };
        // A little above the center, clear of the toolbar.
        let top = canvas_rect.center() - Vec2::new(WIDTH / 2.0, 170.0);
        let rect = Rect::from_min_size(top, Vec2::new(WIDTH, 340.0));
        let mut ui = ui.new_child(UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Center)));
        ui.set_visuals(canvas_ui_visuals(self.background));

        let (logo, _) = ui.allocate_exact_size(Vec2::splat(LOGO_SIZE), egui::Sense::hover());
        paint_deer(ui.painter(), logo, logo_color);
        ui.add_space(12.0);
        ui.label(egui::RichText::new("Add an image to mark it up").size(16.0).color(text_color));
        ui.add_space(12.0);

        let row = |ui: &mut egui::Ui, text: &str, shortcut: Option<KeyboardShortcut>| {
            let button = egui::Button::new(egui::RichText::new(text).size(14.0))
                .frame_when_inactive(false)
                .min_size(Vec2::new(WIDTH, 32.0));
            // Always some shortcut text, so every label stays left-aligned.
            let shortcut = shortcut.map_or(String::new(), |shortcut| ui.ctx().format_shortcut(&shortcut));
            let button = button.shortcut_text(egui::RichText::new(shortcut).color(text_color));
            ui.add(button).clicked()
        };
        let command = |key| Some(KeyboardShortcut::new(Modifiers::COMMAND, key));
        if row(&mut ui, "Paste an image", command(Key::V)) {
            // Like Edit > Paste: placed at the canvas center.
            self.menu_paste_frame = Some(ui.ctx().cumulative_frame_nr());
            ui.send_viewport_cmd(ViewportCommand::RequestPaste);
        }
        #[cfg(any(target_os = "macos", windows))]
        if row(&mut ui, "Take a screenshot", None) && self.screenshot.is_none() {
            self.start_screenshot(ui.ctx());
        }
        #[cfg(not(target_arch = "wasm32"))]
        if row(&mut ui, "Open an image…", command(Key::O)) {
            self.open_file();
        }
        ui.add_space(14.0);
        ui.label(egui::RichText::new("or just start drawing: P pencil · A arrow · T text").size(12.0).color(text_color));
    }
}

/// Paints the deer as round-capped strokes filling `rect` (square), in `color`, so it fits any
/// background (the icon's PNG has its own dark square).
fn paint_deer(painter: &Painter, rect: Rect, color: Color32) {
    // The SVG scales the deer by 1.12 around DEER_CENTER into its 1000-unit box.
    let scale = rect.width() / 1000.0 * 1.12;
    let to_screen = |[x, y]: [f32; 2]| rect.center() + Vec2::new(x - DEER_CENTER[0], y - DEER_CENTER[1]) * scale;
    let stroke = Stroke::new(DEER_STROKE * scale, color);
    for path in DEER_PATHS {
        let mut points = vec![to_screen(path[0])];
        for segment in path[1..].chunks(3) {
            let start = *points.last().unwrap();
            let curve = CubicBezierShape::from_points_stroke(
                [start, to_screen(segment[0]), to_screen(segment[1]), to_screen(segment[2])],
                false,
                Color32::TRANSPARENT,
                stroke,
            );
            points.extend(curve.flatten(Some(0.1)).into_iter().skip(1));
        }
        // Round caps and joins, as in the SVG.
        for p in path.iter().step_by(3) {
            painter.circle_filled(to_screen(*p), stroke.width / 2.0, color);
        }
        painter.add(Shape::line(points, stroke));
    }
    for (center, radius) in DEER_DOTS {
        painter.circle_filled(to_screen(center), radius * scale, color);
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::Pos2;

    use super::*;

    #[test]
    fn deer_paths_are_whole_segments() {
        for path in DEER_PATHS {
            assert_eq!((path.len() - 1) % 3, 0);
        }
        // The head is closed.
        assert_eq!(DEER_PATHS[6].first(), DEER_PATHS[6].last());
    }

    #[test]
    fn deer_fits_its_box() {
        let all = DEER_PATHS.iter().flat_map(|p| p.iter()).map(|&[x, y]| Pos2::new(x, y));
        let bounds = Rect::from_points(&all.collect::<Vec<_>>());
        let half = 500.0 / 1.12 - DEER_STROKE;
        let box_ = Rect::from_center_size(Pos2::new(DEER_CENTER[0], DEER_CENTER[1]), Vec2::splat(2.0 * half));
        assert!(box_.contains_rect(bounds), "{bounds:?}");
    }
}
