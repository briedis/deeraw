//! Objects on the canvas: images, strokes, arrows/lines and text.

use eframe::egui::{Align2, Color32, Pos2, Rect, TextureHandle, Vec2};

use crate::style::SizeKind;

/// An object on the canvas. `rect` is in canvas pixels (physical pixels
/// relative to the canvas origin), so a fresh image paste shows 1:1.
#[derive(Clone, PartialEq)]
pub struct Object {
    pub id: u64,
    pub rect: Rect,
    /// Width / height the object was created with; corner resizing restores it.
    pub aspect: f32,
    pub kind: Kind,
}

#[derive(Clone, PartialEq)]
pub enum Kind {
    /// `uv` is the part of the texture that's kept (0..1); cropping only changes `uv` and `rect`.
    Image { texture: TextureHandle, uv: Rect },
    /// Points are normalized to `rect` (0..1), so moving/resizing only touches `rect`.
    /// `pressures` (0..1, one per point, from drawing speed) are always recorded, but only
    /// used when `hand_drawn`: then the stroke is smoothed and its width varies.
    Stroke { points: Vec<Pos2>, pressures: Vec<f32>, color: Color32, width: f32, hand_drawn: bool },
    /// Endpoints in canvas pixels; `rect` is derived from them. Edited point by point.
    /// Without `head` it's a plain line (Line tool).
    Arrow { start: Pos2, end: Pos2, color: Color32, width: f32, head: bool },
    /// Without `frame` (click to place), `rect` follows the laid-out text, it's one line per
    /// Enter and resizing scales `font_px`. With `frame` (drag a box), see `TextFrame`.
    Text { text: String, font_px: f32, color: Color32, frame: Option<TextFrame> },
}

/// A text box: lines wrap at its width and the text is aligned inside it. Resizing changes
/// the box, not the font. The object's `rect` is the box, grown taller if the text overflows.
#[derive(Clone, Copy, PartialEq)]
pub struct TextFrame {
    /// Canvas pixels.
    pub size: Vec2,
    pub align: Align2,
    /// Background; `None` is transparent.
    pub fill: Option<Color32>,
}

impl Object {
    pub fn translate(&mut self, delta: Vec2) {
        self.rect = self.rect.translate(delta);
        if let Kind::Arrow { start, end, .. } = &mut self.kind {
            *start += delta;
            *end += delta;
        }
    }

    /// `None` for images, which have no color.
    pub fn color_mut(&mut self) -> Option<&mut Color32> {
        match &mut self.kind {
            Kind::Image { .. } => None,
            Kind::Stroke { color, .. } | Kind::Arrow { color, .. } | Kind::Text { color, .. } => Some(color),
        }
    }

    /// Plain or hand-drawn, for pencil strokes only.
    pub fn hand_drawn_mut(&mut self) -> Option<&mut bool> {
        match &mut self.kind {
            Kind::Stroke { hand_drawn, .. } => Some(hand_drawn),
            _ => None,
        }
    }

    /// Alignment, for text boxes only.
    pub fn align_mut(&mut self) -> Option<&mut Align2> {
        match &mut self.kind {
            Kind::Text { frame: Some(frame), .. } => Some(&mut frame.align),
            _ => None,
        }
    }

    /// Background, for text boxes only.
    pub fn fill_mut(&mut self) -> Option<&mut Option<Color32>> {
        match &mut self.kind {
            Kind::Text { frame: Some(frame), .. } => Some(&mut frame.fill),
            _ => None,
        }
    }

    /// Line width or font size, whichever the object has; `None` for images.
    pub fn size_mut(&mut self) -> Option<(SizeKind, &mut f32)> {
        match &mut self.kind {
            Kind::Image { .. } => None,
            Kind::Stroke { width, .. } | Kind::Arrow { width, .. } => Some((SizeKind::Line, width)),
            Kind::Text { font_px, .. } => Some((SizeKind::Font, font_px)),
        }
    }

    pub fn name(&self) -> String {
        let id = self.id;
        match &self.kind {
            Kind::Image { .. } => format!("Image #{id}"),
            Kind::Stroke { .. } => format!("Stroke #{id}"),
            Kind::Arrow { head: true, .. } => format!("Arrow #{id}"),
            Kind::Arrow { head: false, .. } => format!("Line #{id}"),
            Kind::Text { text, .. } => {
                let first: String = text.lines().next().unwrap_or("").chars().take(20).collect();
                format!("Text #{id} \"{first}\"")
            }
        }
    }
}
