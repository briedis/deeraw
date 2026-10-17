use std::sync::{Arc, Mutex};

use eframe::egui::{
    self, Align2, Color32, ColorImage, CursorIcon, FontId, Painter, PointerButton, Pos2, Rect,
    Sense, Stroke, StrokeKind, TextureHandle, Vec2, emath::Rot2,
};

const SELECTION_COLOR: Color32 = Color32::from_rgb(105, 101, 219);
/// Size of a resize handle, in points.
const HANDLE_SIZE: f32 = 8.0;
/// Smallest width/height an object can have, in canvas pixels.
const MIN_SIZE: f32 = 4.0;
/// Colors offered by the style panel; the first is the default.
const PALETTE: [(Color32, &str); 6] = [
    (Color32::BLACK, "Black"),
    (Color32::WHITE, "White"),
    (Color32::from_rgb(224, 49, 49), "Red"),
    (Color32::from_rgb(240, 140, 0), "Orange"),
    (Color32::from_rgb(47, 158, 68), "Green"),
    (Color32::from_rgb(25, 113, 194), "Blue"),
];
/// Line widths (pencil strokes and arrows) offered by the style panel, in canvas pixels.
const LINE_WIDTHS: [f32; 4] = [2.0, 3.0, 6.0, 10.0];
/// Font sizes offered by the style panel, in canvas pixels.
const FONT_SIZES: [f32; 4] = [16.0, 28.0, 44.0, 64.0];
/// Length of an arrow's head lines, in canvas pixels: base plus this much per pixel of line width.
const ARROW_HEAD: f32 = 12.0;
const ARROW_HEAD_PER_WIDTH: f32 = 2.0;
/// How close (in points) the pointer must be to an arrow's line to grab it.
const ARROW_HIT_DISTANCE: f32 = 6.0;
/// Radius of an arrow's endpoint handles, in points.
const POINT_HANDLE_RADIUS: f32 = 5.0;
/// Zoom is 100% + 20% per level, from 20% to 500%.
const ZOOM_STEP: f32 = 0.2;
const MIN_ZOOM_LEVEL: i32 = -4;
const MAX_ZOOM_LEVEL: i32 = 20;
/// Mouse wheel distance (in points) that counts as one zoom step, for smooth-scrolling devices.
const WHEEL_POINTS_PER_STEP: f32 = 50.0;
/// How many undo steps are kept.
const MAX_UNDO: usize = 200;
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
        Box::new(|cc| {
            // Ctrl +/- zooms the canvas, not the whole UI.
            cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
            Ok(Box::new(DeerawApp::default()))
        }),
    )
}

#[derive(Clone, Copy, PartialEq, Default)]
enum Tool {
    #[default]
    Select,
    Pencil,
    Arrow,
    Line,
    Text,
}

/// An object on the canvas. `rect` is in canvas pixels (physical pixels
/// relative to the canvas origin), so a fresh image paste shows 1:1.
#[derive(Clone, PartialEq)]
struct Object {
    id: u64,
    rect: Rect,
    /// Width / height the object was created with; corner resizing restores it.
    aspect: f32,
    kind: Kind,
}

#[derive(Clone, PartialEq)]
enum Kind {
    Image { texture: TextureHandle },
    /// Points are normalized to `rect` (0..1), so moving/resizing only touches `rect`.
    Stroke { points: Vec<Pos2>, color: Color32, width: f32 },
    /// Endpoints in canvas pixels; `rect` is derived from them. Edited point by point.
    /// Without `head` it's a plain line (Line tool).
    Arrow { start: Pos2, end: Pos2, color: Color32, width: f32, head: bool },
    /// `rect` follows the laid-out text; resizing scales `font_px`.
    Text { text: String, font_px: f32, color: Color32 },
}

/// Which part of the style panel is expanded.
#[derive(Clone, Copy, PartialEq)]
enum StyleSection {
    Color,
    Size(SizeKind),
}

/// Which size the style panel edits.
#[derive(Clone, Copy, PartialEq)]
enum SizeKind {
    /// Pencil strokes and arrows.
    Line,
    Font,
}

/// Color and sizes for new objects, set from the style panel.
struct Style {
    color: Color32,
    line_width: f32,
    font_px: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self { color: PALETTE[0].0, line_width: LINE_WIDTHS[1], font_px: FONT_SIZES[1] }
    }
}

impl Style {
    fn size_mut(&mut self, kind: SizeKind) -> &mut f32 {
        match kind {
            SizeKind::Line => &mut self.line_width,
            SizeKind::Font => &mut self.font_px,
        }
    }
}

impl Object {
    fn translate(&mut self, delta: Vec2) {
        self.rect = self.rect.translate(delta);
        if let Kind::Arrow { start, end, .. } = &mut self.kind {
            *start += delta;
            *end += delta;
        }
    }

    /// `None` for images, which have no color.
    fn color_mut(&mut self) -> Option<&mut Color32> {
        match &mut self.kind {
            Kind::Image { .. } => None,
            Kind::Stroke { color, .. } | Kind::Arrow { color, .. } | Kind::Text { color, .. } => Some(color),
        }
    }

    /// Line width or font size, whichever the object has; `None` for images.
    fn size_mut(&mut self) -> Option<(SizeKind, &mut f32)> {
        match &mut self.kind {
            Kind::Image { .. } => None,
            Kind::Stroke { width, .. } | Kind::Arrow { width, .. } => Some((SizeKind::Line, width)),
            Kind::Text { font_px, .. } => Some((SizeKind::Font, font_px)),
        }
    }

    fn name(&self) -> String {
        let id = self.id;
        match &self.kind {
            Kind::Image { .. } => format!("Image #{id}"),
            Kind::Stroke { .. } => format!("Stroke #{id}"),
            Kind::Arrow { head: true, .. } => format!("Arrow #{id}"),
            Kind::Arrow { head: false, .. } => format!("Line #{id}"),
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

/// Undo/redo by snapshots of the object list. Whenever the canvas settles (no mouse button
/// held, nothing being drawn or typed) and differs from `saved`, `saved` goes on the undo
/// stack, so every action becomes one step without per-action bookkeeping.
#[derive(Default)]
struct History {
    undo: Vec<Vec<Object>>,
    redo: Vec<Vec<Object>>,
    /// The state as of the last recorded step.
    saved: Vec<Object>,
}

impl History {
    fn record(&mut self, objects: &[Object]) {
        if self.saved != objects {
            self.undo.push(std::mem::replace(&mut self.saved, objects.to_vec()));
            if self.undo.len() > MAX_UNDO {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
    }

    /// Steps `objects` back (from `from` to `to`), saving the current state on the other stack.
    fn step(from: &mut Vec<Vec<Object>>, to: &mut Vec<Vec<Object>>, saved: &mut Vec<Object>, objects: &mut Vec<Object>) {
        if let Some(state) = from.pop() {
            to.push(std::mem::replace(objects, state));
            *saved = objects.clone();
        }
    }

    fn undo(&mut self, objects: &mut Vec<Object>) {
        Self::step(&mut self.undo, &mut self.redo, &mut self.saved, objects);
    }

    fn redo(&mut self, objects: &mut Vec<Object>) {
        Self::step(&mut self.redo, &mut self.undo, &mut self.saved, objects);
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Background {
    Transparent,
    #[default]
    White,
    Black,
}

impl Background {
    /// `None` for transparent.
    fn color(self) -> Option<Color32> {
        match self {
            Background::Transparent => None,
            Background::White => Some(Color32::WHITE),
            Background::Black => Some(Color32::BLACK),
        }
    }
}

#[derive(Default)]
struct DeerawApp {
    /// Bottom to top: later objects are drawn on top.
    objects: Vec<Object>,
    history: History,
    /// Selected object ids, in the order they were selected.
    selected: Vec<u64>,
    /// Selection box being dragged with the Select tool: start and current corner, canvas pixels.
    marquee: Option<(Pos2, Pos2)>,
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
    /// Built once: the OS caches custom cursors by buffer identity.
    crosshair: Option<egui::CustomCursorImage>,
    i_beam: Option<egui::CustomCursorImage>,
    /// See `zoom()`.
    zoom_level: i32,
    /// Screen offset (points) of the canvas origin from the canvas panel's top-left.
    pan: Vec2,
    panning: bool,
    /// Wheel movement not yet turned into a whole zoom step.
    wheel: f32,
    background: Background,
    style: Style,
    style_open: Option<StyleSection>,
}

impl eframe::App for DeerawApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("layers")
            .resizable(true)
            .default_size(220.0)
            .show(ui, |ui| self.layers_panel(ui));

        // Transparent shows a checkerboard on screen (painted in `canvas`), but nothing at all
        // while copying, so the screenshot keeps the alpha.
        let fill = self.background.color().unwrap_or(Color32::TRANSPARENT);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(fill))
            .show(ui, |ui| self.canvas(ui));
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // The screenshot has the window's alpha, so a transparent copy needs a transparent clear.
        if self.capture.is_some() && self.background == Background::Transparent {
            [0.0; 4]
        } else {
            egui::Visuals::light().panel_fill.to_normalized_gamma_f32()
        }
    }
}

impl DeerawApp {
    fn layers_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Layers");
        ui.separator();
        if self.objects.is_empty() {
            ui.weak("Paste an image with Ctrl+V,\nor draw with the tools below");
        }
        let mut clicked = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            // Topmost first, like most layer panels.
            for obj in self.objects.iter().rev() {
                let selected = self.selected.contains(&obj.id);
                let response = ui.selectable_label(selected, obj.name());
                if response.clicked() {
                    clicked = Some(obj.id);
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
                    Kind::Stroke { points, width, .. } => {
                        info += &format!("\n{} points, width {width:.0}px", points.len())
                    }
                    Kind::Text { font_px, .. } => info += &format!("\nfont {font_px:.0}px"),
                    Kind::Arrow { width, .. } => info += &format!("\nwidth {width:.0}px"),
                }
                ui.weak(info);
            }
        });
        if let Some(id) = clicked {
            let ctrl = ui.input(|i| i.modifiers.command);
            self.click_select(id, ctrl);
        }
    }

    fn zoom(&self) -> f32 {
        1.0 + ZOOM_STEP * self.zoom_level as f32
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let ppp = ui.ctx().pixels_per_point();
        let canvas_rect = ui.max_rect();
        self.navigate(ui, canvas_rect, ppp);

        // Canvas pixels <-> screen points. `scale` is screen points per canvas pixel.
        let origin = canvas_rect.min + self.pan;
        let scale = self.zoom() / ppp;
        let to_screen_pos = |p: Pos2| origin + p.to_vec2() * scale;
        let to_screen = |r: Rect| Rect::from_min_max(to_screen_pos(r.min), to_screen_pos(r.max));
        let to_canvas = |p: Pos2| ((p - origin) / scale).to_pos2();

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
                        let selected = std::mem::take(&mut self.selected);
                        self.objects.retain(|obj| !selected.contains(&obj.id));
                    }
                    egui::Event::Key { key, pressed: true, modifiers, .. } if modifiers.command => {
                        match key {
                            egui::Key::Z if modifiers.shift => self.history.redo(&mut self.objects),
                            egui::Key::Z => self.history.undo(&mut self.objects),
                            egui::Key::Y => self.history.redo(&mut self.objects),
                            _ => {}
                        }
                        let objects = &self.objects;
                        self.selected.retain(|id| objects.iter().any(|o| o.id == *id));
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
        if self.background == Background::Transparent && self.capture.is_none() {
            paint_checkerboard(&painter, canvas_rect, 8.0, 242);
        }
        let selecting = self.tool == Tool::Select;
        let ctrl = ui.input(|i| i.modifiers.command);
        // Dragging any selected object moves all of them; applied after the loop.
        let mut move_by = Vec2::ZERO;
        let mut clicked = None;
        for obj in &mut self.objects {
            let editing = self.editing == Some(obj.id);
            if let Kind::Text { text, font_px, color } = &obj.kind {
                // Keep the rect in sync with the text layout. Measured at a fixed scale (1 point
                // per canvas pixel) so zooming doesn't change it.
                let galley = painter.layout_no_wrap(text.clone(), FontId::proportional(*font_px), *color);
                obj.rect = Rect::from_min_size(obj.rect.min, galley.size().max(Vec2::splat(MIN_SIZE)));
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
                if response.double_clicked() && matches!(obj.kind, Kind::Text { .. }) {
                    self.editing = Some(obj.id);
                    self.focus_editor = true;
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
            for obj in self.objects.iter_mut().filter(|o| self.selected.contains(&o.id)) {
                obj.translate(move_by);
            }
        }
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
            Some(Drawing::Stroke(points)) => {
                let points = points.iter().map(|p| to_screen_pos(*p)).collect();
                painter.line(points, Stroke::new(self.style.line_width * scale, self.style.color));
            }
            Some(Drawing::Arrow(start, end)) => {
                let stroke = Stroke::new(self.style.line_width * scale, self.style.color);
                let head = self.tool == Tool::Arrow;
                paint_arrow(&painter, to_screen_pos(*start), to_screen_pos(*end), stroke, scale, head);
            }
            None => {}
        }

        if selecting && self.capture.is_none() {
            self.selection_handles(ui, to_screen, to_screen_pos, to_canvas, scale);
        }
        self.text_editor(ui, to_screen_pos, scale);
        if self.capture.is_none() {
            let toolbar = self.toolbar(ui, canvas_rect);
            self.style_panel(ui, toolbar);
            self.corner_controls(ui, canvas_rect, ppp);
        }
        // Set last so it wins over hover cursors of whatever is under the pointer.
        if self.panning {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }

        let settled = !ui.input(|i| i.pointer.any_down())
            && self.editing.is_none()
            && self.drawing.is_none();
        if settled {
            self.history.record(&self.objects);
        }
    }

    /// Middle-drag pans; the mouse wheel (around the cursor) and Ctrl +/- (around the canvas
    /// center) zoom in 20% steps; Ctrl+0 resets to 100%.
    fn navigate(&mut self, ui: &egui::Ui, canvas_rect: Rect, ppp: f32) {
        let (hover, delta, middle_down, middle_pressed) = ui.input(|i| {
            (
                i.pointer.hover_pos(),
                i.pointer.delta(),
                i.pointer.middle_down(),
                i.pointer.button_pressed(PointerButton::Middle),
            )
        });
        let over_canvas = hover.filter(|p| canvas_rect.contains(*p));

        if middle_pressed && over_canvas.is_some() {
            self.panning = true;
        }
        if !middle_down {
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

    /// Bottom-right of the canvas: background picker and zoom level.
    fn corner_controls(&mut self, ui: &egui::Ui, canvas_rect: Rect, ppp: f32) {
        egui::Area::new(egui::Id::unique("corner"))
            .pivot(Align2::RIGHT_BOTTOM)
            .fixed_pos(canvas_rect.right_bottom() - Vec2::splat(16.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| self.background_picker(ui));
                    egui::Frame::popup(ui.style()).show(ui, |ui| self.zoom_indicator(ui, canvas_rect, ppp));
                });
            });
    }

    fn background_picker(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (background, name) in [
                (Background::Transparent, "Transparent background"),
                (Background::White, "White background"),
                (Background::Black, "Black background"),
            ] {
                let clicked = color_swatch(ui, self.background == background, name, background.color());
                if clicked {
                    self.background = background;
                }
            }
        });
    }

    /// Just above the toolbar: the current color and line width / font size. Clicking either
    /// expands its options upwards. With a drawing tool it sets the style of new objects; with
    /// the Select tool it restyles the selected objects, each setting only those that have it
    /// (a mix of strokes and text shows both a line width and a font size).
    fn style_panel(&mut self, ui: &egui::Ui, toolbar: Rect) {
        let targets: Vec<usize> = match self.tool {
            Tool::Select => (0..self.objects.len()).filter(|&i| self.selected.contains(&self.objects[i].id)).collect(),
            _ => vec![],
        };
        let (show_color, size_kinds) = match self.tool {
            Tool::Pencil | Tool::Arrow | Tool::Line => (true, vec![SizeKind::Line]),
            Tool::Text => (true, vec![SizeKind::Font]),
            Tool::Select => {
                let kinds: Vec<SizeKind> = targets.iter().filter_map(|&i| self.objects[i].size_mut().map(|(k, _)| k)).collect();
                let size_kinds = [SizeKind::Line, SizeKind::Font].into_iter().filter(|k| kinds.contains(k)).collect();
                (!kinds.is_empty(), size_kinds)
            }
        };
        if !show_color {
            self.style_open = None;
            return;
        }
        // Current values: the first selected object that has the setting, else the defaults.
        let color = targets
            .iter()
            .find_map(|&i| self.objects[i].color_mut().map(|c| *c))
            .unwrap_or(self.style.color);
        let sizes: Vec<(SizeKind, f32)> = size_kinds
            .iter()
            .map(|&kind| {
                let size = targets.iter().find_map(|&i| match self.objects[i].size_mut() {
                    Some((k, size)) if k == kind => Some(*size),
                    _ => None,
                });
                (kind, size.unwrap_or(*self.style.size_mut(kind)))
            })
            .collect();
        if let Some(StyleSection::Size(kind)) = self.style_open
            && !size_kinds.contains(&kind)
        {
            self.style_open = None;
        }

        let mut new_color = None;
        let mut new_size = None;
        egui::Area::new(egui::Id::unique("style"))
            .pivot(Align2::CENTER_BOTTOM)
            .fixed_pos(toolbar.center_top() - Vec2::new(0.0, 6.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    if let Some(section) = self.style_open {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| match section {
                                StyleSection::Color => {
                                    for (swatch, name) in PALETTE {
                                        if color_swatch(ui, color == swatch, name, Some(swatch)) {
                                            new_color = Some(swatch);
                                        }
                                    }
                                }
                                StyleSection::Size(kind) => {
                                    let current = sizes.iter().find(|(k, _)| *k == kind).map(|(_, s)| *s);
                                    let options = match kind {
                                        SizeKind::Line => LINE_WIDTHS,
                                        SizeKind::Font => FONT_SIZES,
                                    };
                                    for option in options {
                                        if size_button(ui, current == Some(option), kind, option) {
                                            new_size = Some((kind, option));
                                        }
                                    }
                                }
                            });
                        });
                    }
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let name = PALETTE.iter().find(|(c, _)| *c == color).map_or("Color", |(_, n)| n);
                            self.style_toggle(StyleSection::Color, |open| color_swatch(ui, open, name, Some(color)));
                            for &(kind, size) in &sizes {
                                self.style_toggle(StyleSection::Size(kind), |open| size_button(ui, open, kind, size));
                            }
                        });
                    });
                });
            });

        if new_color.is_none() && new_size.is_none() {
            return;
        }
        self.style_open = None;
        if let Some(color) = new_color {
            self.style.color = color;
            for &i in &targets {
                if let Some(c) = self.objects[i].color_mut() {
                    *c = color;
                }
            }
        }
        if let Some((kind, size)) = new_size {
            *self.style.size_mut(kind) = size;
            for &i in &targets {
                if let Some((k, s)) = self.objects[i].size_mut()
                    && k == kind
                {
                    *s = size;
                }
            }
        }
    }

    /// A style panel button that opens `section` (or closes it if open).
    fn style_toggle(&mut self, section: StyleSection, button: impl FnOnce(bool) -> bool) {
        let open = self.style_open == Some(section);
        if button(open) {
            self.style_open = (!open).then_some(section);
        }
    }

    /// A click on an object (canvas or layers panel): selects only it, or with Ctrl toggles it.
    fn click_select(&mut self, id: u64, ctrl: bool) {
        if !ctrl {
            self.selected = vec![id];
        } else if let Some(i) = self.selected.iter().position(|s| *s == id) {
            self.selected.remove(i);
        } else {
            self.selected.push(id);
        }
    }

    /// Shows the zoom level; clicking it resets to 100% around the canvas center.
    fn zoom_indicator(&mut self, ui: &mut egui::Ui, canvas_rect: Rect, ppp: f32) {
        let label = format!("{:.0}%", self.zoom() * 100.0);
        let clicked = ui
            .add(egui::Label::new(label).sense(Sense::click()))
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("Reset to 100%")
            .clicked();
        if clicked && self.zoom_level != 0 {
            let old_scale = self.zoom() / ppp;
            self.zoom_level = 0;
            let center = canvas_rect.center() - canvas_rect.min;
            self.pan = pan_for_zoom(self.pan, center, old_scale, self.zoom() / ppp);
        }
    }

    /// What a click or drag on empty canvas does with the current tool.
    fn handle_tool(&mut self, background: &egui::Response, to_canvas: impl Fn(Pos2) -> Pos2) {
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
            Tool::Pencil | Tool::Arrow | Tool::Line => {
                if let (true, Some(p)) = (background.drag_started_by(PointerButton::Primary), pointer) {
                    self.drawing = Some(match self.tool {
                        Tool::Pencil => Drawing::Stroke(vec![p]),
                        _ => Drawing::Arrow(p, p),
                    });
                }
                if let (true, Some(p)) = (background.dragged_by(PointerButton::Primary), pointer) {
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
                if let (true, Some(p)) = (background.clicked(), pointer) {
                    let Style { font_px, color, .. } = self.style;
                    let rect = Rect::from_min_size(p - Vec2::new(0.0, font_px / 2.0), Vec2::splat(MIN_SIZE));
                    let id = self.add(rect, Kind::Text { text: String::new(), font_px, color });
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
                let Style { color, line_width: width, .. } = self.style;
                self.add(rect, Kind::Stroke { points, color, width });
            }
            Some(Drawing::Arrow(start, end)) if start.distance(end) >= MIN_SIZE => {
                let Style { color, line_width: width, .. } = self.style;
                let head = self.tool == Tool::Arrow;
                self.add(Rect::from_two_pos(start, end), Kind::Arrow { start, end, color, width, head });
            }
            _ => {}
        }
    }

    fn selection_handles(
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
        let Some(obj) = self.objects.iter_mut().find(|o| self.selected.contains(&o.id)) else {
            return;
        };
        if self.editing == Some(obj.id) {
            return;
        }
        if let Kind::Arrow { start, end, .. } = &mut obj.kind {
            // Arrows get a handle per endpoint instead of a resize box.
            for (i, point) in [start, end].into_iter().enumerate() {
                let center = to_screen_pos(*point);
                let id = egui::Id::unique(("point-handle", obj.id, i));
                let hit = Rect::from_center_size(center, Vec2::splat(POINT_HANDLE_RADIUS * 2.0 + 4.0));
                let response = ui.interact(hit, id, Sense::drag());
                if response.dragged_by(PointerButton::Primary) {
                    *point += response.drag_delta() / scale;
                }
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(CursorIcon::Grab);
                }
                let painter = ui.painter();
                painter.circle_filled(center, POINT_HANDLE_RADIUS, Color32::WHITE);
                painter.circle_stroke(center, POINT_HANDLE_RADIUS, Stroke::new(1.5, SELECTION_COLOR));
            }
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
            if let (true, Some(p)) = (response.drag_started_by(PointerButton::Primary), pointer) {
                let handle_pos = obj.rect.lerp_inside(Vec2::new(dx as f32 + 1.0, dy as f32 + 1.0) / 2.0);
                self.resize_drag = Some(ResizeDrag { start: obj.rect, grab_offset: p - handle_pos });
            }
            if let (true, Some(p), Some(drag)) =
                (response.dragged_by(PointerButton::Primary), pointer, &self.resize_drag)
            {
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
    fn text_editor(&mut self, ui: &mut egui::Ui, to_screen_pos: impl Fn(Pos2) -> Pos2, scale: f32) {
        let Some(id) = self.editing else { return };
        let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) else {
            self.editing = None;
            return;
        };
        let Kind::Text { text, font_px, color } = &mut obj.kind else {
            self.editing = None;
            return;
        };
        let font = FontId::proportional(*font_px * scale);
        let output = egui::Area::new(egui::Id::unique(("text-editor", id)))
            .fixed_pos(to_screen_pos(obj.rect.min))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                // The dark theme's caret is light blue, invisible on a white canvas; this color
                // shows on every background.
                ui.visuals_mut().text_cursor.stroke = Stroke::new(2.0, SELECTION_COLOR);
                egui::TextEdit::multiline(text)
                    .font(font)
                    .text_color(*color)
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
                self.selected = vec![id];
            }
        }
    }

    /// Returns the toolbar's screen rect.
    fn toolbar(&mut self, ui: &mut egui::Ui, canvas_rect: Rect) -> Rect {
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
                            (Tool::Line, "— Line"),
                            (Tool::Text, "T Text"),
                        ] {
                            if ui.selectable_label(self.tool == tool, label).clicked() {
                                self.tool = tool;
                            }
                        }
                    });
                });
            })
            .response
            .rect
    }

    /// Adds an object on top and selects it.
    fn add(&mut self, rect: Rect, kind: Kind) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let aspect = rect.aspect_ratio();
        self.objects.push(Object { id, rect, aspect, kind });
        self.selected = vec![id];
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

/// Screen-aligned checkerboard standing in for a transparent background.
fn paint_checkerboard(painter: &Painter, rect: Rect, cell: f32, gray: u8) {
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

/// A selectable color button; `None` shows a checkerboard (transparent). Returns whether it was clicked.
fn color_swatch(ui: &mut egui::Ui, selected: bool, name: &str, color: Option<Color32>) -> bool {
    let button = egui::Button::selectable(selected, "").min_size(Vec2::splat(22.0));
    let response = ui.add(button).on_hover_text(name);
    let swatch = Rect::from_center_size(response.rect.center(), Vec2::splat(14.0));
    match color {
        Some(color) => _ = ui.painter().rect_filled(swatch, 2.0, color),
        None => paint_checkerboard(&ui.painter().with_clip_rect(swatch), swatch, 3.5, 150),
    }
    ui.painter().rect_stroke(swatch, 2.0, Stroke::new(1.0, Color32::GRAY), StrokeKind::Outside);
    response.clicked()
}

/// A selectable size button: a line sample for line widths, S/M/L/XL for font sizes.
/// Returns whether it was clicked.
fn size_button(ui: &mut egui::Ui, selected: bool, kind: SizeKind, size: f32) -> bool {
    let hover = format!("{size:.0}px");
    let response = match kind {
        SizeKind::Line => {
            let button = egui::Button::selectable(selected, "").min_size(Vec2::new(30.0, 22.0));
            let response = ui.add(button);
            let r = response.rect.shrink2(Vec2::new(7.0, 0.0));
            let stroke = Stroke::new(size.min(r.height() - 6.0), ui.visuals().text_color());
            ui.painter().line_segment([r.left_center(), r.right_center()], stroke);
            response
        }
        SizeKind::Font => {
            // Resized text can have any size; show it in pixels then.
            let label = match FONT_SIZES.iter().position(|&s| s == size) {
                Some(i) => ["S", "M", "L", "XL"][i].to_owned(),
                None => hover.clone(),
            };
            let button = egui::Button::selectable(selected, label).min_size(Vec2::new(30.0, 22.0));
            ui.add(button)
        }
    };
    response.on_hover_text(hover).clicked()
}

/// A crosshair cursor for the drawing tools. (The stock Windows crosshair can be invisible over
/// the canvas.)
fn crosshair_cursor() -> egui::CustomCursorImage {
    const SIZE: usize = 23;
    const MID: usize = SIZE / 2;
    outlined_cursor(SIZE, [MID, MID], |x, y| x == MID || y == MID)
}

/// A text-insertion (I-beam) cursor for the Text tool; the stock one is invisible on white.
fn i_beam_cursor() -> egui::CustomCursorImage {
    const WIDTH: usize = 9;
    const HEIGHT: usize = 21;
    const MID: usize = WIDTH / 2;
    outlined_cursor(HEIGHT, [MID, HEIGHT / 2], |x, y| {
        let serif = (y == 1 || y == HEIGHT - 2) && (MID - 2..=MID + 2).contains(&x);
        let stem = x == MID && (1..HEIGHT - 1).contains(&y);
        serif || stem
    })
}

/// A `size`×`size` cursor: black where `ink(x, y)`, with a 1px white outline around it so it
/// shows on any background.
fn outlined_cursor(size: usize, hotspot: [usize; 2], ink: impl Fn(usize, usize) -> bool) -> egui::CustomCursorImage {
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let near_ink = (y.saturating_sub(1)..=(y + 1).min(size - 1))
                .any(|ny| (x.saturating_sub(1)..=(x + 1).min(size - 1)).any(|nx| ink(nx, ny)));
            let value = match (ink(x, y), near_ink) {
                (true, _) => [0, 0, 0, 255],
                (false, true) => [255, 255, 255, 255],
                (false, false) => continue,
            };
            rgba[(y * size + x) * 4..][..4].copy_from_slice(&value);
        }
    }
    egui::CustomCursorImage {
        rgba: rgba.into(),
        size: [size as u16; 2],
        hotspot: hotspot.map(|v| v as u16),
    }
}

fn paint_object(painter: &Painter, obj: &Object, rect: Rect, to_screen_pos: impl Fn(Pos2) -> Pos2, scale: f32) {
    match &obj.kind {
        Kind::Image { texture } => {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            painter.image(texture.id(), rect, uv, Color32::WHITE);
        }
        Kind::Stroke { points, color, width } => {
            let points = points.iter().map(|p| rect.lerp_inside(p.to_vec2())).collect();
            painter.line(points, Stroke::new(width * scale, *color));
        }
        Kind::Arrow { start, end, color, width, head } => {
            let stroke = Stroke::new(width * scale, *color);
            paint_arrow(painter, to_screen_pos(*start), to_screen_pos(*end), stroke, scale, *head);
        }
        Kind::Text { text, font_px, color } => {
            let galley = painter.layout_no_wrap(text.clone(), FontId::proportional(font_px * scale), *color);
            painter.galley(rect.min, galley, *color);
        }
    }
}

/// Draws a line from `start` to `end` (screen points), with a head at `end` if `head`; the head
/// grows with the stroke width. `scale` is screen points per canvas pixel.
fn paint_arrow(painter: &Painter, start: Pos2, end: Pos2, stroke: Stroke, scale: f32, head: bool) {
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

/// New pan after changing the scale, so the canvas point at `anchor` (relative to the canvas
/// panel, in points) stays under it.
fn pan_for_zoom(pan: Vec2, anchor: Vec2, old_scale: f32, new_scale: f32) -> Vec2 {
    let canvas_point = (anchor - pan) / old_scale;
    anchor - canvas_point * new_scale
}

/// Distance from `p` to the line segment `a`-`b`.
fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(f32::EPSILON)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
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
    fn zoom_keeps_anchor_fixed() {
        let (pan, anchor) = (Vec2::new(30.0, -10.0), Vec2::new(200.0, 150.0));
        let new_pan = pan_for_zoom(pan, anchor, 1.0, 1.5);
        let canvas_point = (anchor - pan) / 1.0;
        assert_eq!(new_pan + canvas_point * 1.5, anchor);
        assert_eq!(pan_for_zoom(pan, anchor, 1.0, 1.0), pan);
    }

    fn arrow(id: u64) -> Object {
        let (start, end) = (Pos2::ZERO, Pos2::new(10.0, 0.0));
        Object { id, rect: Rect::from_two_pos(start, end), aspect: 1.0, kind: Kind::Arrow { start, end, color: Color32::BLACK, width: 3.0, head: true } }
    }

    #[test]
    fn click_select_toggles_with_ctrl() {
        let mut app = DeerawApp::default();
        app.click_select(1, false);
        app.click_select(2, true);
        app.click_select(3, true);
        assert_eq!(app.selected, [1, 2, 3]);
        app.click_select(2, true);
        assert_eq!(app.selected, [1, 3]);
        app.click_select(3, false);
        assert_eq!(app.selected, [3]);
    }

    #[test]
    fn undo_redo() {
        let mut history = History::default();
        let mut objects = vec![];
        history.record(&objects);
        objects.push(arrow(1));
        history.record(&objects);
        objects.push(arrow(2));
        history.record(&objects);
        history.record(&objects); // unchanged: no new step

        let ids = |objects: &[Object]| objects.iter().map(|o| o.id).collect::<Vec<_>>();
        history.undo(&mut objects);
        assert_eq!(ids(&objects), [1]);
        history.undo(&mut objects);
        assert_eq!(ids(&objects), []);
        history.undo(&mut objects); // nothing left
        assert_eq!(ids(&objects), []);
        history.redo(&mut objects);
        assert_eq!(ids(&objects), [1]);

        // A new change after undo drops the redo stack.
        objects.push(arrow(3));
        history.record(&objects);
        history.redo(&mut objects);
        assert_eq!(ids(&objects), [1, 3]);
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
