//! Attaching arrow endpoints to the sides of text and image objects.

use eframe::egui::{self, Color32, CursorIcon, Painter, PointerButton, Pos2, Rect, Sense, Stroke, Vec2};

use super::{DeerawApp, Drawing, SELECTION_COLOR};
use crate::geometry::snap_angle;
use crate::object::{Binding, Kind, Side, anchor};

/// An object's snap circles show while the pointer is this close to its border, in points.
const SHOW_DISTANCE: f32 = 40.0;
/// An endpoint snaps to a circle this close to the pointer, in points.
const SNAP_DISTANCE: f32 = 12.0;
/// Radius of a snap circle, in points.
const CIRCLE_RADIUS: f32 = 4.0;
/// Radius of the circle an endpoint is snapped to, in points.
const ACTIVE_RADIUS: f32 = 6.0;
/// A moved selection's center snaps to other objects' centers within this, in points.
pub(super) const ALIGN_DISTANCE: f32 = 6.0;
/// Color of the alignment guides.
const GUIDE_COLOR: Color32 = Color32::from_rgb(239, 68, 68);

impl DeerawApp {
    /// What a moved or resized selection's center may line up with: the unselected images, text and
    /// shapes (see `align_candidates` for which of them).
    /// Alt (Option) held: nothing.
    pub(super) fn align_targets(&self, ctx: &egui::Context) -> Vec<Rect> {
        if ctx.input(|i| i.modifiers.alt) {
            return vec![];
        }
        self.objects.iter().filter(|o| o.bindable() && !self.selected.contains(&o.id)).map(|o| o.rect).collect()
    }

    pub(super) fn paint_guides(&self, painter: &Painter, to_screen_pos: impl Fn(Pos2) -> Pos2) {
        for [a, b] in &self.guides {
            painter.line_segment([to_screen_pos(*a), to_screen_pos(*b)], Stroke::new(1.0, GUIDE_COLOR));
        }
    }

    /// The anchor closest to `pointer` (screen) within `SNAP_DISTANCE`: what an endpoint there
    /// attaches to, and where it goes (canvas pixels).
    pub(super) fn snap(&self, pointer: Pos2, to_screen_pos: impl Fn(Pos2) -> Pos2) -> Option<(Binding, Pos2)> {
        self.objects
            .iter()
            .filter(|o| o.bindable())
            .flat_map(|o| Side::ALL.map(|side| (Binding { id: o.id, side }, anchor(o.rect, side))))
            .map(|(binding, p)| (binding, p, to_screen_pos(p).distance(pointer)))
            .filter(|(.., distance)| *distance <= SNAP_DISTANCE)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(binding, p, _)| (binding, p))
    }

    /// Snap circles on the objects near `pointer` (screen) and on those in `active`, which are
    /// what the endpoints being drawn or dragged are attached to; those circles are filled.
    pub(super) fn paint_snap_circles(
        &self,
        painter: &Painter,
        pointer: Option<Pos2>,
        active: &[Option<Binding>],
        to_screen_pos: impl Fn(Pos2) -> Pos2,
    ) {
        for obj in self.objects.iter().filter(|o| o.bindable()) {
            let rect = Rect::from_min_max(to_screen_pos(obj.rect.min), to_screen_pos(obj.rect.max));
            let near = pointer.is_some_and(|p| rect.signed_distance_to_pos(p).abs() <= SHOW_DISTANCE);
            let attached = active.iter().flatten().any(|b| b.id == obj.id);
            if !near && !attached {
                continue;
            }
            for side in Side::ALL {
                let center = to_screen_pos(anchor(obj.rect, side));
                if active.contains(&Some(Binding { id: obj.id, side })) {
                    painter.circle_filled(center, ACTIVE_RADIUS, SELECTION_COLOR);
                } else {
                    painter.circle_filled(center, CIRCLE_RADIUS, Color32::WHITE);
                    painter.circle_stroke(center, CIRCLE_RADIUS, Stroke::new(1.5, SELECTION_COLOR));
                }
            }
        }
    }

    /// With Select: the snap circles of objects near the pointer, and dragging from one draws an
    /// arrow attached there (to another object's circle, or anywhere), without picking the Arrow
    /// tool. The new arrow is selected. Registered after the objects, so a press on a circle
    /// doesn't move its object.
    pub(super) fn connect_ui(&mut self, ui: &egui::Ui, to_canvas: impl Fn(Pos2) -> Pos2, to_screen_pos: impl Fn(Pos2) -> Pos2) {
        let pointer = ui.input(|i| i.pointer.hover_pos());
        if self.drawing.is_none() && ui.ctx().dragged_id().is_none() {
            self.paint_snap_circles(ui.painter(), pointer, &[], &to_screen_pos);
        }
        // Every circle, shown or not: the pointer is only ever over shown ones, and the one
        // being dragged from must stay registered once the pointer leaves its object.
        let circles: Vec<(Binding, Pos2, usize)> = self
            .objects
            .iter()
            .filter(|o| o.bindable())
            .flat_map(|o| Side::ALL.into_iter().enumerate().map(|(i, side)| (Binding { id: o.id, side }, anchor(o.rect, side), i)))
            .collect();
        for (binding, start, i) in circles {
            let hit = Rect::from_center_size(to_screen_pos(start), Vec2::splat(2.0 * SNAP_DISTANCE));
            let response = ui.interact(hit, egui::Id::unique(("connect", binding.id, i)), Sense::drag());
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
            }
            if response.drag_started_by(PointerButton::Primary) {
                self.drawing = Some(Drawing::Arrow(start, start, [Some(binding), None]));
            }
            if let (true, Some(screen)) = (response.dragged_by(PointerButton::Primary), response.interact_pointer_pos()) {
                let snap = self.snap(screen, &to_screen_pos);
                if let Some(Drawing::Arrow(start, end, bindings)) = &mut self.drawing {
                    *end = snap.map_or(snap_angle(*start, to_canvas(screen)), |(_, anchor)| anchor);
                    bindings[1] = snap.map(|(b, _)| b);
                }
            }
            if response.drag_stopped() {
                let count = self.objects.len();
                self.finish_drawing();
                if self.objects.len() > count {
                    self.selected = self.objects.last().map(|o| o.id).into_iter().collect();
                }
            }
        }
    }

    /// Moves attached arrow endpoints to their objects' anchors. Endpoints whose object is
    /// gone stay where they are. An arrow attached at only one end moves along whole instead of
    /// stretching: its free end follows by as much as the attached one.
    pub(super) fn follow_bindings(&mut self) {
        for i in 0..self.objects.len() {
            let Kind::Arrow { bindings, .. } = self.objects[i].kind else { continue };
            let anchors = bindings.map(|binding| {
                let binding = binding?;
                let target = self.objects.iter().find(|o| o.id == binding.id)?;
                Some(anchor(target.rect, binding.side))
            });
            let obj = &mut self.objects[i];
            if let Kind::Arrow { start, end, .. } = &mut obj.kind {
                let delta = match anchors {
                    [Some(a), None] => a - *start,
                    [None, Some(a)] => a - *end,
                    _ => Vec2::ZERO,
                };
                *start = anchors[0].unwrap_or(*start + delta);
                *end = anchors[1].unwrap_or(*end + delta);
                obj.rect = Rect::from_two_pos(*start, *end);
            }
        }
    }

    /// Detaches the selected arrows from objects that aren't selected too, so they can be
    /// moved on their own.
    pub(super) fn detach_selected(&mut self) {
        for obj in self.objects.iter_mut().filter(|o| self.selected.contains(&o.id)) {
            if let Kind::Arrow { bindings, .. } = &mut obj.kind {
                for binding in bindings {
                    if binding.is_some_and(|b| !self.selected.contains(&b.id)) {
                        *binding = None;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Color32, Vec2};

    use super::*;
    use crate::object::{ArrowHeads, Object};

    fn app_with_arrow_on_text() -> DeerawApp {
        let mut app = DeerawApp::default();
        let rect = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(50.0, 20.0));
        let text = Kind::Text { text: "hi".into(), font_px: 16.0, color: Color32::BLACK, align: egui::Align2::LEFT_TOP, fill: None, frame: None };
        let target = app.add(rect, text);
        let end = anchor(rect, Side::Left);
        let binding = Binding { id: target, side: Side::Left };
        let kind = Kind::Arrow { start: Pos2::ZERO, end, color: Color32::BLACK, width: 2.0, heads: Some(ArrowHeads::default()), bindings: [None, Some(binding)] };
        app.add(Rect::from_two_pos(Pos2::ZERO, end), kind);
        app
    }

    fn arrow(app: &DeerawApp) -> &Object {
        app.objects.iter().find(|o| matches!(o.kind, Kind::Arrow { .. })).unwrap()
    }

    #[test]
    fn arrow_attached_at_one_end_moves_with_its_object() {
        let mut app = app_with_arrow_on_text();
        app.objects[0].translate(Vec2::new(10.0, 5.0));
        app.follow_bindings();
        let Kind::Arrow { start, end, .. } = arrow(&app).kind else { unreachable!() };
        assert_eq!(start, Pos2::new(10.0, 5.0));
        assert_eq!(end, anchor(app.objects[0].rect, Side::Left));
    }

    #[test]
    fn arrow_attached_at_both_ends_stretches() {
        let mut app = app_with_arrow_on_text();
        let rect = Rect::from_min_size(Pos2::new(-100.0, -100.0), Vec2::new(50.0, 20.0));
        let other = app.add(rect, Kind::Text { text: "yo".into(), font_px: 16.0, color: Color32::BLACK, align: egui::Align2::LEFT_TOP, fill: None, frame: None });
        let arrow_id = arrow(&app).id;
        if let Some(Object { kind: Kind::Arrow { bindings, .. }, .. }) = app.objects.iter_mut().find(|o| o.id == arrow_id) {
            bindings[0] = Some(Binding { id: other, side: Side::Right });
        }
        app.follow_bindings();
        app.objects[0].translate(Vec2::new(10.0, 5.0));
        app.follow_bindings();
        let Kind::Arrow { start, end, .. } = arrow(&app).kind else { unreachable!() };
        assert_eq!(start, anchor(rect, Side::Right));
        assert_eq!(end, anchor(app.objects[0].rect, Side::Left));
    }

    #[test]
    fn moving_an_arrow_alone_detaches_it() {
        let mut app = app_with_arrow_on_text();
        app.selected = vec![arrow(&app).id];
        app.detach_selected();
        assert!(matches!(arrow(&app).kind, Kind::Arrow { bindings: [None, None], .. }));
    }

    #[test]
    fn moving_an_arrow_with_its_object_keeps_it_attached() {
        let mut app = app_with_arrow_on_text();
        app.select_all();
        app.detach_selected();
        assert!(matches!(arrow(&app).kind, Kind::Arrow { bindings: [None, Some(_)], .. }));
    }

    #[test]
    fn duplicates_attach_to_duplicated_objects_only() {
        let mut app = app_with_arrow_on_text();
        app.selected = vec![arrow(&app).id];
        app.duplicate_selected();
        let copy = app.objects.last().unwrap();
        assert!(matches!(copy.kind, Kind::Arrow { bindings: [None, None], .. }));

        app.select_all();
        app.duplicate_selected();
        let (text_copy, arrow_copy) = (&app.objects[3], &app.objects[4]);
        let Kind::Arrow { bindings: [_, Some(binding)], .. } = arrow_copy.kind else { panic!("copy detached") };
        assert_eq!(binding.id, text_copy.id);
    }

    #[test]
    fn connecting_with_select_draws_an_arrow() {
        let mut app = app_with_arrow_on_text();
        let start = anchor(app.objects[0].rect, Side::Right);
        let binding = Binding { id: app.objects[0].id, side: Side::Right };
        app.drawing = Some(Drawing::Arrow(start, start + Vec2::new(100.0, 0.0), [Some(binding), None]));
        app.finish_drawing();
        let new = app.objects.last().unwrap();
        assert!(matches!(new.kind, Kind::Arrow { heads: Some(_), bindings: [Some(_), None], .. }));
    }

    #[test]
    fn snaps_to_the_nearest_anchor_in_range() {
        let app = app_with_arrow_on_text();
        let target = app.objects[0].rect;
        let top = anchor(target, Side::Top);
        let snapped = app.snap(top + Vec2::new(3.0, -2.0), |p| p);
        assert_eq!(snapped.map(|(b, p)| (b.side, p)), Some((Side::Top, top)));
        assert!(app.snap(top + Vec2::new(0.0, -SNAP_DISTANCE - 1.0), |p| p).is_none());
    }
}
