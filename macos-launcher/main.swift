// Deeraw Launcher: a menu bar icon that opens Deeraw. Left click opens it, right click shows a menu.
import AppKit
import ServiceManagement

let deerawURL = URL(fileURLWithPath: "/Applications/Deeraw.app")

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
            button.toolTip = "Open Deeraw"
            button.target = self
            button.action = #selector(clicked)
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        }

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
            openDeeraw()
        }
    }

    @objc func openDeeraw() {
        NSWorkspace.shared.openApplication(at: deerawURL, configuration: NSWorkspace.OpenConfiguration())
    }
}

let app = NSApplication.shared
let launcher = Launcher()
app.delegate = launcher
app.setActivationPolicy(.accessory)
app.run()
