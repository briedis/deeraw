//! The Blur tool: pixelated rectangles attached to images.

use eframe::egui::{self, PointerButton, Pos2, Rect};

use super::{DeerawApp, Drawing, Notice};
use crate::geometry::{MIN_SIZE, crop_uv, uncropped_rect};
use crate::mosaic::{load_mosaics, source_block};
use crate::object::{Kind, Object};

impl DeerawApp {
    /// Drag a box over (part of) an image to blur that part of it. It can start outside: it
    /// blurs the first image the box touches (the topmost, if it touches several at once).
    /// A box that never touches an image just says so.
    /// Existing blurs can be selected, moved and resized as with Select; clicking elsewhere deselects.
    pub(super) fn blur_tool(&mut self, background: &egui::Response, to_canvas: impl Fn(Pos2) -> Pos2) {
        if background.clicked() {
            self.selected.clear();
        }
        let pointer = background.interact_pointer_pos().map(&to_canvas);
        if let (true, Some(p)) = (background.drag_started_by(PointerButton::Primary), pointer) {
            let origin = background.ctx.input(|i| i.pointer.press_origin()).map_or(p, &to_canvas);
            self.drawing = Some(Drawing::Blur(None, origin, p));
        }
        if let (true, Some(p), Some(Drawing::Blur(image, start, end))) =
            (background.dragged_by(PointerButton::Primary), pointer, &mut self.drawing)
        {
            *end = p;
            if image.is_none() {
                *image = image_touching(&self.objects, Rect::from_two_pos(*start, p));
            }
        }
        if background.drag_stopped() {
            if let Some(Drawing::Blur(None, ..)) = self.drawing {
                let (pos, now) = background.ctx.input(|i| (i.pointer.hover_pos(), i.time));
                self.notice = Some(Notice { text: "Blur works on images: drag over one".into(), pos, shown_at: now });
            }
            self.finish_drawing();
        }
    }

    /// Adds a blur of `rect` (canvas pixels, cut to the image's kept area) right above `image`,
    /// so annotations on the image stay on top, and selects it.
    pub(super) fn add_blur(&mut self, image: u64, rect: Rect) {
        let Some(index) = self.objects.iter().position(|o| o.id == image) else { return };
        let Kind::Image { uv, .. } = self.objects[index].kind else { return };
        let rect = rect.intersect(self.objects[index].rect);
        if rect.width().min(rect.height()) < MIN_SIZE {
            return;
        }
        let area = crop_uv(uncropped_rect(self.objects[index].rect, uv), rect);
        let id = self.next_id;
        self.next_id += 1;
        self.objects.insert(index + 1, Object { id, rect, aspect: rect.aspect_ratio(), kind: Kind::Blur { image, area, strength: self.style.blur } });
        self.selected = vec![id];
    }

    /// Moves blurs onto their images' current place (`rect` from `area`). Blurs whose image is
    /// gone stay where they are.
    pub(super) fn follow_blurs(&mut self) {
        for i in 0..self.objects.len() {
            let Kind::Blur { image, area, .. } = self.objects[i].kind else { continue };
            let Some(full) = self.full_image_rect(image) else { continue };
            self.objects[i].rect = Rect::from_min_max(full.lerp_inside(area.min.to_vec2()), full.lerp_inside(area.max.to_vec2()));
        }
    }

    /// After the selected blurs were moved or resized: their `area` from their `rect`, so
    /// `follow_blurs` keeps them there.
    pub(super) fn update_blur_areas(&mut self) {
        for i in 0..self.objects.len() {
            let Kind::Blur { image, .. } = self.objects[i].kind else { continue };
            if !self.selected.contains(&self.objects[i].id) {
                continue;
            }
            let Some(full) = self.full_image_rect(image) else { continue };
            let rect = self.objects[i].rect;
            if let Kind::Blur { area, .. } = &mut self.objects[i].kind {
                *area = crop_uv(full, rect);
            }
        }
    }

    /// Rebuilds the mosaics of images with blurs whose size changed, so blocks stay
    /// `mosaic::BLOCK` canvas pixels however the image is scaled. Not while a button is held
    /// (mid-resize the old blocks just scale along), so it runs once per resize, before that
    /// step is recorded for undo.
    pub(super) fn update_mosaics(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.pointer.any_down()) {
            return;
        }
        let blurred: Vec<u64> = self.objects.iter().filter_map(|o| o.blurred_image()).collect();
        for obj in self.objects.iter_mut().filter(|o| blurred.contains(&o.id)) {
            let Kind::Image { texture, uv, pixels, mosaics } = &mut obj.kind else { continue };
            let scale = obj.rect.width() / (uv.width() * texture.size()[0] as f32);
            let block = source_block(scale);
            if block != mosaics.block {
                *mosaics = load_mosaics(ctx, &format!("mosaic-{}", obj.id), &pixels.0, block);
            }
        }
    }

    /// Where image `id`'s whole, uncropped texture is (canvas pixels).
    fn full_image_rect(&self, id: u64) -> Option<Rect> {
        self.objects.iter().find_map(|o| match o.kind {
            Kind::Image { uv, .. } if o.id == id => Some(uncropped_rect(o.rect, uv)),
            _ => None,
        })
    }
}

/// The topmost image overlapping `rect` (canvas pixels).
fn image_touching(objects: &[Object], rect: Rect) -> Option<u64> {
    objects.iter().rev().find(|o| matches!(o.kind, Kind::Image { .. }) && o.rect.intersects(rect)).map(|o| o.id)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use eframe::egui::{ColorImage, Vec2};

    use super::*;

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
    }

    #[test]
    fn blurs_stay_on_their_image() {
        let ctx = egui::Context::default();
        let mut app = DeerawApp::default();
        app.add_image(&ctx, Arc::new(ColorImage::filled([100, 50], egui::Color32::RED)), Pos2::new(50.0, 25.0));
        let image = app.objects[0].id;
        // Cut to the image.
        app.add_blur(image, rect(50.0, 10.0, 200.0, 20.0));
        let blur = app.objects[1].id;
        assert_eq!(app.objects[1].rect, rect(50.0, 10.0, 100.0, 20.0));

        // The image moves and doubles in size: the blur goes along.
        app.objects[0].rect = rect(100.0, 100.0, 300.0, 200.0);
        app.follow_blurs();
        assert_eq!(app.objects[1].rect, rect(200.0, 120.0, 300.0, 140.0));

        // The blur itself moves: it stays where it was put.
        app.selected = vec![blur];
        app.objects[1].translate(Vec2::new(-50.0, 0.0));
        app.update_blur_areas();
        app.follow_blurs();
        assert_eq!(app.objects[1].rect, rect(150.0, 120.0, 250.0, 140.0));

        // Not on an image, or too small: nothing.
        app.add_blur(image, rect(0.0, 0.0, 10.0, 10.0));
        app.add_blur(image, rect(110.0, 110.0, 112.0, 150.0));
        assert_eq!(app.objects.len(), 2);

        // A drag from outside picks the image once it's reached.
        assert_eq!(image_touching(&app.objects, rect(0.0, 0.0, 90.0, 90.0)), None);
        assert_eq!(image_touching(&app.objects, rect(0.0, 0.0, 120.0, 120.0)), Some(image));

        // The image is now twice its size: its blocks get half as many source pixels.
        app.update_mosaics(&ctx);
        let Kind::Image { mosaics, .. } = &app.objects[0].kind else { panic!() };
        assert_eq!(mosaics.block, 6);

        // Deleting the image deletes its blur.
        app.selected = vec![image];
        app.delete_selected();
        assert!(app.objects.is_empty());
    }
}
