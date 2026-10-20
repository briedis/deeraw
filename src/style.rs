//! Style settings for new objects (style panel) and the canvas background.

use eframe::egui::{Align2, Color32, CornerRadius, Stroke, Visuals};

use crate::object::{ArrowHeads, BlurStrength, Shape};

/// Colors offered by the style panel; the first is the default.
pub const PALETTE: [(Color32, &str); 6] = [
    (Color32::BLACK, "Black"),
    (Color32::WHITE, "White"),
    (Color32::from_rgb(224, 49, 49), "Red"),
    (Color32::from_rgb(240, 140, 0), "Orange"),
    (Color32::from_rgb(47, 158, 68), "Green"),
    (Color32::from_rgb(25, 113, 194), "Blue"),
];
/// Line widths (pencil strokes, arrows and shape outlines) offered by the style panel, in canvas pixels.
pub const LINE_WIDTHS: [f32; 4] = [2.0, 3.0, 6.0, 10.0];
/// Font sizes offered by the style panel, in canvas pixels.
pub const FONT_SIZES: [f32; 4] = [16.0, 28.0, 44.0, 64.0];

/// Which part of the style panel is expanded.
#[derive(Clone, Copy, PartialEq)]
pub enum StyleSection {
    /// The canvas background (its button is in the toolbar).
    Background,
    Color,
    Size(SizeKind),
    /// Plain or hand-drawn pencil strokes.
    Pen,
    /// Rectangle, ellipse or diamond.
    Shape,
    /// What an arrow has at its start.
    StartHead,
    /// What an arrow has at its end.
    EndHead,
    Align,
    Fill,
}

/// Which size the style panel edits.
#[derive(Clone, Copy, PartialEq)]
pub enum SizeKind {
    /// Pencil strokes and arrows.
    Line,
    Font,
}

/// Color and sizes for new objects, set from the style panel. Remembered between launches;
/// fields missing from older settings get their defaults.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Style {
    pub color: Color32,
    pub line_width: f32,
    pub font_px: f32,
    /// For pencil strokes.
    pub hand_drawn: bool,
    /// For arrows.
    pub arrow_heads: ArrowHeads,
    /// For the Shape tool.
    pub shape: Shape,
    /// For blurs.
    pub blur: BlurStrength,
    /// For text boxes.
    pub align: Align2,
    /// For text boxes and shapes; `None` is transparent (shapes are outline only).
    pub fill: Option<Color32>,
    /// Whether shapes are outlined.
    pub border: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self { color: PALETTE[0].0, line_width: LINE_WIDTHS[1], font_px: FONT_SIZES[1], hand_drawn: false, arrow_heads: ArrowHeads::default(), shape: Shape::default(), blur: BlurStrength::default(), align: Align2::LEFT_TOP, fill: None, border: true }
    }
}

impl Style {
    /// Like `Object::keep_visible`, for new shapes.
    pub fn keep_shapes_visible(&mut self, border_changed: bool) {
        if !self.border && self.fill.is_none() {
            if border_changed {
                self.fill = Some(self.color);
            } else {
                self.border = true;
            }
        }
    }

    pub fn size_mut(&mut self, kind: SizeKind) -> &mut f32 {
        match kind {
            SizeKind::Line => &mut self.line_width,
            SizeKind::Font => &mut self.font_px,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Background {
    Transparent,
    #[default]
    White,
    Black,
}

impl Background {
    /// `None` for transparent.
    pub fn color(self) -> Option<Color32> {
        match self {
            Background::Transparent => None,
            Background::White => Some(Color32::WHITE),
            Background::Black => Some(Color32::BLACK),
        }
    }

    /// The default drawing color that shows on this background (transparent counts as light).
    pub fn ink(self) -> Color32 {
        match self {
            Background::Transparent | Background::White => Color32::BLACK,
            Background::Black => Color32::WHITE,
        }
    }
}

/// Selected toolbar and style panel buttons.
const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);

/// The look of the toolbar and style panel, which float on the canvas: light on a white or
/// transparent canvas, dark on a black one.
pub fn canvas_ui_visuals(background: Background) -> Visuals {
    let (mut visuals, fill, border, hover, text) = match background {
        Background::Transparent | Background::White => (
            Visuals::light(),
            Color32::WHITE,
            Color32::from_rgb(226, 232, 240),
            Color32::from_rgb(241, 245, 249),
            Color32::from_rgb(30, 41, 59),
        ),
        Background::Black => (
            Visuals::dark(),
            Color32::from_rgb(39, 42, 48),
            Color32::from_rgb(63, 68, 77),
            Color32::from_rgb(58, 63, 72),
            Color32::from_rgb(226, 232, 240),
        ),
    };
    visuals.window_fill = fill;
    visuals.window_stroke = Stroke::new(1.0, border);
    visuals.menu_corner_radius = CornerRadius::same(12);
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    let widgets = &mut visuals.widgets;
    for w in [&mut widgets.noninteractive, &mut widgets.inactive, &mut widgets.hovered, &mut widgets.active, &mut widgets.open] {
        w.fg_stroke.color = text;
        w.corner_radius = CornerRadius::same(8);
    }
    widgets.hovered.weak_bg_fill = hover;
    widgets.hovered.bg_stroke = Stroke::NONE;
    widgets.active.weak_bg_fill = hover;
    widgets.active.bg_stroke = Stroke::NONE;
    visuals
}
