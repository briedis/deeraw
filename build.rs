// Embeds the app icon into deeraw.exe (Explorer, taskbar pins, Alt+Tab before the window sets its own).
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/icon.ico")
            .compile()
            .expect("embedding the icon resource");
    }
}
