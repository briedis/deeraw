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

final class Launcher: NSObject, NSApplicationDelegate {
    var item: NSStatusItem!
    let menu = NSMenu()

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

        // Start at login. Only takes effect for an app installed in /Applications.
        if SMAppService.mainApp.status != .enabled {
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

    /// A running Deeraw gets no arguments from macOS, so it's asked by notification; a fresh one
    /// gets --screenshot (and misses the notification, posted before it started).
    @objc func screenshot() {
        DistributedNotificationCenter.default().postNotificationName(
            screenshotNotification, object: nil, userInfo: nil, deliverImmediately: true)
        open(arguments: ["--screenshot"])
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
