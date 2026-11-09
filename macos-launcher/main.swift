// Deeraw Launcher: a menu bar icon for Deeraw. Left click takes a screenshot into a new drawing, right
// click shows a menu.
import AppKit
import ServiceManagement

/// The launcher lives in Deeraw.app/Contents/Library/LoginItems.
let deerawURL = Bundle.main.bundleURL
    .deletingLastPathComponent().deletingLastPathComponent()
    .deletingLastPathComponent().deletingLastPathComponent()
/// Asks a running Deeraw to take a screenshot into a new drawing (`SCREENSHOT_NOTIFICATION` there).
let screenshotNotification = Notification.Name("com.briedis.deeraw.screenshot")
/// Deeraw's answer, once its window comes to the current desktop when activated (`READY_NOTIFICATION`).
let readyNotification = Notification.Name("com.briedis.deeraw.screenshot-ready")
let deerawBundleID = "com.briedis.deeraw"

final class Launcher: NSObject, NSApplicationDelegate {
    var item: NSStatusItem!
    let menu = NSMenu()
    /// A running Deeraw was asked for a screenshot and hasn't answered yet.
    var waiting = false

    func applicationDidFinishLaunching(_ note: Notification) {
        item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        if let button = item.button {
            let image = NSImage(named: "menubar-deer")
            image?.size = NSSize(width: 18, height: 18)
            image?.isTemplate = true
            button.image = image
            button.toolTip = "Take a screenshot in Deeraw"
            button.target = self
            button.action = #selector(clicked)
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        }

        menu.addItem(withTitle: "Take Screenshot", action: #selector(screenshot), keyEquivalent: "").target = self
        menu.addItem(withTitle: "Open Deeraw", action: #selector(openDeeraw), keyEquivalent: "").target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "Quit Launcher", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")

        DistributedNotificationCenter.default().addObserver(
            forName: readyNotification, object: nil, queue: .main
        ) { [weak self] _ in self?.activateDeeraw() }

        // Start at login. Only takes effect for an app installed in /Applications. SMAppService is
        // macOS 13+; on older systems the launcher runs only while Deeraw starts it.
        if #available(macOS 13, *), SMAppService.mainApp.status != .enabled {
            try? SMAppService.mainApp.register()
        }
    }

    @objc func clicked() {
        if NSApp.currentEvent?.type == .rightMouseUp {
            item.menu = menu
            item.button?.performClick(nil)
            item.menu = nil
        } else {
            screenshot()
        }
    }

    /// A running Deeraw gets no arguments from macOS, so it's asked by notification, and activated
    /// once it answers: activating it first would switch to the desktop its window is on. A fresh
    /// one gets --screenshot. An older Deeraw doesn't answer, so it's activated after a second anyway.
    @objc func screenshot() {
        if NSRunningApplication.runningApplications(withBundleIdentifier: deerawBundleID).isEmpty {
            open(arguments: ["--screenshot"])
            return
        }
        waiting = true
        DistributedNotificationCenter.default().postNotificationName(
            screenshotNotification, object: nil, userInfo: nil, deliverImmediately: true)
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in self?.activateDeeraw() }
    }

    func activateDeeraw() {
        guard waiting else { return }
        waiting = false
        open(arguments: [])
    }

    /// Starts Deeraw or brings it to the front, keeping its drawing.
    @objc func openDeeraw() {
        open(arguments: [])
    }

    func open(arguments: [String]) {
        let config = NSWorkspace.OpenConfiguration()
        config.arguments = arguments
        NSWorkspace.shared.openApplication(at: deerawURL, configuration: config)
    }
}

let app = NSApplication.shared
let launcher = Launcher()
app.delegate = launcher
app.setActivationPolicy(.accessory)
app.run()
