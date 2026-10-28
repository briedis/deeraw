//! File > History: drawings kept as PNGs (with a thumbnail each) in an SQLite database,
//! `history.db` in the settings folder. Saving and loading run on a thread of their own
//! (`Worker`), since encoding a big PNG takes a tenth of a second or more. Native only.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use eframe::egui::{self, ColorImage};
use rusqlite::{Connection, OptionalExtension, params};

use crate::export::encode_png;

/// Longest side of a thumbnail, in pixels.
const THUMBNAIL_SIDE: usize = 256;
/// How long a write waits for another Deeraw that's writing to the same database.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// An entry's details, without its images.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: i64,
    /// When it was last saved, Unix milliseconds.
    pub updated: i64,
    pub size: [usize; 2],
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    fn in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.busy_timeout(BUSY_TIMEOUT)?;
        // Writing doesn't block reading. The pragma answers with the mode it ended up in.
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS entry (
                id INTEGER PRIMARY KEY,
                created INTEGER NOT NULL,
                updated INTEGER NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                thumbnail BLOB NOT NULL,
                png BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS entry_updated ON entry (updated);",
        )?;
        Ok(Self { conn })
    }

    /// Replaces the images of entry `id`, or adds a new entry if it's `None` (or gone); returns its id.
    pub fn save(&self, id: Option<i64>, now: i64, size: [usize; 2], thumbnail: &[u8], png: &[u8]) -> rusqlite::Result<i64> {
        let [width, height] = size.map(|n| n as i64);
        if let Some(id) = id {
            let updated = self.conn.execute(
                "UPDATE entry SET updated = ?2, width = ?3, height = ?4, thumbnail = ?5, png = ?6 WHERE id = ?1",
                params![id, now, width, height, thumbnail, png],
            )?;
            if updated == 1 {
                return Ok(id);
            }
        }
        self.conn.execute(
            "INSERT INTO entry (created, updated, width, height, thumbnail, png) VALUES (?1, ?1, ?2, ?3, ?4, ?5)",
            params![now, width, height, thumbnail, png],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Deletes all but the `keep` most recently saved entries; returns how many went. Their space in the
    /// file is reused by later saves (the file doesn't shrink).
    pub fn prune(&self, keep: usize) -> rusqlite::Result<usize> {
        self.conn.execute(
            "DELETE FROM entry WHERE id NOT IN (SELECT id FROM entry ORDER BY updated DESC, id DESC LIMIT ?1)",
            [keep as i64],
        )
    }

    /// Every entry, the most recently saved first.
    pub fn entries(&self) -> rusqlite::Result<Vec<Entry>> {
        let mut query = self.conn.prepare("SELECT id, updated, width, height FROM entry ORDER BY updated DESC, id DESC")?;
        query
            .query_map([], |row| {
                Ok(Entry { id: row.get(0)?, updated: row.get(1)?, size: [row.get::<_, i64>(2)? as usize, row.get::<_, i64>(3)? as usize] })
            })?
            .collect()
    }

    pub fn png(&self, id: i64) -> rusqlite::Result<Option<Vec<u8>>> {
        self.conn.query_row("SELECT png FROM entry WHERE id = ?1", [id], |row| row.get(0)).optional()
    }

    /// Entry `id`'s thumbnail, and when it was saved (`Entry::updated`).
    pub fn thumbnail(&self, id: i64) -> rusqlite::Result<Option<(i64, Vec<u8>)>> {
        self.conn
            .query_row("SELECT updated, thumbnail FROM entry WHERE id = ?1", [id], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional()
    }
}

/// `image` scaled down so its longer side is at most `THUMBNAIL_SIDE`.
fn thumbnail(image: &ColorImage) -> ColorImage {
    let [w, h] = image.size;
    let scale = (THUMBNAIL_SIDE as f64 / w.max(h) as f64).min(1.0);
    let side = |n: usize| ((n as f64 * scale).round() as u32).max(1);
    let rgba = image::RgbaImage::from_raw(w as u32, h as u32, image.as_raw().to_vec()).expect("ColorImage is RGBA");
    // Premultiplied, which is what filtering should average anyway.
    let small = image::imageops::resize(&rgba, side(w), side(h), image::imageops::FilterType::Triangle);
    ColorImage::from_rgba_premultiplied([small.width() as usize, small.height() as usize], small.as_raw())
}

/// PNG bytes as an image, the way egui keeps them (premultiplied).
pub fn decode_png(bytes: &[u8]) -> image::ImageResult<ColorImage> {
    let rgba = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)?.to_rgba8();
    Ok(ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw()))
}

enum Job {
    /// Saves drawing number `drawing` (the app's count, see `Worker::save`).
    Save { drawing: u64, image: ColorImage, now: i64 },
    Load(i64),
    Thumbnail(i64),
}

pub enum Reply {
    /// Every entry, newest first, after opening the database and after every save.
    Entries(Vec<Entry>),
    Loaded(ColorImage),
    /// Entry `id`'s thumbnail as of `updated`.
    Thumbnail { id: i64, updated: i64, image: ColorImage },
    Failed(String),
}

/// The thread that owns the database.
pub struct Worker {
    jobs: Option<mpsc::Sender<Job>>,
    replies: mpsc::Receiver<Reply>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    /// Opens (or creates) the database at `path` on a new thread, keeping only the newest `keep`
    /// entries; `ctx` is woken for each reply.
    pub fn start(path: PathBuf, keep: usize, ctx: egui::Context) -> Self {
        let (jobs, job_rx) = mpsc::channel();
        let (reply_tx, replies) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let reply = |reply: Reply| {
                let _ = reply_tx.send(reply);
                ctx.request_repaint();
            };
            let opened = path.parent().map_or(Ok(()), std::fs::create_dir_all).map_err(|e| e.to_string());
            let store = match opened.and_then(|()| Store::open(&path).map_err(|e| e.to_string())) {
                Ok(store) => store,
                Err(err) => return reply(Reply::Failed(format!("Couldn't open {}: {err}", path.display()))),
            };
            let listed = store.prune(keep).and_then(|_| store.entries());
            reply(listed.map_or_else(|e| Reply::Failed(e.to_string()), Reply::Entries));
            // Each drawing's entry, once it has one.
            let mut entries: HashMap<u64, i64> = HashMap::new();
            for job in job_rx {
                match job {
                    Job::Save { drawing, image, now } => {
                        let saved = encode_png(&image)
                            .and_then(|png| Ok((png, encode_png(&thumbnail(&image))?)))
                            .map_err(|e| e.to_string())
                            .and_then(|(png, thumb)| {
                                store.save(entries.get(&drawing).copied(), now, image.size, &thumb, &png).map_err(|e| e.to_string())
                            })
                            .and_then(|id| {
                                entries.insert(drawing, id);
                                store.entries().map_err(|e| e.to_string())
                            });
                        reply(saved.map_or_else(|e| Reply::Failed(format!("Couldn't save to history: {e}")), Reply::Entries));
                    }
                    Job::Load(id) => {
                        let loaded = match store.png(id) {
                            Ok(Some(png)) => decode_png(&png).map_err(|e| e.to_string()),
                            Ok(None) => Err("it's no longer there".to_string()),
                            Err(err) => Err(err.to_string()),
                        };
                        reply(loaded.map_or_else(|e| Reply::Failed(format!("Couldn't open from history: {e}")), Reply::Loaded));
                    }
                    Job::Thumbnail(id) => match store.thumbnail(id) {
                        Ok(Some((updated, png))) => match decode_png(&png) {
                            Ok(image) => reply(Reply::Thumbnail { id, updated, image }),
                            Err(err) => reply(Reply::Failed(format!("Couldn't show a thumbnail: {err}"))),
                        },
                        // Deleted meanwhile; the next list leaves it out.
                        Ok(None) => {}
                        Err(err) => reply(Reply::Failed(format!("Couldn't show a thumbnail: {err}"))),
                    },
                }
            }
        });
        Self { jobs: Some(jobs), replies, thread: Some(thread) }
    }

    /// Saves drawing number `drawing` as `image`: its first save adds an entry, later ones replace it.
    pub fn save(&self, drawing: u64, image: ColorImage) {
        let now = chrono::Utc::now().timestamp_millis();
        self.send(Job::Save { drawing, image, now });
    }

    /// Loads entry `id`; it arrives as `Reply::Loaded`.
    pub fn load(&self, id: i64) {
        self.send(Job::Load(id));
    }

    /// Loads entry `id`'s thumbnail; it arrives as `Reply::Thumbnail`.
    pub fn thumbnail(&self, id: i64) {
        self.send(Job::Thumbnail(id));
    }

    fn send(&self, job: Job) {
        // If the database couldn't be opened, the thread is gone and said so already.
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(job);
        }
    }

    pub fn replies(&self) -> impl Iterator<Item = Reply> + '_ {
        self.replies.try_iter()
    }

    /// Waits for the jobs sent so far (e.g. a last save on exit) to be done.
    pub fn finish(&mut self) {
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Color32;

    /// An opaque image with a pattern, so a mix-up of pixels would show.
    fn pattern(size: [usize; 2]) -> ColorImage {
        let pixels = (0..size[0] * size[1]).map(|i| Color32::from_rgb((i % 251) as u8, (i % 13 * 19) as u8, (i / size[0]) as u8)).collect();
        ColorImage::new(size, pixels)
    }

    #[test]
    fn saving_adds_then_replaces_an_entry() {
        let store = Store::in_memory().unwrap();
        let a = store.save(None, 1000, [10, 20], b"t1", b"p1").unwrap();
        let b = store.save(None, 2000, [30, 40], b"t2", b"p2").unwrap();
        assert_ne!(a, b);
        assert_eq!(store.save(Some(a), 3000, [50, 60], b"t3", b"p3").unwrap(), a);
        assert_eq!(
            store.entries().unwrap(),
            vec![Entry { id: a, updated: 3000, size: [50, 60] }, Entry { id: b, updated: 2000, size: [30, 40] }]
        );
        assert_eq!(store.png(a).unwrap().as_deref(), Some(&b"p3"[..]));
        assert_eq!(store.thumbnail(a).unwrap(), Some((3000, b"t3".to_vec())));
        assert_eq!(store.thumbnail(b + 100).unwrap(), None);
        assert_eq!(store.png(b + 100).unwrap(), None);
        // An entry deleted meanwhile is added again.
        store.conn.execute("DELETE FROM entry WHERE id = ?1", [b]).unwrap();
        let c = store.save(Some(b), 4000, [1, 1], b"t", b"p").unwrap();
        assert_eq!(store.entries().unwrap().len(), 2);
        assert_eq!(store.entries().unwrap()[0].id, c);
    }

    #[test]
    fn pruning_keeps_the_most_recently_saved() {
        let store = Store::in_memory().unwrap();
        let ids: Vec<i64> = (0..5).map(|i| store.save(None, i, [1, 1], b"t", b"p").unwrap()).collect();
        // The oldest, saved again, is now the newest.
        store.save(Some(ids[0]), 10, [1, 1], b"t", b"p").unwrap();
        assert_eq!(store.prune(10).unwrap(), 0);
        assert_eq!(store.prune(3).unwrap(), 2);
        let kept: Vec<i64> = store.entries().unwrap().iter().map(|e| e.id).collect();
        assert_eq!(kept, [ids[0], ids[4], ids[3]]);
        assert_eq!(store.png(ids[1]).unwrap(), None);
    }

    #[test]
    fn thumbnails_fit_but_never_grow() {
        assert_eq!(thumbnail(&pattern([1024, 512])).size, [256, 128]);
        assert_eq!(thumbnail(&pattern([100, 3000])).size, [9, 256]);
        assert_eq!(thumbnail(&pattern([40, 30])).size, [40, 30]);
    }

    #[test]
    fn png_round_trips() {
        let image = pattern([300, 200]);
        assert_eq!(decode_png(&encode_png(&image).unwrap()).unwrap(), image);
        // Transparent and opaque pixels come back exactly (half-transparent ones may round).
        let mut clear = ColorImage::filled([4, 4], Color32::TRANSPARENT);
        clear.pixels[5] = Color32::RED;
        assert_eq!(decode_png(&encode_png(&clear).unwrap()).unwrap(), clear);
    }

    /// Save, update and load through the thread and a real database file, then reopen it, as a relaunch would.
    #[test]
    fn worker_saves_and_loads_across_restarts() {
        let path = std::env::temp_dir().join(format!("deeraw-history-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let next = |worker: &Worker| worker.replies.recv_timeout(Duration::from_secs(10)).expect("a reply");
        let recent = |reply| match reply {
            Reply::Entries(entries) => entries,
            Reply::Failed(err) => panic!("{err}"),
            _ => panic!("unexpected image"),
        };

        let mut worker = Worker::start(path.clone(), 10, egui::Context::default());
        assert_eq!(recent(next(&worker)), vec![]);
        worker.save(1, pattern([64, 48]));
        let first = recent(next(&worker));
        assert_eq!(first.len(), 1);
        // Drawing 1 again replaces its entry; drawing 2 gets its own.
        worker.save(1, pattern([80, 60]));
        assert_eq!(recent(next(&worker))[0].id, first[0].id);
        worker.save(2, pattern([10, 10]));
        let entries = recent(next(&worker));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1], Entry { size: [80, 60], ..entries[1].clone() });
        worker.finish();

        let mut worker = Worker::start(path.clone(), 10, egui::Context::default());
        let reopened = recent(next(&worker));
        assert_eq!(reopened.iter().map(|e| e.id).collect::<Vec<_>>(), entries.iter().map(|e| e.id).collect::<Vec<_>>());
        worker.load(first[0].id);
        match next(&worker) {
            Reply::Loaded(image) => assert_eq!(image, pattern([80, 60])),
            _ => panic!("expected the image"),
        }
        worker.thumbnail(first[0].id);
        match next(&worker) {
            Reply::Thumbnail { id, updated, image } => {
                assert_eq!((id, updated), (first[0].id, reopened[1].updated));
                assert_eq!(image, thumbnail(&pattern([80, 60])));
            }
            _ => panic!("expected the thumbnail"),
        }
        worker.load(-1);
        assert!(matches!(next(&worker), Reply::Failed(_)));
        // A new launch numbers its drawings from the start again, but they're new entries.
        worker.save(1, pattern([5, 5]));
        assert_eq!(recent(next(&worker)).len(), 3);
        worker.finish();

        // Opened with a smaller limit, only the newest are left.
        let mut worker = Worker::start(path.clone(), 2, egui::Context::default());
        let pruned = recent(next(&worker));
        assert_eq!(pruned.len(), 2);
        assert_eq!(pruned[0].size, [5, 5]);
        worker.finish();
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }
}
