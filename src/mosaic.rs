//! Pixelation for the Blur tool.

use eframe::egui::{self, Color32, ColorImage};

use crate::object::{BlurStrength, Mosaics};

/// Side of a mosaic block at the lightest strength, in canvas pixels, whatever the image's size.
pub const BLOCK: f32 = 12.0;

/// Light blocks' side in source pixels for an image shown at `scale` canvas pixels per source pixel.
pub fn source_block(scale: f32) -> usize {
    (BLOCK / scale).round().max(1.0) as usize
}

/// `image`'s mosaics with light blocks of `block` source pixels, as textures. Nearest sampling
/// keeps the blocks sharp.
pub fn load_mosaics(ctx: &egui::Context, name: &str, image: &ColorImage, block: usize) -> Mosaics {
    let textures = mosaics(image, block).map(|m| ctx.load_texture(name, m, egui::TextureOptions::NEAREST));
    Mosaics { block, textures }
}

/// `image`'s mosaic at each strength, in `BlurStrength::ALL` order, light blocks `block` source
/// pixels. The stronger ones are made from the lightest (its blocks combined), which is much
/// quicker than from the image.
fn mosaics(image: &ColorImage, block: usize) -> [ColorImage; 3] {
    let light = mosaic(image, block);
    let [_, medium, strong] = BlurStrength::ALL.map(|s| s.blocks());
    let (medium, strong) = (mosaic(&light, medium), mosaic(&light, strong));
    [light, medium, strong]
}

/// `image` shrunk so each `block`×`block` block becomes one pixel, its average color (the last
/// row and column of blocks may be cut short by the image's edge). Shown with nearest sampling
/// it's the pixelated image; the original pixels can't be recovered from it.
fn mosaic(image: &ColorImage, block: usize) -> ColorImage {
    let [w, h] = image.size;
    let size = [w.div_ceil(block), h.div_ceil(block)];
    let mut pixels = Vec::with_capacity(size[0] * size[1]);
    for by in 0..size[1] {
        for bx in 0..size[0] {
            let mut sum = [0u32; 4];
            let mut count = 0;
            for y in by * block..((by + 1) * block).min(h) {
                for x in bx * block..((bx + 1) * block).min(w) {
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
        let mut pixels: Vec<Color32> = (0..12).map(|x| if x % 2 == 0 { Color32::BLACK } else { Color32::WHITE }).collect();
        pixels.push(Color32::RED);
        let small = mosaic(&ColorImage::new([13, 1], pixels), 12);
        assert_eq!(small.size, [2, 1]);
        assert_eq!(small.pixels, [Color32::from_gray(127), Color32::RED]);
    }

    #[test]
    fn stronger_mosaics_have_bigger_blocks() {
        let sizes = mosaics(&ColorImage::filled([100, 50], Color32::RED), 12).map(|m| m.size);
        // 12, 24 and 48 pixel blocks.
        assert_eq!(sizes, [[9, 5], [5, 3], [3, 2]]);
    }

    #[test]
    fn blocks_keep_their_canvas_size() {
        assert_eq!(source_block(1.0), 12);
        assert_eq!(source_block(2.0), 6);
        assert_eq!(source_block(0.5), 24);
        assert_eq!(source_block(100.0), 1);
    }
}
