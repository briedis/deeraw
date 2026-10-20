//! The Screenshot button: Deeraw hides, the OS lets you drag over a screen area, and the
//! capture is placed like an opened image. macOS only for now (`screencapture -i`).
//!
//! The window is ordered out and back in through AppKit, not eframe: eframe runs no frames for
//! a hidden window, so it could never show itself again. Hiding only the window (not the app)
//! keeps Deeraw the active app, so it comes back in front; macOS no longer lets an app that
//! lost activation take it back.

use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc;

use dispatch2::DispatchQueue;
use eframe::egui::{self, ColorImage};
use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;

use super::DeerawApp;

impl DeerawApp {
    /// Hides the window and starts the system's area selection on another thread. False if
    /// there's no key window yet (just after starting).
    pub(super) fn start_screenshot(&mut self, ctx: &egui::Context) -> bool {
        if self.screenshot.is_some() {
            return true;
        }
        // `ui` runs on the main thread.
        let Some(mtm) = MainThreadMarker::new() else { return false };
        let Some(window) = NSApplication::sharedApplication(mtm).keyWindow() else { return false };
        let number = window.windowNumber();
        window.orderOut(None);
        let (sender, receiver) = mpsc::channel();
        self.screenshot = Some(receiver);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let image = capture_area();
            DispatchQueue::main().exec_async(move || {
                let mtm = MainThreadMarker::new().expect("main queue");
                if let Some(window) = NSApplication::sharedApplication(mtm).windowWithWindowNumber(number) {
                    window.makeKeyAndOrderFront(None);
                }
            });
            let _ = sender.send(image);
            ctx.request_repaint();
        });
        true
    }

    /// Once the selection is done (or cancelled), hands the capture to `canvas`.
    pub(super) fn finish_screenshot(&mut self) {
        let Some(receiver) = &self.screenshot else { return };
        let image = match receiver.try_recv() {
            Ok(image) => image,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => None,
        };
        self.screenshot = None;
        self.opened = image.map(Arc::new);
    }
}

/// Runs `screencapture -i` (drag to select, Space for a window, Escape cancels) into a temporary
/// file; `None` when cancelled or it failed.
fn capture_area() -> Option<ColorImage> {
    let path = std::env::temp_dir().join(format!("deeraw-screenshot-{}.png", std::process::id()));
    let _ = std::fs::remove_file(&path);
    // -x: no camera sound.
    let status = Command::new("screencapture").args(["-i", "-x"]).arg(&path).status().ok()?;
    let image = status.success().then(|| image::open(&path).ok()).flatten();
    let _ = std::fs::remove_file(&path);
    let rgba = image?.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}
