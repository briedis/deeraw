//! Moving objects up and down the stack: Bring to front/forward, Send backward/to back.

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

use super::DeerawApp;
use super::menu::{MENU_MIN_WIDTH, MENU_SEPARATOR_SPACING, menu_style};
use crate::object::Object;
use crate::widgets::menu_item;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Arrange {
    Front,
    Forward,
    Backward,
    Back,
}

impl Arrange {
    pub(super) const ALL: [Arrange; 4] = [Arrange::Front, Arrange::Forward, Arrange::Backward, Arrange::Back];

    fn label(self) -> &'static str {
        match self {
            Arrange::Front => "Bring to front",
            Arrange::Forward => "Bring forward",
            Arrange::Backward => "Send backward",
            Arrange::Back => "Send to back",
        }
    }

    /// Ctrl+] forward, Ctrl+[ backward, with Shift all the way.
    fn shortcut(self) -> KeyboardShortcut {
        match self {
            Arrange::Front => KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::CloseBracket),
            Arrange::Forward => KeyboardShortcut::new(Modifiers::COMMAND, Key::CloseBracket),
            Arrange::Backward => KeyboardShortcut::new(Modifiers::COMMAND, Key::OpenBracket),
            Arrange::Back => KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::OpenBracket),
        }
    }

    /// The arrangement a Ctrl key press asks for. With Shift, layouts like US report `{`/`}`.
    pub(super) fn from_key(key: Key, shift: bool) -> Option<Arrange> {
        match key {
            Key::CloseCurlyBracket => Some(Arrange::Front),
            Key::OpenCurlyBracket => Some(Arrange::Back),
            Key::CloseBracket if shift => Some(Arrange::Front),
            Key::CloseBracket => Some(Arrange::Forward),
            Key::OpenBracket if shift => Some(Arrange::Back),
            Key::OpenBracket => Some(Arrange::Backward),
            _ => None,
        }
    }
}

impl DeerawApp {
    /// Moves the selection up or down the stack, along with what's on a selected image (`riders`).
    pub(super) fn arrange_selected(&mut self, to: Arrange) {
        let mut ids = self.selected.clone();
        ids.extend(self.riders());
        arrange(&mut self.objects, &ids, to);
    }

    /// Whether `arrange_selected` would change anything.
    fn can_arrange(&self, to: Arrange) -> bool {
        let mut objects = self.objects.clone();
        let mut ids = self.selected.clone();
        ids.extend(self.riders());
        arrange(&mut objects, &ids, to);
        objects.iter().map(|o| o.id).ne(self.objects.iter().map(|o| o.id))
    }

    /// The menu of a right-clicked object (`menu_on`, on the frame it was clicked), which selects
    /// it unless it's selected already. Shown at the pointer, attached to the canvas background
    /// since arrows and shapes only have a widget while the pointer is near them.
    pub(super) fn context_menu(&mut self, background: &egui::Response, menu_on: Option<u64>) {
        if let Some(id) = menu_on
            && !self.selected.contains(&id)
        {
            self.selected = vec![id];
        }
        let open = menu_on.is_some().then_some(egui::SetOpenCommand::Bool(true));
        egui::Popup::context_menu(background)
            .id(egui::Id::unique("object menu"))
            .open_memory(open)
            .style(menu_style)
            .show(|ui| {
                ui.set_min_width(MENU_MIN_WIDTH);
                self.arrange_items(ui);
                ui.add(egui::Separator::default().spacing(MENU_SEPARATOR_SPACING));
                if menu_item(ui, "Duplicate", Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::D)), true) {
                    self.duplicate_selected();
                }
                if menu_item(ui, "Delete", Some(KeyboardShortcut::new(Modifiers::NONE, Key::Delete)), true) {
                    self.delete_selected();
                }
            });
    }

    /// The four arrange items, for the Edit menu and the context menu.
    pub(super) fn arrange_items(&mut self, ui: &mut egui::Ui) {
        for to in Arrange::ALL {
            if menu_item(ui, to.label(), Some(to.shortcut()), self.can_arrange(to)) {
                self.arrange_selected(to);
            }
        }
    }
}

/// Moves the objects in `ids` to the top or bottom of the stack, or one step: past the nearest
/// object they overlap (one they don't overlap wouldn't look any different). Blurs go with their
/// image and never move on their own, so they stay above it. Both the moved and the other objects
/// keep their order among themselves.
pub(super) fn arrange(objects: &mut Vec<Object>, ids: &[u64], to: Arrange) {
    let moving = |o: &Object| ids.contains(&o.blurred_image().unwrap_or(o.id));
    match to {
        Arrange::Front | Arrange::Back => {
            let (moved, rest): (Vec<Object>, Vec<Object>) = objects.drain(..).partition(moving);
            let (below, above) = if to == Arrange::Front { (rest, moved) } else { (moved, rest) };
            objects.extend(below);
            objects.extend(above);
        }
        // Each run of moved objects goes past the nearest object above (or below) it that it
        // overlaps, not past another moved one. Top run first, so a run never lands on one still to move.
        Arrange::Forward => {
            let mut i = objects.len();
            while i > 0 {
                i -= 1;
                if !moving(&objects[i]) {
                    continue;
                }
                let top = i;
                let mut bottom = top;
                while bottom > 0 && moving(&objects[bottom - 1]) {
                    bottom -= 1;
                }
                let overlaps = |o: &Object| objects[bottom..=top].iter().any(|m| m.rect.intersects(o.rect));
                let past = (top + 1..objects.len()).take_while(|&j| !moving(&objects[j])).find(|&j| overlaps(&objects[j]));
                if let Some(past) = past {
                    objects[bottom..=past].rotate_left(top - bottom + 1);
                }
                i = bottom;
            }
        }
        Arrange::Backward => {
            let mut i = 0;
            while i < objects.len() {
                if !moving(&objects[i]) {
                    i += 1;
                    continue;
                }
                let bottom = i;
                let mut top = bottom;
                while top + 1 < objects.len() && moving(&objects[top + 1]) {
                    top += 1;
                }
                let overlaps = |o: &Object| objects[bottom..=top].iter().any(|m| m.rect.intersects(o.rect));
                let past = (0..bottom).rev().take_while(|&j| !moving(&objects[j])).find(|&j| overlaps(&objects[j]));
                if let Some(past) = past {
                    objects[past..=top].rotate_right(top - bottom + 1);
                }
                i = top + 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Color32, Pos2, Rect, Vec2};

    use super::*;
    use crate::object::{BlurStrength, Kind};

    fn object(id: u64, x: f32) -> Object {
        let rect = Rect::from_min_size(Pos2::new(x, 0.0), Vec2::splat(10.0));
        let kind = Kind::Stroke { points: vec![], pressures: vec![], color: Color32::BLACK, width: 2.0, hand_drawn: false };
        Object { id, rect, aspect: 1.0, kind }
    }

    fn blur(id: u64, image: u64, x: f32) -> Object {
        let rect = Rect::from_min_size(Pos2::new(x, 0.0), Vec2::splat(10.0));
        Object { id, rect, aspect: 1.0, kind: Kind::Blur { image, area: rect, strength: BlurStrength::default() } }
    }

    fn ids(objects: &[Object]) -> Vec<u64> {
        objects.iter().map(|o| o.id).collect()
    }

    fn arranged(mut objects: Vec<Object>, selected: &[u64], to: Arrange) -> Vec<u64> {
        arrange(&mut objects, selected, to);
        ids(&objects)
    }

    #[test]
    fn front_and_back_keep_the_order_among_moved_objects() {
        let objects: Vec<Object> = (1..=5).map(|id| object(id, 0.0)).collect();
        assert_eq!(arranged(objects.clone(), &[2, 4], Arrange::Front), [1, 3, 5, 2, 4]);
        assert_eq!(arranged(objects, &[2, 4], Arrange::Back), [2, 4, 1, 3, 5]);
    }

    #[test]
    fn one_step_goes_past_the_nearest_overlapping_object() {
        // 2 doesn't overlap 1, so forward skips it and goes past 3.
        let objects = vec![object(1, 0.0), object(2, 100.0), object(3, 5.0), object(4, 0.0)];
        assert_eq!(arranged(objects.clone(), &[1], Arrange::Forward), [2, 3, 1, 4]);
        assert_eq!(arranged(objects.clone(), &[4], Arrange::Backward), [1, 2, 4, 3]);
        // Nothing to go past: unchanged.
        assert_eq!(arranged(objects.clone(), &[2], Arrange::Forward), [1, 2, 3, 4]);
        assert_eq!(arranged(objects, &[4], Arrange::Forward), [1, 2, 3, 4]);
    }

    #[test]
    fn one_step_stops_at_other_moved_objects() {
        let objects: Vec<Object> = (1..=4).map(|id| object(id, 0.0)).collect();
        assert_eq!(arranged(objects.clone(), &[1, 3], Arrange::Forward), [2, 1, 4, 3]);
        assert_eq!(arranged(objects.clone(), &[3, 4], Arrange::Forward), [1, 2, 3, 4]);
        assert_eq!(arranged(objects, &[2, 4], Arrange::Backward), [2, 1, 4, 3]);
    }

    #[test]
    fn blurs_go_with_their_image_and_stay_above_it() {
        let objects = vec![object(1, 0.0), object(2, 0.0), blur(3, 2, 0.0), object(4, 0.0)];
        assert_eq!(arranged(objects.clone(), &[2], Arrange::Back), [2, 3, 1, 4]);
        assert_eq!(arranged(objects.clone(), &[2], Arrange::Forward), [1, 4, 2, 3]);
        // A blur alone doesn't move.
        assert_eq!(arranged(objects, &[3], Arrange::Back), [1, 2, 3, 4]);
    }
}
