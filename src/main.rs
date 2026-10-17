// Release builds are GUI-only on Windows (no console window); debug builds keep it for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cursor;
mod export;
mod geometry;
mod history;
mod object;
mod paint;
mod stroke;
mod style;
mod widgets;

use eframe::egui;

use app::DeerawApp;

/// Window/taskbar icon and the About window's logo; rendered from `assets/deer.svg`.
const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Deeraw")
            .with_inner_size([1024.0, 768.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(ICON_PNG).expect("bundled icon is a valid PNG"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "Deeraw",
        options,
        Box::new(|cc| {
            // Ctrl +/- zooms the canvas, not the whole UI.
            cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
            Ok(Box::new(DeerawApp::default()))
        }),
    )
}
