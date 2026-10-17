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

use app::DeerawApp;

/// Window/taskbar icon and the About window's logo; rendered from `assets/deer.svg`.
const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

/// Shared setup for the native and web builds.
fn create_app(cc: &eframe::CreationContext) -> Box<dyn eframe::App> {
    // Ctrl +/- zooms the canvas, not the whole UI.
    cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
    Box::new(DeerawApp::default())
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Deeraw")
            .with_inner_size([1024.0, 768.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(ICON_PNG).expect("bundled icon is a valid PNG"),
            ),
        // eframe's default (vsync, 1 queued frame) makes dragging the window choppy on Windows:
        // every move repaints and blocks on vsync. Two queued frames keep it smooth.
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration::default()
            .with_surface_config(eframe::egui_wgpu::SurfaceConfig::HIGH_THROUGHPUT),
        ..Default::default()
    };
    eframe::run_native("Deeraw", options, Box::new(|cc| Ok(create_app(cc))))
}

/// The web build runs in the `<canvas id="deeraw">` of `index.html`.
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast;

    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("deeraw"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("index.html has a <canvas id=\"deeraw\">");
        eframe::WebRunner::new()
            .start(canvas, eframe::WebOptions::default(), Box::new(|cc| Ok(create_app(cc))))
            .await
            .expect("starting the app");
    });
}
