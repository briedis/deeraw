//! File > Preferences…: what happens when Deeraw opens, and resetting everything that's remembered.

use eframe::egui::{self, Align2, Vec2};

use super::{DeerawApp, Notice, Settings};

impl DeerawApp {
    pub(super) fn preferences_window(&mut self, ctx: &egui::Context) {
        if !self.show_preferences {
            return;
        }
        let mut open = true;
        egui::Window::new("Preferences")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(egui::Frame::window(&ctx.global_style()).inner_margin(20.0))
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.spacing_mut().item_spacing.y = 10.0;
                ui.spacing_mut().button_padding = Vec2::new(10.0, 5.0);
                ui.add_space(4.0);
                ui.label(egui::RichText::new("When Deeraw opens").strong());
                ui.indent("on launch", |ui| {
                    #[cfg(any(target_os = "macos", windows))]
                    ui.checkbox(&mut self.preferences.screenshot_on_launch, "Take a screenshot");
                    ui.checkbox(&mut self.preferences.check_for_updates, "Check for updates");
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
