//! Undo/redo.

use crate::object::Object;

/// How many undo steps are kept.
const MAX_UNDO: usize = 200;

/// Undo/redo by snapshots of the object list. Whenever the canvas settles (no mouse button
/// held, nothing being drawn or typed) and differs from `saved`, `saved` goes on the undo
/// stack, so every action becomes one step without per-action bookkeeping.
#[derive(Default)]
pub struct History {
    undo: Vec<Vec<Object>>,
    redo: Vec<Vec<Object>>,
    /// The state as of the last recorded step.
    saved: Vec<Object>,
}

impl History {
    pub fn record(&mut self, objects: &[Object]) {
        if self.saved != objects {
            self.undo.push(std::mem::replace(&mut self.saved, objects.to_vec()));
            if self.undo.len() > MAX_UNDO {
                self.undo.remove(0);
            }
            self.redo.clear();
        }
    }

    /// Steps `objects` back (from `from` to `to`), saving the current state on the other stack.
    pub fn step(from: &mut Vec<Vec<Object>>, to: &mut Vec<Vec<Object>>, saved: &mut Vec<Object>, objects: &mut Vec<Object>) {
        if let Some(state) = from.pop() {
            to.push(std::mem::replace(objects, state));
            *saved = objects.clone();
        }
    }

    pub fn undo(&mut self, objects: &mut Vec<Object>) {
        Self::step(&mut self.undo, &mut self.redo, &mut self.saved, objects);
    }

    pub fn redo(&mut self, objects: &mut Vec<Object>) {
        Self::step(&mut self.redo, &mut self.undo, &mut self.saved, objects);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}


#[cfg(test)]
mod tests {
    use eframe::egui::{Color32, Pos2, Rect};

    use super::*;
    use crate::object::{ArrowHeads, Kind};

    fn arrow(id: u64) -> Object {
        let (start, end) = (Pos2::ZERO, Pos2::new(10.0, 0.0));
        Object { id, rect: Rect::from_two_pos(start, end), aspect: 1.0, kind: Kind::Arrow { start, end, color: Color32::BLACK, width: 3.0, heads: Some(ArrowHeads::default()), bindings: [None; 2] } }
    }

    #[test]
    fn undo_redo() {
        let mut history = History::default();
        let mut objects = vec![];
        history.record(&objects);
        objects.push(arrow(1));
        history.record(&objects);
        objects.push(arrow(2));
        history.record(&objects);
        history.record(&objects); // unchanged: no new step

        let ids = |objects: &[Object]| objects.iter().map(|o| o.id).collect::<Vec<_>>();
        history.undo(&mut objects);
        assert_eq!(ids(&objects), [1]);
        history.undo(&mut objects);
        assert_eq!(ids(&objects), []);
        history.undo(&mut objects); // nothing left
        assert!(!history.can_undo() && history.can_redo());
        assert_eq!(ids(&objects), []);
        history.redo(&mut objects);
        assert_eq!(ids(&objects), [1]);

        // A new change after undo drops the redo stack.
        objects.push(arrow(3));
        history.record(&objects);
        history.redo(&mut objects);
        assert_eq!(ids(&objects), [1, 3]);
    }
}
