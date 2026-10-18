//! The canvas: navigation, input, drawing with the tools, painting and text editing.

use eframe::egui::{self, Align2, CursorIcon, FontId, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use super::{DeerawApp, Drawing, MAX_ZOOM_LEVEL, MIN_TEXT_BOX, MIN_ZOOM_LEVEL, SELECTION_COLOR, Tool};
use crate::cursor::{crosshair_cursor, grabbing_cursor, hand_cursor, i_beam_cursor};
use crate::export::{Export, ExportTarget};
use crate::geometry::{MIN_SIZE, distance_to_segment, normalize, pan_for_zoom};
use crate::object::{Kind, TextFrame};
use crate::paint::{layout_text, paint_arrow, paint_checkerboard, paint_object, text_layout_job};
use crate::stroke::{add_stroke_points, paint_hand_drawn, speed_pressures};
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
        if let Some(previous) = self.canvas_min.replace(canvas_rect.min) {
            self.pan -= canvas_rect.min - previous;
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
                        let from_menu = self
                            .menu_paste_frame
                            .take()
                            .is_some_and(|frame| ui.ctx().cumulative_frame_nr() <= frame + 2);
                        let center = ui
                            .input(|i| i.pointer.hover_pos())
                            .filter(|p| canvas_rect.contains(*p) && !from_menu)
                            .unwrap_or(canvas_rect.center());
                        self.place_image(ui.ctx(), image, center, canvas_rect, to_canvas);
                    }
                    egui::Event::Copy if !self.objects.is_empty() => {
                        self.export = Some(Export::new(self.objects.clone(), ExportTarget::Clipboard));
                    }
                    egui::Event::Cut if !self.selected.is_empty() => {
                        let cut = self.objects.iter().filter(|o| self.selected.contains(&o.id)).cloned().collect();
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
                            egui::Key::T => Some(Tool::Text),
                            egui::Key::C => {
                                self.toggle_crop();
                                None
                            }
                            _ => None,
                        };
                        if let Some(tool) = tool {
                            self.tool = tool;
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
        let ctrl = ui.input(|i| i.modifiers.command);
        // Dragging any selected object moves all of them; applied after the loop.
        let mut move_by = Vec2::ZERO;
        let mut clicked = None;
        for obj in &mut self.objects {
            let editing = self.editing == Some(obj.id);
            if let Kind::Text { text, font_px, color, frame } = &obj.kind {
                // Keep the rect in sync with the text layout. Measured at a fixed scale (1 point
                // per canvas pixel) so zooming doesn't change it.
                let galley = layout_text(&painter, text, *font_px, *color, *frame);
                let mut size = galley.size().max(Vec2::splat(MIN_SIZE));
                if let Some(frame) = frame {
                    size = Vec2::new(frame.size.x, frame.size.y.max(size.y));
                }
                obj.rect = Rect::from_min_size(obj.rect.min, size);
            }
            if let Kind::Arrow { start, end, .. } = obj.kind {
                obj.rect = Rect::from_two_pos(start, end);
            }
            let rect = to_screen(obj.rect);
            let id = egui::Id::unique(("object", obj.id));
            // Arrows are only grabbed near their line, not anywhere in their bounding box.
            let hit = match obj.kind {
                Kind::Arrow { start, end, width, .. } => {
                    let near = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| {
                        distance_to_segment(p, to_screen_pos(start), to_screen_pos(end))
                            <= ARROW_HIT_DISTANCE + width * scale / 2.0
                    });
                    near || ui.ctx().is_being_dragged(id)
                }
                _ => true,
            };
            if selecting && !editing && hit {
                let response = ui.interact(rect.expand(ARROW_HIT_DISTANCE), id, Sense::click_and_drag());
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
            self.detach_selected();
            for obj in self.objects.iter_mut().filter(|o| self.selected.contains(&o.id)) {
                obj.translate(move_by);
            }
        }
        self.follow_bindings();
        for obj in &self.objects {
            if self.editing != Some(obj.id) {
                paint_object(&painter, obj, to_screen(obj.rect), to_screen_pos, scale);
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
                } else if let [dot] = screen[..] {
                    painter.circle_filled(dot, width / 2.0, color);
                } else {
                    painter.line(screen, Stroke::new(width, color));
                }
            }
            Some(Drawing::Arrow(start, end, bindings)) => {
                let stroke = Stroke::new(self.style.line_width * scale, self.style.color);
                let head = self.tool == Tool::Arrow;
                paint_arrow(&painter, to_screen_pos(*start), to_screen_pos(*end), stroke, scale, head);
                self.paint_snap_circles(&painter, ui.input(|i| i.pointer.hover_pos()), bindings, to_screen_pos);
            }
            Some(Drawing::TextBox(a, b)) => {
                let rect = Rect::from_two_pos(to_screen_pos(*a), to_screen_pos(*b));
                painter.rect_stroke(rect, 0.0, Stroke::new(1.0, SELECTION_COLOR), StrokeKind::Inside);
            }
            None => {}
        }

        if self.cropping.is_some() {
            self.crop_ui(ui, to_screen, to_canvas, scale);
        } else if selecting {
            self.selection_handles(ui, to_screen, to_screen_pos, to_canvas, scale);
        }
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
            // Resizing (after painting) moved objects this frame; their arrows go in the same undo step.
            self.follow_bindings();
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

        let level = if reset {
            0
        } else {
            let steps = key_steps + wheel_steps as i32;
            (self.zoom_level + steps).clamp(MIN_ZOOM_LEVEL, MAX_ZOOM_LEVEL)
        };
        if level == self.zoom_level {
            return;
        }
        // Keep the canvas point under the anchor fixed on screen.
        let anchor = over_canvas
            .filter(|_| wheel_steps != 0.0)
            .unwrap_or(canvas_rect.center());
        let old_scale = self.zoom() / ppp;
        self.zoom_level = level;
        self.pan = pan_for_zoom(self.pan, anchor - canvas_rect.min, old_scale, self.zoom() / ppp);
    }

    /// What a click or drag on empty canvas does with the current tool.
    pub(super) fn handle_tool(
        &mut self,
        background: &egui::Response,
        to_canvas: impl Fn(Pos2) -> Pos2,
        to_screen_pos: impl Fn(Pos2) -> Pos2,
    ) {
        let pointer = background.interact_pointer_pos().map(&to_canvas);
        // The cursor image is sticky between frames, so it must be cleared explicitly.
        let cursor = match self.tool {
            _ if !background.hovered() => None,
            Tool::Pencil | Tool::Arrow | Tool::Line => Some(self.crosshair.get_or_insert_with(crosshair_cursor)),
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
                // Either end snaps to a nearby text or image side and attaches to it.
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
                    if let Some(Drawing::Arrow(_, end, bindings)) = &mut self.drawing {
                        *end = snap.map_or(p, |(_, anchor)| anchor);
                        bindings[1] = snap.map(|(b, _)| b);
                    }
                }
                if background.drag_stopped() {
                    self.finish_drawing();
                }
            }
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

    /// Adds an empty text object (at `min`, or as the box `frame`) and starts editing it.
    pub(super) fn start_text(&mut self, min: Pos2, frame: Option<Vec2>) {
        let Style { font_px, color, align, fill, .. } = self.style;
        let rect = Rect::from_min_size(min, frame.unwrap_or(Vec2::splat(MIN_SIZE)));
        let frame = frame.map(|size| TextFrame { size, align, fill });
        let id = self.add(rect, Kind::Text { text: String::new(), font_px, color, frame });
        self.editing = Some(id);
        self.focus_editor = true;
        self.tool = Tool::Select;
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
                let Style { color, line_width: width, .. } = self.style;
                let head = self.tool == Tool::Arrow;
                self.add(Rect::from_two_pos(start, end), Kind::Arrow { start, end, color, width, head, bindings });
            }
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
        let Kind::Text { text, font_px, color, frame } = &mut obj.kind else {
            self.editing = None;
            return;
        };
        let font = FontId::proportional(*font_px * scale);
        // Lay out exactly like `layout_text` (egui would otherwise wrap at the available width).
        let screen_frame = frame.map(|f| TextFrame { size: f.size * scale, ..f });
        let color = *color;
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _: f32| {
            let mut job = text_layout_job(buf.as_str(), font.clone(), color, screen_frame);
            job.keep_trailing_whitespace = true;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        // A text box's editor fills the box, with the text aligned in it like when painted.
        let (box_size, align) = match screen_frame {
            Some(f) => (obj.rect.size() * scale, f.align),
            None => (Vec2::splat(1.0), Align2::LEFT_TOP),
        };
        if let Some(frame) = screen_frame {
            // The object itself isn't painted while being edited.
            let outline = Rect::from_min_size(to_screen_pos(obj.rect.min), box_size);
            if let Some(fill) = frame.fill {
                ui.painter().rect_filled(outline, 0.0, fill);
            }
            let stroke = Stroke::new(1.0, SELECTION_COLOR.gamma_multiply(0.5));
            ui.painter().rect_stroke(outline, 0.0, stroke, StrokeKind::Outside);
        }
        let output = egui::Area::new(egui::Id::unique(("text-editor", id)))
            .fixed_pos(to_screen_pos(obj.rect.min))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                // The dark theme's caret is light blue, invisible on a white canvas; this color
                // shows on every background.
                ui.visuals_mut().text_cursor.stroke = Stroke::new(2.0, SELECTION_COLOR);
                egui::TextEdit::multiline(text)
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
        // Escape or clicking elsewhere ends editing; empty text is discarded.
        if output.response.lost_focus() {
            self.editing = None;
            if text.trim().is_empty() {
                self.objects.retain(|o| o.id != id);
            } else {
                self.selected = vec![id];
            }
        }
    }
}
