//! Pixelation for the Blur tool.

use eframe::egui::{Color32, ColorImage};

/// Side of a mosaic block, in source image pixels.
pub const BLOCK: usize = 12;

/// `image` shrunk so each `BLOCK`×`BLOCK` block becomes one pixel, its average color (the last
/// row and column of blocks may be cut short by the image's edge). Shown with nearest sampling
/// it's the pixelated image; the original pixels can't be recovered from it.
pub fn mosaic(image: &ColorImage) -> ColorImage {
    let [w, h] = image.size;
    let size = [w.div_ceil(BLOCK), h.div_ceil(BLOCK)];
    let mut pixels = Vec::with_capacity(size[0] * size[1]);
    for by in 0..size[1] {
        for bx in 0..size[0] {
            let mut sum = [0u32; 4];
            let mut count = 0;
            for y in by * BLOCK..((by + 1) * BLOCK).min(h) {
                for x in bx * BLOCK..((bx + 1) * BLOCK).min(w) {
                    for (s, c) in sum.iter_mut().zip(image.pixels[y * w + x].to_array()) {
                        *s += c as u32;
                    }
                    count += 1;
                }
            }
            let [r, g, b, a] = sum.map(|s| (s / count) as u8);
            pixels.push(Color32::from_rgba_premultiplied(r, g, b, a));
        }
    }
    ColorImage::new(size, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_become_their_average() {
        // 13×1: one full block of alternating black and white, then a lone red pixel.
        let mut pixels: Vec<Color32> = (0..BLOCK).map(|x| if x % 2 == 0 { Color32::BLACK } else { Color32::WHITE }).collect();
        pixels.push(Color32::RED);
        let small = mosaic(&ColorImage::new([BLOCK + 1, 1], pixels));
        assert_eq!(small.size, [2, 1]);
        assert_eq!(small.pixels, [Color32::from_gray(127), Color32::RED]);
    }
}
