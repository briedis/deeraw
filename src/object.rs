//! Objects on the canvas: images, strokes, arrows/lines, shapes and text.

use std::sync::Arc;

use eframe::egui::{Align2, Color32, ColorImage, Pos2, Rect, TextureHandle, Vec2};

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
    /// `pixels` are the texture's, kept to rebuild `mosaics` (for blurs on it) when it's resized.
    Image { texture: TextureHandle, uv: Rect, pixels: Pixels, mosaics: Mosaics },
    /// Points are normalized to `rect` (0..1), so moving/resizing only touches `rect`.
    /// `pressures` (0..1, one per point, from drawing speed) are always recorded, but only
    /// used when `hand_drawn`: then the stroke is smoothed and its width varies.
    Stroke { points: Vec<Pos2>, pressures: Vec<f32>, color: Color32, width: f32, hand_drawn: bool },
    /// Endpoints in canvas pixels; `rect` is derived from them. Edited point by point.
    /// Without `heads` it's a plain line (Line tool). `bindings` (start, end) attach an endpoint
    /// to a side of a text or image object, which it then follows.
    Arrow { start: Pos2, end: Pos2, color: Color32, width: f32, heads: Option<ArrowHeads>, bindings: [Option<Binding>; 2] },
    /// A rectangle, ellipse or diamond filling `rect`, outlined `width` wide; `fill` `None` is
    /// outline only (then it's grabbed only near the outline).
    Shape { shape: Shape, color: Color32, width: f32, fill: Option<Color32> },
    /// Without `frame` (click to place), `rect` follows the laid-out text, it's one line per
    /// Enter and resizing scales `font_px`. With `frame` (drag a box), see `TextFrame`.
    Text { text: String, font_px: f32, color: Color32, frame: Option<TextFrame> },
    /// Pixelates the part `area` (0..1 of the whole, uncropped texture) of image `image`; `rect`
    /// is derived from it, so it moves, resizes and crops with the image. Only the part inside
    /// the image's kept area shows. Kept right above its image.
    Blur { image: u64, area: Rect, strength: BlurStrength },
}

/// How coarse a blur's pixelation is.
#[derive(Clone, Copy, Default, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum BlurStrength {
    #[default]
    Light,
    Medium,
    Strong,
}

impl BlurStrength {
    pub const ALL: [BlurStrength; 3] = [BlurStrength::Light, BlurStrength::Medium, BlurStrength::Strong];

    pub fn name(self) -> &'static str {
        match self {
            BlurStrength::Light => "light",
            BlurStrength::Medium => "medium",
            BlurStrength::Strong => "strong",
        }
    }

    /// Its block size in lightest blocks (`mosaic::BLOCK` source pixels each).
    pub fn blocks(self) -> usize {
        match self {
            BlurStrength::Light => 1,
            BlurStrength::Medium => 2,
            BlurStrength::Strong => 4,
        }
    }

    /// Index into an image's `mosaics`.
    pub fn index(self) -> usize {
        self as usize
    }
}

/// An image's pixels. Compared by identity, which is cheap (undo compares every frame) and
/// enough: an image's pixels never change.
#[derive(Clone)]
pub struct Pixels(pub Arc<ColorImage>);

impl PartialEq for Pixels {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// An image pixelated at each `BlurStrength` (`BlurStrength::ALL` order), for blurs on it.
#[derive(Clone, PartialEq)]
pub struct Mosaics {
    /// Light blocks' side, in source pixels; chosen so they're `mosaic::BLOCK` canvas pixels
    /// at the image's size.
    pub block: usize,
    pub textures: [TextureHandle; 3],
}

/// What the Shape tool draws.
#[derive(Clone, Copy, Default, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Shape {
    #[default]
    Rectangle,
    Ellipse,
    Diamond,
}

impl Shape {
    pub const ALL: [Shape; 3] = [Shape::Rectangle, Shape::Ellipse, Shape::Diamond];

    pub fn name(self) -> &'static str {
        match self {
            Shape::Rectangle => "Rectangle",
            Shape::Ellipse => "Ellipse",
            Shape::Diamond => "Diamond",
        }
    }
}

/// What an arrow has at its start and at its end.
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
/// Far enough that its circle clears the edge resize handles at 100% zoom.
const ANCHOR_GAP: f32 = 16.0;

/// Where an arrow attached to `side` of `rect` ends: just outside the side's center.
pub fn anchor(rect: Rect, side: Side) -> Pos2 {
    let outward = side.outward();
    rect.center() + outward * (rect.size() / 2.0 + Vec2::splat(ANCHOR_GAP))
}

impl Object {
    /// Text, images and shapes; arrows attach to them.
    pub fn bindable(&self) -> bool {
        matches!(self.kind, Kind::Image { .. } | Kind::Text { .. } | Kind::Shape { .. })
    }

    /// The image a blur is on.
    pub fn blurred_image(&self) -> Option<u64> {
        match self.kind {
            Kind::Blur { image, .. } => Some(image),
            _ => None,
        }
    }

    pub fn translate(&mut self, delta: Vec2) {
        self.rect = self.rect.translate(delta);
        if let Kind::Arrow { start, end, .. } = &mut self.kind {
            *start += delta;
            *end += delta;
        }
    }

    /// `None` for images and blurs, which have no color.
    pub fn color_mut(&mut self) -> Option<&mut Color32> {
        match &mut self.kind {
            Kind::Image { .. } | Kind::Blur { .. } => None,
            Kind::Stroke { color, .. } | Kind::Arrow { color, .. } | Kind::Shape { color, .. } | Kind::Text { color, .. } => Some(color),
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

    /// Strength, for blurs only.
    pub fn blur_strength_mut(&mut self) -> Option<&mut BlurStrength> {
        match &mut self.kind {
            Kind::Blur { strength, .. } => Some(strength),
            _ => None,
        }
    }

    /// Which shape, for shapes only.
    pub fn shape_mut(&mut self) -> Option<&mut Shape> {
        match &mut self.kind {
            Kind::Shape { shape, .. } => Some(shape),
            _ => None,
        }
    }

    /// Background, for text boxes and shapes.
    pub fn fill_mut(&mut self) -> Option<&mut Option<Color32>> {
        match &mut self.kind {
            Kind::Shape { fill, .. } => Some(fill),
            Kind::Text { frame: Some(frame), .. } => Some(&mut frame.fill),
            _ => None,
        }
    }

    /// Line width or font size, whichever the object has; `None` for images and blurs.
    pub fn size_mut(&mut self) -> Option<(SizeKind, &mut f32)> {
        match &mut self.kind {
            Kind::Image { .. } | Kind::Blur { .. } => None,
            Kind::Stroke { width, .. } | Kind::Arrow { width, .. } | Kind::Shape { width, .. } => Some((SizeKind::Line, width)),
            Kind::Text { font_px, .. } => Some((SizeKind::Font, font_px)),
        }
    }

    pub fn name(&self) -> String {
        let id = self.id;
        match &self.kind {
            Kind::Image { .. } => format!("Image #{id}"),
            Kind::Blur { .. } => format!("Blur #{id}"),
            Kind::Stroke { .. } => format!("Stroke #{id}"),
            Kind::Arrow { heads: Some(_), .. } => format!("Arrow #{id}"),
            Kind::Arrow { heads: None, .. } => format!("Line #{id}"),
            Kind::Shape { shape, .. } => format!("{} #{id}", shape.name()),
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
