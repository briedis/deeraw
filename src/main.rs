// Release builds are GUI-only on Windows (no console window); debug builds keep it for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cursor;
mod export;
mod geometry;
mod history;
mod mosaic;
mod object;
mod paint;
mod stroke;
mod style;
mod widgets;

use app::DeerawApp;

/// Window/taskbar icon and the About window's logo; rendered from `assets/deer.svg`.
const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");
/// Where crash reports should go.
#[cfg(not(target_arch = "wasm32"))]
const ISSUES_URL: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/issues");

/// wgpu's surface and device settings.
///
/// The surface uses eframe's `LOW_LATENCY` default (1 queued frame). `HIGH_THROUGHPUT` (2 frames)
/// made window resizing jittery on Windows: the presented frame lagged the window size and was
/// stretched to fit.
///
/// Indirect call validation is turned off because it keeps the app from starting at all on some
/// older GPUs: wgpu validates indirect draws with an internal compute pipeline built while the
/// device is created, and Metal's shader compiler fails to build it on an Intel Iris Pro
/// ("Compiler encountered an internal error"). `request_device` then fails with `Device(Lost)`
/// and `run_native` returns before a window appears. Nothing here draws indirectly, so the
/// validation only costs us that startup — egui submits ordinary indexed draws.
#[cfg(not(target_arch = "wasm32"))]
fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    use eframe::egui_wgpu::{self, wgpu};

    let mut options = egui_wgpu::WgpuConfiguration::default()
        .with_surface_config(egui_wgpu::SurfaceConfig::LOW_LATENCY);
    if let egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_setup {
        setup
            .instance_descriptor
            .flags
            .remove(wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL);
    }
    options
}

/// On a panic, besides the usual message on stderr (which release builds on Windows have no
/// console for), writes a crash report (version, OS, message with where it happened, backtrace)
/// to `crash-<time>.txt` in the settings folder and shows where, so it can be attached to an issue.
#[cfg(not(target_arch = "wasm32"))]
fn install_crash_report() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        let report = format!(
            "Deeraw {} on {} {}\n\n{info}\n\nBacktrace:\n{}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::backtrace::Backtrace::force_capture(),
        );
        let Some(dir) = eframe::storage_dir("Deeraw") else { return };
        let path = dir.join(chrono::Local::now().format("crash-%Y-%m-%d_%H%M%S.txt").to_string());
        if std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, report)).is_ok() {
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_title("Deeraw crashed")
                .set_description(format!(
                    "A crash report was saved to\n{}\n\nPlease attach it to an issue at\n{ISSUES_URL}",
                    path.display()
                ))
                .show();
        }
    }));
}

/// Shared setup for the native and web builds.
fn create_app(cc: &eframe::CreationContext) -> Box<dyn eframe::App> {
    // Ctrl +/- zooms the canvas, not the whole UI.
    cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
    Box::new(DeerawApp::new(cc.storage))
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    install_crash_report();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Deeraw")
            .with_inner_size([1024.0, 768.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(ICON_PNG).expect("bundled icon is a valid PNG"),
            ),
        wgpu_options: wgpu_options(),
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
