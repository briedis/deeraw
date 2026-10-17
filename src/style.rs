//! Style settings for new objects (style panel) and the canvas background.

use eframe::egui::{Align2, Color32};

/// Colors offered by the style panel; the first is the default.
pub const PALETTE: [(Color32, &str); 6] = [
    (Color32::BLACK, "Black"),
    (Color32::WHITE, "White"),
    (Color32::from_rgb(224, 49, 49), "Red"),
    (Color32::from_rgb(240, 140, 0), "Orange"),
    (Color32::from_rgb(47, 158, 68), "Green"),
    (Color32::from_rgb(25, 113, 194), "Blue"),
];
/// Line widths (pencil strokes and arrows) offered by the style panel, in canvas pixels.
pub const LINE_WIDTHS: [f32; 4] = [2.0, 3.0, 6.0, 10.0];
/// Font sizes offered by the style panel, in canvas pixels.
pub const FONT_SIZES: [f32; 4] = [16.0, 28.0, 44.0, 64.0];

/// Which part of the style panel is expanded.
#[derive(Clone, Copy, PartialEq)]
pub enum StyleSection {
    Color,
    Size(SizeKind),
    /// Plain or hand-drawn pencil strokes.
    Pen,
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

/// Color and sizes for new objects, set from the style panel.
pub struct Style {
    pub color: Color32,
    pub line_width: f32,
    pub font_px: f32,
    /// For pencil strokes.
    pub hand_drawn: bool,
    /// For text boxes.
    pub align: Align2,
    pub fill: Option<Color32>,
}

impl Default for Style {
    fn default() -> Self {
        Self { color: PALETTE[0].0, line_width: LINE_WIDTHS[1], font_px: FONT_SIZES[1], hand_drawn: false, align: Align2::LEFT_TOP, fill: None }
    }
}

impl Style {
    pub fn size_mut(&mut self, kind: SizeKind) -> &mut f32 {
        match kind {
            SizeKind::Line => &mut self.line_width,
            SizeKind::Font => &mut self.font_px,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
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
