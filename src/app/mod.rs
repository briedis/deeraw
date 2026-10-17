//! The app: its state, the frame layout, and operations on the objects.

mod canvas;
mod files;
mod menu;
mod panels;
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

#[derive(Clone, Copy, PartialEq, Default)]
enum Tool {
    #[default]
    Select,
    Pencil,
    Arrow,
    Line,
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
}

/// A stroke, arrow or text box being drawn, in canvas pixels.
enum Drawing {
    /// Points and when each was added (seconds), for the speed-based pressure.
    Stroke(Vec<Pos2>, Vec<f64>),
    /// Start, end, and what each is attached to.
    Arrow(Pos2, Pos2, [Option<Binding>; 2]),
    TextBox(Pos2, Pos2),
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
    drawing: Option<Drawing>,
    resize_drag: Option<ResizeDrag>,
    /// Where the arrow endpoint being dragged would be without snapping, canvas pixels.
    point_drag: Option<Pos2>,
    /// Image currently being cropped; see `crop_ui`.
    cropping: Option<u64>,
    /// Text object currently being edited.
    editing: Option<u64>,
    focus_editor: bool,
    /// The layers panel, toggled with Ctrl+L.
    show_layers: bool,
    /// Last frame's canvas panel position, to keep the drawing still when the panel moves.
    canvas_min: Option<Pos2>,
    /// Copy, Cut or Save as was requested; see `export_step`.
    export: Option<Export>,
    /// Folder of the last opened or saved file, where the next file dialog opens.
    save_dir: Option<PathBuf>,
    /// Image from File > Open, added by `canvas` (which knows where to put it).
    opened: Option<Arc<ColorImage>>,
    /// Frame of the last Edit > Paste: that paste goes to the canvas center, not under the
    /// pointer (which is still where the menu was).
    menu_paste_frame: Option<u64>,
    show_about: bool,
    about_logo: Option<egui::TextureHandle>,
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
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::L)) {
            self.show_layers = !self.show_layers;
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
    fn zoom(&self) -> f32 {
        1.0 + ZOOM_STEP * self.zoom_level as f32
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
    /// arrows attach to the copies of their objects, or detach if those weren't copied.
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
            if let Kind::Arrow { bindings, .. } = &mut copy.kind {
                for binding in bindings {
                    *binding = binding.and_then(|b| {
                        let i = originals.iter().position(|id| *id == b.id)?;
                        Some(Binding { id: first_id + i as u64, ..b })
                    });
                }
            }
            self.selected.push(copy.id);
            self.objects.push(copy);
        }
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

    fn delete_selected(&mut self) {
        let selected = std::mem::take(&mut self.selected);
        self.objects.retain(|obj| !selected.contains(&obj.id));
    }

    fn select_all(&mut self) {
        self.tool = Tool::Select;
        self.selected = self.objects.iter().map(|o| o.id).collect();
    }
}


#[cfg(test)]
mod tests {
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
}
