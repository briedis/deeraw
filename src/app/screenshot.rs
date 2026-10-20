//! The Screenshot button: Deeraw gets out of the way, the OS lets you drag over a screen area, and
//! the capture is placed like an opened image. The selection runs on another thread, which sends
//! the capture (or `None` if cancelled) to `finish_screenshot`.
//!
//! macOS: `screencapture -i`. The window is ordered out and back in through AppKit, not eframe:
//! eframe runs no frames for a hidden window, so it could never show itself again. Hiding only the
//! window (not the app) keeps Deeraw the active app, so it comes back in front; macOS no longer
//! lets an app that lost activation take it back.
//!
//! Windows: the window is minimized and Snipping Tool's area selection (`ms-screenclip:`, as with
//! Win+Shift+S) is opened; it has no result but the clipboard, so the thread waits for a new image
//! there. The selection takes the foreground while open, so once something else has it again
//! without a new image, it was cancelled. The window is restored from the thread through Win32,
//! as eframe may run no frames while minimized.

use std::sync::Arc;
use std::sync::mpsc;

use super::DeerawApp;

impl DeerawApp {
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

#[cfg(target_os = "macos")]
mod platform {
    use std::process::Command;
    use std::sync::mpsc;

    use dispatch2::DispatchQueue;
    use eframe::egui::{self, ColorImage};
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    use super::super::DeerawApp;

    impl DeerawApp {
        /// Hides the window and starts the system's area selection on another thread. False if
        /// there's no key window yet (just after starting).
        pub(in crate::app) fn start_screenshot(&mut self, ctx: &egui::Context) -> bool {
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
}

#[cfg(windows)]
mod platform {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use eframe::egui::{self, ColorImage};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber;
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, IsIconic, SW_MINIMIZE, SW_RESTORE, SW_SHOWNORMAL,
        SetForegroundWindow, ShowWindow,
    };

    use super::super::DeerawApp;

    /// Time for the minimize animation, so the selection doesn't freeze the screen with Deeraw on it.
    const MINIMIZE_WAIT: Duration = Duration::from_millis(300);
    /// How often the thread checks the clipboard and the foreground window.
    const POLL: Duration = Duration::from_millis(50);
    /// Given up if the selection hasn't taken the foreground by then (Snipping Tool missing).
    const OPEN_TIMEOUT: Duration = Duration::from_secs(5);
    /// After the selection closes, how long its image may take to reach the clipboard.
    const CLIPBOARD_WAIT: Duration = Duration::from_secs(1);

    impl DeerawApp {
        /// Minimizes the window and starts Snipping Tool's area selection, waited for on another
        /// thread. False if the window isn't active yet (just after starting).
        pub(in crate::app) fn start_screenshot(&mut self, ctx: &egui::Context) -> bool {
            if self.screenshot.is_some() {
                return true;
            }
            // `ui` runs on the window's thread, so this is Deeraw's window while it's active.
            let window = unsafe { GetActiveWindow() };
            if window.is_null() {
                return false;
            }
            unsafe { ShowWindow(window, SW_MINIMIZE) };
            let (sender, receiver) = mpsc::channel();
            self.screenshot = Some(receiver);
            let ctx = ctx.clone();
            // HWND is a pointer, which isn't Send; it's only a handle.
            let window = window as usize;
            std::thread::spawn(move || {
                let window = window as HWND;
                let image = capture_area(window);
                bring_back(window);
                let _ = sender.send(image);
                ctx.request_repaint();
            });
            true
        }
    }

    /// Restores the window (unless that was done by hand) and brings it to the front. Windows lets
    /// only the foreground app take the foreground, which is the window the selection gave it back
    /// to, so this thread shares that one's input state meanwhile.
    fn bring_back(window: HWND) {
        unsafe {
            let foreground = GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
            let this = GetCurrentThreadId();
            let attached = foreground != 0 && foreground != this && AttachThreadInput(this, foreground, 1) != 0;
            if IsIconic(window) != 0 {
                ShowWindow(window, SW_RESTORE);
            }
            SetForegroundWindow(window);
            if attached {
                AttachThreadInput(this, foreground, 0);
            }
        }
    }

    /// Opens the area selection and waits for its image on the clipboard; `None` when it was
    /// cancelled, Snipping Tool didn't open, or Deeraw was brought back by hand.
    fn capture_area(window: HWND) -> Option<ColorImage> {
        std::thread::sleep(MINIMIZE_WAIT);
        let mut clipboard = arboard::Clipboard::new().ok()?;
        let sequence = unsafe { GetClipboardSequenceNumber() };
        let before = unsafe { GetForegroundWindow() };
        let uri: Vec<u16> = "ms-screenclip:\0".encode_utf16().collect();
        let open: Vec<u16> = "open\0".encode_utf16().collect();
        unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                open.as_ptr(),
                uri.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        let started = Instant::now();
        let mut selection = None;
        let mut closed = None;
        loop {
            std::thread::sleep(POLL);
            // Several formats may be written one by one, so this retries until an image reads.
            if unsafe { GetClipboardSequenceNumber() } != sequence {
                if let Ok(image) = clipboard.get_image() {
                    let size = [image.width, image.height];
                    return Some(ColorImage::from_rgba_unmultiplied(size, &image.bytes));
                }
            }
            if unsafe { IsIconic(window) } == 0 {
                return None;
            }
            let foreground = unsafe { GetForegroundWindow() };
            match selection {
                None if foreground != before && !foreground.is_null() => selection = Some(foreground),
                None if started.elapsed() > OPEN_TIMEOUT => return None,
                None => {}
                Some(selection) if foreground != selection => {
                    if closed.get_or_insert_with(Instant::now).elapsed() > CLIPBOARD_WAIT {
                        return None;
                    }
                }
                Some(_) => {}
            }
        }
    }
}
