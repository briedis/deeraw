//! The check for a newer version: on launch (and from Help > Check for updates…) a thread asks
//! GitHub for the latest release and the answer may open the "Update available" window.
//!
//! No HTTP crate: the system's `curl` (in Windows since 10 1803, and in macOS) asks for
//! `releases/latest`, which redirects to the latest release's tag page; the tag is read from the
//! `Location` header, so there's no JSON to parse either. Microsoft Store installs don't check:
//! the Store updates them. Deeraw isn't downloaded from the release: the window points Windows users
//! to the Store and macOS users to `brew upgrade`.

use std::sync::mpsc;

use eframe::egui;

#[cfg(not(windows))]
use super::Notice;
use super::DeerawApp;
use crate::widgets::dialog;

/// The page that redirects to the latest release, `…/releases/tag/vX.Y.Z`.
const LATEST_URL: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases/latest");
/// How long "Remind me later" keeps the window from showing on launch.
const REMIND_LATER_SECONDS: i64 = 24 * 60 * 60;
const TIMEOUT_SECONDS: &str = "10";
/// Deeraw's Microsoft Store page; the Store app is where Windows users get updates.
#[cfg(windows)]
const STORE_URL: &str = "https://apps.microsoft.com/detail/9PNMW6XTFLBM";
/// macOS users install with Homebrew (`brew install --cask briedis/tap/deeraw`).
#[cfg(not(windows))]
const BREW_UPGRADE: &str = "brew upgrade --cask deeraw";

/// What the update window shows.
pub(super) enum UpdateWindow {
    /// A newer version, e.g. "0.7.0".
    Available(String),
    /// Only after a check from the menu.
    UpToDate,
    Failed,
}

/// A check in progress: the latest version (`None` if the check failed), and whether it was
/// asked for from the menu (which also reports "up to date" and failures, and ignores skipping).
pub(super) struct UpdateCheck {
    receiver: mpsc::Receiver<Option<String>>,
    manual: bool,
}

impl DeerawApp {
    /// Starts a check, unless one is running. The launch check is skipped when turned off in
    /// Preferences, for Store installs and while "Remind me later" is in effect.
    pub(super) fn check_for_updates(&mut self, ctx: &egui::Context, manual: bool) {
        if self.update_check.is_some() {
            return;
        }
        if !manual && (!self.preferences.check_for_updates || store_install() || self.update_reminder.is_some_and(|t| now() < t)) {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(latest_version());
            ctx.request_repaint();
        });
        self.update_check = Some(UpdateCheck { receiver, manual });
    }

    /// Picks up a finished check and shows the update window (`update_window`).
    pub(super) fn update_dialog(&mut self, ctx: &egui::Context) {
        if let Some(check) = &self.update_check
            && let Ok(latest) = check.receiver.try_recv()
        {
            let manual = check.manual;
            self.update_check = None;
            self.update_window = match latest {
                Some(latest) if is_newer(&latest, env!("CARGO_PKG_VERSION")) => {
                    let skipped = self.skipped_version.as_deref() == Some(latest.as_str());
                    (manual || !skipped).then_some(UpdateWindow::Available(latest))
                }
                Some(_) => manual.then_some(UpdateWindow::UpToDate),
                None => manual.then_some(UpdateWindow::Failed),
            }
            .or(self.update_window.take());
        }
        let Some(window) = &self.update_window else { return };
        let title = match window {
            UpdateWindow::Available(_) => "Update available",
            UpdateWindow::UpToDate | UpdateWindow::Failed => "Check for updates",
        };
        let mut open = true;
        let mut close = false;
        dialog(ctx, title, &mut open, |ui| {
            ui.set_min_width(300.0);
            match window {
                UpdateWindow::Available(latest) => {
                    ui.label(egui::RichText::new(format!("Deeraw {latest} is available.")).strong());
                    ui.weak(format!("You have version {}.", env!("CARGO_PKG_VERSION")));
                    ui.add_space(4.0);
                    let page = format!("{}/releases/tag/v{latest}", env!("CARGO_PKG_REPOSITORY"));
                    ui.hyperlink_to("What's new", &page);
                    ui.add_space(8.0);
                    #[cfg(windows)]
                    ui.label("Get it from the Microsoft Store.");
                    #[cfg(not(windows))]
                    {
                        ui.label("Update with Homebrew, in Terminal:");
                        ui.code(BREW_UPGRADE);
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        #[cfg(windows)]
                        if ui.button("Open Microsoft Store").clicked() {
                            ctx.open_url(egui::OpenUrl::new_tab(STORE_URL));
                            close = true;
                        }
                        #[cfg(not(windows))]
                        if ui.button("Copy command").clicked() {
                            ctx.copy_text(BREW_UPGRADE.to_string());
                            let pos = ctx.pointer_latest_pos();
                            let shown_at = ctx.input(|i| i.time);
                            self.notice = Some(Notice { text: "Copied".to_string(), pos, shown_at });
                        }
                        if ui.button("Remind me later").clicked() {
                            self.update_reminder = Some(now() + REMIND_LATER_SECONDS);
                            close = true;
                        }
                        if ui.button("Skip this version").clicked() {
                            self.skipped_version = Some(latest.clone());
                            close = true;
                        }
                    });
                }
                UpdateWindow::UpToDate => {
                    ui.label(format!("Deeraw {} is the latest version.", env!("CARGO_PKG_VERSION")));
                }
                UpdateWindow::Failed => {
                    ui.label("Couldn't reach GitHub to check for updates.");
                }
            }
            ui.add_space(4.0);
        });
        if !open || close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.update_window = None;
        }
    }
}

/// Seconds since the Unix epoch.
fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// The latest release's version ("0.7.0"), from the redirect of `LATEST_URL`.
fn latest_version() -> Option<String> {
    let mut command = std::process::Command::new("curl");
    let agent = concat!("Deeraw/", env!("CARGO_PKG_VERSION"));
    command.args(["--silent", "--head", "--max-time", TIMEOUT_SECONDS, "--user-agent", agent, LATEST_URL]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Release builds have no console, so curl would open one.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().ok()?;
    version_from_headers(&String::from_utf8_lossy(&output.stdout))
}

/// The version in a `Location: …/releases/tag/vX.Y.Z` header.
fn version_from_headers(headers: &str) -> Option<String> {
    headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if !name.trim().eq_ignore_ascii_case("location") {
            return None;
        }
        let tag = value.trim().rsplit_once("/releases/tag/")?.1;
        let version = tag.strip_prefix('v').unwrap_or(tag);
        parse_version(version).is_some().then(|| version.to_string())
    })
}

fn parse_version(version: &str) -> Option<Vec<u32>> {
    version.split('.').map(|part| part.parse().ok()).collect()
}

/// Whether `latest` is a later version than `current` (by number, so 0.10.0 is after 0.9.0).
fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

/// Installed from the Microsoft Store (running from an MSIX package), which handles updates.
#[cfg(windows)]
fn store_install() -> bool {
    use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
    use windows_sys::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
    let mut length = 0;
    // Asked with no buffer, it wants a bigger one if there is a package (else APPMODEL_ERROR_NO_PACKAGE).
    let result = unsafe { GetCurrentPackageFullName(&mut length, std::ptr::null_mut()) };
    result == ERROR_INSUFFICIENT_BUFFER
}

#[cfg(not(windows))]
fn store_install() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_version_from_the_redirect() {
        let headers = "HTTP/1.1 302 Found\r\nlocation: https://github.com/briedis/deeraw/releases/tag/v0.7.1\r\n\r\n";
        assert_eq!(version_from_headers(headers).as_deref(), Some("0.7.1"));
        assert_eq!(version_from_headers("HTTP/1.1 200 OK\r\n"), None);
        let not_a_version = "Location: https://github.com/briedis/deeraw/releases/tag/latest\r\n";
        assert_eq!(version_from_headers(not_a_version), None);
    }

    #[test]
    fn compares_versions_by_number() {
        assert!(is_newer("0.7.0", "0.6.2"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.6.2", "0.6.2"));
        assert!(!is_newer("0.6.1", "0.6.2"));
        assert!(!is_newer("junk", "0.6.2"));
    }
}
