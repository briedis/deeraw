//! The menu bar and the About window.

use eframe::egui::{self, Align2, ColorImage, Vec2};

use super::DeerawApp;
use crate::ICON_PNG;
use crate::widgets::menu_item;

impl DeerawApp {
    /// File, Edit and Help menus. Cut/Copy/Paste go through egui's clipboard events, so they
    /// act on a text being edited just like the keyboard shortcuts do.
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        use egui::{Key, KeyboardShortcut, Modifiers, ViewportCommand};
        let command = |key| Some(KeyboardShortcut::new(Modifiers::COMMAND, key));
        let has_objects = !self.objects.is_empty();
        let has_selection = !self.selected.is_empty();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if menu_item(ui, "Open image…", command(Key::O), true) {
                    self.open_file();
                }
                if menu_item(ui, "Save as…", command(Key::S), has_objects) {
                    self.save_as();
                }
                ui.separator();
                if menu_item(ui, "Exit", None, true) {
                    ui.send_viewport_cmd(ViewportCommand::Close);
                }
            });
            ui.menu_button("Edit", |ui| {
                if menu_item(ui, "Undo", command(Key::Z), true) {
                    self.history.undo(&mut self.objects);
                }
                if menu_item(ui, "Redo", command(Key::Y), true) {
                    self.history.redo(&mut self.objects);
                }
                ui.separator();
                if menu_item(ui, "Cut", command(Key::X), has_selection) {
                    ui.send_viewport_cmd(ViewportCommand::RequestCut);
                }
                if menu_item(ui, "Copy", command(Key::C), has_objects) {
                    ui.send_viewport_cmd(ViewportCommand::RequestCopy);
                }
                if menu_item(ui, "Paste", command(Key::V), true) {
                    self.menu_paste_frame = Some(ui.ctx().cumulative_frame_nr());
                    ui.send_viewport_cmd(ViewportCommand::RequestPaste);
                }
                let delete = Some(KeyboardShortcut::new(Modifiers::NONE, Key::Delete));
                if menu_item(ui, "Delete", delete, has_selection) {
                    self.delete_selected();
                }
                ui.separator();
                if menu_item(ui, "Select all", command(Key::A), has_objects) {
                    self.select_all();
                }
                if menu_item(ui, "Duplicate", command(Key::D), has_selection) {
                    self.duplicate_selected();
                }
            });
            ui.menu_button("Help", |ui| {
                if menu_item(ui, "About Deeraw", None, true) {
                    self.show_about = true;
                }
            });
        });
        let objects = &self.objects;
        self.selected.retain(|id| objects.iter().any(|o| o.id == *id));
    }

    pub(super) fn about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let logo = self.about_logo.get_or_insert_with(|| {
            let icon = eframe::icon_data::from_png_bytes(ICON_PNG).expect("bundled icon is a valid PNG");
            let size = [icon.width as usize, icon.height as usize];
            let image = ColorImage::from_rgba_unmultiplied(size, &icon.rgba);
            ctx.load_texture("about-logo", image, egui::TextureOptions::LINEAR)
        });
        let logo = egui::Image::new(&*logo).fit_to_exact_size(Vec2::splat(96.0));
        let mut open = true;
        egui::Window::new("About Deeraw")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(260.0);
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.add(logo);
                    ui.add_space(4.0);
                    ui.heading("Deeraw");
                    ui.label(env!("CARGO_PKG_DESCRIPTION"));
                    ui.add_space(8.0);
                    ui.weak(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.weak(format!("by {}", env!("CARGO_PKG_AUTHORS")));
                    ui.add_space(8.0);
                    ui.weak("Free to use, at home or at work, under the");
                    ui.hyperlink_to(
                        "PolyForm Shield License 1.0.0",
                        "https://polyformproject.org/licenses/shield/1.0.0",
                    );
                    ui.add_space(8.0);
                });
            });
        self.show_about = open && !ctx.input(|i| i.key_pressed(egui::Key::Escape));
    }
}
