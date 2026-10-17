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

/// An open hand, shown while Space is held over the canvas (Windows has no stock hand cursor:
/// `CursorIcon::Grab` shows the four-way move arrows).
pub fn hand_cursor() -> egui::CustomCursorImage {
    pixel_art_cursor(
        [
            "       ##       ",
            "   ## #..###    ",
            "  #..##..#..#   ",
            "  #..##..#..# # ",
            "   #..#..#..##.#",
            "   #..#..#..#..#",
            " ## #.......#..#",
            "#..##..........#",
            "#...#.........# ",
            " #............# ",
            "  #...........# ",
            "  #..........#  ",
            "   #.........#  ",
            "    #........#  ",
            "     #......#   ",
            "     #......#   ",
        ],
        [8, 8],
    )
}

/// A closed hand, shown while panning.
pub fn grabbing_cursor() -> egui::CustomCursorImage {
    pixel_art_cursor(
        [
            "                ",
            "                ",
            "                ",
            "                ",
            "    ## ## ##    ",
            "   #..#..#..##  ",
            "   #.........#.#",
            "    #..........#",
            "   ##..........#",
            "  #............#",
            "  #...........# ",
            "   #..........# ",
            "    #........#  ",
            "     #.......#  ",
            "     #......#   ",
            "     #......#   ",
        ],
        [8, 8],
    )
}

/// A cursor drawn as text: `#` black, `.` white, anything else transparent.
fn pixel_art_cursor<const N: usize>(rows: [&str; N], hotspot: [u16; 2]) -> egui::CustomCursorImage {
    let width = rows[0].len();
    let mut rgba = Vec::with_capacity(width * N * 4);
    for row in rows {
        assert_eq!(row.len(), width, "cursor rows differ in width");
        for c in row.chars() {
            rgba.extend_from_slice(match c {
                '#' => &[0, 0, 0, 255],
                '.' => &[255, 255, 255, 255],
                _ => &[0, 0, 0, 0],
            });
        }
    }
    egui::CustomCursorImage { rgba: rgba.into(), size: [width as u16, N as u16], hotspot }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hand_cursors_are_rectangular() {
        for cursor in [hand_cursor(), grabbing_cursor()] {
            assert_eq!(cursor.rgba.len(), cursor.size[0] as usize * cursor.size[1] as usize * 4);
        }
    }
}
