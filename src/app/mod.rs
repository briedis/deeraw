//! The app: its state, the frame layout, and operations on the objects.

mod blur;
mod canvas;
mod files;
mod menu;
mod panels;
#[cfg(any(target_os = "macos", windows))]
mod screenshot;
mod selection;
mod snap;

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Vec2};

use crate::export::Export;
use crate::history::History;
use crate::object::{Binding, Kind, Object};
use crate::style::{Background, Style, StyleSection};

const SELECTION_COLOR: Color32 = Color32::from_rgb(105, 101, 219);
/// Narrowest text box, in canvas pixels; narrower drags place single-line text instead.
const MIN_TEXT_BOX: f32 = 20.0;
/// Zoom is 100% + 20% per level, from 20% to 500%.
const ZOOM_STEP: f32 = 0.2;
const MIN_ZOOM_LEVEL: i32 = -4;
const MAX_ZOOM_LEVEL: i32 = 20;
/// How far Ctrl+D copies are moved from the original, in canvas pixels.
const DUPLICATE_OFFSET: f32 = 20.0;
/// How many frames `--screenshot` waits for the window to become key.
#[cfg(any(target_os = "macos", windows))]
const SCREENSHOT_START_FRAMES: u64 = 300;
/// Key of `Settings` in eframe's storage.
const SETTINGS_KEY: &str = "settings";

/// What's remembered between launches, in eframe's storage (`app.ron` in the app's data
/// folder, e.g. `%APPDATA%\deeraw\data`; localStorage on the web). Saved on exit and every
/// 30 seconds. Unreadable settings are ignored, missing fields get their defaults. The drawing
/// isn't kept, and the app always starts with Select.
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Settings {
    style: Style,
    background: Background,
    save_dir: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
enum Tool {
    #[default]
    Select,
    Pencil,
    Arrow,
    Line,
    Shape,
    Blur,
    Text,
}

/// A resize handle being dragged.
struct ResizeDrag {
    /// Rect when the drag started; the new rect is always derived from it.
    start: Rect,
    /// Pointer minus handle position at drag start, so the handle stays under the cursor.
    grab_offset: Vec2,
    /// Font size at drag start (plain text). The new size is derived from it, not from the
    /// previous frame, because the text layout re-sizes the rect every frame.
    start_font_px: f32,
    /// Per axis, whether the center was lined up with another object at drag start: it then
    /// resizes from the center on that axis, so it stays lined up.
    centered: [bool; 2],
}

/// A short message shown like a tooltip, e.g. after copying; see `notice`.
struct Notice {
    text: String,
    /// Where the pointer was (screen), if anywhere.
    pos: Option<Pos2>,
    shown_at: f64,
}

/// A stroke, arrow, shape or text box being drawn, in canvas pixels.
enum Drawing {
    /// Points and when each was added (seconds), for the speed-based pressure.
    Stroke(Vec<Pos2>, Vec<f64>),
    /// Start, end, and what each is attached to.
    Arrow(Pos2, Pos2, [Option<Binding>; 2]),
    TextBox(Pos2, Pos2),
    /// Two opposite corners.
    Shape(Pos2, Pos2),
    /// The image (the first one the box touched, once it has), and two opposite corners.
    Blur(Option<u64>, Pos2, Pos2),
}

#[derive(Default)]
pub struct DeerawApp {
    /// Bottom to top: later objects are drawn on top.
    objects: Vec<Object>,
    history: History,
    /// Selected object ids, in the order they were selected.
    selected: Vec<u64>,
    /// Selection box being dragged with the Select tool: start and current corner, canvas pixels.
    marquee: Option<(Pos2, Pos2)>,
    next_id: u64,
    tool: Tool,
    /// The Blur tool was switched to by picking a blur with Select: clicking anything but a blur
    /// goes back to Select, until a new blur is started.
    blur_from_select: bool,
    drawing: Option<Drawing>,
    resize_drag: Option<ResizeDrag>,
    /// Where the arrow endpoint being dragged would be without snapping, canvas pixels.
    point_drag: Option<Pos2>,
    /// While moving the selection: how far it was dragged and how far it was actually moved
    /// (with alignment snapping) since the drag started, canvas pixels.
    move_drag: Option<(Vec2, Vec2)>,
    /// The image pasted or opened onto an empty canvas, until it's moved: it resizes from its
    /// center, so it stays centered where the window was fitted to it.
    first_image: Option<u64>,
    /// Alignment guides (canvas pixels) of the move or resize in progress; see `align_targets`.
    guides: Vec<[Pos2; 2]>,
    /// Image currently being cropped; see `crop_ui`.
    cropping: Option<u64>,
    /// Text object currently being edited.
    editing: Option<u64>,
    focus_editor: bool,
    /// The layers panel, toggled with Ctrl+L.
    show_layers: bool,
    /// Last frame's canvas panel, to keep the drawing still when the panel moves and to scale
    /// it along when the window is resized.
    canvas_rect: Option<Rect>,
    /// Last frame's window size (points), to tell window resizes from layers panel changes.
    window_size: Option<Vec2>,
    /// Canvas panel size (points) at which the window resize factor in `zoom()` is 1; `None`
    /// (factor 1) until the window is first resized after a zoom reset.
    fit_size: Option<Vec2>,
    /// Copy, Cut or Save as was requested; see `export_step`.
    export: Option<Export>,
    /// Folder of the last opened or saved file, where the next file dialog opens.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    save_dir: Option<PathBuf>,
    /// Image from File > Open, added by `canvas` (which knows where to put it).
    opened: Option<Arc<ColorImage>>,
    /// Frame of the last Edit > Paste: that paste goes to the canvas center, not under the
    /// pointer (which is still where the menu was).
    menu_paste_frame: Option<u64>,
    /// Started with `--paste` (by the macOS menu bar launcher): paste the clipboard on the first frame.
    paste_on_start: bool,
    /// A screenshot being taken (`screenshot.rs`): its image, or `None` if cancelled, arrives here.
    #[cfg(any(target_os = "macos", windows))]
    screenshot: Option<std::sync::mpsc::Receiver<Option<ColorImage>>>,
    /// Asked for a screenshot (by the launcher: `--screenshot`, or its notification when already
    /// running): start one once the window is key, retried until this frame.
    #[cfg(any(target_os = "macos", windows))]
    screenshot_until: Option<u64>,
    /// The screenshot was asked for by the launcher: once taken, it starts a new drawing.
    #[cfg(any(target_os = "macos", windows))]
    screenshot_new_drawing: bool,
    /// Set by the launcher's screenshot notification; see `listen_for_launcher`.
    #[cfg(target_os = "macos")]
    launcher_requests: Option<Arc<std::sync::atomic::AtomicBool>>,
    /// Frame of that paste, which places only an image: text left on the clipboard isn't meant
    /// for the new drawing.
    start_paste_frame: Option<u64>,
    notice: Option<Notice>,
    /// Last window title sent; see `update_title`.
    title: Option<String>,
    show_about: bool,
    about_logo: Option<egui::TextureHandle>,
    /// Built once: the OS caches custom cursors by buffer identity.
    crosshair: Option<egui::CustomCursorImage>,
    i_beam: Option<egui::CustomCursorImage>,
    hand: Option<egui::CustomCursorImage>,
    grabbing: Option<egui::CustomCursorImage>,
    /// See `zoom()`.
    zoom_level: i32,
    /// Screen offset (points) of the canvas origin from the canvas panel's top-left.
    pan: Vec2,
    panning: bool,
    /// Space is held (not while typing): a left-drag pans instead of using the tool.
    space_pan: bool,
    /// Wheel movement not yet turned into a whole zoom step.
    wheel: f32,
    background: Background,
    style: Style,
    style_open: Option<StyleSection>,
    /// Center of the toolbar button the style panel is centered above (screen, this frame): the
    /// current tool's, or with Select the one that draws the first selected object.
    tool_anchor: egui::Pos2,
    /// Center of the button whose section is open (screen, this frame): its options are centered above it.
    style_anchor: egui::Pos2,
}

impl DeerawApp {
    pub fn new(storage: Option<&dyn eframe::Storage>) -> Self {
        let Settings { style, background, save_dir } =
            storage.and_then(|s| eframe::get_value(s, SETTINGS_KEY)).unwrap_or_default();
        let paste_on_start = std::env::args().any(|a| a == "--paste");
        #[cfg(any(target_os = "macos", windows))]
        let screenshot_on_start = std::env::args().any(|a| a == "--screenshot");
        Self {
            style,
            background,
            save_dir,
            paste_on_start,
            #[cfg(any(target_os = "macos", windows))]
            screenshot_until: screenshot_on_start.then_some(SCREENSHOT_START_FRAMES),
            #[cfg(any(target_os = "macos", windows))]
            screenshot_new_drawing: screenshot_on_start,
            ..Default::default()
        }
    }
}

impl eframe::App for DeerawApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let settings = Settings { style: self.style, background: self.background, save_dir: self.save_dir.clone() };
        eframe::set_value(storage, SETTINGS_KEY, &settings);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::L)) {
            self.show_layers = !self.show_layers;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::W)) {
            ui.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if std::mem::take(&mut self.paste_on_start) {
            // Like Edit > Paste, but only an image is placed.
            self.menu_paste_frame = Some(ui.ctx().cumulative_frame_nr());
            self.start_paste_frame = self.menu_paste_frame;
            ui.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        }
        #[cfg(target_os = "macos")]
        {
            let requests = self.launcher_requests.get_or_insert_with(|| screenshot::listen_for_launcher(ui.ctx()));
            if requests.swap(false, std::sync::atomic::Ordering::Relaxed) {
                self.screenshot_until = Some(ui.ctx().cumulative_frame_nr() + SCREENSHOT_START_FRAMES);
                self.screenshot_new_drawing = true;
            }
        }
        #[cfg(any(target_os = "macos", windows))]
        {
            // Retried until the window is key (it isn't on the first frames, nor until the launcher
            // has brought it to the front); given up after a while, e.g. if another app was clicked.
            if let Some(until) = self.screenshot_until {
                let started = self.start_screenshot(ui.ctx());
                self.screenshot_until = (!started && ui.ctx().cumulative_frame_nr() < until).then_some(until);
                if !started && self.screenshot_until.is_none() {
                    self.screenshot_new_drawing = false;
                }
                ui.ctx().request_repaint();
            }
            self.finish_screenshot();
        }
        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        if self.show_layers {
            egui::Panel::left("layers")
                .resizable(true)
                .default_size(220.0)
                .show(ui, |ui| self.layers_panel(ui));
        }

        // Transparent shows a checkerboard (painted in `canvas`).
        let fill = self.background.color().unwrap_or(Color32::TRANSPARENT);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(fill))
            .show(ui, |ui| self.canvas(ui));
        self.about_window(ui.ctx());

        if let Some(export) = self.export.take() {
            self.export_step(ui.ctx(), frame, export);
        }
    }
}

impl DeerawApp {
    /// 100% + 20% × level, times how much the canvas panel grew or shrank with the window since
    /// the zoom was last reset (the smaller of its width and height ratios).
    fn zoom(&self) -> f32 {
        let resized = match (self.fit_size, self.canvas_rect) {
            (Some(fit), Some(canvas)) => (canvas.size() / fit).min_elem(),
            _ => 1.0,
        };
        (1.0 + ZOOM_STEP * self.zoom_level as f32) * resized
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

    /// Copies the selected objects on top, offset a little, and selects the copies. Copied
    /// arrows attach to the copies of their objects, or detach if those weren't copied. Copied
    /// blurs go on the copy of their image, or stay on the image if it wasn't copied.
    /// (Copies of blurs are on top, not right above their image.)
    fn duplicate_selected(&mut self) {
        let copies: Vec<Object> = self.objects.iter().filter(|o| self.selected.contains(&o.id)).cloned().collect();
        // Copy i gets id `first_id + i`.
        let first_id = self.next_id;
        let originals: Vec<u64> = copies.iter().map(|o| o.id).collect();
        self.selected.clear();
        for mut copy in copies {
            copy.id = self.next_id;
            self.next_id += 1;
            copy.translate(Vec2::splat(DUPLICATE_OFFSET));
            let copy_of = |id: u64| originals.iter().position(|o| *o == id).map(|i| first_id + i as u64);
            match &mut copy.kind {
                Kind::Arrow { bindings, .. } => {
                    for binding in bindings {
                        *binding = binding.and_then(|b| Some(Binding { id: copy_of(b.id)?, ..b }));
                    }
                }
                Kind::Blur { image, .. } => *image = copy_of(*image).unwrap_or(*image),
                _ => {}
            }
            self.selected.push(copy.id);
            self.objects.push(copy);
        }
        self.update_blur_areas();
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

    /// Deleting an image deletes its blurs.
    fn delete_selected(&mut self) {
        let selected = std::mem::take(&mut self.selected);
        self.objects.retain(|obj| !selected.contains(&obj.id) && !obj.blurred_image().is_some_and(|i| selected.contains(&i)));
    }

    fn select_all(&mut self) {
        self.tool = Tool::Select;
        self.selected = self.objects.iter().map(|o| o.id).collect();
    }

    /// Empties the canvas for a new drawing; undo brings the old one back.
    #[cfg(any(target_os = "macos", windows))]
    fn new_drawing(&mut self) {
        self.objects.clear();
        self.selected.clear();
        self.tool = Tool::Select;
        self.drawing = None;
        self.marquee = None;
        self.editing = None;
        self.cropping = None;
        self.first_image = None;
    }
}


#[cfg(test)]
mod tests {
    use eframe::Storage as _;

    use super::*;

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

    /// In-memory stand-in for eframe's file storage.
    #[derive(Default)]
    struct Memory(std::collections::HashMap<String, String>);

    impl eframe::Storage for Memory {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }

    #[test]
    fn settings_survive_a_relaunch() {
        let mut app = DeerawApp::default();
        app.style.color = Color32::RED;
        app.style.text_color = Color32::BLUE;
        app.style.line_width = 10.0;
        app.background = Background::Black;
        app.tool = Tool::Pencil;
        let mut storage = Memory::default();
        eframe::App::save(&mut app, &mut storage);

        let app = DeerawApp::new(Some(&storage));
        assert_eq!((app.style.color, app.style.text_color, app.style.line_width, app.background), (Color32::RED, Color32::BLUE, 10.0, Background::Black));
        assert_eq!(app.tool, Tool::Select);
    }

    #[test]
    fn missing_or_unreadable_settings_fall_back_to_defaults() {
        let mut storage = Memory::default();
        storage.set_string(SETTINGS_KEY, "(background: Black)".into());
        let app = DeerawApp::new(Some(&storage));
        assert_eq!((app.background, app.style.line_width), (Background::Black, Style::default().line_width));

        storage.set_string(SETTINGS_KEY, "not settings".into());
        assert_eq!(DeerawApp::new(Some(&storage)).background, Background::default());
    }
}
