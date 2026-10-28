//! File > History: when the drawing is saved there (`history_store`), and opening it again.
//!
//! Each drawing gets one entry, which its later saves replace: a drawing starts at launch, when
//! the launcher's screenshot replaces it (`new_drawing`) and when an entry is opened. It's saved
//! `AUTOSAVE_DELAY` after it changes (once the canvas is settled), before it's replaced, and on exit.
//! An empty drawing isn't saved, so its entry keeps what it last had. Only the newest
//! `Preferences::history_size` entries are kept: the rest are deleted when Deeraw opens.

use std::sync::Arc;

use eframe::egui;

use super::DeerawApp;
use crate::export::{Export, ExportTarget};
use crate::history_store::{Entry, Reply, Worker};
use crate::object::Object;
use crate::style::Background;
use crate::widgets::menu_item;

/// How long after a change the drawing is saved, in seconds.
const AUTOSAVE_DELAY: f64 = 3.0;

#[derive(Default)]
pub(super) struct Backup {
    /// Started on the first frame, which has the context it wakes for replies.
    worker: Option<Worker>,
    /// The current drawing's number; the worker keeps which entry each one has.
    drawing: u64,
    /// The current drawing as last saved (or as it started), so only changes are saved.
    saved: (Vec<Object>, Background),
    /// When the current drawing first differed from `saved` (egui time), if it does.
    changed_at: Option<f64>,
    /// An opened entry is about to be placed: what it's placed as is where its drawing starts.
    opening: bool,
    /// Renders of drawings to save, two frames each (see `export_step`).
    exports: Vec<Export>,
    /// The window was asked to close and waits for the last save to be rendered.
    closing: bool,
    /// The newest entries, as the worker last listed them.
    recent: Vec<Entry>,
    /// The last thing that went wrong, shown in the History menu.
    error: Option<String>,
}

impl Backup {
    /// Hands a rendered drawing to the worker.
    pub(super) fn save(&self, drawing: u64, image: egui::ColorImage) {
        if let Some(worker) = &self.worker {
            worker.save(drawing, image);
        }
    }
}

impl DeerawApp {
    /// The current drawing differs from what was last saved of it, and isn't empty.
    fn unsaved(&self) -> bool {
        !self.objects.is_empty() && (self.objects.as_slice(), self.background) != (self.backup.saved.0.as_slice(), self.backup.saved.1)
    }

    /// Starts saving the current drawing if it changed (it's rendered from this moment's objects).
    pub(super) fn backup_now(&mut self) {
        if self.unsaved() {
            self.backup.exports.push(Export::new(self.objects.clone(), ExportTarget::History(self.backup.drawing)));
        }
        self.backup.saved = (self.objects.clone(), self.background);
        self.backup.changed_at = None;
    }

    /// Saves the drawing being replaced, and starts a new one (with its own entry).
    pub(super) fn backup_new_drawing(&mut self) {
        self.backup_now();
        self.backup.drawing += 1;
        self.backup.saved = (vec![], self.background);
    }

    /// Run at the end of each frame: handles the worker's replies, saves after changes and before closing.
    pub(super) fn backup_frame(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        if self.backup.worker.is_none()
            && let Some(dir) = eframe::storage_dir("Deeraw")
        {
            self.backup.worker = Some(Worker::start(dir.join("history.db"), self.preferences.history_size, ctx.clone()));
        }
        let replies: Vec<Reply> = self.backup.worker.iter().flat_map(Worker::replies).collect();
        for reply in replies {
            match reply {
                Reply::Recent(entries) => {
                    self.backup.recent = entries;
                    self.backup.error = None;
                }
                Reply::Loaded(image) => {
                    self.new_drawing();
                    self.opened = Some(Arc::new(image));
                    self.backup.opening = true;
                    ctx.request_repaint();
                }
                Reply::Failed(err) => {
                    eprintln!("{err}");
                    self.backup.error = Some(err);
                }
            }
        }
        // `canvas` has placed the opened image by now.
        if self.backup.opening && self.opened.is_none() {
            self.backup.opening = false;
            self.backup.saved = (self.objects.clone(), self.background);
        }

        if ctx.input(|i| i.viewport().close_requested()) {
            self.backup_now();
            if !self.backup.exports.is_empty() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.backup.closing = true;
            }
        } else {
            let settled = !ctx.input(|i| i.pointer.any_down())
                && self.editing.is_none()
                && self.cropping.is_none()
                && self.drawing.is_none();
            if !self.unsaved() {
                self.backup.changed_at = None;
            } else if settled {
                let now = ctx.input(|i| i.time);
                let since = now - *self.backup.changed_at.get_or_insert(now);
                if since >= AUTOSAVE_DELAY {
                    self.backup_now();
                } else {
                    ctx.request_repaint_after_secs((AUTOSAVE_DELAY - since) as f32);
                }
            }
        }

        let exports = std::mem::take(&mut self.backup.exports);
        self.backup.exports = exports.into_iter().filter_map(|export| self.export_step(ctx, frame, export)).collect();
        // Rendered and handed to the worker, which `on_exit` waits for.
        if self.backup.closing && self.backup.exports.is_empty() {
            self.backup.closing = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Waits for the last saves to be written.
    pub(super) fn backup_exit(&mut self) {
        if let Some(worker) = &mut self.backup.worker {
            worker.finish();
        }
    }

    /// File > History: the newest drawings; picking one opens it as a new drawing (undo brings the
    /// current one back). Its changes go to a new entry, so the one opened stays as it was.
    pub(super) fn history_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("History", |ui| {
            if let Some(err) = &self.backup.error {
                ui.add_enabled(false, egui::Label::new(err));
                ui.separator();
            }
            if self.backup.recent.is_empty() {
                ui.add_enabled(false, egui::Label::new("No drawings yet"));
            }
            for entry in &self.backup.recent {
                if menu_item(ui, &entry_label(entry), None, true)
                    && let Some(worker) = &self.backup.worker
                {
                    worker.load(entry.id);
                }
            }
        });
    }
}

/// "Oct 28, 14:32 · 1280 × 720", in local time.
fn entry_label(entry: &Entry) -> String {
    let [w, h] = entry.size;
    let time = chrono::DateTime::from_timestamp_millis(entry.updated)
        .map(|t| t.with_timezone(&chrono::Local).format("%b %-d, %H:%M").to_string())
        .unwrap_or_default();
    format!("{time} · {w} × {h}")
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Color32, Pos2, Rect, Vec2};

    use super::*;
    use crate::object::Kind;

    fn stroke() -> Kind {
        Kind::Stroke { points: vec![], pressures: vec![], color: Color32::BLACK, width: 2.0, hand_drawn: false }
    }

    fn add(app: &mut DeerawApp, x: f32) {
        app.add(Rect::from_min_size(Pos2::new(x, 0.0), Vec2::splat(10.0)), stroke());
    }

    /// The drawing number of each save queued so far.
    fn queued(app: &DeerawApp) -> Vec<u64> {
        app.backup
            .exports
            .iter()
            .map(|e| match e.target {
                ExportTarget::History(drawing) => drawing,
                _ => panic!("not a History save"),
            })
            .collect()
    }

    #[test]
    fn only_changes_are_saved() {
        let mut app = DeerawApp::default();
        app.backup_now();
        assert_eq!(queued(&app), [] as [u64; 0], "an empty drawing isn't saved");
        add(&mut app, 0.0);
        assert!(app.unsaved());
        app.backup_now();
        assert_eq!(queued(&app), [0]);
        assert!(app.backup.exports[0].objects == app.objects);
        app.backup_now();
        assert_eq!(queued(&app), [0], "nothing changed since");
        app.objects[0].translate(Vec2::X);
        app.backup_now();
        assert_eq!(queued(&app), [0, 0], "a change replaces the same entry");
        app.set_background(Background::Black);
        assert!(app.unsaved(), "the background is part of the image");
    }

    #[test]
    fn a_new_drawing_saves_the_old_one_and_gets_its_own_entry() {
        let mut app = DeerawApp::default();
        add(&mut app, 0.0);
        app.new_drawing();
        assert!(app.objects.is_empty());
        assert_eq!(queued(&app), [0]);
        add(&mut app, 0.0);
        app.backup_now();
        assert_eq!(queued(&app), [0, 1]);
        // Emptied, it isn't saved, so its entry keeps the last drawing.
        app.objects.clear();
        app.new_drawing();
        assert_eq!(queued(&app), [0, 1]);
        assert_eq!(app.backup.drawing, 2);
    }

    #[test]
    fn an_opened_entry_is_saved_only_once_changed() {
        let mut app = DeerawApp::default();
        add(&mut app, 0.0);
        // As `backup_frame` does with a loaded entry, and `canvas` then places it.
        app.new_drawing();
        app.backup.opening = true;
        add(&mut app, 50.0);
        app.backup.opening = false;
        app.backup.saved = (app.objects.clone(), app.background);
        assert!(!app.unsaved());
        add(&mut app, 100.0);
        app.backup_now();
        assert_eq!(queued(&app), [0, 1], "the old drawing, then the opened one's changes as a new entry");
    }
}
