//! UI around the canvas: layers panel, toolbar, style panel and the corner controls.

use eframe::egui::{self, Align2, CursorIcon, Pos2, Rect, Sense, Vec2};

use super::{DeerawApp, Tool};
use crate::export::bounds;
use crate::geometry::pan_for_zoom;
use crate::object::Kind;
use crate::style::{Background, FONT_SIZES, LINE_WIDTHS, PALETTE, SizeKind, StyleSection, canvas_ui_visuals};
use crate::widgets::{Icon, align_button, align_name, color_swatch, icon_button, pen_button, size_button};

/// How long a notice shows, in seconds, including its fade-out.
const NOTICE_SECONDS: f64 = 2.5;
const NOTICE_FADE: f64 = 0.5;
/// Toolbar icons, in points.
const TOOLBAR_ICON_SIZE: f32 = 24.0;

impl DeerawApp {
    pub(super) fn layers_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Layers");
        ui.separator();
        if self.objects.is_empty() {
            ui.weak("Paste an image with Ctrl+V,\nor draw with the tools below");
        }
        let mut clicked = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            // Topmost first, like most layer panels.
            for obj in self.objects.iter().rev() {
                let selected = self.selected.contains(&obj.id);
                let response = ui.selectable_label(selected, obj.name());
                if response.clicked() {
                    clicked = Some(obj.id);
                }
                let r = obj.rect;
                let mut info = format!(
                    "pos {:.0}, {:.0}  size {:.0}×{:.0}",
                    r.min.x,
                    r.min.y,
                    r.width(),
                    r.height()
                );
                match &obj.kind {
                    Kind::Image { texture, uv } => {
                        let [w, h] = texture.size();
                        info += &format!("\nsource {w}×{h}");
                        if *uv != Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)) {
                            let crop = uv.size() * Vec2::new(w as f32, h as f32);
                            info += &format!(", cropped to {:.0}×{:.0}", crop.x, crop.y);
                        }
                    }
                    Kind::Stroke { points, width, hand_drawn, .. } => {
                        let pen = if *hand_drawn { ", hand-drawn" } else { "" };
                        info += &format!("\n{} points, width {width:.0}px{pen}", points.len())
                    }
                    Kind::Text { font_px, frame, .. } => {
                        info += &format!("\nfont {font_px:.0}px");
                        if let Some(frame) = frame {
                            info += &format!(", box {}", align_name(frame.align));
                        }
                    }
                    Kind::Arrow { width, bindings, .. } => {
                        info += &format!("\nwidth {width:.0}px");
                        for (end, binding) in ["start", "end"].iter().zip(bindings) {
                            if let Some(b) = binding {
                                info += &format!(", {end} on #{} {:?}", b.id, b.side);
                            }
                        }
                    }
                }
                ui.weak(info);
            }
        });
        if let Some(id) = clicked {
            let ctrl = ui.input(|i| i.modifiers.command);
            self.click_select(id, ctrl);
        }
    }

    /// The current notice, next to where the pointer was (or at the canvas center), for
    /// `NOTICE_SECONDS`, fading out at the end.
    pub(super) fn notice(&mut self, ui: &egui::Ui, canvas_rect: Rect) {
        let Some(notice) = &self.notice else { return };
        let age = ui.input(|i| i.time) - notice.shown_at;
        if age > NOTICE_SECONDS {
            self.notice = None;
            return;
        }
        let opacity = ((NOTICE_SECONDS - age) / NOTICE_FADE).min(1.0) as f32;
        let pos = notice.pos.filter(|p| canvas_rect.contains(*p)).unwrap_or(canvas_rect.center());
        egui::Area::new(egui::Id::unique("notice"))
            .fixed_pos(pos + Vec2::new(16.0, 16.0))
            .order(egui::Order::Tooltip)
            .interactable(false)
            .show(ui.ctx(), |ui| {
                ui.set_opacity(opacity);
                egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(&notice.text));
            });
        ui.ctx().request_repaint();
    }

    /// The window title shows the size of the drawing, from the objects' rects, so a copy
    /// (trimmed to visible pixels) can come out a little smaller.
    pub(super) fn update_title(&mut self, ctx: &egui::Context) {
        let title = match bounds(&self.objects) {
            Some(bounds) => {
                let size = bounds.size().ceil();
                format!("Deeraw ({} × {})", size.x, size.y)
            }
            None => "Deeraw".to_owned(),
        };
        if self.title.as_ref() != Some(&title) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = Some(title);
        }
    }

    /// Bottom-right of the canvas: background picker and zoom level.
    pub(super) fn corner_controls(&mut self, ui: &egui::Ui, canvas_rect: Rect, ppp: f32) {
        let area = egui::Area::new(egui::Id::unique("corner"))
            .pivot(Align2::RIGHT_BOTTOM)
            .fixed_pos(canvas_rect.right_bottom() - Vec2::splat(16.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                ui.set_visuals(canvas_ui_visuals(self.background));
                ui.horizontal(|ui| {
                    egui::Frame::popup(ui.style()).inner_margin(6).show(ui, |ui| self.background_picker(ui));
                    egui::Frame::popup(ui.style()).inner_margin(6).show(ui, |ui| {
                        // As tall as the background swatches.
                        ui.set_min_height(22.0);
                        self.zoom_indicator(ui, canvas_rect, ppp);
                    });
                });
            });
        // The zoom level's width changes, e.g. from 90% to 100%.
        redo_frame_if_resized(ui.ctx(), &area.response);
    }

    pub(super) fn background_picker(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (background, name) in [
                (Background::Transparent, "Transparent background"),
                (Background::White, "White background"),
                (Background::Black, "Black background"),
            ] {
                let clicked = color_swatch(ui, self.background == background, name, background.color());
                if clicked {
                    // Drawing with the old background's default ink switches to the new one's
                    // (black on black would be invisible); a picked color like red stays.
                    if self.style.color == self.background.ink() {
                        self.style.color = background.ink();
                    }
                    self.background = background;
                }
            }
        });
    }

    /// Just above the toolbar: the current color and line width / font size. Clicking either
    /// expands its options upwards. With a drawing tool it sets the style of new objects; with
    /// the Select tool it restyles the selected objects, each setting only those that have it
    /// (a mix of strokes and text shows both a line width and a font size).
    pub(super) fn style_panel(&mut self, ui: &egui::Ui, toolbar: Rect) {
        let targets: Vec<usize> = match self.tool {
            Tool::Select => (0..self.objects.len()).filter(|&i| self.selected.contains(&self.objects[i].id)).collect(),
            _ => vec![],
        };
        let (show_color, size_kinds) = match self.tool {
            Tool::Pencil | Tool::Arrow | Tool::Line => (true, vec![SizeKind::Line]),
            Tool::Text => (true, vec![SizeKind::Font]),
            Tool::Select => {
                let kinds: Vec<SizeKind> = targets.iter().filter_map(|&i| self.objects[i].size_mut().map(|(k, _)| k)).collect();
                let size_kinds = [SizeKind::Line, SizeKind::Font].into_iter().filter(|k| kinds.contains(k)).collect();
                (!kinds.is_empty(), size_kinds)
            }
        };
        if !show_color {
            self.style_open = None;
            return;
        }
        // Current values: the first selected object that has the setting, else the defaults.
        let color = targets
            .iter()
            .find_map(|&i| self.objects[i].color_mut().map(|c| *c))
            .unwrap_or(self.style.color);
        let sizes: Vec<(SizeKind, f32)> = size_kinds
            .iter()
            .map(|&kind| {
                let size = targets.iter().find_map(|&i| match self.objects[i].size_mut() {
                    Some((k, size)) if k == kind => Some(*size),
                    _ => None,
                });
                (kind, size.unwrap_or(*self.style.size_mut(kind)))
            })
            .collect();
        let show_pen = match self.tool {
            Tool::Pencil => true,
            Tool::Select => targets.iter().any(|&i| self.objects[i].hand_drawn_mut().is_some()),
            _ => false,
        };
        let hand_drawn = targets
            .iter()
            .find_map(|&i| self.objects[i].hand_drawn_mut().map(|h| *h))
            .unwrap_or(self.style.hand_drawn);
        let show_align = match self.tool {
            Tool::Text => true,
            Tool::Select => targets.iter().any(|&i| self.objects[i].align_mut().is_some()),
            _ => false,
        };
        let align = targets
            .iter()
            .find_map(|&i| self.objects[i].align_mut().map(|a| *a))
            .unwrap_or(self.style.align);
        let fill = targets
            .iter()
            .find_map(|&i| self.objects[i].fill_mut().map(|f| *f))
            .unwrap_or(self.style.fill);
        match self.style_open {
            Some(StyleSection::Size(kind)) if !size_kinds.contains(&kind) => self.style_open = None,
            Some(StyleSection::Align | StyleSection::Fill) if !show_align => self.style_open = None,
            Some(StyleSection::Pen) if !show_pen => self.style_open = None,
            _ => {}
        }

        let mut new_color = None;
        let mut new_size = None;
        let mut new_align = None;
        let mut new_fill = None;
        let mut new_pen = None;
        let area = egui::Area::new(egui::Id::unique("style"))
            .pivot(Align2::CENTER_BOTTOM)
            .fixed_pos(toolbar.center_top() - Vec2::new(0.0, 6.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                ui.set_visuals(canvas_ui_visuals(self.background));
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    if let Some(section) = self.style_open {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| match section {
                                StyleSection::Color => {
                                    for (swatch, name) in PALETTE {
                                        if color_swatch(ui, color == swatch, name, Some(swatch)) {
                                            new_color = Some(swatch);
                                        }
                                    }
                                }
                                StyleSection::Size(kind) => {
                                    let current = sizes.iter().find(|(k, _)| *k == kind).map(|(_, s)| *s);
                                    let options = match kind {
                                        SizeKind::Line => LINE_WIDTHS,
                                        SizeKind::Font => FONT_SIZES,
                                    };
                                    for option in options {
                                        if size_button(ui, current == Some(option), kind, option) {
                                            new_size = Some((kind, option));
                                        }
                                    }
                                }
                                StyleSection::Pen => {
                                    for option in [false, true] {
                                        if pen_button(ui, hand_drawn == option, option) {
                                            new_pen = Some(option);
                                        }
                                    }
                                }
                                StyleSection::Fill => {
                                    if color_swatch(ui, fill.is_none(), "Transparent", None) {
                                        new_fill = Some(None);
                                    }
                                    for (swatch, name) in PALETTE {
                                        if color_swatch(ui, fill == Some(swatch), name, Some(swatch)) {
                                            new_fill = Some(Some(swatch));
                                        }
                                    }
                                }
                                StyleSection::Align => {
                                    ui.vertical(|ui| {
                                        for y in [egui::Align::Min, egui::Align::Center, egui::Align::Max] {
                                            ui.horizontal(|ui| {
                                                for x in [egui::Align::Min, egui::Align::Center, egui::Align::Max] {
                                                    let option = Align2([x, y]);
                                                    if align_button(ui, align == option, option) {
                                                        new_align = Some(option);
                                                    }
                                                }
                                            });
                                        }
                                    });
                                }
                            });
                        });
                    }
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let name = PALETTE.iter().find(|(c, _)| *c == color).map_or("Color", |(_, n)| n);
                            self.style_toggle(StyleSection::Color, |open| color_swatch(ui, open, name, Some(color)));
                            for &(kind, size) in &sizes {
                                self.style_toggle(StyleSection::Size(kind), |open| match kind {
                                    SizeKind::Line => icon_button(ui, open, Icon::StrokeWidth, 14.0, &format!("Line width: {size:.0}px")),
                                    SizeKind::Font => size_button(ui, open, kind, size),
                                });
                            }
                            if show_pen {
                                self.style_toggle(StyleSection::Pen, |open| pen_button(ui, open, hand_drawn));
                            }
                            if show_align {
                                self.style_toggle(StyleSection::Align, |open| align_button(ui, open, align));
                                let name = match fill {
                                    None => "Background: transparent".to_owned(),
                                    Some(fill) => {
                                        let name = PALETTE.iter().find(|(c, _)| *c == fill).map_or("color", |(_, n)| n);
                                        format!("Background: {name}")
                                    }
                                };
                                self.style_toggle(StyleSection::Fill, |open| color_swatch(ui, open, &name, fill));
                            }
                        });
                    });
                });
            });
        redo_frame_if_resized(ui.ctx(), &area.response);

        if new_color.is_none() && new_size.is_none() && new_align.is_none() && new_fill.is_none() && new_pen.is_none() {
            return;
        }
        self.style_open = None;
        if let Some(color) = new_color {
            self.style.color = color;
            for &i in &targets {
                if let Some(c) = self.objects[i].color_mut() {
                    *c = color;
                }
            }
        }
        if let Some(fill) = new_fill {
            self.style.fill = fill;
            for &i in &targets {
                if let Some(f) = self.objects[i].fill_mut() {
                    *f = fill;
                }
            }
        }
        if let Some(pen) = new_pen {
            self.style.hand_drawn = pen;
            for &i in &targets {
                if let Some(h) = self.objects[i].hand_drawn_mut() {
                    *h = pen;
                }
            }
        }
        if let Some(align) = new_align {
            self.style.align = align;
            for &i in &targets {
                if let Some(a) = self.objects[i].align_mut() {
                    *a = align;
                }
            }
        }
        if let Some((kind, size)) = new_size {
            *self.style.size_mut(kind) = size;
            for &i in &targets {
                if let Some((k, s)) = self.objects[i].size_mut()
                    && k == kind
                {
                    *s = size;
                }
            }
        }
    }

    /// A style panel button that opens `section` (or closes it if open).
    pub(super) fn style_toggle(&mut self, section: StyleSection, button: impl FnOnce(bool) -> bool) {
        let open = self.style_open == Some(section);
        if button(open) {
            self.style_open = (!open).then_some(section);
        }
    }

    /// Shows the zoom level; clicking it resets to 100% around the canvas center.
    pub(super) fn zoom_indicator(&mut self, ui: &mut egui::Ui, canvas_rect: Rect, ppp: f32) {
        let label = format!("{:.0}%", self.zoom() * 100.0);
        let clicked = ui
            .add(egui::Label::new(label).sense(Sense::click()))
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("Reset to 100%")
            .clicked();
        if clicked && self.zoom_level != 0 {
            let old_scale = self.zoom() / ppp;
            self.zoom_level = 0;
            let center = canvas_rect.center() - canvas_rect.min;
            self.pan = pan_for_zoom(self.pan, center, old_scale, self.zoom() / ppp);
        }
    }

    /// Returns the toolbar's screen rect.
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui, canvas_rect: Rect) -> Rect {
        let area = egui::Area::new(egui::Id::unique("toolbar"))
            .pivot(Align2::CENTER_BOTTOM)
            .fixed_pos(canvas_rect.center_bottom() - Vec2::new(0.0, 16.0))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                ui.set_visuals(canvas_ui_visuals(self.background));
                egui::Frame::popup(ui.style()).inner_margin(6).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (tool, icon, hint) in [
                            (Tool::Select, Icon::Select, "Select (V)"),
                            (Tool::Pencil, Icon::Pencil, "Pencil (P)"),
                            (Tool::Arrow, Icon::Arrow, "Arrow (A)"),
                            (Tool::Line, Icon::Line, "Line (L)"),
                            (Tool::Text, Icon::Text, "Text (T)"),
                        ] {
                            if icon_button(ui, self.tool == tool, icon, TOOLBAR_ICON_SIZE, hint) {
                                self.tool = tool;
                            }
                        }
                        // Only offered while an image is selected (or being cropped).
                        if self.cropping.is_some() || self.selected_image().is_some() {
                            ui.separator();
                            if icon_button(ui, self.cropping.is_some(), Icon::Crop, TOOLBAR_ICON_SIZE, "Crop image (C, or double-click it)") {
                                self.toggle_crop();
                            }
                        }
                    });
                });
            });
        redo_frame_if_resized(ui.ctx(), &area.response);
        area.response.rect
    }
}

/// An area placed by a pivot other than its top left corner is positioned using last frame's
/// size, so for one frame after its content changes size (e.g. opening another style section)
/// it's drawn in the wrong place. This redoes that frame, now with the new size.
fn redo_frame_if_resized(ctx: &egui::Context, response: &egui::Response) {
    let id = response.id.with("last-size");
    let size = response.rect.size();
    let last = ctx.data_mut(|d| {
        let last = d.get_temp::<Vec2>(id);
        d.insert_temp(id, size);
        last
    });
    if last.is_some_and(|last| last != size) {
        ctx.request_discard("canvas panel resized");
    }
}
