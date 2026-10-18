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
    /// Without `heads` it's a plain line (Line tool). `bindings` (start, end) attach an endpoint
    /// to a side of a text or image object, which it then follows.
    Arrow { start: Pos2, end: Pos2, color: Color32, width: f32, heads: Option<ArrowHeads>, bindings: [Option<Binding>; 2] },
    /// Without `frame` (click to place), `rect` follows the laid-out text, it's one line per
    /// Enter and resizing scales `font_px`. With `frame` (drag a box), see `TextFrame`.
    Text { text: String, font_px: f32, color: Color32, frame: Option<TextFrame> },
}

/// What an arrow has at its start and at its end.
#[derive(Clone, Copy, PartialEq)]
pub struct ArrowHeads {
    pub start: Head,
    pub end: Head,
}

impl ArrowHeads {
    /// A plain line's.
    pub const NONE: ArrowHeads = ArrowHeads { start: Head::None, end: Head::None };
}

impl Default for ArrowHeads {
    fn default() -> Self {
        Self { start: Head::None, end: Head::Solid }
    }
}

/// One end of an arrow.
#[derive(Clone, Copy, PartialEq)]
pub enum Head {
    /// The line just ends.
    None,
    /// Two lines.
    Open,
    /// A filled triangle.
    Solid,
}

impl Head {
    pub const ALL: [Head; 3] = [Head::None, Head::Open, Head::Solid];

    pub fn name(self) -> &'static str {
        match self {
            Head::None => "none",
            Head::Open => "open",
            Head::Solid => "solid",
        }
    }
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

/// A side of an object's rect.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::Left, Side::Right, Side::Top, Side::Bottom];

    fn outward(self) -> Vec2 {
        match self {
            Side::Left => -Vec2::X,
            Side::Right => Vec2::X,
            Side::Top => -Vec2::Y,
            Side::Bottom => Vec2::Y,
        }
    }
}

/// An arrow endpoint attached to the side `side` of object `id`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Binding {
    pub id: u64,
    pub side: Side,
}

/// How far outside a side's center an attached arrow ends, in canvas pixels.
const ANCHOR_GAP: f32 = 8.0;

/// Where an arrow attached to `side` of `rect` ends: just outside the side's center.
pub fn anchor(rect: Rect, side: Side) -> Pos2 {
    let outward = side.outward();
    rect.center() + outward * (rect.size() / 2.0 + Vec2::splat(ANCHOR_GAP))
}

impl Object {
    /// Text and images; arrows attach to them.
    pub fn bindable(&self) -> bool {
        matches!(self.kind, Kind::Image { .. } | Kind::Text { .. })
    }

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

    /// Heads, for arrows only (not lines).
    pub fn arrow_heads_mut(&mut self) -> Option<&mut ArrowHeads> {
        match &mut self.kind {
            Kind::Arrow { heads: Some(heads), .. } => Some(heads),
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
            Kind::Arrow { heads: Some(_), .. } => format!("Arrow #{id}"),
            Kind::Arrow { heads: None, .. } => format!("Line #{id}"),
            Kind::Text { text, .. } => {
                let first: String = text.lines().next().unwrap_or("").chars().take(20).collect();
                format!("Text #{id} \"{first}\"")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_sit_outside_side_centers() {
        let rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 40.0));
        assert_eq!(anchor(rect, Side::Left), Pos2::new(-ANCHOR_GAP, 20.0));
        assert_eq!(anchor(rect, Side::Right), Pos2::new(100.0 + ANCHOR_GAP, 20.0));
        assert_eq!(anchor(rect, Side::Top), Pos2::new(50.0, -ANCHOR_GAP));
        assert_eq!(anchor(rect, Side::Bottom), Pos2::new(50.0, 40.0 + ANCHOR_GAP));
    }
}
