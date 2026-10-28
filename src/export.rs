//! Copy/Cut/Save (and History): rendering objects offscreen to an image and writing PNGs.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, ColorImage, Painter, Pos2, Rect, Vec2};
use eframe::egui_wgpu::{self, wgpu};

use crate::object::Object;
use crate::paint::paint_object;

/// Bounding box of `objects`, in canvas pixels.
pub fn bounds(objects: &[Object]) -> Option<Rect> {
    let first = objects.first()?;
    Some(objects.iter().fold(first.rect, |acc, obj| acc.union(obj.rect)))
}

/// Paints `objects` with `region` (canvas pixels) at the origin and `scale` points per
/// canvas pixel, into a throwaway layer whose shapes are returned instead of shown.
pub fn export_shapes(
    ctx: &egui::Context,
    objects: &[Object],
    region: Rect,
    scale: f32,
    size: Vec2,
) -> Vec<egui::epaint::ClippedShape> {
    let layer = egui::LayerId::new(egui::Order::Background, egui::Id::unique("export"));
    let painter = Painter::new(ctx.clone(), layer, Rect::from_min_size(Pos2::ZERO, size));
    let to_screen_pos = |p: Pos2| Pos2::ZERO + (p - region.min) * scale;
    for obj in objects {
        let rect = Rect::from_min_max(to_screen_pos(obj.rect.min), to_screen_pos(obj.rect.max));
        paint_object(&painter, obj, objects, rect, to_screen_pos, scale);
    }
    ctx.graphics_mut(|graphics| {
        let list = std::mem::take(graphics.entry(layer));
        list.all_entries().cloned().collect()
    })
}

pub struct Export {
    pub step: ExportStep,
    pub target: ExportTarget,
    /// What to render, taken when the export was requested (Cut removes them right away).
    pub objects: Vec<Object>,
}

impl Export {
    pub fn new(objects: Vec<Object>, target: ExportTarget) -> Self {
        Self { step: ExportStep::LayOut, target, objects }
    }
}

pub enum ExportStep {
    LayOut,
    Render,
}

pub enum ExportTarget {
    Clipboard,
    /// Save as; the web build has no file dialog to ask for one.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    File(PathBuf),
    /// Saving drawing number `.0` to File > History (`app/backup.rs`).
    #[cfg(not(target_arch = "wasm32"))]
    History(u64),
}

/// Writes `image` (premultiplied, as egui keeps it) as an RGBA PNG.
pub fn write_png(path: &Path, image: &ColorImage) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::write(path, encode_png(image)?)?;
    Ok(())
}

/// `image` (premultiplied, as egui keeps it) as RGBA PNG bytes.
pub fn encode_png(image: &ColorImage) -> Result<Vec<u8>, png::EncodingError> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, image.size[0] as u32, image.size[1] as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let data: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_srgba_unmultiplied()).collect();
    writer.write_image_data(&data)?;
    writer.finish()?;
    Ok(bytes)
}

/// A byte count for people: "512 B", "245 KB", "1.2 MB".
pub fn format_bytes(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    match bytes as f64 {
        b if b < KB => format!("{bytes} B"),
        b if b < KB * KB => format!("{:.0} KB", b / KB),
        b => format!("{:.1} MB", b / (KB * KB)),
    }
}

/// Crops `image` to the pixels that aren't `background`; unchanged if there are none.
pub fn trim(image: &ColorImage, background: Color32) -> ColorImage {
    let [width, height] = image.size;
    let visible = |x: usize, y: usize| image.pixels[y * width + x] != background;
    let rows: Vec<usize> = (0..height).filter(|&y| (0..width).any(|x| visible(x, y))).collect();
    let cols: Vec<usize> = (0..width).filter(|&x| (0..height).any(|y| visible(x, y))).collect();
    let (Some(&top), Some(&bottom), Some(&left), Some(&right)) = (rows.first(), rows.last(), cols.first(), cols.last())
    else {
        return image.clone();
    };
    let pixels = (top..=bottom)
        .flat_map(|y| image.pixels[y * width + left..=y * width + right].iter().copied())
        .collect();
    ColorImage::new([right - left + 1, bottom - top + 1], pixels)
}

/// Renders tessellated shapes into an offscreen texture with egui's own wgpu renderer (so it
/// shares the font atlas and image textures) and reads it back. `size` is in pixels.
pub fn render_offscreen(
    render_state: &egui_wgpu::RenderState,
    primitives: &[egui::ClippedPrimitive],
    size: [u32; 2],
    pixels_per_point: f32,
    clear: [f32; 4],
) -> Option<ColorImage> {
    let egui_wgpu::RenderState { device, queue, target_format, .. } = render_state;
    // The renderer's pipeline is built for the window's format; egui renders gamma-space RGBA/BGRA.
    let to_rgba = match target_format {
        wgpu::TextureFormat::Rgba8Unorm => [0, 1, 2, 3],
        wgpu::TextureFormat::Bgra8Unorm => [2, 1, 0, 3],
        _ => return None,
    };
    let extent = wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("deeraw_export"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: *target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let row_bytes = wgpu::util::align_to(size[0] * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("deeraw_export"),
        size: (row_bytes * size[1]) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: size, pixels_per_point };
    let mut encoder = device.create_command_encoder(&Default::default());
    let mut renderer = render_state.renderer.write();
    let mut commands = renderer.update_buffers(device, queue, &mut encoder, primitives, &screen);
    let [r, g, b, a] = clear.map(f64::from);
    let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("deeraw_export"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    renderer.render(&mut pass.forget_lifetime(), primitives, &screen);
    drop(renderer);
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: None,
            },
        },
        extent,
    );
    commands.push(encoder.finish());
    queue.submit(commands);

    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).ok()?;
    let data = buffer.slice(..).get_mapped_range().ok()?;
    let pixels = data
        .chunks(row_bytes as usize)
        .flat_map(|row| row[..size[0] as usize * 4].chunks(4))
        .map(|c| Color32::from_rgba_premultiplied(c[to_rgba[0]], c[to_rgba[1]], c[to_rgba[2]], c[to_rgba[3]]))
        .collect();
    Some(ColorImage::new([size[0] as usize, size[1] as usize], pixels))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_keeps_only_non_background_pixels() {
        let (o, x) = (Color32::WHITE, Color32::RED);
        #[rustfmt::skip]
        let pixels = vec![
            o, o, o, o,
            o, x, o, o,
            o, o, x, o,
            o, o, o, o,
        ];
        let trimmed = trim(&ColorImage::new([4, 4], pixels), o);
        assert_eq!(trimmed.size, [2, 2]);
        assert_eq!(trimmed.pixels, [x, o, o, x]);

        let blank = ColorImage::new([2, 1], vec![o, o]);
        assert_eq!(trim(&blank, o).size, [2, 1]);
    }

    #[test]
    fn byte_counts_read_well() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(250_000), "244 KB");
        assert_eq!(format_bytes(1_300_000), "1.2 MB");
    }

    #[test]
    fn png_is_written_unmultiplied() {
        let pixels = vec![Color32::from_rgba_unmultiplied(255, 0, 0, 128), Color32::TRANSPARENT];
        let path = std::env::temp_dir().join("deeraw-test.png");
        write_png(&path, &ColorImage::new([2, 1], pixels)).unwrap();
        let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut data = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut data).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(reader.info().size(), (2, 1));
        assert_eq!(data, [255, 0, 0, 128, 0, 0, 0, 0]);
    }
}
