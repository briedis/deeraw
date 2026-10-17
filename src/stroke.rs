//! Pencil strokes: recording points and speed, and painting the hand-drawn style.

use eframe::egui::{self, Color32, Painter, Pos2, Vec2};

/// Drawing speed (canvas pixels per second) at which a hand-drawn stroke is thinnest.
const THIN_SPEED: f32 = 2500.0;
/// How quickly a stroke's pressure follows the speed (0..1 per point); lower is smoother.
const PRESSURE_EASING: f32 = 0.25;

/// Appends the positions one frame's moves reached (at least 1px apart) to a stroke being
/// drawn. They all arrive at frame time `now`, so their times are spread between the previous
/// point's time and `now` by distance, which keeps the speed (and so the pressure) right.
pub fn add_stroke_points(points: &mut Vec<Pos2>, times: &mut Vec<f64>, moves: &[Pos2], now: f64) {
    let (Some(&start), Some(&start_time)) = (points.last(), times.last()) else { return };
    let mut new = vec![];
    for &p in moves {
        if new.last().unwrap_or(&start).distance(p) >= 1.0 {
            new.push(p);
        }
    }
    let total: f32 = new.iter().scan(start, |prev, &p| Some(std::mem::replace(prev, p).distance(p))).sum();
    let mut traveled = 0.0;
    let mut prev = start;
    for p in new {
        traveled += prev.distance(p);
        prev = p;
        points.push(p);
        times.push(start_time + (now - start_time) * (traveled / total) as f64);
    }
}

/// Pressure (0..1) per stroke point from how fast it was drawn: slow is 1, `THIN_SPEED` and
/// faster is 0. Eased from point to point so the width changes gradually.
pub fn speed_pressures(points: &[Pos2], times: &[f64]) -> Vec<f32> {
    let mut pressure = 0.5;
    let mut pressures = Vec::with_capacity(points.len());
    for i in 0..points.len() {
        if i > 0 {
            let dt = ((times[i] - times[i - 1]) as f32).max(0.001);
            let speed = points[i].distance(points[i - 1]) / dt;
            let target = (1.0 - speed / THIN_SPEED).clamp(0.0, 1.0);
            pressure += (target - pressure) * PRESSURE_EASING;
        }
        pressures.push(pressure);
    }
    pressures
}

/// Paints a hand-drawn stroke: the points (screen) are smoothed, and the width varies from
/// 0.4× (`pressure` 0) to 1.5× `width` (pressure 1). One anti-aliased mesh (a strip with a
/// feathered edge on both sides), plus round caps and round joins at sharp turns.
pub fn paint_hand_drawn(painter: &Painter, points: &[Pos2], pressures: &[f32], width: f32, color: Color32) {
    let Some(&first) = points.first() else { return };
    let radius = |pressure: f32| width * (0.4 + 1.1 * pressure) / 2.0;
    // Each point is pulled towards the previous smoothed point (the last one stays): strongly
    // for close points (slow, jittery drawing), barely for far ones (fast drawing, where it would
    // flatten the shape; the spline rounds those instead).
    let mut smooth = Vec::with_capacity(points.len());
    let mut current = first;
    for (i, &p) in points.iter().enumerate() {
        let follow = (current.distance(p) / 12.0).clamp(0.3, 1.0);
        current = if i + 1 == points.len() { p } else { current.lerp(p, follow) };
        smooth.push(current);
    }
    let (smooth, pressures) = spline(&smooth, pressures);
    let n = smooth.len();
    let feather = 1.0 / painter.pixels_per_point();
    let mut mesh = egui::epaint::Mesh::default();
    let mut normal = Vec2::ZERO;
    for i in 0..n {
        let along = smooth[(i + 1).min(n - 1)] - smooth[i.saturating_sub(1)];
        if along.length() > 0.001 {
            normal = along.normalized().rot90();
        }
        let r = radius(pressures[i]);
        let (inner, outer) = ((r - feather / 2.0).max(0.0), r + feather / 2.0);
        let p = smooth[i];
        mesh.colored_vertex(p + normal * outer, Color32::TRANSPARENT);
        mesh.colored_vertex(p + normal * inner, color);
        mesh.colored_vertex(p - normal * inner, color);
        mesh.colored_vertex(p - normal * outer, Color32::TRANSPARENT);
        if i > 0 {
            let (a, b) = (4 * (i as u32 - 1), 4 * i as u32);
            for k in 0..3 {
                mesh.add_triangle(a + k, a + k + 1, b + k);
                mesh.add_triangle(a + k + 1, b + k + 1, b + k);
            }
        }
    }
    painter.add(mesh);
    for i in 0..n {
        let sharp = i > 0 && i + 1 < n && {
            let (a, b) = (smooth[i] - smooth[i - 1], smooth[i + 1] - smooth[i]);
            a.length() > 0.001 && b.length() > 0.001 && a.normalized().dot(b.normalized()) < 0.7
        };
        if i == 0 || i + 1 == n || sharp {
            painter.circle_filled(smooth[i], radius(pressures[i]), color);
        }
    }
}

/// Subdivides a polyline into a Catmull-Rom spline through its points (about one point per 2
/// units of length, at most 16 per segment), so sparse points still give a round curve.
/// Pressures are interpolated along with it.
pub fn spline(points: &[Pos2], pressures: &[f32]) -> (Vec<Pos2>, Vec<f32>) {
    let n = points.len();
    if n < 3 {
        return (points.to_vec(), pressures.to_vec());
    }
    let (mut out, mut out_pressures) = (vec![points[0]], vec![pressures[0]]);
    for i in 0..n - 1 {
        let [p0, p1, p2, p3] = [i.saturating_sub(1), i, i + 1, (i + 2).min(n - 1)].map(|j| points[j].to_vec2());
        let steps = ((p2 - p1).length() / 2.0).ceil().clamp(1.0, 16.0) as usize;
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            let q = 0.5
                * (2.0 * p1
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3);
            out.push(q.to_pos2());
            out_pressures.push(egui::lerp(pressures[i]..=pressures[i + 1], t));
        }
    }
    (out, out_pressures)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_strokes_are_thicker_than_fast_ones() {
        let points: Vec<Pos2> = (0..20).map(|i| Pos2::new(i as f32 * 10.0, 0.0)).collect();
        let slow: Vec<f64> = (0..20).map(|i| i as f64 * 0.1).collect(); // 100 px/s
        let fast: Vec<f64> = (0..20).map(|i| i as f64 * 0.002).collect(); // 5000 px/s
        let (slow, fast) = (speed_pressures(&points, &slow), speed_pressures(&points, &fast));
        assert!(slow[19] > 0.9 && fast[19] < 0.1, "{slow:?} {fast:?}");
        assert!(slow.iter().chain(&fast).all(|p| (0.0..=1.0).contains(p)));
    }

    #[test]
    fn frame_moves_get_times_spread_by_distance() {
        let (mut points, mut times) = (vec![Pos2::ZERO], vec![1.0]);
        let moves = [Pos2::new(10.0, 0.0), Pos2::new(10.5, 0.0), Pos2::new(40.0, 0.0)];
        add_stroke_points(&mut points, &mut times, &moves, 2.0);
        // The 0.5px move is dropped; the rest arrive in proportion to the distance.
        assert_eq!(points, vec![Pos2::ZERO, Pos2::new(10.0, 0.0), Pos2::new(40.0, 0.0)]);
        assert_eq!(times, vec![1.0, 1.25, 2.0]);
    }

    #[test]
    fn spline_passes_through_points() {
        let points = [Pos2::new(0.0, 0.0), Pos2::new(20.0, 10.0), Pos2::new(40.0, 0.0)];
        let (curve, pressures) = spline(&points, &[0.0, 0.5, 1.0]);
        assert!(curve.len() > 10 && curve.len() == pressures.len());
        for p in points {
            assert!(curve.iter().any(|q| q.distance(p) < 1e-4), "{p:?} not on the curve");
        }
        assert_eq!(pressures.last(), Some(&1.0));
    }
}
