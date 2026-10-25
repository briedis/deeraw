//! The menu bar, and the About and Changelog windows.

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
            // The browser has no file dialogs and nothing to exit.
            #[cfg(not(target_arch = "wasm32"))]
            ui.menu_button("File", |ui| {
                if menu_item(ui, "Open image…", command(Key::O), true) {
                    self.open_file();
                }
                if menu_item(ui, "Save as…", command(Key::S), has_objects) {
                    self.save_as();
                }
                ui.separator();
                if menu_item(ui, "Preferences…", command(Key::Comma), true) {
                    self.show_preferences = true;
                }
                ui.separator();
                if menu_item(ui, "Exit", command(Key::W), true) {
                    ui.send_viewport_cmd(ViewportCommand::Close);
                }
            });
            ui.menu_button("Edit", |ui| {
                if menu_item(ui, "Undo", command(Key::Z), self.history.can_undo()) {
                    self.history.undo(&mut self.objects);
                }
                if menu_item(ui, "Redo", command(Key::Y), self.history.can_redo()) {
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
                if menu_item(ui, "View on GitHub", None, true) {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(env!("CARGO_PKG_REPOSITORY")));
                }
                if menu_item(ui, "Changelog", None, true) {
                    self.show_changelog = true;
                }
                #[cfg(not(target_arch = "wasm32"))]
                if menu_item(ui, "Check for updates…", None, true) {
                    self.check_for_updates(ui.ctx(), true);
                }
                ui.separator();
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
            let icon = image::load_from_memory(ICON_PNG).expect("bundled icon is a valid PNG").to_rgba8();
            let size = [icon.width() as usize, icon.height() as usize];
            let image = ColorImage::from_rgba_unmultiplied(size, icon.as_raw());
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

    /// CHANGELOG.md as built into the app: a heading per release, its items as bullets.
    pub(super) fn changelog_window(&mut self, ctx: &egui::Context) {
        if !self.show_changelog {
            return;
        }
        let mut open = true;
        egui::Window::new("Changelog")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(440.0);
                egui::ScrollArea::vertical().max_height(ctx.content_rect().height() * 0.6).show(ui, |ui| {
                    for section in changelog_sections(CHANGELOG) {
                        let current = section.title.starts_with(concat!(env!("CARGO_PKG_VERSION"), " "));
                        let title = if current { format!("{} (this version)", section.title) } else { section.title.to_string() };
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(title).strong().size(16.0));
                        for item in &section.items {
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                ui.label("•");
                                ui.add(egui::Label::new(item).wrap());
                            });
                        }
                    }
                    ui.add_space(6.0);
                });
            });
        self.show_changelog = open && !ctx.input(|i| i.key_pressed(egui::Key::Escape));
    }
}

const CHANGELOG: &str = include_str!("../../CHANGELOG.md");

/// A `## ` section of the changelog.
struct ChangelogSection<'a> {
    title: &'a str,
    /// Its `- ` items (or plain paragraphs), each with its indented continuation lines joined on.
    items: Vec<String>,
}

fn changelog_sections(markdown: &str) -> Vec<ChangelogSection<'_>> {
    let mut sections: Vec<ChangelogSection> = Vec::new();
    for line in markdown.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            sections.push(ChangelogSection { title: title.trim(), items: Vec::new() });
            continue;
        }
        // The title and intro above the first release.
        let Some(section) = sections.last_mut() else { continue };
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        match (text.strip_prefix("- "), section.items.last_mut()) {
            (Some(item), _) => section.items.push(item.to_string()),
            (None, Some(last)) if line.starts_with(' ') => {
                last.push(' ');
                last.push_str(text);
            }
            (None, _) => section.items.push(text.to_string()),
        }
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_the_changelog_into_releases() {
        let markdown = "# Changelog\n\nIntro.\n\n## 0.2.0 (2026-10-20)\n\n- One\n- Two,\n  continued.\n\n## 0.1.0\n\nFirst release.\n";
        let sections = changelog_sections(markdown);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "0.2.0 (2026-10-20)");
        assert_eq!(sections[0].items, ["One", "Two, continued."]);
        assert_eq!(sections[1].items, ["First release."]);
    }

    #[test]
    fn the_built_in_changelog_has_releases() {
        assert!(changelog_sections(CHANGELOG).iter().all(|s| !s.items.is_empty()));
    }
}
