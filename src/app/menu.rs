//! The menu bar, and the About and Changelog windows.

use eframe::egui::{self, ColorImage, Vec2};

use super::DeerawApp;
use crate::ICON_PNG;
use crate::widgets::{DIALOG_MARGIN, dialog, menu_item};

impl DeerawApp {
    /// File, Edit and Help menus. Cut/Copy/Paste go through egui's clipboard events, so they
    /// act on a text being edited just like the keyboard shortcuts do.
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        use egui::{Key, KeyboardShortcut, Modifiers, ViewportCommand};
        let command = |key| Some(KeyboardShortcut::new(Modifiers::COMMAND, key));
        let has_objects = !self.objects.is_empty();
        let has_selection = !self.selected.is_empty();
        let config = egui::containers::menu::MenuConfig::new().style(menu_style);
        egui::MenuBar::new().style(menu_bar_style).config(config).ui(ui, |ui| {
            // The browser has no file dialogs and nothing to exit.
            #[cfg(not(target_arch = "wasm32"))]
            ui.menu_button("File", |ui| {
                ui.set_min_width(MENU_MIN_WIDTH);
                if menu_item(ui, "Open image…", command(Key::O), true) {
                    self.open_file();
                }
                if menu_item(ui, "Save as…", command(Key::S), has_objects) {
                    self.save_as();
                }
                ui.add(egui::Separator::default().spacing(MENU_SEPARATOR_SPACING));
                if menu_item(ui, "Preferences…", command(Key::Comma), true) {
                    self.show_preferences = true;
                }
                ui.add(egui::Separator::default().spacing(MENU_SEPARATOR_SPACING));
                if menu_item(ui, "Exit", command(Key::W), true) {
                    ui.send_viewport_cmd(ViewportCommand::Close);
                }
            });
            ui.menu_button("Edit", |ui| {
                ui.set_min_width(MENU_MIN_WIDTH);
                if menu_item(ui, "Undo", command(Key::Z), self.history.can_undo()) {
                    self.history.undo(&mut self.objects);
                }
                if menu_item(ui, "Redo", command(Key::Y), self.history.can_redo()) {
                    self.history.redo(&mut self.objects);
                }
                ui.add(egui::Separator::default().spacing(MENU_SEPARATOR_SPACING));
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
                ui.add(egui::Separator::default().spacing(MENU_SEPARATOR_SPACING));
                if menu_item(ui, "Select all", command(Key::A), has_objects) {
                    self.select_all();
                }
                if menu_item(ui, "Duplicate", command(Key::D), has_selection) {
                    self.duplicate_selected();
                }
                ui.add(egui::Separator::default().spacing(MENU_SEPARATOR_SPACING));
                self.arrange_items(ui);
            });
            ui.menu_button("Help", |ui| {
                ui.set_min_width(MENU_MIN_WIDTH);
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
                ui.add(egui::Separator::default().spacing(MENU_SEPARATOR_SPACING));
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
        dialog(ctx, "About Deeraw", &mut open, |ui| {
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

    /// CHANGELOG.md as built into the app: a heading per release, its items as bullets. After an update
    /// (`whats_new_since`) it's "What's new", with only the releases since the version that ran before.
    pub(super) fn changelog_window(&mut self, ctx: &egui::Context) {
        if !self.show_changelog {
            return;
        }
        let mut open = true;
        let since = self.whats_new_since.clone();
        let title = if since.is_some() { "What's new" } else { "Changelog" };
        dialog(ctx, title, &mut open, |ui| {
            ui.set_width(CHANGELOG_WIDTH);
            if let Some(since) = &since {
                ui.label(egui::RichText::new(format!("Deeraw was updated to {}.", env!("CARGO_PKG_VERSION"))).strong());
                ui.weak(format!("Here's what changed since {since}:"));
                ui.add_space(4.0);
            }
            let sections = changelog_sections(CHANGELOG)
                .into_iter()
                .filter(|s| since.as_ref().is_none_or(|since| is_newer(s.version(), since)));
            // The list reaches into the window's right margin, so the scroll bar sits there, clear of the text.
            let margin = egui::Margin { right: DIALOG_MARGIN as i8, ..Default::default() };
            let outside = egui::Margin { right: -margin.right, ..Default::default() };
            egui::Frame::NONE.outer_margin(outside).show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(ctx.content_rect().height() * 0.55)
                    .content_margin(margin)
                    .show(ui, |ui| {
                        for (i, section) in sections.enumerate() {
                            if i > 0 {
                                ui.add(egui::Separator::default().spacing(CHANGELOG_RELEASE_SPACING));
                            }
                            let version = section.version();
                            let date = section.title[version.len()..].trim().trim_matches(['(', ')']);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(version).strong().size(15.0));
                                ui.weak(date);
                                if version == env!("CARGO_PKG_VERSION") {
                                    ui.weak("· this version");
                                }
                            });
                            for item in &section.items {
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    ui.label("•");
                                    ui.add(egui::Label::new(item).wrap());
                                });
                            }
                        }
                    });
            });
            if since.is_some() {
                ui.add_space(4.0);
                if ui.button("Full changelog").clicked() {
                    self.whats_new_since = None;
                }
            }
        });
        self.show_changelog = open && !ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if !self.show_changelog {
            self.whats_new_since = None;
        }
    }
}

/// The Changelog window's text width and the height between releases (a line in the middle).
const CHANGELOG_WIDTH: f32 = 520.0;
const CHANGELOG_RELEASE_SPACING: f32 = 24.0;

/// Narrowest a menu gets, so shortcuts sit well clear of their labels.
pub(super) const MENU_MIN_WIDTH: f32 = 200.0;

/// Height a separator takes in a menu, its line in the middle.
pub(super) const MENU_SEPARATOR_SPACING: f32 = 12.0;

/// egui's menu style with roomier items: taller rows, a wider margin and more space around separators.
pub(super) fn menu_style(style: &mut egui::Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.button_padding = Vec2::new(8.0, 4.0);
    style.spacing.item_spacing.y = 2.0;
    style.spacing.menu_margin = egui::Margin::same(8);
}

/// The bar's File/Edit/Help buttons, padded like the items under them.
fn menu_bar_style(style: &mut egui::Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.button_padding = Vec2::new(8.0, 3.0);
}

const CHANGELOG: &str = include_str!("../../CHANGELOG.md");

/// A `## ` section of the changelog.
struct ChangelogSection<'a> {
    title: &'a str,
    /// Its `- ` items (or plain paragraphs), each with its indented continuation lines joined on.
    items: Vec<String>,
}

pub(super) fn parse_version(version: &str) -> Option<Vec<u32>> {
    version.split('.').map(|part| part.parse().ok()).collect()
}

/// Whether `latest` is a later version than `current` (by number, so 0.10.0 is after 0.9.0).
pub(super) fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

impl ChangelogSection<'_> {
    /// "0.8.1" from "0.8.1 (2026-10-27)" ("Unreleased" stays as it is).
    fn version(&self) -> &str {
        self.title.split_once(' ').map_or(self.title, |(version, _)| version)
    }
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
        assert_eq!((sections[0].version(), sections[1].version()), ("0.2.0", "0.1.0"));
    }

    #[test]
    fn compares_versions_by_number() {
        assert!(is_newer("0.7.0", "0.6.2"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.6.2", "0.6.2"));
        assert!(!is_newer("0.6.1", "0.6.2"));
        assert!(!is_newer("junk", "0.6.2"));
    }

    #[test]
    fn the_built_in_changelog_has_releases() {
        assert!(changelog_sections(CHANGELOG).iter().all(|s| !s.items.is_empty()));
    }
}
