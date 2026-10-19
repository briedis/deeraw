//! Selection handles (resize, arrow endpoints) and image crop mode.

use eframe::egui::{self, Color32, CursorIcon, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use super::{DeerawApp, MIN_TEXT_BOX, ResizeDrag, SELECTION_COLOR, Tool};
use crate::geometry::{align_candidates, align_guides, centered_on, crop_uv, resize_to, snap_angle, uncropped_rect};
use crate::object::{Kind, Object};

/// Size of a resize handle, in points.
const HANDLE_SIZE: f32 = 8.0;
/// Radius of an arrow's endpoint handles, in points.
const POINT_HANDLE_RADIUS: f32 = 5.0;

impl DeerawApp {
    pub(super) fn selection_handles(
        &mut self,
        ui: &mut egui::Ui,
        to_screen: impl Fn(Rect) -> Rect,
        to_screen_pos: impl Fn(Pos2) -> Pos2,
        to_canvas: impl Fn(Pos2) -> Pos2,
        scale: f32,
    ) {
        // Several selected: just outline each, no handles.
        if self.selected.len() > 1 {
            for obj in self.objects.iter().filter(|o| self.selected.contains(&o.id)) {
                let rect = to_screen(obj.rect).expand(2.0);
                ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, SELECTION_COLOR), StrokeKind::Outside);
            }
            return;
        }
        let targets = self.align_targets(ui.ctx());
        let Some(obj) = self.objects.iter_mut().find(|o| self.selected.contains(&o.id)) else {
            return;
        };
        // Blurs, strokes and arrows don't line up with anything.
        let targets = if obj.bindable() { align_candidates(self.resize_drag.as_ref().map_or(obj.rect, |d| d.start), &targets) } else { vec![] };
        if self.editing == Some(obj.id) {
            return;
        }
        if let Kind::Arrow { start, end, mut bindings, .. } = obj.kind {
            // Arrows get a handle per endpoint instead of a resize box. A dragged endpoint
            // snaps to a nearby text or image side and attaches to it, or detaches; a free end
            // snaps to nearly horizontal, vertical or 45° lines from the other end.
            let arrow = obj.id;
            let mut points = [start, end];
            let mut dragging = None;
            for (i, point) in points.iter_mut().enumerate() {
                let center = to_screen_pos(*point);
                let id = egui::Id::unique(("point-handle", arrow, i));
                let hit = Rect::from_center_size(center, Vec2::splat(POINT_HANDLE_RADIUS * 2.0 + 4.0));
                let response = ui.interact(hit, id, Sense::drag());
                if response.drag_started_by(PointerButton::Primary) {
                    self.point_drag = Some(*point);
                }
                if let (true, Some(raw)) = (response.dragged_by(PointerButton::Primary), &mut self.point_drag) {
                    *raw += response.drag_delta() / scale;
                    let raw = *raw;
                    let snap = self.snap(to_screen_pos(raw), &to_screen_pos);
                    let other = [start, end][1 - i];
                    *point = snap.map_or(snap_angle(other, raw), |(_, anchor)| anchor);
                    bindings[i] = snap.map(|(b, _)| b);
                    dragging = Some(to_screen_pos(raw));
                }
                if response.drag_stopped() {
                    self.point_drag = None;
                }
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(CursorIcon::Grab);
                }
                let painter = ui.painter();
                painter.circle_filled(center, POINT_HANDLE_RADIUS, Color32::WHITE);
                painter.circle_stroke(center, POINT_HANDLE_RADIUS, Stroke::new(1.5, SELECTION_COLOR));
            }
            if let Some(pointer) = dragging {
                self.paint_snap_circles(ui.painter(), Some(pointer), &bindings, &to_screen_pos);
                if let Some(Object { kind: Kind::Arrow { start, end, bindings: b, .. }, .. }) =
                    self.objects.iter_mut().find(|o| o.id == arrow)
                {
                    [*start, *end] = points;
                    *b = bindings;
                }
            }
            return;
        }
        let rect = to_screen(obj.rect);
        let painter = ui.painter();
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, SELECTION_COLOR), StrokeKind::Outside);

        let is_text = matches!(obj.kind, Kind::Text { .. });
        let boxed = matches!(obj.kind, Kind::Text { frame: Some(_), .. });
        let from_center = ui.input(|i| i.modifiers.shift);
        // (x, y) direction of each handle: -1 = min edge, 1 = max edge, 0 = that axis is fixed.
        for (dx, dy) in [
            (-1, -1), (1, -1), (-1, 1), (1, 1), // corners: both axes, keep aspect ratio
            (0, -1), (0, 1), (-1, 0), (1, 0),   // edges: one axis
        ] {
            // Plain text can't be stretched, only scaled.
            if is_text && !boxed && (dx == 0 || dy == 0) {
                continue;
            }
            let center = Pos2::new(
                [rect.min.x, rect.center().x, rect.max.x][(dx + 1) as usize],
                [rect.min.y, rect.center().y, rect.max.y][(dy + 1) as usize],
            );
            let handle = Rect::from_center_size(center, Vec2::splat(HANDLE_SIZE));
            let id = egui::Id::unique(("handle", obj.id, dx, dy));
            let response = ui.interact(handle.expand(2.0), id, Sense::drag());
            let pointer = response.interact_pointer_pos().map(&to_canvas);
            if let (true, Some(p)) = (response.drag_started_by(PointerButton::Primary), pointer) {
                let handle_pos = obj.rect.lerp_inside(Vec2::new(dx as f32 + 1.0, dy as f32 + 1.0) / 2.0);
                let start_font_px = match obj.kind {
                    Kind::Text { font_px, .. } => font_px,
                    _ => 0.0,
                };
                let centered = centered_on(obj.rect, &targets);
                self.resize_drag = Some(ResizeDrag { start: obj.rect, grab_offset: p - handle_pos, start_font_px, centered });
            }
            if let (true, Some(p), Some(drag)) =
                (response.dragged_by(PointerButton::Primary), pointer, &self.resize_drag)
            {
                // Plain text keeps whatever aspect the layout gives it; text boxes, shapes and blurs resize freely.
                let aspect = match obj.kind {
                    Kind::Text { frame: Some(_), .. } | Kind::Shape { .. } | Kind::Blur { .. } => None,
                    Kind::Text { .. } => Some(drag.start.aspect_ratio()),
                    _ => Some(obj.aspect),
                };
                let from_center = drag.centered.map(|centered| centered || from_center || self.first_image == Some(obj.id));
                obj.rect = resize_to(drag.start, dx, dy, p - drag.grab_offset, aspect, from_center);
                self.guides = align_guides(obj.rect, &targets);
                match &mut obj.kind {
                    Kind::Text { frame: Some(frame), .. } => *frame = obj.rect.size().max(Vec2::splat(MIN_TEXT_BOX)),
                    Kind::Text { font_px, .. } => *font_px = drag.start_font_px * obj.rect.height() / drag.start.height(),
                    _ => {}
                }
            }
            if response.drag_stopped() {
                self.resize_drag = None;
            }
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(match (dx, dy) {
                    (0, _) => CursorIcon::ResizeVertical,
                    (_, 0) => CursorIcon::ResizeHorizontal,
                    _ if dx == dy => CursorIcon::ResizeNwSe,
                    _ => CursorIcon::ResizeNeSw,
                });
            }
            painter.rect_filled(handle, 2.0, Color32::WHITE);
            painter.rect_stroke(handle, 2.0, Stroke::new(1.0, SELECTION_COLOR), StrokeKind::Inside);
        }
    }

    /// The id of the selected image, if exactly one image is selected.
    pub(super) fn selected_image(&self) -> Option<u64> {
        let [id] = self.selected[..] else { return None };
        self.objects.iter().any(|o| o.id == id && matches!(o.kind, Kind::Image { .. })).then_some(id)
    }

    /// Starts cropping the selected image, or finishes the current crop.
    pub(super) fn toggle_crop(&mut self) {
        if self.cropping.is_some() {
            self.cropping = None;
        } else if let Some(id) = self.selected_image() {
            self.tool = Tool::Select;
            self.cropping = Some(id);
        }
    }

    /// Crop mode: the whole source image shows dimmed around the kept part. The kept part's
    /// edges and corners drag freely within the source; dragging inside it slides it over the
    /// source. Enter, Escape, C, clicking elsewhere or switching tools finishes. The crop stays
    /// editable: `uv` keeps the whole texture around.
    pub(super) fn crop_ui(&mut self, ui: &mut egui::Ui, to_screen: impl Fn(Rect) -> Rect, to_canvas: impl Fn(Pos2) -> Pos2, scale: f32) {
        let Some(id) = self.cropping else { return };
        let finish = ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape));
        let still_cropping = !finish && self.tool == Tool::Select && self.selected == [id];
        let obj = self.objects.iter_mut().find(|o| o.id == id).filter(|_| still_cropping);
        let Some(Object { rect: kept, aspect, kind: Kind::Image { texture, uv, .. }, .. }) = obj else {
            self.cropping = None;
            return;
        };
        let full = uncropped_rect(*kept, *uv);
        let painter = ui.painter();
        let whole = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(texture.id(), to_screen(full), whole, Color32::WHITE.gamma_multiply(0.35));
        painter.image(texture.id(), to_screen(*kept), *uv, Color32::WHITE);
        painter.rect_stroke(to_screen(full), 0.0, Stroke::new(1.0, Color32::GRAY), StrokeKind::Outside);
        painter.rect_stroke(to_screen(*kept), 0.0, Stroke::new(1.5, SELECTION_COLOR), StrokeKind::Outside);

        let inside = ui.interact(to_screen(*kept), egui::Id::unique(("crop-move", id)), Sense::drag());
        if inside.dragged_by(PointerButton::Primary) {
            let moved = kept.translate(inside.drag_delta() / scale);
            let min = moved.min.clamp(full.min, full.max - kept.size());
            *kept = Rect::from_min_size(min, kept.size());
        }
        if inside.hovered() || inside.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::Move);
        }

        for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1), (0, -1), (0, 1), (-1, 0), (1, 0)] {
            let screen = to_screen(*kept);
            let center = Pos2::new(
                [screen.min.x, screen.center().x, screen.max.x][(dx + 1) as usize],
                [screen.min.y, screen.center().y, screen.max.y][(dy + 1) as usize],
            );
            let handle = Rect::from_center_size(center, Vec2::splat(HANDLE_SIZE));
            let response = ui.interact(handle.expand(2.0), egui::Id::unique(("crop-handle", id, dx, dy)), Sense::drag());
            let pointer = response.interact_pointer_pos().map(&to_canvas);
            if let (true, Some(p)) = (response.drag_started_by(PointerButton::Primary), pointer) {
                let handle_pos = kept.lerp_inside(Vec2::new(dx as f32 + 1.0, dy as f32 + 1.0) / 2.0);
                self.resize_drag = Some(ResizeDrag { start: *kept, grab_offset: p - handle_pos, start_font_px: 0.0, centered: [false; 2] });
            }
            if let (true, Some(p), Some(drag)) =
                (response.dragged_by(PointerButton::Primary), pointer, &self.resize_drag)
            {
                *kept = resize_to(drag.start, dx, dy, p - drag.grab_offset, None, [false; 2]).intersect(full);
            }
            if response.drag_stopped() {
                self.resize_drag = None;
            }
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(match (dx, dy) {
                    (0, _) => CursorIcon::ResizeVertical,
                    (_, 0) => CursorIcon::ResizeHorizontal,
                    _ if dx == dy => CursorIcon::ResizeNwSe,
                    _ => CursorIcon::ResizeNeSw,
                });
            }
            painter.rect_filled(handle, 2.0, Color32::WHITE);
            painter.rect_stroke(handle, 2.0, Stroke::new(1.0, SELECTION_COLOR), StrokeKind::Inside);
        }
        *uv = crop_uv(full, *kept);
        // Corner resizing keeps the cropped part's undistorted proportions.
        let [w, h] = texture.size();
        *aspect = (uv.width() * w as f32) / (uv.height() * h as f32);
    }
}
