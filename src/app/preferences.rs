//! File > Preferences…: what happens when Deeraw opens, and resetting everything that's remembered.

use eframe::egui;

use super::{DeerawApp, Notice, Settings};
use crate::widgets::dialog;

/// The most drawings History can be set to keep.
const MAX_HISTORY_SIZE: usize = 1000;

impl DeerawApp {
    pub(super) fn preferences_window(&mut self, ctx: &egui::Context) {
        if !self.show_preferences {
            return;
        }
        let mut open = true;
        dialog(ctx, "Preferences", &mut open, |ui| {
            ui.set_width(360.0);
            ui.add_space(4.0);
            ui.label(egui::RichText::new("When Deeraw opens").strong());
            ui.indent("on launch", |ui| {
                #[cfg(any(target_os = "macos", windows))]
                ui.checkbox(&mut self.preferences.screenshot_on_launch, "Take a screenshot");
                ui.checkbox(&mut self.preferences.check_for_updates, "Check for updates");
            });
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Text").strong());
            ui.indent("text", |ui| {
                ui.checkbox(&mut self.preferences.markdown, "Format markdown")
                    .on_hover_text("*bold*, _italic_, ~strikethrough~, `code`, # headings and - lists.\nOff: text shows exactly as typed.");
            });
            ui.add_space(8.0);
            ui.label(egui::RichText::new("History").strong());
            ui.indent("history", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Keep the last");
                    ui.add(egui::DragValue::new(&mut self.preferences.history_size).range(1..=MAX_HISTORY_SIZE));
                    ui.label("drawings");
                })
                .response
                .on_hover_text("Older drawings are deleted from File > History when Deeraw opens.");
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Reset").strong());
            ui.label("Resets these preferences, the drawing style, canvas background, file folder and skipped updates.");
            if ui.button("Reset to defaults").clicked() {
                self.apply_settings(Settings::default());
                let pos = ctx.pointer_latest_pos();
                let shown_at = ctx.input(|i| i.time);
                self.notice = Some(Notice { text: "Preferences reset".into(), pos, shown_at });
            }
            ui.add_space(4.0);
        });
        self.show_preferences = open && !ctx.input(|i| i.key_pressed(egui::Key::Escape));
    }
}
