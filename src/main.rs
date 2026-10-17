use std::sync::{Arc, Mutex};

use eframe::egui::{
    self, Align2, Color32, ColorImage, CursorIcon, FontId, Painter, Pos2, Rect, Sense, Stroke,
    StrokeKind, TextureHandle, Vec2, emath::Rot2,
};

const SELECTION_COLOR: Color32 = Color32::from_rgb(105, 101, 219);
const INK: Color32 = Color32::BLACK;
/// Size of a resize handle, in points.
const HANDLE_SIZE: f32 = 8.0;
/// Smallest width/height an object can have, in canvas pixels.
const MIN_SIZE: f32 = 4.0;
/// Line width of pencil strokes and arrows, in canvas pixels.
const LINE_WIDTH: f32 = 3.0;
/// Length of an arrow's head lines, in canvas pixels.
const ARROW_HEAD: f32 = 18.0;
/// Font size of new text, in canvas pixels.
const TEXT_SIZE: f32 = 28.0;
/// Margin around the content when copying the canvas, in canvas pixels.
const EXPORT_PADDING: f32 = 10.0;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Deeraw")
            .with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Deeraw",
        options,
        Box::new(|_cc| Ok(Box::new(DeerawApp::default()))),
    )
}

#[derive(Clone, Copy, PartialEq, Default)]
enum Tool {
    #[default]
    Select,
    Pencil,
    Arrow,
    Text,
}

/// An object on the canvas. `rect` is in canvas pixels (physical pixels
/// relative to the canvas origin), so a fresh image paste shows 1:1.
struct Object {
    id: u64,
    rect: Rect,
    /// Width / height the object was created with; corner resizing restores it.
    aspect: f32,
    kind: Kind,
}

enum Kind {
    Image { texture: TextureHandle },
    /// Points are normalized to `rect` (0..1), so moving/resizing only touches `rect`.
    Stroke { points: Vec<Pos2> },
    /// Normalized to `rect`, like `Stroke`.
    Arrow { start: Pos2, end: Pos2 },
    /// `rect` follows the laid-out text; resizing scales `font_px`.
    Text { text: String, font_px: f32 },
}

impl Object {
    fn name(&self) -> String {
        let id = self.id;
        match &self.kind {
            Kind::Image { .. } => format!("Image #{id}"),
            Kind::Stroke { .. } => format!("Stroke #{id}"),
            Kind::Arrow { .. } => format!("Arrow #{id}"),
            Kind::Text { text, .. } => {
                let first: String = text.lines().next().unwrap_or("").chars().take(20).collect();
                format!("Text #{id} \"{first}\"")
            }
        }
    }
}

/// A resize handle being dragged.
struct ResizeDrag {
    /// Rect when the drag started; the new rect is always derived from it.
    start: Rect,
    /// Pointer minus handle position at drag start, so the handle stays under the cursor.
    grab_offset: Vec2,
}

/// A stroke or arrow being drawn, in canvas pixels.
enum Drawing {
    Stroke(Vec<Pos2>),
    Arrow(Pos2, Pos2),
}

#[derive(Default)]
struct DeerawApp {
    /// Bottom to top: later objects are drawn on top.
    objects: Vec<Object>,
    selected: Option<u64>,
    next_id: u64,
    tool: Tool,
    drawing: Option<Drawing>,
    resize_drag: Option<ResizeDrag>,
    /// Text object currently being edited.
    editing: Option<u64>,
    focus_editor: bool,
    /// While copying: the screen region (points) to crop from the screenshot.
    /// Selection and toolbar are hidden until the screenshot arrives.
    capture: Option<Rect>,
    screenshot: Arc<Mutex<Option<Arc<ColorImage>>>>,
}

impl eframe::App for DeerawApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("layers")
            .resizable(true)
            .default_size(220.0)
            .show(ui, |ui| self.layers_panel(ui));

        let frame = egui::Frame::new().fill(Color32::WHITE);
        egui::CentralPanel::default()
            .frame(frame)
            .show(ui, |ui| self.canvas(ui));
    }
}

impl DeerawApp {
    fn layers_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Layers");
        ui.separator();
        if self.objects.is_empty() {
            ui.weak("Paste an image with Ctrl+V,\nor draw with the tools below");
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            // Topmost first, like most layer panels.
            for obj in self.objects.iter().rev() {
                let selected = self.selected == Some(obj.id);
                if ui.selectable_label(selected, obj.name()).clicked() {
                    self.selected = Some(obj.id);
                }
                let r = obj.rect;
                let mut info = format!(
                    "pos {:.0}, {:.0}  size {:.0}×{:.0}",
                    r.min.x,
                    r.min.y,
                    r.width(),
                    r.height()
                );
                match &obj.kind {
                    Kind::Image { texture } => {
                        let [w, h] = texture.size();
                        info += &format!("\nsource {w}×{h}");
                    }
                    Kind::Stroke { points } => info += &format!("\n{} points", points.len()),
                    Kind::Text { font_px, .. } => info += &format!("\nfont {font_px:.0}px"),
                    Kind::Arrow { .. } => {}
                }
                ui.weak(info);
            }
        });
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let ppp = ui.ctx().pixels_per_point();
        let canvas_rect = ui.max_rect();
        let origin = canvas_rect.min;
        // Canvas pixels <-> screen points.
        let to_screen = |r: Rect| {
            Rect::from_min_max(origin + r.min.to_vec2() / ppp, origin + r.max.to_vec2() / ppp)
        };
        let to_canvas = |p: Pos2| ((p - origin) * ppp).to_pos2();

        self.finish_copy(ui.ctx(), ppp);

        // Canvas shortcuts don't apply while typing into a text object.
        if self.editing.is_none() {
            for event in ui.input(|i| i.events.clone()) {
                match event {
                    egui::Event::PasteImage(image) => {
                        let center = ui
                            .input(|i| i.pointer.hover_pos())
                            .filter(|p| canvas_rect.contains(*p))
                            .unwrap_or(canvas_rect.center());
                        self.add_image(ui.ctx(), image, to_canvas(center));
                    }
                    egui::Event::Copy => {
                        if let Some(bounds) = self.bounds() {
                            let region = to_screen(bounds.expand(EXPORT_PADDING));
                            self.start_copy(ui.ctx(), region.intersect(canvas_rect));
                        }
                    }
                    egui::Event::Key { key: egui::Key::Delete, pressed: true, .. } => {
                        if let Some(id) = self.selected.take() {
                            self.objects.retain(|obj| obj.id != id);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Widgets registered later win the hit test: background, then objects
        // bottom to top, then the selected object's handles.
        let background =
            ui.interact(canvas_rect, egui::Id::unique("canvas"), Sense::click_and_drag());
        self.handle_tool(&background, to_canvas);

        let painter = ui.painter().clone();
        let selecting = self.tool == Tool::Select;
        for obj in &mut self.objects {
            let editing = self.editing == Some(obj.id);
            if let Kind::Text { text, font_px } = &obj.kind {
                // Keep the rect in sync with the text layout.
                let galley = painter.layout_no_wrap(text.clone(), FontId::proportional(font_px / ppp), INK);
                obj.rect = Rect::from_min_size(obj.rect.min, (galley.size() * ppp).max(Vec2::splat(MIN_SIZE)));
            }
            let rect = to_screen(obj.rect);
            if selecting && !editing {
                let id = egui::Id::unique(("object", obj.id));
                let response = ui.interact(rect.expand(4.0), id, Sense::click_and_drag());
                if response.clicked() || response.drag_started() {
                    self.selected = Some(obj.id);
                }
                if response.double_clicked() && matches!(obj.kind, Kind::Text { .. }) {
                    self.editing = Some(obj.id);
                    self.focus_editor = true;
                }
                if response.dragged() {
                    obj.rect = obj.rect.translate(response.drag_delta() * ppp);
                }
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(CursorIcon::Move);
                }
            }
            if !editing {
                paint_object(&painter, obj, rect, ppp);
            }
        }

        match &self.drawing {
            Some(Drawing::Stroke(points)) => {
                let points = points.iter().map(|p| origin + p.to_vec2() / ppp).collect();
                painter.line(points, Stroke::new(LINE_WIDTH / ppp, INK));
            }
            Some(Drawing::Arrow(start, end)) => {
                let (start, end) = (origin + start.to_vec2() / ppp, origin + end.to_vec2() / ppp);
                paint_arrow(&painter, start, end, ppp);
            }
            None => {}
        }

        if selecting && self.capture.is_none() {
            self.selection_handles(ui, to_screen, to_canvas);
        }
        self.text_editor(ui, origin, ppp);
        if self.capture.is_none() {
            self.toolbar(ui, canvas_rect);
        }
    }

    /// What a click or drag on empty canvas does with the current tool.
    fn handle_tool(&mut self, background: &egui::Response, to_canvas: impl Fn(Pos2) -> Pos2) {
        let pointer = background.interact_pointer_pos().map(&to_canvas);
        match self.tool {
            Tool::Select => {
                if background.clicked() {
                    self.selected = None;
                }
            }
            Tool::Pencil | Tool::Arrow => {
                if background.hovered() {
                    background.ctx.set_cursor_icon(CursorIcon::Crosshair);
                }
                if let (true, Some(p)) = (background.drag_started(), pointer) {
                    self.drawing = Some(match self.tool {
                        Tool::Pencil => Drawing::Stroke(vec![p]),
                        _ => Drawing::Arrow(p, p),
                    });
                }
                if let (true, Some(p)) = (background.dragged(), pointer) {
                    match &mut self.drawing {
                        Some(Drawing::Stroke(points)) => {
                            if points.last().is_none_or(|last| last.distance(p) >= 1.0) {
                                points.push(p);
                            }
                        }
                        Some(Drawing::Arrow(_, end)) => *end = p,
                        None => {}
                    }
                }
                if background.drag_stopped() {
                    self.finish_drawing();
                }
            }
            Tool::Text => {
                if background.hovered() {
                    background.ctx.set_cursor_icon(CursorIcon::Text);
                }
                if let (true, Some(p)) = (background.clicked(), pointer) {
                    let font_px = TEXT_SIZE;
                    let rect = Rect::from_min_size(p - Vec2::new(0.0, font_px / 2.0), Vec2::splat(MIN_SIZE));
                    let id = self.add(rect, Kind::Text { text: String::new(), font_px });
                    self.editing = Some(id);
                    self.focus_editor = true;
                    self.tool = Tool::Select;
                }
            }
        }
    }

    fn finish_drawing(&mut self) {
        match self.drawing.take() {
            Some(Drawing::Stroke(points)) if points.len() >= 2 => {
                let (rect, points) = normalize(&points);
                self.add(rect, Kind::Stroke { points });
            }
            Some(Drawing::Arrow(start, end)) if start.distance(end) >= MIN_SIZE => {
                let (rect, points) = normalize(&[start, end]);
                self.add(rect, Kind::Arrow { start: points[0], end: points[1] });
                self.tool = Tool::Select;
            }
            _ => {}
        }
    }

    fn selection_handles(
        &mut self,
        ui: &mut egui::Ui,
        to_screen: impl Fn(Rect) -> Rect,
        to_canvas: impl Fn(Pos2) -> Pos2,
    ) {
        let Some(obj) = self.objects.iter_mut().find(|o| Some(o.id) == self.selected) else {
            return;
        };
        if self.editing == Some(obj.id) {
            return;
        }
        let rect = to_screen(obj.rect);
        let painter = ui.painter();
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, SELECTION_COLOR), StrokeKind::Outside);

        let is_text = matches!(obj.kind, Kind::Text { .. });
        let from_center = ui.input(|i| i.modifiers.shift);
        // (x, y) direction of each handle: -1 = min edge, 1 = max edge, 0 = that axis is fixed.
        for (dx, dy) in [
            (-1, -1), (1, -1), (-1, 1), (1, 1), // corners: both axes, keep aspect ratio
            (0, -1), (0, 1), (-1, 0), (1, 0),   // edges: one axis
        ] {
            // Text can't be stretched, only scaled.
            if is_text && (dx == 0 || dy == 0) {
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
            if let (true, Some(p)) = (response.drag_started(), pointer) {
                let handle_pos = obj.rect.lerp_inside(Vec2::new(dx as f32 + 1.0, dy as f32 + 1.0) / 2.0);
                self.resize_drag = Some(ResizeDrag { start: obj.rect, grab_offset: p - handle_pos });
            }
            if let (true, Some(p), Some(drag)) = (response.dragged(), pointer, &self.resize_drag) {
                // Text has no fixed aspect: it keeps whatever the layout gives it.
                let aspect = match obj.kind {
                    Kind::Text { .. } => drag.start.aspect_ratio(),
                    _ => obj.aspect,
                };
                let old_height = obj.rect.height();
                obj.rect = resize_to(drag.start, dx, dy, p - drag.grab_offset, aspect, from_center);
                if let Kind::Text { font_px, .. } = &mut obj.kind {
                    *font_px *= obj.rect.height() / old_height;
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

    /// Shows an inline text box over the text object being edited.
    fn text_editor(&mut self, ui: &mut egui::Ui, origin: Pos2, ppp: f32) {
        let Some(id) = self.editing else { return };
        let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) else {
            self.editing = None;
            return;
        };
        let Kind::Text { text, font_px } = &mut obj.kind else {
            self.editing = None;
            return;
        };
        let font = FontId::proportional(*font_px / ppp);
        let output = egui::Area::new(egui::Id::unique(("text-editor", id)))
            .fixed_pos(origin + obj.rect.min.to_vec2() / ppp)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::TextEdit::multiline(text)
                    .font(font)
                    .text_color(INK)
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::ZERO)
                    .desired_width(1.0)
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
                self.selected = Some(id);
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, canvas_rect: Rect) {
        egui::Area::new(egui::Id::unique("toolbar"))
            .pivot(Align2::CENTER_BOTTOM)
            .fixed_pos(canvas_rect.center_bottom() - Vec2::new(0.0, 16.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (tool, label) in [
                            (Tool::Select, "↖ Select"),
                            (Tool::Pencil, "✏ Pencil"),
                            (Tool::Arrow, "➡ Arrow"),
                            (Tool::Text, "T Text"),
                        ] {
                            if ui.selectable_label(self.tool == tool, label).clicked() {
                                self.tool = tool;
                            }
                        }
                    });
                });
            });
    }

    /// Adds an object on top and selects it.
    fn add(&mut self, rect: Rect, kind: Kind) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let aspect = rect.aspect_ratio();
        self.objects.push(Object { id, rect, aspect, kind });
        self.selected = Some(id);
        id
    }

    /// Adds an image centered on `center` (canvas pixels).
    fn add_image(&mut self, ctx: &egui::Context, image: Arc<ColorImage>, center: Pos2) {
        let texture = ctx.load_texture(
            format!("image-{}", self.next_id),
            (*image).clone(),
            egui::TextureOptions::LINEAR,
        );
        let size = Vec2::new(image.size[0] as f32, image.size[1] as f32);
        let min = (center - size / 2.0).round();
        self.add(Rect::from_min_size(min, size), Kind::Image { texture });
    }

    /// Bounding box of all objects, in canvas pixels.
    fn bounds(&self) -> Option<Rect> {
        let first = self.objects.first()?;
        Some(self.objects.iter().fold(first.rect, |acc, obj| acc.union(obj.rect)))
    }

    /// Copying works by screenshotting the canvas: this frame and the next are
    /// drawn without selection and toolbar, then `finish_copy` crops the result.
    fn start_copy(&mut self, ctx: &egui::Context, region: Rect) {
        self.capture = Some(region);
        let slot = self.screenshot.clone();
        let repaint = ctx.clone();
        ctx.request_screenshot(move |image| {
            *slot.lock().unwrap() = Some(image);
            repaint.request_repaint();
        });
    }

    fn finish_copy(&mut self, ctx: &egui::Context, ppp: f32) {
        let Some(screenshot) = self.screenshot.lock().unwrap().take() else { return };
        if let Some(region) = self.capture.take() {
            ctx.copy_image(screenshot.region(&region, Some(ppp)));
        }
    }
}

fn paint_object(painter: &Painter, obj: &Object, rect: Rect, ppp: f32) {
    let stroke = Stroke::new(LINE_WIDTH / ppp, INK);
    match &obj.kind {
        Kind::Image { texture } => {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            painter.image(texture.id(), rect, uv, Color32::WHITE);
        }
        Kind::Stroke { points } => {
            let points = points.iter().map(|p| rect.lerp_inside(p.to_vec2())).collect();
            painter.line(points, stroke);
        }
        Kind::Arrow { start, end } => {
            let start = rect.lerp_inside(start.to_vec2());
            let end = rect.lerp_inside(end.to_vec2());
            paint_arrow(painter, start, end, ppp);
        }
        Kind::Text { text, font_px } => {
            let galley = painter.layout_no_wrap(text.clone(), FontId::proportional(font_px / ppp), INK);
            painter.galley(rect.min, galley, INK);
        }
    }
}

/// Draws a line from `start` to `end` (screen points) with a head at `end`.
fn paint_arrow(painter: &Painter, start: Pos2, end: Pos2, ppp: f32) {
    let stroke = Stroke::new(LINE_WIDTH / ppp, INK);
    painter.line_segment([start, end], stroke);
    let along = end - start;
    let length = along.length();
    if length < 1.0 {
        return;
    }
    let back = -along / length * (ARROW_HEAD / ppp).min(length / 2.0);
    let spread = Rot2::from_angle(0.45);
    painter.line_segment([end, end + spread * back], stroke);
    painter.line_segment([end, end + spread.inverse() * back], stroke);
}

/// Returns the bounding box of `points` (at least `MIN_SIZE` on each axis)
/// and the points normalized to it.
fn normalize(points: &[Pos2]) -> (Rect, Vec<Pos2>) {
    let bounds = Rect::from_points(points);
    let rect = Rect::from_center_size(bounds.center(), bounds.size().max(Vec2::splat(MIN_SIZE)));
    let normalized = points
        .iter()
        .map(|p| ((*p - rect.min) / rect.size()).to_pos2())
        .collect();
    (rect, normalized)
}

/// Resizes `start` so the handle at direction (`dx`, `dy`) lands on `target` (canvas pixels).
/// -1 = min edge, 1 = max edge, 0 = that axis is untouched. Corner handles fit the size to
/// `aspect`, letting the axis the cursor pulled further win. With `from_center` the center
/// stays put; otherwise the opposite side does. Never shrinks below `MIN_SIZE`.
fn resize_to(start: Rect, dx: i32, dy: i32, target: Pos2, aspect: f32, from_center: bool) -> Rect {
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
    if dx != 0 && dy != 0 {
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

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
    }

    #[test]
    fn corner_follows_cursor_keeping_aspect() {
        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), 1, 1, Pos2::new(400.0, 150.0), 2.0, false);
        assert_eq!(r, rect(0.0, 0.0, 400.0, 200.0));

        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), -1, -1, Pos2::new(100.0, 0.0), 2.0, false);
        assert_eq!(r, rect(0.0, 0.0, 200.0, 100.0));

        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), -1, -1, Pos2::new(100.0, 80.0), 2.0, false);
        assert_eq!(r, rect(100.0, 50.0, 200.0, 100.0));
    }

    #[test]
    fn corner_restores_original_aspect() {
        // Stretched to 3:1 with an edge handle; the corner snaps back to 2:1.
        let r = resize_to(rect(0.0, 0.0, 300.0, 100.0), 1, 1, Pos2::new(300.0, 100.0), 2.0, false);
        assert_eq!(r, rect(0.0, 0.0, 300.0, 150.0));
    }

    #[test]
    fn corner_respects_min_size() {
        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), 1, -1, Pos2::new(-1000.0, 1000.0), 2.0, false);
        assert_eq!(r, rect(0.0, 96.0, 8.0, 100.0));
    }

    #[test]
    fn edge_follows_cursor_on_one_axis() {
        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), 1, 0, Pos2::new(250.0, 999.0), 2.0, false);
        assert_eq!(r, rect(0.0, 0.0, 250.0, 100.0));

        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), 0, -1, Pos2::new(999.0, -20.0), 2.0, false);
        assert_eq!(r, rect(0.0, -20.0, 200.0, 100.0));
    }

    #[test]
    fn from_center_keeps_center() {
        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), 1, 1, Pos2::new(300.0, 150.0), 2.0, true);
        assert_eq!(r, rect(-100.0, -50.0, 300.0, 150.0));

        let r = resize_to(rect(0.0, 0.0, 200.0, 100.0), 0, -1, Pos2::new(0.0, 10.0), 2.0, true);
        assert_eq!(r, rect(0.0, 10.0, 200.0, 90.0));
    }

    #[test]
    fn normalize_points() {
        let (r, points) = normalize(&[Pos2::new(10.0, 20.0), Pos2::new(110.0, 20.0)]);
        assert_eq!(r, rect(10.0, 18.0, 110.0, 22.0));
        assert_eq!(points, vec![Pos2::new(0.0, 0.5), Pos2::new(1.0, 0.5)]);
    }
}
