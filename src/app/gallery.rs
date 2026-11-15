//! File > History…: a window (movable and resizable) with a thumbnail of every drawing in History, newest first. Clicking
//! one opens it as a new drawing (undo brings the current one back). Hovering one shows a delete
//! button, which hides it at once with an Undo bar on top; it's deleted for good once another is, the window closes or
//! the app exits. Right-clicking one offers Open and Delete. Thumbnails are loaded by the worker as their rows scroll into view.

use chrono::{DateTime, Datelike, Local};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use super::DeerawApp;
use crate::history_store::Entry;
use crate::paint::paint_checkerboard;
use crate::widgets::{Icon, movable_dialog, paint_icon};

/// Columns the window starts with; resizing it fits as many as there's room for.
const COLUMNS: usize = 4;
/// The width of the box a thumbnail is fitted into when the window opens. Tiles stretch to fill the window's width,
/// their boxes between half and all of `MAX_THUMBNAIL_WIDTH`, at 4:3.
const START_THUMBNAIL_WIDTH: f32 = 192.0;
/// Thumbnails are at most 256px, so a wider box would only scale them up.
const MAX_THUMBNAIL_WIDTH: f32 = 256.0;
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
    Delete,
}

impl DeerawApp {
    pub(super) fn gallery_window(&mut self, ctx: &egui::Context) {
        if !self.show_history {
            return;
        }
        let now = Local::now();
        let backup = &self.backup;
        let saving = self.preferences.history;
        let mut open = true;
        let mut action = None;
        let pending = self.backup.pending_delete;
        let mut undo = false;
        // Entries whose thumbnail isn't loaded (or is of an older save), with the save it should be of.
        let mut wanted = vec![];
        let start_tile = START_THUMBNAIL_WIDTH + 2.0 * TILE_PADDING;
        let size = Vec2::new(COLUMNS as f32 * (start_tile + TILE_GAP) - TILE_GAP, HEIGHT);
        movable_dialog(ctx, "History", &mut open, size, |ui| {
            if let Some(err) = &backup.error {
                ui.colored_label(ui.visuals().error_fg_color, err);
            }
            if !saving {
                ui.weak("Saving drawings to History is off (File > Preferences…).");
            }
            if let Some(id) = pending
                && let Some(entry) = backup.entries.iter().find(|e| e.id == id)
            {
                ui.horizontal(|ui| {
                    ui.label(format!("Deleted the drawing from {}.", time_label(entry.updated, now)));
                    undo = ui.button("Undo").clicked();
                });
                ui.add_space(4.0);
            }
            let entries: Vec<&Entry> = backup.entries.iter().filter(|e| Some(e.id) != pending).collect();
            if entries.is_empty() {
                ui.label("No drawings yet. They're saved here as you draw.");
                return;
            }
            ui.spacing_mut().item_spacing = Vec2::splat(TILE_GAP);
            let width = ui.available_width();
            let max_tile = MAX_THUMBNAIL_WIDTH + 2.0 * TILE_PADDING;
            // The fewest columns whose tiles aren't wider than the largest.
            let columns = (((width + TILE_GAP) / (max_tile + TILE_GAP)).ceil() as usize).max(1);
            let tile_width = ((width + TILE_GAP) / columns as f32 - TILE_GAP).floor().max(2.0 * TILE_PADDING + 1.0);
            let thumbnail_box = Vec2::new(tile_width - 2.0 * TILE_PADDING, ((tile_width - 2.0 * TILE_PADDING) * 0.75).round());
            let tile = thumbnail_box + Vec2::new(2.0 * TILE_PADDING, TEXT_HEIGHT + 2.0 * TILE_PADDING);
            let rows = entries.len().div_ceil(columns);
            egui::ScrollArea::vertical().auto_shrink(false).show_rows(ui, tile.y, rows, |ui, rows| {
                for row in rows {
                    ui.horizontal(|ui| {
                        for &entry in entries.iter().skip(row * columns).take(columns) {
                            let thumbnail = backup.thumbnails.get(&entry.id);
                            if thumbnail.is_none_or(|(updated, _)| *updated < entry.updated) {
                                wanted.push((entry.id, entry.updated));
                            }
                            // An older save's thumbnail shows until the new one is in.
                            let texture = thumbnail.and_then(|(_, texture)| texture.as_ref());
                            if let Some(chosen) = tile_ui(ui, tile, thumbnail_box, entry, texture, now) {
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
                Some((id, TileAction::Delete)) => {
                    if let Some(previous) = self.backup.pending_delete.replace(id) {
                        worker.delete(previous);
                    }
                }
                None => {}
            }
        }
        if undo {
            self.backup.pending_delete = None;
        }
        self.show_history = open && !ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if !self.show_history
            && let Some(id) = self.backup.pending_delete.take()
            && let Some(worker) = &self.backup.worker
        {
            worker.delete(id);
        }
    }
}

/// One entry: its thumbnail (on a checkerboard, for a transparent background) with its time and size below,
/// and while hovered a delete button.
fn tile_ui(
    ui: &mut egui::Ui,
    size: Vec2,
    thumbnail_box: Vec2,
    entry: &Entry,
    texture: Option<&egui::TextureHandle>,
    now: DateTime<Local>,
) -> Option<TileAction> {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = ui.rect_contains_pointer(rect);
    let visuals = ui.visuals().clone();
    let painter = ui.painter_at(rect);
    if hovered {
        painter.rect_filled(rect, 6.0, visuals.widgets.hovered.weak_bg_fill);
    }
    let area = Rect::from_min_size(rect.min + Vec2::splat(TILE_PADDING), thumbnail_box);
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
        if ui.button("Delete").clicked() {
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
