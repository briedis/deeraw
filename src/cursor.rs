//! Custom mouse cursors for the drawing tools.

use eframe::egui;

/// A crosshair cursor for the drawing tools. (The stock Windows crosshair can be invisible over
/// the canvas.)
pub fn crosshair_cursor() -> egui::CustomCursorImage {
    const SIZE: usize = 23;
    const MID: usize = SIZE / 2;
    outlined_cursor(SIZE, [MID, MID], |x, y| x == MID || y == MID)
}

/// A text-insertion (I-beam) cursor for the Text tool; the stock one is invisible on white.
pub fn i_beam_cursor() -> egui::CustomCursorImage {
    const WIDTH: usize = 9;
    const HEIGHT: usize = 21;
    const MID: usize = WIDTH / 2;
    outlined_cursor(HEIGHT, [MID, HEIGHT / 2], |x, y| {
        let serif = (y == 1 || y == HEIGHT - 2) && (MID - 2..=MID + 2).contains(&x);
        let stem = x == MID && (1..HEIGHT - 1).contains(&y);
        serif || stem
    })
}

/// A `size`×`size` cursor: black where `ink(x, y)`, with a 1px white outline around it so it
/// shows on any background.
pub fn outlined_cursor(size: usize, hotspot: [usize; 2], ink: impl Fn(usize, usize) -> bool) -> egui::CustomCursorImage {
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let near_ink = (y.saturating_sub(1)..=(y + 1).min(size - 1))
                .any(|ny| (x.saturating_sub(1)..=(x + 1).min(size - 1)).any(|nx| ink(nx, ny)));
            let value = match (ink(x, y), near_ink) {
                (true, _) => [0, 0, 0, 255],
                (false, true) => [255, 255, 255, 255],
                (false, false) => continue,
            };
            rgba[(y * size + x) * 4..][..4].copy_from_slice(&value);
        }
    }
    egui::CustomCursorImage {
        rgba: rgba.into(),
        size: [size as u16; 2],
        hotspot: hotspot.map(|v| v as u16),
    }
}
