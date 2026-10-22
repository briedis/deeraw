//! The canvas: navigation, input, drawing with the tools, painting and text editing.

use eframe::egui::{self, Align2, CursorIcon, FontId, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use super::snap::ALIGN_DISTANCE;
use super::{DeerawApp, Drawing, MAX_ZOOM_LEVEL, MIN_TEXT_BOX, MIN_ZOOM_LEVEL, SELECTION_COLOR, Tool, ZOOM_STEP};
use crate::cursor::{crosshair_cursor, grabbing_cursor, hand_cursor, i_beam_cursor};
use crate::export::{Export, ExportTarget};
use crate::markdown::toggle_marker;
use crate::geometry::{MIN_SIZE, align_candidates, align_guides, align_offset, distance_to_outline, distance_to_segment, normalize, pan_for_zoom, snap_angle};
use crate::object::{ArrowHeads, Kind};
use crate::paint::{layout_text, paint_arrow, paint_row_fills, text_padding, paint_checkerboard, paint_mosaic, paint_object, paint_shape, text_layout_job};
use crate::stroke::{add_stroke_points, paint_hand_drawn, paint_plain, speed_pressures};
use crate::style::{Background, Style};

/// How close (in points) the pointer must be to an arrow's line to grab it.
const ARROW_HIT_DISTANCE: f32 = 6.0;
/// Mouse wheel distance (in points) that counts as one zoom step, for smooth-scrolling devices.
const WHEEL_POINTS_PER_STEP: f32 = 50.0;

impl DeerawApp {
    pub(super) fn canvas(&mut self, ui: &mut egui::Ui) {
        let ppp = ui.ctx().pixels_per_point();
        let canvas_rect = ui.max_rect();
        // Showing, hiding or resizing the layers panel moves the canvas panel; the drawing stays put.
        // Resizing the window scales the drawing along, keeping what's at the canvas center there.
        // (Not while minimized, when the panel has no size.)
        if let (true, Some(previous)) = (canvas_rect.is_positive(), self.canvas_rect) {
            self.pan -= canvas_rect.min - previous.min;
            let window = ui.ctx().content_rect().size();
            if self.window_size == Some(window) {
                self.fit_size = self.fit_size.map(|fit| fit * canvas_rect.size() / previous.size());
            } else {
                self.fit_size.get_or_insert(previous.size());
                let old_scale = self.zoom() / ppp;
                self.canvas_rect = Some(canvas_rect);
                let center = previous.center() - canvas_rect.min;
                self.pan = pan_for_zoom(self.pan, center, old_scale, self.zoom() / ppp) + (canvas_rect.center() - previous.center());
            }
        }
        if canvas_rect.is_positive() {
            self.canvas_rect = Some(canvas_rect);
            self.window_size = Some(ui.ctx().content_rect().size());
        }
        self.navigate(ui, canvas_rect, ppp);

        // Canvas pixels <-> screen points. `scale` is screen points per canvas pixel.
        let origin = canvas_rect.min + self.pan;
        let scale = self.zoom() / ppp;
        let to_screen_pos = |p: Pos2| origin + p.to_vec2() * scale;
        let to_screen = |r: Rect| Rect::from_min_max(to_screen_pos(r.min), to_screen_pos(r.max));
        let to_canvas = |p: Pos2| ((p - origin) / scale).to_pos2();

        // Canvas shortcuts don't apply while typing into a text object.
        if self.editing.is_none() {
            for event in ui.input(|i| i.events.clone()) {
                match event {
                    egui::Event::PasteImage(image) => {
                        let center = self.paste_center(ui, canvas_rect);
                        self.place_image(ui.ctx(), image, center, canvas_rect, to_canvas);
                    }
                    egui::Event::Paste(text) if !text.trim().is_empty() => {
                        let at_start = self.start_paste_frame.take().is_some_and(|frame| ui.ctx().cumulative_frame_nr() <= frame + 2);
                        let center = self.paste_center(ui, canvas_rect);
                        if !at_start {
                            self.place_text(ui.painter(), &text, to_canvas(center));
                        }
                    }
                    egui::Event::Copy if !self.objects.is_empty() => {
                        self.export = Some(Export::new(self.objects.clone(), ExportTarget::Clipboard));
                    }
                    egui::Event::Cut if !self.selected.is_empty() => {
                        // A cut image takes its blurs along, so what's blurred stays blurred.
                        let cut = self
                            .objects
                            .iter()
                            .filter(|o| [Some(o.id), o.blurred_image()].iter().flatten().any(|id| self.selected.contains(id)))
                            .cloned()
                            .collect();
                        self.export = Some(Export::new(cut, ExportTarget::Clipboard));
                        self.delete_selected();
                    }
                    egui::Event::Key { key: egui::Key::Delete | egui::Key::Backspace, pressed: true, .. } => {
                        self.delete_selected();
                    }
                    egui::Event::Key { key, pressed: true, modifiers, .. } if modifiers.is_none() => {
                        // Escape first just ends a crop; otherwise it also deselects.
                        if key == egui::Key::Escape && self.cropping.is_none() {
                            self.selected.clear();
                        }
                        let tool = match key {
                            egui::Key::V | egui::Key::Escape => Some(Tool::Select),
                            egui::Key::P => Some(Tool::Pencil),
                            egui::Key::A => Some(Tool::Arrow),
                            egui::Key::L => Some(Tool::Line),
                            egui::Key::S => Some(Tool::Shape),
                            egui::Key::B => Some(Tool::Blur),
                            egui::Key::T => Some(Tool::Text),
                            egui::Key::C => {
                                self.toggle_crop();
                                None
                            }
                            _ => None,
                        };
                        if let Some(tool) = tool {
                            self.tool = tool;
                            self.blur_from_select = false;
                            // Escape (or any tool switch) also cancels a half-drawn shape.
                            self.drawing = None;
                            self.marquee = None;
                        }
                    }
                    egui::Event::Key { key, pressed: true, modifiers, .. } if modifiers.command => {
                        match key {
                            egui::Key::Z if modifiers.shift => self.history.redo(&mut self.objects),
                            egui::Key::Z => self.history.undo(&mut self.objects),
                            egui::Key::Y => self.history.redo(&mut self.objects),
                            egui::Key::D => self.duplicate_selected(),
                            #[cfg(not(target_arch = "wasm32"))]
                            egui::Key::S => self.save_as(),
                            #[cfg(not(target_arch = "wasm32"))]
                            egui::Key::O => self.open_file(),
                            egui::Key::A => self.select_all(),
                            _ => {}
                        }
                        let objects = &self.objects;
                        self.selected.retain(|id| objects.iter().any(|o| o.id == *id));
                    }
                    _ => {}
                }
            }
        }
        if let Some(image) = self.opened.take() {
            self.place_image(ui.ctx(), image, canvas_rect.center(), canvas_rect, to_canvas);
        }

        // Widgets registered later win the hit test: background, then objects
        // bottom to top, then the selected object's handles.
        let background =
            ui.interact(canvas_rect, egui::Id::unique("canvas"), Sense::click_and_drag());
        self.handle_tool(&background, to_canvas, to_screen_pos);

        let painter = ui.painter().clone();
        if self.background == Background::Transparent {
            paint_checkerboard(&painter, canvas_rect, 8.0, 242);
        }
        let selecting = self.tool == Tool::Select;
        // The Blur tool also selects, moves and resizes blurs (and only those).
        let blurring = self.tool == Tool::Blur;
        let ctrl = ui.input(|i| i.modifiers.command);
        if !ui.input(|i| i.pointer.primary_down()) {
            self.move_drag = None;
            self.guides.clear();
        }
        // Dragging any selected object moves all of them; applied after the loop.
        let mut move_by = Vec2::ZERO;
        let mut clicked = None;
        for obj in &mut self.objects {
            let editing = self.editing == Some(obj.id);
            if let Kind::Text { text, font_px, color, align, frame, .. } = &obj.kind {
                // Keep the rect in sync with the text layout. Measured at a fixed scale (1 point
                // per canvas pixel) so zooming doesn't change it.
                let galley = layout_text(&painter, text, *font_px, *color, *align, *frame, editing);
                let size = match frame {
                    Some(frame) => Vec2::new(frame.x, frame.y.max(galley.size().y)),
                    None => galley.size() + 2.0 * text_padding(*font_px),
                };
                let size = size.max(Vec2::splat(MIN_SIZE));
                obj.rect = Rect::from_min_size(obj.rect.min, size);
            }
            if let Kind::Arrow { start, end, .. } = obj.kind {
                obj.rect = Rect::from_two_pos(start, end);
            }
            let rect = to_screen(obj.rect);
            let id = egui::Id::unique(("object", obj.id));
            // Arrows and unfilled shapes are only grabbed near their line, not anywhere in their
            // bounding box (a selected shape is, so it's easy to move).
            let hit = match obj.kind {
                Kind::Shape { shape, width, fill: None, .. } if !self.selected.contains(&obj.id) => {
                    let near = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| {
                        distance_to_outline(shape, rect, p) <= ARROW_HIT_DISTANCE + width * scale / 2.0
                    });
                    near || ui.ctx().is_being_dragged(id)
                }
                Kind::Arrow { start, end, width, .. } => {
                    let near = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| {
                        distance_to_segment(p, to_screen_pos(start), to_screen_pos(end))
                            <= ARROW_HIT_DISTANCE + width * scale / 2.0
                    });
                    near || ui.ctx().is_being_dragged(id)
                }
                _ => true,
            };
            let is_blur = matches!(obj.kind, Kind::Blur { .. });
            // Back from a picked blur, other objects take clicks (back to Select), but drags still
            // reach the background to draw a blur.
            let back_to_select = blurring && !is_blur && self.blur_from_select;
            // With Select, an unselected blur also takes only clicks, so dragging over one moves
            // the image under it instead; once clicked (selected), it drags.
            let click_only = back_to_select || selecting && is_blur && !self.selected.contains(&obj.id);
            if (selecting || blurring && is_blur || click_only) && !editing && hit {
                let sense = if click_only { Sense::click() } else { Sense::click_and_drag() };
                let response = ui.interact(rect.expand(ARROW_HIT_DISTANCE), id, sense);
                if response.clicked() {
                    clicked = Some(obj.id);
                }
                // Starting to drag an unselected object selects it (adds it with Ctrl), so the
                // drag moves it; dragging a selected one keeps the whole selection.
                if response.drag_started_by(PointerButton::Primary) && !self.selected.contains(&obj.id) {
                    if !ctrl {
                        self.selected.clear();
                    }
                    self.selected.push(obj.id);
                }
                // Picking a blur with Select switches to the Blur tool, which then edits it.
                if is_blur && selecting && (response.clicked() || response.drag_started_by(PointerButton::Primary)) {
                    self.tool = Tool::Blur;
                    self.blur_from_select = true;
                }
                if back_to_select && response.clicked() {
                    self.tool = Tool::Select;
                }
                if response.double_clicked() {
                    match obj.kind {
                        Kind::Text { .. } => {
                            self.editing = Some(obj.id);
                            self.focus_editor = true;
                        }
                        Kind::Image { .. } => self.cropping = Some(obj.id),
                        _ => {}
                    }
                }
                if response.dragged_by(PointerButton::Primary) {
                    move_by = response.drag_delta() / scale;
                }
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(CursorIcon::Move);
                }
            }
        }
        if let Some(id) = clicked {
            self.click_select(id, ctrl);
        }
        if move_by != Vec2::ZERO {
            // The selection's bounds snap to line up their center with other objects' centers;
            // only if it's all images, text and shapes (blurs, strokes and arrows move freely).
            self.detach_selected();
            let (dragged, moved) = self.move_drag.unwrap_or_default();
            let dragged = dragged + move_by;
            let selected = || self.objects.iter().filter(|o| self.selected.contains(&o.id));
            let bounds = selected().fold(Rect::NOTHING, |b, o| b.union(o.rect));
            let targets = if selected().all(|o| o.bindable()) { align_candidates(bounds, &self.align_targets(ui.ctx())) } else { vec![] };
            let snap = align_offset(bounds.translate(dragged - moved), &targets, ALIGN_DISTANCE / scale);
            let delta = dragged + snap - moved;
            for obj in self.objects.iter_mut().filter(|o| self.selected.contains(&o.id)) {
                obj.translate(delta);
            }
            self.move_drag = Some((dragged, dragged + snap));
            if self.first_image.is_some_and(|id| self.selected.contains(&id)) {
                self.first_image = None;
            }
            self.guides = align_guides(bounds.translate(delta), &targets);
            self.update_blur_areas();
        }
        self.follow_blurs();
        self.follow_bindings();
        self.update_mosaics(ui.ctx());
        for obj in &self.objects {
            if self.editing != Some(obj.id) {
                paint_object(&painter, obj, &self.objects, to_screen(obj.rect), to_screen_pos, scale);
            }
        }
        if let Some((a, b)) = self.marquee {
            let rect = Rect::from_two_pos(to_screen_pos(a), to_screen_pos(b));
            painter.rect_filled(rect, 0.0, SELECTION_COLOR.gamma_multiply(0.1));
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, SELECTION_COLOR), StrokeKind::Inside);
        }

        match &self.drawing {
            Some(Drawing::Stroke(points, times)) => {
                let screen: Vec<Pos2> = points.iter().map(|p| to_screen_pos(*p)).collect();
                let (width, color) = (self.style.line_width * scale, self.style.color);
                if self.style.hand_drawn {
                    paint_hand_drawn(&painter, &screen, &speed_pressures(points, times), width, color);
                } else {
                    paint_plain(&painter, &screen, width, color);
                }
            }
            Some(Drawing::Arrow(start, end, bindings)) => {
                let stroke = Stroke::new(self.style.line_width * scale, self.style.color);
                // Only the Line tool draws lines; connecting objects with Select draws arrows.
                let heads = if self.tool == Tool::Line { ArrowHeads::NONE } else { self.style.arrow_heads };
                paint_arrow(&painter, to_screen_pos(*start), to_screen_pos(*end), stroke, scale, heads);
                self.paint_snap_circles(&painter, ui.input(|i| i.pointer.hover_pos()), bindings, to_screen_pos);
            }
            Some(Drawing::TextBox(a, b)) => {
                let rect = Rect::from_two_pos(to_screen_pos(*a), to_screen_pos(*b));
                painter.rect_stroke(rect, 0.0, Stroke::new(1.0, SELECTION_COLOR), StrokeKind::Inside);
            }
            Some(Drawing::Shape(a, b)) => {
                let rect = Rect::from_two_pos(to_screen_pos(*a), to_screen_pos(*b));
                let stroke = if self.style.border { Stroke::new(self.style.line_width * scale, self.style.color) } else { Stroke::NONE };
                paint_shape(&painter, self.style.shape, rect, stroke, self.style.fill);
            }
            Some(Drawing::Blur(image, a, b)) => {
                // Until it reaches an image, just the box.
                let mut area = Rect::from_two_pos(*a, *b);
                if let Some(image) = self.objects.iter().find(|o| Some(o.id) == *image) {
                    area = area.intersect(image.rect);
                    paint_mosaic(&painter, image, area, self.style.blur, to_screen_pos);
                }
                painter.rect_stroke(to_screen(area), 0.0, Stroke::new(1.0, SELECTION_COLOR), StrokeKind::Inside);
            }
            None => {}
        }

        if self.cropping.is_some() {
            self.crop_ui(ui, to_screen, to_canvas, scale);
        } else if blurring {
            let objects = &self.objects;
            if self.selected.iter().all(|id| objects.iter().any(|o| o.id == *id && matches!(o.kind, Kind::Blur { .. }))) {
                self.selection_handles(ui, to_screen, to_screen_pos, to_canvas, scale);
                if self.resize_drag.is_some() {
                    self.update_blur_areas();
                }
            }
        } else if selecting {
            if self.editing.is_none() {
                self.connect_ui(ui, to_canvas, to_screen_pos);
            }
            self.selection_handles(ui, to_screen, to_screen_pos, to_canvas, scale);
            if self.resize_drag.is_some() {
                self.update_blur_areas();
            }
        }
        self.paint_guides(ui.painter(), to_screen_pos);
        self.text_editor(ui, to_screen_pos, scale);
        // Registered after the objects and handles, so a space-drag doesn't reach them.
        if self.space_pan {
            ui.interact(canvas_rect, egui::Id::unique("space-pan"), Sense::drag());
        }
        let toolbar = self.toolbar(ui, canvas_rect, ppp);
        self.style_panel(ui, toolbar);
        self.update_title(ui.ctx());
        self.notice(ui, canvas_rect);
        // Set last so it wins over the tool's cursor and hover cursors of whatever is under the pointer.
        if self.panning {
            ui.ctx().set_cursor_image(Some(self.grabbing.get_or_insert_with(grabbing_cursor).clone()));
        } else if self.space_pan && ui.rect_contains_pointer(canvas_rect) {
            ui.ctx().set_cursor_image(Some(self.hand.get_or_insert_with(hand_cursor).clone()));
        }

        let settled = !ui.input(|i| i.pointer.any_down())
            && self.editing.is_none()
            && self.cropping.is_none()
            && self.drawing.is_none();
        if settled {
            // Resizing (after painting) moved objects this frame; their arrows and blurs go in the same undo step.
            self.follow_blurs();
            self.follow_bindings();
            self.update_mosaics(ui.ctx());
            self.history.record(&self.objects);
        }
    }

    /// Middle-drag, or left-drag while holding Space, pans; the mouse wheel (around the cursor) and Ctrl +/- (around the canvas
    /// center) zoom in 20% steps; Ctrl+0 resets to 100%.
    pub(super) fn navigate(&mut self, ui: &egui::Ui, canvas_rect: Rect, ppp: f32) {
        let (hover, delta, space, any_down, pan_down, pan_pressed) = ui.input(|i| {
            (
                i.pointer.hover_pos(),
                i.pointer.delta(),
                i.key_down(egui::Key::Space),
                i.pointer.any_down(),
                i.pointer.middle_down() || i.pointer.primary_down(),
                i.pointer.button_pressed(PointerButton::Middle)
                    || (self.space_pan && i.pointer.button_pressed(PointerButton::Primary)),
            )
        });
        let over_canvas = hover.filter(|p| canvas_rect.contains(*p));

        // Pressing Space mid-drag doesn't turn that drag into a pan.
        if space && self.editing.is_none() && (!any_down || self.space_pan) {
            self.space_pan = true;
        } else if !space {
            self.space_pan = false;
        }
        if pan_pressed && over_canvas.is_some() {
            self.panning = true;
        }
        if !pan_down {
            self.panning = false;
        }
        if self.panning {
            self.pan += delta;
        }

        let mut key_steps = 0;
        let mut reset = false;
        for event in ui.input(|i| i.events.clone()) {
            match event {
                egui::Event::MouseWheel { unit, delta, .. } if over_canvas.is_some() => {
                    self.wheel += match unit {
                        // macOS accelerates wheel deltas, so a slow notch is only a fraction of a line:
                        // there each notch (one event) is one step.
                        egui::MouseWheelUnit::Line if cfg!(target_os = "macos") => delta.y.signum(),
                        egui::MouseWheelUnit::Line => delta.y,
                        egui::MouseWheelUnit::Point => delta.y / WHEEL_POINTS_PER_STEP,
                        egui::MouseWheelUnit::Page => delta.y * 10.0,
                    };
                }
                egui::Event::Key { key, pressed: true, modifiers, .. }
                    if modifiers.command && self.editing.is_none() =>
                {
                    match key {
                        egui::Key::Plus | egui::Key::Equals => key_steps += 1,
                        egui::Key::Minus => key_steps -= 1,
                        egui::Key::Num0 => reset = true,
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        let wheel_steps = self.wheel.trunc();
        self.wheel -= wheel_steps;

        let old_scale = self.zoom() / ppp;
        if reset {
            self.zoom_level = 0;
            self.fit_size = None;
        } else if key_steps + wheel_steps as i32 != 0 {
            let steps = key_steps + wheel_steps as i32;
            // From a zoom off the 20% grid (after a window resize), the first step lands on the
            // next grid level in that direction, dropping the resize factor.
            let level = (self.zoom() - 1.0) / ZOOM_STEP;
            let from = if steps > 0 { (level + 0.01).floor() } else { (level - 0.01).ceil() };
            self.zoom_level = (from as i32 + steps).clamp(MIN_ZOOM_LEVEL, MAX_ZOOM_LEVEL);
            self.fit_size = None;
        }
        if self.zoom() / ppp == old_scale {
            return;
        }
        // Keep the canvas point under the anchor fixed on screen.
        let anchor = over_canvas
            .filter(|_| wheel_steps != 0.0)
            .unwrap_or(canvas_rect.center());
        self.pan = pan_for_zoom(self.pan, anchor - canvas_rect.min, old_scale, self.zoom() / ppp);
    }

    /// What a click or drag on empty canvas does with the current tool.
    pub(super) fn handle_tool(
        &mut self,
        background: &egui::Response,
        to_canvas: impl Fn(Pos2) -> Pos2,
        to_screen_pos: impl Fn(Pos2) -> Pos2,
    ) {
        // Clicking away from the text being typed ends it (and the Text tool) instead of starting
        // another text there. This runs before the editor sees it lose focus.
        if self.editing.is_some() && background.is_pointer_button_down_on() {
            self.end_editing();
        }
        let pointer = background.interact_pointer_pos().map(&to_canvas);
        // The cursor image is sticky between frames, so it must be cleared explicitly.
        let cursor = match self.tool {
            _ if !background.hovered() => None,
            Tool::Pencil | Tool::Arrow | Tool::Line | Tool::Shape | Tool::Blur => Some(self.crosshair.get_or_insert_with(crosshair_cursor)),
            Tool::Text => Some(self.i_beam.get_or_insert_with(i_beam_cursor)),
            Tool::Select => None,
        };
        background.ctx.set_cursor_image(cursor.cloned());
        match self.tool {
            Tool::Select => {
                let ctrl = background.ctx.input(|i| i.modifiers.command);
                if background.clicked() && !ctrl {
                    self.selected.clear();
                }
                // Double-clicking empty space starts typing there, as a click does with Text.
                if let (true, Some(p)) = (background.double_clicked(), pointer) {
                    self.tool = Tool::Text;
                    self.start_text(p - Vec2::new(0.0, self.style.font_px / 2.0), None);
                }
                if let (true, Some(p)) = (background.drag_started_by(PointerButton::Primary), pointer) {
                    self.marquee = Some((p, p));
                }
                if let (true, Some(p), Some((_, end))) =
                    (background.dragged_by(PointerButton::Primary), pointer, &mut self.marquee)
                {
                    *end = p;
                }
                // Selects the objects entirely inside the box (adds them with Ctrl).
                if background.drag_stopped()
                    && let Some((a, b)) = self.marquee.take()
                {
                    let area = Rect::from_two_pos(a, b);
                    if !ctrl {
                        self.selected.clear();
                    }
                    for obj in &self.objects {
                        if area.contains_rect(obj.rect) && !self.selected.contains(&obj.id) {
                            self.selected.push(obj.id);
                        }
                    }
                }
            }
            Tool::Pencil => {
                // Starts on press, not after the drag threshold, so a click leaves a dot.
                let (now, primary_down, origin) =
                    background.ctx.input(|i| (i.time, i.pointer.primary_down(), i.pointer.press_origin()));
                let down = primary_down && background.is_pointer_button_down_on();
                match (&mut self.drawing, down, pointer) {
                    (None, true, Some(p)) => {
                        let start = origin.map_or(p, &to_canvas);
                        self.drawing = Some(Drawing::Stroke(vec![start], vec![now]));
                    }
                    (Some(Drawing::Stroke(points, times)), true, Some(_)) => {
                        // Every move since last frame, not just the latest position: fast
                        // strokes would otherwise turn into a few long straight segments.
                        let moves: Vec<Pos2> = background.ctx.input(|i| {
                            i.events
                                .iter()
                                .filter_map(|e| match e {
                                    egui::Event::PointerMoved(pos) => Some(to_canvas(*pos)),
                                    _ => None,
                                })
                                .collect()
                        });
                        add_stroke_points(points, times, &moves, now);
                    }
                    (Some(_), false, _) => self.finish_drawing(),
                    _ => {}
                }
            }
            Tool::Arrow | Tool::Line => {
                // Either end snaps to a nearby text or image side and attaches to it; a free end
                // snaps to nearly horizontal, vertical or 45° lines.
                if let (true, Some(p)) = (background.drag_started_by(PointerButton::Primary), pointer) {
                    let origin = background.ctx.input(|i| i.pointer.press_origin());
                    let snap = origin.and_then(|o| self.snap(o, &to_screen_pos));
                    let start = snap.map_or(p, |(_, anchor)| anchor);
                    self.drawing = Some(Drawing::Arrow(start, start, [snap.map(|(b, _)| b), None]));
                }
                if let (true, Some(p), Some(screen)) =
                    (background.dragged_by(PointerButton::Primary), pointer, background.interact_pointer_pos())
                {
                    let snap = self.snap(screen, &to_screen_pos);
                    if let Some(Drawing::Arrow(start, end, bindings)) = &mut self.drawing {
                        *end = snap.map_or(snap_angle(*start, p), |(_, anchor)| anchor);
                        bindings[1] = snap.map(|(b, _)| b);
                    }
                }
                if background.drag_stopped() {
                    self.finish_drawing();
                }
            }
            Tool::Shape => {
                // Drag from corner to corner; Shift makes it a square (or circle).
                if let (true, Some(p)) = (background.drag_started_by(PointerButton::Primary), pointer) {
                    let origin = background.ctx.input(|i| i.pointer.press_origin()).map_or(p, &to_canvas);
                    self.drawing = Some(Drawing::Shape(origin, p));
                }
                let shift = background.ctx.input(|i| i.modifiers.shift);
                if let (true, Some(p), Some(Drawing::Shape(start, end))) =
                    (background.dragged_by(PointerButton::Primary), pointer, &mut self.drawing)
                {
                    *end = if shift { square_corner(*start, p) } else { p };
                }
                if background.drag_stopped() {
                    self.finish_drawing();
                }
            }
            Tool::Blur => self.blur_tool(background, to_canvas),
            Tool::Text => {
                // Click: single-line text centered vertically on the click.
                if let (true, Some(p)) = (background.clicked(), pointer) {
                    self.start_text(p - Vec2::new(0.0, self.style.font_px / 2.0), None);
                }
                // Drag: a box whose width the text wraps at.
                if let (true, Some(p)) = (background.drag_started_by(PointerButton::Primary), pointer) {
                    self.drawing = Some(Drawing::TextBox(p, p));
                }
                if let (true, Some(p), Some(Drawing::TextBox(_, end))) =
                    (background.dragged_by(PointerButton::Primary), pointer, &mut self.drawing)
                {
                    *end = p;
                }
                if background.drag_stopped() {
                    self.finish_drawing();
                }
            }
        }
    }

    /// Where a paste goes (screen): under the pointer, or the canvas center for Edit > Paste
    /// (the pointer is still where the menu was) or when the pointer is elsewhere.
    fn paste_center(&mut self, ui: &egui::Ui, canvas_rect: Rect) -> Pos2 {
        let from_menu = self.menu_paste_frame.take().is_some_and(|frame| ui.ctx().cumulative_frame_nr() <= frame + 2);
        ui.input(|i| i.pointer.hover_pos())
            .filter(|p| canvas_rect.contains(*p) && !from_menu)
            .unwrap_or(canvas_rect.center())
    }

    /// Adds pasted text as inline text in the Text tool's style, centered on `center` (canvas), and selects it.
    fn place_text(&mut self, painter: &egui::Painter, text: &str, center: Pos2) {
        let text = text.replace("\r\n", "\n").trim_end().to_owned();
        let Style { font_px, text_color: color, align, fill, .. } = self.style;
        let size = layout_text(painter, &text, font_px, color, align, None, false).size() + 2.0 * text_padding(font_px);
        self.add(Rect::from_center_size(center, size), Kind::Text { text, font_px, color, align, fill, frame: None });
        self.tool = Tool::Select;
    }

    /// Adds an empty text object (its text at `min`, or as the box `frame` there) and starts editing it.
    pub(super) fn start_text(&mut self, min: Pos2, frame: Option<Vec2>) {
        let Style { font_px, text_color: color, align, fill, .. } = self.style;
        // Inline text: the text itself (inside the padding) starts at `min`.
        let min = if frame.is_none() { min - text_padding(font_px) } else { min };
        let rect = Rect::from_min_size(min, frame.unwrap_or(Vec2::splat(MIN_SIZE)));
        let id = self.add(rect, Kind::Text { text: String::new(), font_px, color, align, fill, frame });
        self.editing = Some(id);
        self.focus_editor = true;
    }

    pub(super) fn finish_drawing(&mut self) {
        match self.drawing.take() {
            Some(Drawing::Stroke(points, times)) => {
                let pressures = speed_pressures(&points, &times);
                let (rect, points) = normalize(&points);
                let Style { color, line_width: width, hand_drawn, .. } = self.style;
                self.add(rect, Kind::Stroke { points, pressures, color, width, hand_drawn });
            }
            Some(Drawing::Arrow(start, end, bindings)) if start.distance(end) >= MIN_SIZE => {
                let Style { color, line_width: width, arrow_heads, .. } = self.style;
                let heads = (self.tool != Tool::Line).then_some(arrow_heads);
                self.add(Rect::from_two_pos(start, end), Kind::Arrow { start, end, color, width, heads, bindings });
            }
            Some(Drawing::Shape(a, b)) => {
                let rect = Rect::from_two_pos(a, b);
                if rect.width().max(rect.height()) >= MIN_SIZE {
                    let Style { color, line_width: width, shape, border, fill, .. } = self.style;
                    self.add(rect.expand2((Vec2::splat(MIN_SIZE) - rect.size()).max(Vec2::ZERO) / 2.0), Kind::Shape { shape, color, width, border, fill });
                }
            }
            Some(Drawing::Blur(Some(image), a, b)) => self.add_blur(image, Rect::from_two_pos(a, b)),
            Some(Drawing::TextBox(a, b)) => {
                let rect = Rect::from_two_pos(a, b);
                // Too narrow to be deliberate: treat it as a click.
                if rect.width() < MIN_TEXT_BOX {
                    self.start_text(a - Vec2::new(0.0, self.style.font_px / 2.0), None);
                } else {
                    self.start_text(rect.min, Some(rect.size().max(Vec2::splat(MIN_TEXT_BOX))));
                }
            }
            _ => {}
        }
    }

    /// Shows an inline text box over the text object being edited.
    pub(super) fn text_editor(&mut self, ui: &mut egui::Ui, to_screen_pos: impl Fn(Pos2) -> Pos2, scale: f32) {
        let Some(id) = self.editing else { return };
        let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) else {
            self.editing = None;
            return;
        };
        let Kind::Text { text, font_px, color, align, fill, frame } = &mut obj.kind else {
            self.editing = None;
            return;
        };
        let font = FontId::proportional(*font_px * scale);
        let edit_id = egui::Id::unique(("text-edit", id));
        // Ctrl+B and Ctrl+I wrap the selected text in bold or italic markers (or unwrap it).
        let marker = ui.input_mut(|i| {
            if i.consume_key(egui::Modifiers::COMMAND, egui::Key::B) {
                Some("**")
            } else if i.consume_key(egui::Modifiers::COMMAND, egui::Key::I) {
                Some("*")
            } else {
                None
            }
        });
        if let Some(marker) = marker
            && let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), edit_id)
            && let Some(range) = state.cursor.char_range()
        {
            let [start, end] = range.sorted_cursors();
            let (start, end) = toggle_marker(text, start.index.0, end.index.0, marker);
            let range = egui::text_selection::CCursorRange::two(egui::text::CCursor::new(start), egui::text::CCursor::new(end));
            state.cursor.set_char_range(Some(range));
            state.store(ui.ctx(), edit_id);
        }
        // Lay out exactly like `layout_text` (egui would otherwise wrap at the available width).
        let screen_frame = frame.map(|f| f * scale);
        let (color, text_align, fill) = (*color, *align, *fill);
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _: f32| {
            let mut job = text_layout_job(buf.as_str(), *font_px * scale, color, text_align, screen_frame, true);
            job.keep_trailing_whitespace = true;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        // A text box's editor fills the box, with the text aligned in it like when painted.
        let (box_size, align) = match screen_frame {
            Some(_) => (obj.rect.size() * scale, text_align),
            None => (Vec2::splat(1.0), Align2::LEFT_TOP),
        };
        // The object itself isn't painted while being edited.
        if screen_frame.is_some() {
            let outline = Rect::from_min_size(to_screen_pos(obj.rect.min), box_size);
            if let Some(fill) = fill {
                ui.painter().rect_filled(outline, 0.0, fill);
            }
            let stroke = Stroke::new(1.0, SELECTION_COLOR.gamma_multiply(0.5));
            ui.painter().rect_stroke(outline, 0.0, stroke, StrokeKind::Outside);
        }
        // Inline text sits inside its padding.
        let editor_pos = match screen_frame {
            Some(_) => to_screen_pos(obj.rect.min),
            None => to_screen_pos(obj.rect.min) + text_padding(*font_px * scale),
        };
        if let (None, Some(fill)) = (screen_frame, fill) {
            let galley = layout_text(ui.painter(), text, *font_px * scale, color, text_align, None, true);
            let pos = editor_pos - Vec2::new(galley.rect.left(), 0.0);
            paint_row_fills(ui.painter(), &galley, pos, *font_px * scale, fill);
        }
        let output = egui::Area::new(egui::Id::unique(("text-editor", id)))
            .fixed_pos(editor_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                // The dark theme's caret is light blue, invisible on a white canvas; this color
                // shows on every background.
                ui.visuals_mut().text_cursor.stroke = Stroke::new(2.0, SELECTION_COLOR);
                egui::TextEdit::multiline(text)
                    .id(edit_id)
                    .font(font.clone())
                    .layouter(&mut layouter)
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::ZERO)
                    .desired_width(box_size.x)
                    .min_size(box_size)
                    .align(align)
                    .desired_rows(1)
                    .clip_text(false)
                    .show(ui)
            })
            .inner;
        if std::mem::take(&mut self.focus_editor) {
            output.response.request_focus();
        }
        // Escape or clicking elsewhere (e.g. the toolbar) ends editing.
        if output.response.lost_focus() {
            self.end_editing();
        }
    }

    /// Ends text editing and switches to Select; empty text is discarded, otherwise it stays selected.
    pub(super) fn end_editing(&mut self) {
        let Some(id) = self.editing.take() else { return };
        self.tool = Tool::Select;
        let empty = self
            .objects
            .iter()
            .any(|o| o.id == id && matches!(&o.kind, Kind::Text { text, .. } if text.trim().is_empty()));
        if empty {
            self.objects.retain(|o| o.id != id);
        } else {
            self.selected = vec![id];
        }
    }
}

/// The corner opposite `start` of the square spanning to `p` (its larger side).
fn square_corner(start: Pos2, p: Pos2) -> Pos2 {
    let d = p - start;
    let side = d.x.abs().max(d.y.abs());
    start + Vec2::new(side * d.x.signum(), side * d.y.signum())
}
