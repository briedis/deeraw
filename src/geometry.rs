//! Rect and point math in canvas pixels (no egui UI).

use eframe::egui::{Pos2, Rect, Vec2};

use crate::object::Shape;

/// Smallest width/height an object can have, in canvas pixels.
pub const MIN_SIZE: f32 = 4.0;

/// New pan after changing the scale, so the canvas point at `anchor` (relative to the canvas
/// panel, in points) stays under it.
pub fn pan_for_zoom(pan: Vec2, anchor: Vec2, old_scale: f32, new_scale: f32) -> Vec2 {
    let canvas_point = (anchor - pan) / old_scale;
    anchor - canvas_point * new_scale
}

/// Distance from `p` to the line segment `a`-`b`.
pub fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(f32::EPSILON)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// The corners of the diamond filling `rect`: top, right, bottom, left.
pub fn diamond(rect: Rect) -> [Pos2; 4] {
    [rect.center_top(), rect.right_center(), rect.center_bottom(), rect.left_center()]
}

/// Roughly how far `p` is from the outline of `shape` filling `rect` (exact for rectangles and
/// diamonds; for ellipses, measured along the line to the center).
pub fn distance_to_outline(shape: Shape, rect: Rect, p: Pos2) -> f32 {
    match shape {
        Shape::Rectangle => rect.signed_distance_to_pos(p).abs(),
        Shape::Diamond => {
            let c = diamond(rect);
            (0..4).map(|i| distance_to_segment(p, c[i], c[(i + 1) % 4])).fold(f32::INFINITY, f32::min)
        }
        Shape::Ellipse => {
            let radius = (rect.size() / 2.0).max(Vec2::splat(f32::EPSILON));
            let offset = p - rect.center();
            let along = (offset / radius).length();
            if along < f32::EPSILON {
                return radius.min_elem();
            }
            // Where the line from the center through `p` crosses the ellipse.
            offset.length() * (1.0 - 1.0 / along).abs()
        }
    }
}

/// Returns the bounding box of `points` (at least `MIN_SIZE` on each axis)
/// and the points normalized to it.
pub fn normalize(points: &[Pos2]) -> (Rect, Vec<Pos2>) {
    let bounds = Rect::from_points(points);
    let rect = Rect::from_center_size(bounds.center(), bounds.size().max(Vec2::splat(MIN_SIZE)));
    let normalized = points
        .iter()
        .map(|p| ((*p - rect.min) / rect.size()).to_pos2())
        .collect();
    (rect, normalized)
}

/// Where the whole source image would be, for an image showing the `uv` part of it in `rect`.
pub fn uncropped_rect(rect: Rect, uv: Rect) -> Rect {
    let size = rect.size() / uv.size();
    Rect::from_min_size(rect.min - uv.min.to_vec2() * size, size)
}

/// The `uv` that shows the part `rect` of an image whose whole source is at `full`.
pub fn crop_uv(full: Rect, rect: Rect) -> Rect {
    let to_uv = |p: Pos2| ((p - full.min) / full.size()).to_pos2();
    Rect::from_min_max(to_uv(rect.min), to_uv(rect.max))
}

/// Resizes `start` so the handle at direction (`dx`, `dy`) lands on `target` (canvas pixels).
/// -1 = min edge, 1 = max edge, 0 = that axis is untouched. Corner handles fit the size to
/// `aspect` (if given; otherwise both axes follow the cursor), letting the axis the cursor pulled further win. With `from_center` the center
/// stays put; otherwise the opposite side does. Never shrinks below `MIN_SIZE`.
pub fn resize_to(start: Rect, dx: i32, dy: i32, target: Pos2, aspect: Option<f32>, from_center: bool) -> Rect {
    let dir = Vec2::new(dx as f32, dy as f32);
    let anchor = if from_center {
        start.center()
    } else {
        start.center() - dir * start.size() / 2.0
    };
    let reach = (target - anchor) * dir * if from_center { 2.0 } else { 1.0 };

    let mut size = start.size();
    if dx != 0 {
        size.x = reach.x.max(MIN_SIZE);
    }
    if dy != 0 {
        size.y = reach.y.max(MIN_SIZE);
    }
    if let (true, Some(aspect)) = (dx != 0 && dy != 0, aspect) {
        if size.x / size.y > aspect {
            size.y = size.x / aspect;
        } else {
            size.x = size.y * aspect;
        }
        size *= (MIN_SIZE / size.min_elem()).max(1.0);
    }

    let center = if from_center { anchor } else { anchor + dir * size / 2.0 };
    Rect::from_center_size(center, size)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_to_shape_outlines() {
        let r = Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 50.0));
        assert_eq!(distance_to_outline(Shape::Rectangle, r, Pos2::new(50.0, 25.0)), 25.0);
        assert_eq!(distance_to_outline(Shape::Rectangle, r, Pos2::new(110.0, 25.0)), 10.0);
        assert!(distance_to_outline(Shape::Diamond, r, Pos2::new(50.0, 0.0)) < 0.01);
        assert!(distance_to_outline(Shape::Diamond, r, Pos2::new(5.0, 5.0)) > 10.0);
        assert!(distance_to_outline(Shape::Ellipse, r, Pos2::new(100.0, 25.0)) < 0.01);
        assert!((distance_to_outline(Shape::Ellipse, r, Pos2::new(120.0, 25.0)) - 20.0).abs() < 0.01);
        assert_eq!(distance_to_outline(Shape::Ellipse, r, Pos2::new(50.0, 25.0)), 25.0);
    }

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
    }

    #[test]
    fn crop_uv_round_trips() {
        let full = rect(100.0, 50.0, 500.0, 250.0);
        let kept = rect(200.0, 100.0, 300.0, 250.0);
        let uv = crop_uv(full, kept);
        assert_eq!(uv, rect(0.25, 0.25, 0.5, 1.0));
        assert_eq!(uncropped_rect(kept, uv), full);
    }

    #[test]
    fn corner_follows_cursor_keeping_aspect() {
        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), 1, 1, Pos2::new(400.0, 150.0), Some(2.0), false);
        assert_eq!(r, rect(0.0, 0.0, 400.0, 200.0));

        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), -1, -1, Pos2::new(100.0, 0.0), Some(2.0), false);
        assert_eq!(r, rect(0.0, 0.0, 200.0, 100.0));

        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), -1, -1, Pos2::new(100.0, 80.0), Some(2.0), false);
        assert_eq!(r, rect(100.0, 50.0, 200.0, 100.0));
    }

    #[test]
    fn corner_restores_original_aspect() {
        // Stretched to 3:1 with an edge handle; the corner snaps back to 2:1.
        let r = resize_to(rect(0.0, 0.0, 300.0, 100.0), 1, 1, Pos2::new(300.0, 100.0), Some(2.0), false);
        assert_eq!(r, rect(0.0, 0.0, 300.0, 150.0));
    }

    #[test]
    fn corner_respects_min_size() {
        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), 1, -1, Pos2::new(-1000.0, 1000.0), Some(2.0), false);
        assert_eq!(r, rect(0.0, 96.0, 8.0, 100.0));
    }

    #[test]
    fn edge_follows_cursor_on_one_axis() {
        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), 1, 0, Pos2::new(250.0, 999.0), Some(2.0), false);
        assert_eq!(r, rect(0.0, 0.0, 250.0, 100.0));

        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), 0, -1, Pos2::new(999.0, -20.0), Some(2.0), false);
        assert_eq!(r, rect(0.0, -20.0, 200.0, 100.0));
    }

    #[test]
    fn from_center_keeps_center() {
        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), 1, 1, Pos2::new(300.0, 150.0), Some(2.0), true);
        assert_eq!(r, rect(-100.0, -50.0, 300.0, 150.0));

        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), 0, -1, Pos2::new(0.0, 10.0), Some(2.0), true);
        assert_eq!(r, rect(0.0, 10.0, 200.0, 90.0));
    }

    #[test]
    fn zoom_keeps_anchor_fixed() {
        let (pan, anchor) = (Vec2::new(30.0, -10.0), Vec2::new(200.0, 150.0));
        let new_pan = pan_for_zoom(pan, anchor, 1.0, 1.5);
        let canvas_point = (anchor - pan) / 1.0;
        assert_eq!(new_pan + canvas_point * 1.5, anchor);
        assert_eq!(pan_for_zoom(pan, anchor, 1.0, 1.0), pan);
    }

    #[test]
    fn segment_distance() {
        let (a, b) = (Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0));
        assert_eq!(distance_to_segment(Pos2::new(5.0, 3.0), a, b), 3.0);
        assert_eq!(distance_to_segment(Pos2::new(14.0, 3.0), a, b), 5.0);
        assert_eq!(distance_to_segment(Pos2::new(1.0, 1.0), a, a), 2f32.sqrt());
    }

    #[test]
    fn normalize_points() {
        let (r, points) = normalize(&[Pos2::new(10.0, 20.0), Pos2::new(110.0, 20.0)]);
        assert_eq!(r, rect(10.0, 18.0, 110.0, 22.0));
        assert_eq!(points, vec![Pos2::new(0.0, 0.5), Pos2::new(1.0, 0.5)]);
    }
}
