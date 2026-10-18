//! Painting objects onto the canvas (or an export).

use std::sync::Arc;

use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, emath::Rot2, epaint::PathShape};

use crate::geometry::diamond;
use crate::object::{ArrowHeads, Head, Kind, Object, Shape, TextFrame};
use crate::stroke::paint_hand_drawn;

/// Length of an arrow's head, in canvas pixels: base plus this much per pixel of line width.
const ARROW_HEAD: f32 = 12.0;
const ARROW_HEAD_PER_WIDTH: f32 = 2.0;
/// Angle between an arrow's line and each side of its head, in radians.
const ARROW_HEAD_SPREAD: f32 = 0.45;

pub fn paint_object(painter: &Painter, obj: &Object, rect: Rect, to_screen_pos: impl Fn(Pos2) -> Pos2, scale: f32) {
    match &obj.kind {
        Kind::Image { texture, uv } => {
            painter.image(texture.id(), rect, *uv, Color32::WHITE);
        }
        Kind::Stroke { points, pressures, color, width, hand_drawn } => {
            let points: Vec<Pos2> = points.iter().map(|p| rect.lerp_inside(p.to_vec2())).collect();
            if *hand_drawn {
                paint_hand_drawn(painter, &points, pressures, width * scale, *color);
            } else if let [dot] = points[..] {
                painter.circle_filled(dot, width * scale / 2.0, *color);
            } else {
                painter.line(points, Stroke::new(width * scale, *color));
            }
        }
        Kind::Arrow { start, end, color, width, heads, .. } => {
            let stroke = Stroke::new(width * scale, *color);
            paint_arrow(painter, to_screen_pos(*start), to_screen_pos(*end), stroke, scale, heads.unwrap_or(ArrowHeads::NONE));
        }
        Kind::Shape { shape, color, width, fill } => {
            paint_shape(painter, *shape, rect, Stroke::new(width * scale, *color), *fill);
        }
        Kind::Text { text, font_px, color, frame } => {
            let frame = frame.map(|f| TextFrame { size: f.size * scale, ..f });
            if let Some(fill) = frame.and_then(|f| f.fill) {
                painter.rect_filled(rect, 0.0, fill);
            }
            let galley = layout_text(painter, text, font_px * scale, *color, frame);
            // Same placement as `TextEdit` uses, so editing doesn't shift the text.
            let align = frame.map_or(Align2::LEFT_TOP, |f| f.align);
            let pos = align.align_size_within_rect(galley.size(), rect).min - Vec2::new(galley.rect.left(), 0.0);
            painter.galley(pos, galley, *color);
        }
    }
}

/// Lays out a text object's text. `font_px` and `frame` are in the same unit (canvas pixels
/// for measuring, screen points for painting).
pub fn layout_text(painter: &Painter, text: &str, font_px: f32, color: Color32, frame: Option<TextFrame>) -> Arc<egui::Galley> {
    painter.layout_job(text_layout_job(text, FontId::proportional(font_px), color, frame))
}

/// Plain text doesn't wrap; a text box wraps at its width with its rows aligned horizontally.
pub fn text_layout_job(text: &str, font: FontId, color: Color32, frame: Option<TextFrame>) -> egui::text::LayoutJob {
    let wrap = frame.map_or(f32::INFINITY, |f| f.size.x);
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), font, color, wrap);
    if let Some(frame) = frame {
        job.halign = frame.align.x();
    }
    job
}

/// Draws `shape` filling `rect` (screen points), the outline centered on its edge.
pub fn paint_shape(painter: &Painter, shape: Shape, rect: Rect, stroke: Stroke, fill: Option<Color32>) {
    let fill = fill.unwrap_or(Color32::TRANSPARENT);
    match shape {
        Shape::Rectangle => {
            painter.rect(rect, 0.0, fill, stroke, StrokeKind::Middle);
        }
        Shape::Ellipse => {
            painter.add(PathShape::convex_polygon(ellipse_points(painter, rect), fill, stroke));
        }
        Shape::Diamond => {
            painter.add(PathShape::convex_polygon(diamond(rect).to_vec(), fill, stroke));
        }
    }
}

/// Points around the ellipse filling `rect` (screen points), about every 2 pixels so it's
/// smooth at any size (egui's own ellipse uses as few as 8 points per quarter).
fn ellipse_points(painter: &Painter, rect: Rect) -> Vec<Pos2> {
    let radius = rect.size() / 2.0;
    let pixels = std::f32::consts::TAU * radius.max_elem() * painter.pixels_per_point();
    let count = (pixels / 2.0).clamp(32.0, 1024.0) as usize;
    (0..count)
        .map(|i| {
            let angle = std::f32::consts::TAU * i as f32 / count as f32;
            rect.center() + Vec2::new(radius.x * angle.cos(), radius.y * angle.sin())
        })
        .collect()
}

/// Draws a line from `start` to `end` (screen points) with `heads`; they grow with the stroke
/// width. `scale` is screen points per canvas pixel.
pub fn paint_arrow(painter: &Painter, start: Pos2, end: Pos2, stroke: Stroke, scale: f32, heads: ArrowHeads) {
    let along = end - start;
    let length = along.length();
    if length < 1.0 {
        painter.line_segment([start, end], stroke);
        return;
    }
    let ends = if heads.start != Head::None && heads.end != Head::None { 3.0 } else { 2.0 };
    let head_length = (ARROW_HEAD * scale + ARROW_HEAD_PER_WIDTH * stroke.width).min(length / ends);
    let dir = along / length;
    let line_start = paint_head(painter, start, -dir, head_length, stroke, heads.start);
    let line_end = paint_head(painter, end, dir, head_length, stroke, heads.end);
    painter.line_segment([line_start, line_end], stroke);
}

/// Draws `head` with its tip at `tip`, pointing along `dir` (unit); returns where the line
/// should end so its square end stays hidden under the head.
fn paint_head(painter: &Painter, tip: Pos2, dir: Vec2, length: f32, stroke: Stroke, head: Head) -> Pos2 {
    let back = -dir * length;
    let spread = Rot2::from_angle(ARROW_HEAD_SPREAD);
    let sides = [tip + spread * back, tip + spread.inverse() * back];
    match head {
        Head::None => tip,
        Head::Open => {
            // One path, so the sides are joined in a point however thick they are.
            painter.add(PathShape::line(vec![sides[0], tip, sides[1]], stroke));
            tip
        }
        Head::Solid => {
            painter.add(PathShape::convex_polygon(vec![sides[0], tip, sides[1]], stroke.color, Stroke::NONE));
            // Halfway into the head, which is still wider than the line there.
            tip + back * ARROW_HEAD_SPREAD.cos() / 2.0
        }
    }
}

/// Screen-aligned checkerboard standing in for a transparent background.
pub fn paint_checkerboard(painter: &Painter, rect: Rect, cell: f32, gray: u8) {
    painter.rect_filled(rect, 0.0, Color32::WHITE);
    let cols = (rect.width() / cell).ceil() as i32;
    let rows = (rect.height() / cell).ceil() as i32;
    for row in 0..rows {
        for col in (row % 2..cols).step_by(2) {
            let min = rect.min + Vec2::new(col as f32, row as f32) * cell;
            painter.rect_filled(Rect::from_min_size(min, Vec2::splat(cell)), 0.0, Color32::from_gray(gray));
        }
    }
}
