//! File > History…: a window (movable and resizable) with a thumbnail of every drawing in History, newest first. Clicking
//! one opens it as a new drawing (undo brings the current one back). Hovering one shows a delete
//! button, which asks on the tile first (a deleted entry can't be brought back); right-clicking one
//! offers Open and Delete. Thumbnails are loaded by the worker as their rows scroll into view.

use chrono::{DateTime, Datelike, Local};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use super::DeerawApp;
use crate::history_store::Entry;
use crate::paint::paint_checkerboard;
use crate::widgets::{Icon, movable_dialog, paint_icon};

/// Columns the window starts with; resizing it fits as many as there's room for.
const COLUMNS: usize = 4;
/// The box a thumbnail is fitted into (thumbnails are at most 256px, so they're only scaled down).
const THUMBNAIL_BOX: Vec2 = Vec2::new(176.0, 132.0);
/// Room around a tile's thumbnail and text, which the hover highlight fills.
const TILE_PADDING: f32 = 8.0;
/// Below the thumbnail: the time and the size, each a line.
const TEXT_HEIGHT: f32 = 38.0;
const TILE_GAP: f32 = 8.0;
/// The window's starting height, which it scrolls past.
const HEIGHT: f32 = 480.0;
/// The delete button in a hovered tile's top right corner.
const DELETE_SIZE: f32 = 28.0;

/// What a tile was asked to do.
enum TileAction {
    Open,
    /// Ask whether to delete it.
    Delete,
    ConfirmDelete,
    CancelDelete,
}

impl DeerawApp {
    pub(super) fn gallery_window(&mut self, ctx: &egui::Context) {
        if !self.show_history {
            return;
        }
        let tile = THUMBNAIL_BOX + Vec2::new(0.0, TEXT_HEIGHT) + Vec2::splat(2.0 * TILE_PADDING);
        let now = Local::now();
        let backup = &self.backup;
        let saving = self.preferences.history;
        let mut open = true;
        let mut action = None;
        let confirming = self.backup.confirm_delete;
        // Entries whose thumbnail isn't loaded (or is of an older save), with the save it should be of.
        let mut wanted = vec![];
        let size = Vec2::new(COLUMNS as f32 * (tile.x + TILE_GAP) - TILE_GAP, HEIGHT);
        movable_dialog(ctx, "History", &mut open, size, |ui| {
            if let Some(err) = &backup.error {
                ui.colored_label(ui.visuals().error_fg_color, err);
            }
            if !saving {
                ui.weak("Saving drawings to History is off (File > Preferences…).");
            }
            if backup.entries.is_empty() {
                ui.label("No drawings yet. They're saved here as you draw.");
                return;
            }
            ui.spacing_mut().item_spacing = Vec2::splat(TILE_GAP);
            let columns = (((ui.available_width() + TILE_GAP) / (tile.x + TILE_GAP)) as usize).max(1);
            let rows = backup.entries.len().div_ceil(columns);
            egui::ScrollArea::vertical().auto_shrink(false).show_rows(ui, tile.y, rows, |ui, rows| {
                for row in rows {
                    ui.horizontal(|ui| {
                        for entry in backup.entries.iter().skip(row * columns).take(columns) {
                            let thumbnail = backup.thumbnails.get(&entry.id);
                            if thumbnail.is_none_or(|(updated, _)| *updated < entry.updated) {
                                wanted.push((entry.id, entry.updated));
                            }
                            // An older save's thumbnail shows until the new one is in.
                            let texture = thumbnail.and_then(|(_, texture)| texture.as_ref());
                            if let Some(chosen) = tile_ui(ui, tile, entry, texture, now, confirming == Some(entry.id)) {
                                action = Some((entry.id, chosen));
                            }
                        }
                    });
                }
            });
        });
        if let Some(worker) = &self.backup.worker {
            for (id, updated) in wanted {
                let thumbnail = self.backup.thumbnails.entry(id).or_insert((updated, None));
                thumbnail.0 = updated;
                worker.thumbnail(id);
            }
            match action {
                Some((id, TileAction::Open)) => {
                    worker.load(id);
                    open = false;
                }
                Some((id, TileAction::Delete)) => self.backup.confirm_delete = Some(id),
                Some((id, TileAction::ConfirmDelete)) => {
                    worker.delete(id);
                    self.backup.confirm_delete = None;
                }
                Some((_, TileAction::CancelDelete)) => self.backup.confirm_delete = None,
                None => {}
            }
        }
        self.show_history = open && !ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if !self.show_history {
            self.backup.confirm_delete = None;
        }
    }
}

/// One entry: its thumbnail (on a checkerboard, for a transparent background) with its time and size below,
/// and while hovered a delete button. While `confirming`, the thumbnail is covered by the question whether to delete it.
fn tile_ui(
    ui: &mut egui::Ui,
    size: Vec2,
    entry: &Entry,
    texture: Option<&egui::TextureHandle>,
    now: DateTime<Local>,
    confirming: bool,
) -> Option<TileAction> {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = ui.rect_contains_pointer(rect);
    let visuals = ui.visuals().clone();
    let painter = ui.painter_at(rect);
    if hovered || confirming {
        painter.rect_filled(rect, 6.0, visuals.widgets.hovered.weak_bg_fill);
    }
    let area = Rect::from_min_size(rect.min + Vec2::splat(TILE_PADDING), THUMBNAIL_BOX);
    match texture {
        Some(texture) => {
            let size = texture.size_vec2();
            let image = Rect::from_center_size(area.center(), size * (area.size() / size).min_elem().min(1.0)).round();
            paint_checkerboard(&painter.with_clip_rect(image), image, 6.0, 230);
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            painter.image(texture.id(), image, uv, Color32::WHITE);
            painter.rect_stroke(image, 0.0, Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color), egui::StrokeKind::Outside);
        }
        None => {
            painter.rect_filled(area, 4.0, visuals.faint_bg_color);
        }
    }
    let [w, h] = entry.size;
    let text = Pos2::new(area.left(), area.bottom() + 6.0);
    painter.text(text, Align2::LEFT_TOP, time_label(entry.updated, now), FontId::proportional(14.0), visuals.text_color());
    painter.text(text + Vec2::new(0.0, 18.0), Align2::LEFT_TOP, format!("{w} × {h}"), FontId::proportional(12.0), visuals.weak_text_color());

    let mut action = None;
    if confirming {
        painter.rect_filled(area, 4.0, visuals.window_fill.gamma_multiply(0.92));
        // Registered after the tile, so its buttons get the clicks.
        let mut question = ui.new_child(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::top_down(egui::Align::Center)));
        question.add_space(area.height() / 2.0 - 30.0);
        question.label("Delete this drawing?");
        question.horizontal(|ui| {
            let buttons = 2.0 * 64.0 + ui.spacing().item_spacing.x;
            ui.add_space((area.width() - buttons) / 2.0);
            let delete = egui::Button::new(egui::RichText::new("Delete").color(Color32::WHITE)).fill(visuals.error_fg_color).min_size(Vec2::new(64.0, 0.0));
            if ui.add(delete).clicked() {
                action = Some(TileAction::ConfirmDelete);
            }
            if ui.add(egui::Button::new("Cancel").min_size(Vec2::new(64.0, 0.0))).clicked() {
                action = Some(TileAction::CancelDelete);
            }
        });
        return action;
    }
    if hovered {
        let button = Rect::from_min_size(Pos2::new(rect.right() - 4.0 - DELETE_SIZE, rect.top() + 4.0), Vec2::splat(DELETE_SIZE));
        let delete = ui.interact(button, response.id.with("delete"), Sense::click());
        let color = if delete.hovered() { visuals.error_fg_color } else { visuals.text_color() };
        painter.circle(button.center(), DELETE_SIZE / 2.0, visuals.window_fill, visuals.widgets.noninteractive.bg_stroke);
        paint_icon(&painter, Icon::Trash, button.center(), 16.0, color);
        if delete.on_hover_text("Delete from History").clicked() {
            action = Some(TileAction::Delete);
        }
    }
    response.context_menu(|ui| {
        if ui.button("Open").clicked() {
            action = Some(TileAction::Open);
        }
        if ui.button("Delete…").clicked() {
            action = Some(TileAction::Delete);
        }
    });
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Open as a new drawing");
    if response.clicked() && action.is_none() {
        action = Some(TileAction::Open);
    }
    action
}

/// When `updated` (Unix milliseconds) was, in local time: "Today 14:32", "Yesterday 09:05", "Oct 3, 18:20",
/// with the year if it isn't this one ("Dec 30 2025, 11:00").
fn time_label(updated: i64, now: DateTime<Local>) -> String {
    let Some(time) = DateTime::from_timestamp_millis(updated).map(|t| t.with_timezone(&Local)) else {
        return String::new();
    };
    let days = (now.date_naive() - time.date_naive()).num_days();
    match days {
        0 => time.format("Today %H:%M").to_string(),
        1 => time.format("Yesterday %H:%M").to_string(),
        _ if time.year() == now.year() => time.format("%b %-d, %H:%M").to_string(),
        _ => time.format("%b %-d %Y, %H:%M").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn times_are_relative_to_today() {
        let now = Local.with_ymd_and_hms(2026, 10, 29, 10, 0, 0).unwrap();
        let at = |y, mo, d, h, mi| Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap().timestamp_millis();
        assert_eq!(time_label(at(2026, 10, 29, 9, 5), now), "Today 09:05");
        assert_eq!(time_label(at(2026, 10, 28, 23, 59), now), "Yesterday 23:59");
        assert_eq!(time_label(at(2026, 10, 3, 18, 20), now), "Oct 3, 18:20");
        assert_eq!(time_label(at(2025, 12, 30, 11, 0), now), "Dec 30 2025, 11:00");
    }
}
