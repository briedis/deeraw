//! Images in and out: pasting/opening (and fitting the window), saving and exporting.

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::sync::Arc;

use eframe::egui::{self, ColorImage, Pos2, Rect, Vec2};

use super::{DeerawApp, MIN_ZOOM_LEVEL, Notice, ZOOM_STEP};
use crate::export::{Export, ExportStep, ExportTarget, bounds, encode_png, export_shapes, format_bytes, render_offscreen, trim, write_png};
use crate::object::Kind;

/// Room rendered around the objects' rects when exporting, in canvas pixels, so line widths and
/// arrow heads sticking out of them aren't cut off. The image is trimmed afterwards.
const EXPORT_MARGIN: f32 = 40.0;
/// Free space around the first pasted image when the window is fitted to it, in points.
const FIT_MARGIN: f32 = 80.0;
/// The window is never fitted smaller than this (inner size, in points), so the UI still fits.
const FIT_MIN_WINDOW: Vec2 = Vec2::new(800.0, 600.0);
/// The fitted window may take at most this fraction of the screen; bigger images get zoomed out.
const FIT_MAX_SCREEN: f32 = 0.75;
/// Image files File > Open accepts (what the `image` crate is built to decode).
#[cfg(not(target_arch = "wasm32"))]
const OPEN_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "bmp", "gif", "webp"];

impl DeerawApp {
    /// Adds an image centered on `center` (canvas pixels).
    pub(super) fn add_image(&mut self, ctx: &egui::Context, image: Arc<ColorImage>, center: Pos2) {
        let texture = ctx.load_texture(
            format!("image-{}", self.next_id),
            (*image).clone(),
            egui::TextureOptions::LINEAR,
        );
        let size = Vec2::new(image.size[0] as f32, image.size[1] as f32);
        let min = (center - size / 2.0).round();
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        self.add(Rect::from_min_size(min, size), Kind::Image { texture, uv });
    }

    /// For the first image on an empty canvas: resizes the window to show `size` (canvas pixels,
    /// placed at the canvas origin) centered with `FIT_MARGIN` around it, zooming out if the window
    /// would exceed `FIT_MAX_SCREEN` of the screen, and never below `FIT_MIN_WINDOW`. Returns
    /// false (changing nothing) if the window is maximized/fullscreen or its size is unknown.
    pub(super) fn fit_window(&mut self, ctx: &egui::Context, size: Vec2, canvas_rect: Rect) -> bool {
        let (inner, monitor, locked) = ctx.input(|i| {
            let v = i.viewport();
            (v.inner_rect, v.monitor_size, v.maximized == Some(true) || v.fullscreen == Some(true))
        });
        let (Some(inner), Some(monitor)) = (inner, monitor) else { return false };
        if locked {
            return false;
        }
        let ppp = ctx.pixels_per_point();
        // Everything around the canvas (layers panel etc.) keeps its size.
        let chrome = inner.size() - canvas_rect.size();
        let max_window = monitor * FIT_MAX_SCREEN;
        let window_at = |level: i32| {
            let zoom = 1.0 + ZOOM_STEP * level as f32;
            chrome + size * zoom / ppp + Vec2::splat(2.0 * FIT_MARGIN)
        };
        self.zoom_level = (MIN_ZOOM_LEVEL..=0)
            .rev()
            .find(|&level| {
                let window = window_at(level);
                window.x <= max_window.x && window.y <= max_window.y
            })
            .unwrap_or(MIN_ZOOM_LEVEL);
        let window = window_at(self.zoom_level).max(FIT_MIN_WINDOW).round();
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(window));
        let canvas = window - chrome;
        self.pan = ((canvas - size * self.zoom() / ppp) / 2.0).round();
        true
    }

    /// Adds a pasted or opened image centered on `center` (screen), or, on an empty canvas,
    /// fits the window to it.
    pub(super) fn place_image(
        &mut self,
        ctx: &egui::Context,
        image: Arc<ColorImage>,
        center: Pos2,
        canvas_rect: Rect,
        to_canvas: impl Fn(Pos2) -> Pos2,
    ) {
        let size = Vec2::new(image.size[0] as f32, image.size[1] as f32);
        if self.objects.is_empty() && self.fit_window(ctx, size, canvas_rect) {
            self.add_image(ctx, image, (size / 2.0).to_pos2());
        } else {
            self.add_image(ctx, image, to_canvas(center));
        }
    }

    /// File > Open (Ctrl+O): picks an image file and decodes it; `canvas` adds it. Native only.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn open_file(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Open image")
            .add_filter("Images", &OPEN_EXTENSIONS);
        if let Some(dir) = &self.save_dir {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.pick_file() else { return };
        self.save_dir = path.parent().map(Path::to_path_buf);
        match image::open(&path) {
            Ok(image) => {
                let rgba = image.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                self.opened = Some(Arc::new(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw())));
            }
            Err(err) => {
                rfd::MessageDialog::new()
                    .set_level(rfd::MessageLevel::Error)
                    .set_title("Couldn't open image")
                    .set_description(format!("{}\n\n{err}", path.display()))
                    .show();
            }
        }
    }

    /// Ctrl+S: asks where to save (named after the current local time), then exports there.
    /// Native only.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn save_as(&mut self) {
        if self.objects.is_empty() {
            return;
        }
        let name = chrono::Local::now().format("%Y-%m-%d_%H%M%S.png").to_string();
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save as image")
            .add_filter("PNG image", &["png"])
            .set_file_name(name);
        if let Some(dir) = &self.save_dir {
            dialog = dialog.set_directory(dir);
        }
        let Some(mut path) = dialog.save_file() else { return };
        if path.extension().is_none() {
            path.set_extension("png");
        }
        self.save_dir = path.parent().map(Path::to_path_buf);
        self.export = Some(Export::new(self.objects.clone(), ExportTarget::File(path)));
    }

    /// Exporting renders the export's objects offscreen at 1 canvas pixel per image pixel (less only if
    /// that exceeds the GPU's texture size limit), whatever the view. Text needs its glyphs in
    /// the GPU font atlas, which is only uploaded at the end of a frame, so the first step just
    /// lays everything out and the render happens a frame later. The image is trimmed to the
    /// pixels that differ from the background.
    pub(super) fn export_step(&mut self, ctx: &egui::Context, frame: &eframe::Frame, export: Export) {
        let (Some(bounds), Some(render_state)) = (bounds(&export.objects), frame.wgpu_render_state()) else {
            return;
        };
        let region = bounds.expand(EXPORT_MARGIN);
        let max_size = render_state.device.limits().max_texture_dimension_2d as f32;
        let pixel_scale = (max_size / region.size().max_elem()).min(1.0);
        let size = (region.size() * pixel_scale).ceil();
        let ppp = ctx.pixels_per_point();
        let shapes = export_shapes(ctx, &export.objects, region, pixel_scale / ppp, size / ppp);
        if let ExportStep::LayOut = export.step {
            self.export = Some(Export { step: ExportStep::Render, ..export });
            ctx.request_repaint();
            return;
        }
        let primitives = ctx.tessellate(shapes, ppp);
        let clear = self.background.color().map_or([0.0; 4], |c| c.to_normalized_gamma_f32());
        let size = [size.x as u32, size.y as u32];
        let Some(image) = render_offscreen(render_state, &primitives, size, ppp, clear) else { return };
        // The corner is in the margin, so it's the background exactly as rendered.
        let image = trim(&image, image.pixels[0]);
        match export.target {
            ExportTarget::Clipboard => {
                // The clipboard gets its own PNG encoding; ours is just to tell its size.
                let bytes = encode_png(&image).map_or(String::new(), |png| format!(" · {}", format_bytes(png.len())));
                let [w, h] = image.size;
                let text = format!("Copied to clipboard\n{w} × {h} px{bytes}");
                let pos = ctx.input(|i| i.pointer.hover_pos());
                self.notice = Some(Notice { text, pos, shown_at: ctx.input(|i| i.time) });
                ctx.copy_image(image);
            }
            ExportTarget::File(path) => {
                if let Err(err) = write_png(&path, &image) {
                    rfd::MessageDialog::new()
                        .set_level(rfd::MessageLevel::Error)
                        .set_title("Couldn't save image")
                        .set_description(format!("{}\n\n{err}", path.display()))
                        .show();
                }
            }
        }
    }
}
