//! The app: its state, the frame layout, and operations on the objects.

mod arrange;
mod blur;
mod canvas;
mod files;
mod intro;
mod menu;
mod panels;
#[cfg(not(target_arch = "wasm32"))]
mod preferences;
#[cfg(any(target_os = "macos", windows))]
mod screenshot;
mod selection;
mod snap;
#[cfg(not(target_arch = "wasm32"))]
mod update;

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Vec2};

use crate::export::Export;
use crate::geometry::mostly_inside;
use crate::history::History;
use crate::markdown;
use crate::object::{Binding, Kind, Object, Outline};
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
    /// "Skip this version" in the update window: that version isn't offered on launch.
    skipped_version: Option<String>,
    /// "Remind me later": no update window on launch before this (Unix seconds).
    update_reminder: Option<i64>,
    preferences: Preferences,
    /// The version that ran last; when this one is newer, it opens with "What's new" (`whats_new_since`).
    last_version: Option<String>,
}

/// What File > Preferences… sets.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Preferences {
    /// Start a screenshot when Deeraw opens, as the launcher's `--screenshot` does.
    screenshot_on_launch: bool,
    check_for_updates: bool,
    /// Format markdown in text (`markdown::set_enabled`); off shows it as typed.
    markdown: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self { screenshot_on_launch: false, check_for_updates: true, markdown: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
enum Tool {
    #[default]
    Select,
    Pencil,
    /// Also draws lines (`Style::line`).
    Arrow,
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
    /// Objects moved along with the selection by that move; see `riders`.
    riders: Vec<u64>,
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
    /// The intro is gone for good: something was added or a tool other than Select picked.
    intro_dismissed: bool,
    show_about: bool,
    show_changelog: bool,
    /// The Changelog window shows only the releases after this version, as "What's new" after an update.
    whats_new_since: Option<String>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    show_preferences: bool,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    preferences: Preferences,
    /// The launch update check was started.
    #[cfg(not(target_arch = "wasm32"))]
    update_started: bool,
    #[cfg(not(target_arch = "wasm32"))]
    update_check: Option<update::UpdateCheck>,
    #[cfg(not(target_arch = "wasm32"))]
    update_window: Option<update::UpdateWindow>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    skipped_version: Option<String>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    update_reminder: Option<i64>,
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
        let settings: Settings = storage.and_then(|s| eframe::get_value(s, SETTINGS_KEY)).unwrap_or_default();
        let paste_on_start = std::env::args().any(|a| a == "--paste");
        #[cfg(any(target_os = "macos", windows))]
        let screenshot_on_start = std::env::args().any(|a| a == "--screenshot");
        let mut app = Self { paste_on_start, ..Default::default() };
        // Not on a first run, which has nothing to compare with.
        if let Some(last) = settings.last_version.as_deref()
            && menu::is_newer(env!("CARGO_PKG_VERSION"), last)
        {
            app.whats_new_since = Some(last.to_string());
            app.show_changelog = true;
        }
        app.apply_settings(settings);
        #[cfg(any(target_os = "macos", windows))]
        {
            // The preference adds the screenshot to the (empty) canvas; the launcher's starts a new drawing.
            if screenshot_on_start || app.preferences.screenshot_on_launch {
                app.screenshot_until = Some(SCREENSHOT_START_FRAMES);
            }
            app.screenshot_new_drawing = screenshot_on_start;
        }
        app
    }

    fn settings(&self) -> Settings {
        Settings {
            style: self.style,
            background: self.background,
            save_dir: self.save_dir.clone(),
            skipped_version: self.skipped_version.clone(),
            update_reminder: self.update_reminder,
            preferences: self.preferences,
            last_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }
    }

    fn apply_settings(&mut self, settings: Settings) {
        let Settings { style, background, save_dir, skipped_version, update_reminder, preferences, last_version: _ } =
            settings;
        self.style = style;
        self.background = background;
        self.save_dir = save_dir;
        self.skipped_version = skipped_version;
        self.update_reminder = update_reminder;
        self.preferences = preferences;
    }
}

impl eframe::App for DeerawApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, SETTINGS_KEY, &self.settings());
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        markdown::set_enabled(self.preferences.markdown);
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
        self.changelog_window(ui.ctx());
        #[cfg(not(target_arch = "wasm32"))]
        self.preferences_window(ui.ctx());
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !std::mem::replace(&mut self.update_started, true) {
                self.check_for_updates(ui.ctx(), false);
            }
            self.update_dialog(ui.ctx());
        }

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

    /// The unselected objects on a selected image, which moving it takes along: above it, at least
    /// half inside it, and it's the topmost image they're on. Nested, so an image on an image takes
    /// its own along. Blurs follow their image anyway, and attached arrows their objects.
    fn riders(&self) -> Vec<u64> {
        let mut moving = self.selected.clone();
        let mut riders = vec![];
        for (i, obj) in self.objects.iter().enumerate() {
            let attached = matches!(obj.kind, Kind::Arrow { bindings, .. } if bindings.iter().any(Option::is_some));
            if moving.contains(&obj.id) || obj.blurred_image().is_some() || attached {
                continue;
            }
            let base = self.objects[..i]
                .iter()
                .rev()
                .find(|o| matches!(o.kind, Kind::Image { .. }) && mostly_inside(obj.rect, o.rect));
            if base.is_some_and(|b| moving.contains(&b.id)) {
                moving.push(obj.id);
                riders.push(obj.id);
            }
        }
        riders
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

    /// Switches the canvas background. When its default ink changes (black on white or transparent,
    /// white on black), every object's black and white (colors and fills) swap, so black lines and
    /// text don't vanish on black and white-on-black text boxes stay readable; other colors like red
    /// stay, and so do objects on an image (above one and at least half inside it), which shows the
    /// same on any background. Swapping both ways makes switching back restore the drawing. The drawing color follows
    /// only if it was the old ink. Undo reverts the colors (the background isn't in history).
    pub(super) fn set_background(&mut self, background: Background) {
        let (old, new) = (self.background.ink(), background.ink());
        self.background = background;
        if old == new {
            return;
        }
        for color in [&mut self.style.color, &mut self.style.text_color] {
            if *color == old {
                *color = new;
            }
        }
        let swap = |c: &mut Color32| {
            if *c == old {
                *c = new;
            } else if *c == new {
                *c = old;
            }
        };
        let on_image: Vec<bool> = (0..self.objects.len())
            .map(|i| {
                self.objects[..i]
                    .iter()
                    .any(|o| matches!(o.kind, Kind::Image { .. }) && mostly_inside(self.objects[i].rect, o.rect))
            })
            .collect();
        for (obj, on_image) in self.objects.iter_mut().zip(on_image) {
            if on_image {
                continue;
            }
            if let Some(c) = obj.color_mut() {
                swap(c);
            }
            if let Some(Some(f)) = obj.fill_mut() {
                swap(f);
            }
            if let Some(Outline { color: Some(c), .. }) = obj.outline_mut() {
                swap(c);
            }
        }
    }

    /// About, Changelog, Preferences or the update window is shown.
    pub(super) fn window_open(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if self.show_preferences || self.update_window.is_some() {
            return true;
        }
        self.show_about || self.show_changelog
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

    #[test]
    fn black_background_swaps_black_and_white_objects() {
        let mut app = DeerawApp::default();
        let stroke = |color| Kind::Stroke { points: vec![], pressures: vec![], color, width: 2.0, hand_drawn: false };
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(10.0));
        for color in [Color32::BLACK, Color32::WHITE, Color32::RED] {
            app.add(rect, stroke(color));
        }
        // A black stroke on an image keeps its color.
        let ctx = egui::Context::default();
        app.add_image(&ctx, Arc::new(ColorImage::filled([10, 10], Color32::RED)), Pos2::ZERO);
        app.objects.last_mut().unwrap().rect = rect.translate(Vec2::new(100.0, 0.0));
        app.add(rect.translate(Vec2::new(102.0, 0.0)), stroke(Color32::BLACK));
        let colors = |app: &mut DeerawApp| app.objects.iter_mut().filter_map(|o| o.color_mut().copied()).collect::<Vec<_>>();
        app.set_background(Background::Black);
        assert_eq!(colors(&mut app), [Color32::WHITE, Color32::BLACK, Color32::RED, Color32::BLACK]);
        assert_eq!(app.style.color, Color32::WHITE);
        // Transparent has the same ink as white: nothing more changes.
        app.set_background(Background::Transparent);
        assert_eq!(colors(&mut app), [Color32::BLACK, Color32::WHITE, Color32::RED, Color32::BLACK]);
        app.set_background(Background::White);
        assert_eq!(colors(&mut app), [Color32::BLACK, Color32::WHITE, Color32::RED, Color32::BLACK]);
    }

    #[test]
    fn riders_are_the_objects_on_a_selected_image() {
        let ctx = egui::Context::default();
        let mut app = DeerawApp::default();
        let rect = |x: f32, y: f32, w: f32| Rect::from_min_size(Pos2::new(x, y), Vec2::splat(w));
        let image = |app: &mut DeerawApp, r: Rect| {
            app.add_image(&ctx, Arc::new(ColorImage::filled([10, 10], Color32::RED)), Pos2::ZERO);
            app.objects.last_mut().unwrap().rect = r;
            app.objects.last().unwrap().id
        };
        let stroke = || Kind::Stroke { points: vec![], pressures: vec![], color: Color32::BLACK, width: 2.0, hand_drawn: false };
        let below = app.add(rect(10.0, 10.0, 20.0), stroke());
        let big = image(&mut app, rect(0.0, 0.0, 100.0));
        let on_big = app.add(rect(10.0, 10.0, 20.0), stroke());
        let mostly_off = app.add(rect(91.0, 10.0, 20.0), stroke());
        let small = image(&mut app, rect(50.0, 50.0, 40.0));
        let on_small = app.add(rect(60.0, 60.0, 10.0), stroke());
        let elsewhere = image(&mut app, rect(300.0, 0.0, 100.0));
        let _ = (below, mostly_off, elsewhere);
        app.selected = vec![big];
        assert_eq!(app.riders(), [on_big, small, on_small]);
        app.selected = vec![small];
        assert_eq!(app.riders(), [on_small]);
        app.selected = vec![on_big];
        assert!(app.riders().is_empty());
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

    #[test]
    fn older_settings_check_for_updates() {
        let mut storage = Memory::default();
        storage.set_string(SETTINGS_KEY, "(preferences: (screenshot_on_launch: true))".into());
        let app = DeerawApp::new(Some(&storage));
        assert!(app.preferences.screenshot_on_launch && app.preferences.check_for_updates);
        storage.set_string(SETTINGS_KEY, "(background: Black)".into());
        assert_eq!(DeerawApp::new(Some(&storage)).preferences, Preferences::default());
    }
}
