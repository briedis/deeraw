//! Painting objects onto the canvas (or an export).

use std::sync::Arc;

use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2, emath::Rot2};

use crate::object::{Kind, Object, TextFrame};
use crate::stroke::paint_hand_drawn;

/// Length of an arrow's head lines, in canvas pixels: base plus this much per pixel of line width.
const ARROW_HEAD: f32 = 12.0;
const ARROW_HEAD_PER_WIDTH: f32 = 2.0;

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
        Kind::Arrow { start, end, color, width, head } => {
            let stroke = Stroke::new(width * scale, *color);
            paint_arrow(painter, to_screen_pos(*start), to_screen_pos(*end), stroke, scale, *head);
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

/// Draws a line from `start` to `end` (screen points), with a head at `end` if `head`; the head
/// grows with the stroke width. `scale` is screen points per canvas pixel.
pub fn paint_arrow(painter: &Painter, start: Pos2, end: Pos2, stroke: Stroke, scale: f32, head: bool) {
    painter.line_segment([start, end], stroke);
    let along = end - start;
    let length = along.length();
    if !head || length < 1.0 {
        return;
    }
    let head_length = ARROW_HEAD * scale + ARROW_HEAD_PER_WIDTH * stroke.width;
    let back = -along / length * head_length.min(length / 2.0);
    let spread = Rot2::from_angle(0.45);
    painter.line_segment([end, end + spread * back], stroke);
    painter.line_segment([end, end + spread.inverse() * back], stroke);
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
