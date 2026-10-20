// Deeraw Launcher: a menu bar icon that opens Deeraw. Left click opens it, right click shows a menu.
import AppKit
import ServiceManagement

let deerawURL = URL(fileURLWithPath: "/Applications/Deeraw.app")
/// A clipboard image is pasted only if it was copied this recently; an older one is probably not
/// what Deeraw was opened for, so it starts a screenshot instead.
let freshClipboardSeconds: TimeInterval = 5 * 60

final class Launcher: NSObject, NSApplicationDelegate {
    var item: NSStatusItem!
    let menu = NSMenu()
    /// macOS keeps no time for the clipboard, only a counter that goes up on every copy, so it's
    /// polled to note when it last changed (`nil`: not since the launcher started).
    var clipboardCount = NSPasteboard.general.changeCount
    var clipboardChanged: Date?

    func applicationDidFinishLaunching(_ note: Notification) {
        item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        if let button = item.button {
            let image = NSImage(named: "menubar-deer")
            image?.size = NSSize(width: 18, height: 18)
            image?.isTemplate = true
            button.image = image
            button.toolTip = "Open Deeraw"
            button.target = self
            button.action = #selector(clicked)
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        }

        menu.addItem(withTitle: "Open Deeraw", action: #selector(openDeeraw), keyEquivalent: "").target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "Quit Launcher", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")

        Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in self?.watchClipboard() }

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
            openDeeraw()
        }
    }

    func watchClipboard() {
        let count = NSPasteboard.general.changeCount
        if count != clipboardCount {
            clipboardCount = count
            clipboardChanged = Date()
        }
    }

    @objc func openDeeraw() {
        // --paste: Deeraw places the clipboard's image; --screenshot: it starts a screenshot (when
        // there's no image, or it was copied long ago). Only on a fresh start; a running Deeraw is
        // just brought to the front.
        watchClipboard()
        let hasImage = NSPasteboard.general.canReadObject(forClasses: [NSImage.self], options: nil)
        let fresh = clipboardChanged.map { Date().timeIntervalSince($0) < freshClipboardSeconds } ?? false
        let config = NSWorkspace.OpenConfiguration()
        config.arguments = [hasImage && fresh ? "--paste" : "--screenshot"]
        NSWorkspace.shared.openApplication(at: deerawURL, configuration: config)
    }
}

let app = NSApplication.shared
let launcher = Launcher()
app.delegate = launcher
app.setActivationPolicy(.accessory)
app.run()
