//! A drawing as History keeps it, to open again with everything still editable: the objects as
//! JSON, with each image's pixels stored apart (`history_store`'s `image` table) under a key.

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui::{ColorImage, Rect};

use crate::object::{Binding, Kind, Object};
use crate::style::Background;

/// Bumped when the format changes in a way older readers can't follow.
pub const VERSION: u32 = 1;

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedDrawing {
    pub version: u32,
    pub background: Background,
    pub objects: Vec<SavedObject>,
}

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedObject {
    pub id: u64,
    pub rect: Rect,
    pub aspect: f32,
    pub kind: SavedKind,
}

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SavedKind {
    /// The image stored under key `image`; its texture and mosaics are made again when it's opened.
    Image { image: String, uv: Rect },
    /// Anything else, as it is.
    #[serde(untagged)]
    Other(Kind),
}

/// `objects` on `background`, with each image's pixels named by `key` (the same pixels, the same key).
pub fn save(objects: &[Object], background: Background, mut key: impl FnMut(&Arc<ColorImage>) -> String) -> SavedDrawing {
    let objects = objects
        .iter()
        .map(|obj| {
            let kind = match &obj.kind {
                Kind::Image { uv, pixels, .. } => SavedKind::Image { image: key(&pixels.0), uv: *uv },
                kind => SavedKind::Other(kind.clone()),
            };
            SavedObject { id: obj.id, rect: obj.rect, aspect: obj.aspect, kind }
        })
        .collect();
    SavedDrawing { version: VERSION, background, objects }
}

/// The keys of the images `drawing` uses.
pub fn image_keys(drawing: &SavedDrawing) -> Vec<&str> {
    let mut keys: Vec<&str> = drawing
        .objects
        .iter()
        .filter_map(|obj| match &obj.kind {
            SavedKind::Image { image, .. } => Some(image.as_str()),
            SavedKind::Other(_) => None,
        })
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// `drawing`'s objects with new ids from `next_id` on (ids are never reused, and the undo history may still
/// hold the old ones), arrows' and blurs' references following along. Images are made by `image`, from their
/// pixels and their new id; an image whose pixels are missing is left out, and so are its blurs (arrows
/// attached to it come loose).
pub fn restore(
    drawing: SavedDrawing,
    pixels: &HashMap<String, Arc<ColorImage>>,
    next_id: &mut u64,
    mut image: impl FnMut(Arc<ColorImage>, Rect, u64) -> Kind,
) -> Vec<Object> {
    let ids: HashMap<u64, u64> = drawing.objects.iter().enumerate().map(|(i, obj)| (obj.id, *next_id + i as u64)).collect();
    *next_id += drawing.objects.len() as u64;
    let mut objects: Vec<Object> = drawing
        .objects
        .into_iter()
        .filter_map(|obj| {
            let id = ids[&obj.id];
            let kind = match obj.kind {
                SavedKind::Image { image: key, uv } => image(pixels.get(&key)?.clone(), uv, id),
                SavedKind::Other(mut kind) => {
                    match &mut kind {
                        Kind::Arrow { bindings, .. } => {
                            for binding in bindings {
                                *binding = binding.and_then(|b| Some(Binding { id: *ids.get(&b.id)?, ..b }));
                            }
                        }
                        Kind::Blur { image, .. } => *image = *ids.get(image)?,
                        _ => {}
                    }
                    kind
                }
            };
            Some(Object { id, rect: obj.rect, aspect: obj.aspect, kind })
        })
        .collect();
    let images: Vec<u64> = objects.iter().filter(|o| matches!(o.kind, Kind::Image { .. })).map(|o| o.id).collect();
    objects.retain(|o| o.blurred_image().is_none_or(|i| images.contains(&i)));
    let kept: Vec<u64> = objects.iter().map(|o| o.id).collect();
    for obj in &mut objects {
        if let Kind::Arrow { bindings, .. } = &mut obj.kind {
            for binding in bindings {
                *binding = binding.filter(|b| kept.contains(&b.id));
            }
        }
    }
    objects
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Align2, Color32, Pos2, Vec2};

    use super::*;
    use crate::object::{ArrowHeads, BlurStrength, Outline, Pixels, Shape, Side};

    fn image_kind(pixels: Arc<ColorImage>, uv: Rect, id: u64) -> Kind {
        let ctx = eframe::egui::Context::default();
        let texture = ctx.load_texture(format!("image-{id}"), (*pixels).clone(), Default::default());
        let mosaics = crate::mosaic::load_mosaics(&ctx, "mosaic", &pixels, 12);
        Kind::Image { texture, uv, pixels: Pixels(pixels), mosaics }
    }

    fn object(id: u64, kind: Kind) -> Object {
        Object { id, rect: Rect::from_min_size(Pos2::new(id as f32, 2.0), Vec2::new(30.0, 20.0)), aspect: 1.5, kind }
    }

    /// One of each kind, with an arrow attached to the image and a blur on it.
    fn drawing(pixels: &Arc<ColorImage>) -> Vec<Object> {
        let uv = Rect::from_min_max(Pos2::new(0.1, 0.0), Pos2::new(1.0, 0.5));
        let outline = Outline { on: true, color: Some(Color32::RED), width: 4.0 };
        vec![
            object(3, image_kind(pixels.clone(), uv, 3)),
            object(4, Kind::Blur { image: 3, area: Rect::from_min_size(Pos2::ZERO, Vec2::splat(0.5)), strength: BlurStrength::Strong }),
            object(7, Kind::Stroke { points: vec![Pos2::ZERO, Pos2::new(0.5, 1.0)], pressures: vec![0.2, 0.9], color: Color32::BLUE, width: 3.0, hand_drawn: true }),
            object(
                8,
                Kind::Arrow {
                    start: Pos2::new(1.0, 2.0),
                    end: Pos2::new(50.0, 60.0),
                    color: Color32::BLACK,
                    width: 6.0,
                    heads: Some(ArrowHeads::default()),
                    bindings: [Some(Binding { id: 3, side: Side::Right }), None],
                    outline,
                },
            ),
            object(9, Kind::Shape { shape: Shape::Diamond, color: Color32::GREEN, width: 2.0, border: false, fill: Some(Color32::WHITE) }),
            object(
                10,
                Kind::Text {
                    text: "*Hi* there\nsecond line".into(),
                    font_px: 28.0,
                    color: Color32::WHITE,
                    align: Align2::RIGHT_BOTTOM,
                    fill: None,
                    frame: Some(Vec2::new(200.0, 50.0)),
                    outline,
                },
            ),
        ]
    }

    #[test]
    fn everything_comes_back_through_json() {
        let pixels = Arc::new(ColorImage::filled([4, 2], Color32::RED));
        let objects = drawing(&pixels);
        let saved = save(&objects, Background::Black, |_| "key".into());
        assert_eq!(image_keys(&saved), ["key"]);
        let json = serde_json::to_string(&saved).unwrap();
        let read: SavedDrawing = serde_json::from_str(&json).unwrap();
        assert_eq!(read, saved);
        assert_eq!(read.background, Background::Black);

        let mut next_id = 100;
        let images = HashMap::from([("key".to_string(), pixels.clone())]);
        let restored = restore(read, &images, &mut next_id, image_kind);
        assert_eq!(next_id, 106);
        assert_eq!(restored.iter().map(|o| o.id).collect::<Vec<_>>(), [100, 101, 102, 103, 104, 105]);
        // The same as before but for the ids (and the image's new texture).
        for (old, new) in objects.iter().zip(&restored) {
            assert_eq!((old.rect, old.aspect), (new.rect, new.aspect));
            match (&old.kind, &new.kind) {
                (Kind::Image { uv: a, pixels: p, .. }, Kind::Image { uv: b, pixels: q, .. }) => {
                    assert_eq!(a, b);
                    assert!(p == q, "the same pixels");
                }
                (Kind::Blur { area: a, strength: s, .. }, Kind::Blur { image, area: b, strength: t }) => {
                    assert_eq!((a, s, *image), (b, t, 100));
                }
                (Kind::Arrow { bindings, .. }, Kind::Arrow { bindings: new_bindings, .. }) => {
                    assert_eq!(new_bindings[0], Some(Binding { id: 100, side: Side::Right }));
                    let mut old = old.kind.clone();
                    if let Kind::Arrow { bindings: b, .. } = &mut old {
                        b[0] = new_bindings[0];
                    }
                    assert!(old == new.kind, "{bindings:?}");
                }
                (old, new) => assert!(old == new, "{old:?} changed"),
            }
        }
    }

    #[test]
    fn a_missing_image_is_left_out_with_its_blurs() {
        let pixels = Arc::new(ColorImage::filled([4, 2], Color32::RED));
        let saved = save(&drawing(&pixels), Background::White, |_| "key".into());
        let mut next_id = 0;
        let restored = restore(saved, &HashMap::new(), &mut next_id, image_kind);
        assert_eq!(restored.len(), 4);
        assert!(restored.iter().all(|o| !matches!(o.kind, Kind::Image { .. } | Kind::Blur { .. })));
        // The arrow lost what it was attached to.
        assert!(restored.iter().any(|o| matches!(o.kind, Kind::Arrow { bindings: [None, None], .. })));
    }
}
