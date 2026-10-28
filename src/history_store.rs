//! File > History: drawings kept in an SQLite database, `history.db` in the settings folder: each
//! entry's objects (`drawing.rs`, so it opens editable), flattened PNG and thumbnail, and the images
//! the entries use, each stored once. Saving and loading run on a thread of their own
//! (`Worker`), since encoding a big PNG takes a tenth of a second or more. Native only.

use std::collections::HashMap;
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::Duration;

use eframe::egui::{self, ColorImage};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::drawing::{self, SavedDrawing};
use crate::export::encode_png;
use crate::object::Object;
use crate::style::Background;

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

/// What an entry is saved as.
pub struct Saved<'a> {
    pub size: [usize; 2],
    pub thumbnail: &'a [u8],
    /// The drawing flattened (as Ctrl+C copies it), for entries that can't be opened editable.
    pub png: &'a [u8],
    /// A `drawing::SavedDrawing` as JSON, and the keys of the images it uses.
    pub drawing: Option<(&'a str, &'a [String])>,
    /// Images the drawing uses that aren't stored yet (`Store::has_image`): key and PNG.
    pub new_images: &'a [(String, Vec<u8>)],
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    fn in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    /// Creates the tables, or brings older ones up to date (`PRAGMA user_version`).
    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.busy_timeout(BUSY_TIMEOUT)?;
        // Writing doesn't block reading. The pragma answers with the mode it ended up in.
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        // Immediate, so another Deeraw opening it at the same time waits and then finds it done.
        let tx = Transaction::new_unchecked(&conn, TransactionBehavior::Immediate)?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version < 1 {
            // Version 0 kept only the flattened image; 1 adds the drawing and its images, stored once
            // however many entries use them.
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS entry (
                    id INTEGER PRIMARY KEY,
                    created INTEGER NOT NULL,
                    updated INTEGER NOT NULL,
                    width INTEGER NOT NULL,
                    height INTEGER NOT NULL,
                    thumbnail BLOB NOT NULL,
                    png BLOB NOT NULL
                );
                CREATE INDEX IF NOT EXISTS entry_updated ON entry (updated);
                ALTER TABLE entry ADD COLUMN drawing TEXT;
                CREATE TABLE image (key TEXT PRIMARY KEY, png BLOB NOT NULL);
                CREATE TABLE entry_image (entry INTEGER NOT NULL, image TEXT NOT NULL, PRIMARY KEY (entry, image));
                CREATE INDEX entry_image_image ON entry_image (image);
                PRAGMA user_version = 1;",
            )?;
        }
        tx.commit()?;
        Ok(Self { conn })
    }

    /// Saves entry `id`, or adds a new entry if it's `None` (or gone); returns its id.
    pub fn save(&self, id: Option<i64>, now: i64, saved: &Saved) -> rusqlite::Result<i64> {
        let tx = self.conn.unchecked_transaction()?;
        for (key, png) in saved.new_images {
            tx.execute("INSERT OR IGNORE INTO image (key, png) VALUES (?1, ?2)", params![key, png])?;
        }
        let [width, height] = saved.size.map(|n| n as i64);
        let (drawing, images) = saved.drawing.unzip();
        let updated = match id {
            Some(id) => tx.execute(
                "UPDATE entry SET updated = ?2, width = ?3, height = ?4, thumbnail = ?5, png = ?6, drawing = ?7 WHERE id = ?1",
                params![id, now, width, height, saved.thumbnail, saved.png, drawing],
            )?,
            None => 0,
        };
        let id = match id {
            Some(id) if updated == 1 => id,
            _ => {
                tx.execute(
                    "INSERT INTO entry (created, updated, width, height, thumbnail, png, drawing) VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6)",
                    params![now, width, height, saved.thumbnail, saved.png, drawing],
                )?;
                tx.last_insert_rowid()
            }
        };
        tx.execute("DELETE FROM entry_image WHERE entry = ?1", [id])?;
        for key in images.unwrap_or_default() {
            tx.execute("INSERT OR IGNORE INTO entry_image (entry, image) VALUES (?1, ?2)", params![id, key])?;
        }
        Self::collect_garbage(&tx)?;
        tx.commit()?;
        Ok(id)
    }

    /// Deletes the images no entry uses anymore.
    fn collect_garbage(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "DELETE FROM entry_image WHERE entry NOT IN (SELECT id FROM entry);
            DELETE FROM image WHERE key NOT IN (SELECT image FROM entry_image);",
        )
    }

    pub fn has_image(&self, key: &str) -> rusqlite::Result<bool> {
        self.conn.query_row("SELECT EXISTS (SELECT 1 FROM image WHERE key = ?1)", [key], |row| row.get(0))
    }

    pub fn image(&self, key: &str) -> rusqlite::Result<Option<Vec<u8>>> {
        self.conn.query_row("SELECT png FROM image WHERE key = ?1", [key], |row| row.get(0)).optional()
    }

    /// Deletes all but the `keep` most recently saved entries; returns how many went. Their space in the
    /// file is reused by later saves (the file doesn't shrink).
    pub fn prune(&self, keep: usize) -> rusqlite::Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let deleted = tx.execute(
            "DELETE FROM entry WHERE id NOT IN (SELECT id FROM entry ORDER BY updated DESC, id DESC LIMIT ?1)",
            [keep as i64],
        )?;
        Self::collect_garbage(&tx)?;
        tx.commit()?;
        Ok(deleted)
    }

    pub fn delete(&self, id: i64) -> rusqlite::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM entry WHERE id = ?1", [id])?;
        Self::collect_garbage(&tx)?;
        tx.commit()
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

    /// Entry `id`'s flattened PNG and its drawing's JSON (none in entries from before it was kept).
    pub fn load(&self, id: i64) -> rusqlite::Result<Option<(Vec<u8>, Option<String>)>> {
        self.conn.query_row("SELECT png, drawing FROM entry WHERE id = ?1", [id], |row| Ok((row.get(0)?, row.get(1)?))).optional()
    }

    /// Entry `id`'s thumbnail, and when it was saved (`Entry::updated`).
    pub fn thumbnail(&self, id: i64) -> rusqlite::Result<Option<(i64, Vec<u8>)>> {
        self.conn
            .query_row("SELECT updated, thumbnail FROM entry WHERE id = ?1", [id], |row| Ok((row.get(0)?, row.get(1)?)))
            .optional()
    }
}

/// What identifies an image's pixels in the `image` table: its size and a hash of them.
fn image_key(image: &ColorImage) -> String {
    let mut hasher = std::hash::DefaultHasher::new();
    hasher.write(image.as_raw());
    let [w, h] = image.size;
    format!("{w}x{h}-{:016x}", hasher.finish())
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
    Save { drawing: u64, image: ColorImage, objects: Vec<Object>, background: Background, now: i64 },
    Load(i64),
    Thumbnail(i64),
    Delete(i64),
}

pub enum Reply {
    /// Every entry, newest first, after opening the database and after every save.
    Entries(Vec<Entry>),
    Loaded(Loaded),
    /// Entry `id`'s thumbnail as of `updated`.
    Thumbnail { id: i64, updated: i64, image: ColorImage },
    Failed(String),
}

/// An opened entry.
pub struct Loaded {
    /// The drawing flattened.
    pub image: ColorImage,
    /// The drawing, with the images it uses by key; none if it wasn't kept (older entries) or can't be read.
    pub drawing: Option<(SavedDrawing, HashMap<String, Arc<ColorImage>>)>,
}

/// Saves `objects` (on `background`, flattened as `image`) as entry `id`, or a new entry; returns its id.
/// The images in it are hashed (`image_key`), and only those not stored yet are encoded.
fn save_entry(store: &Store, id: Option<i64>, now: i64, image: &ColorImage, objects: &[Object], background: Background) -> Result<i64, String> {
    let mut images: Vec<(String, Arc<ColorImage>)> = vec![];
    let drawing = drawing::save(objects, background, |pixels| {
        let key = image_key(pixels);
        images.push((key.clone(), pixels.clone()));
        key
    });
    let json = serde_json::to_string(&drawing).map_err(|e| e.to_string())?;
    images.sort_by(|a, b| a.0.cmp(&b.0));
    images.dedup_by(|a, b| a.0 == b.0);
    let mut new_images = vec![];
    for (key, pixels) in &images {
        if !store.has_image(key).map_err(|e| e.to_string())? {
            new_images.push((key.clone(), encode_png(pixels).map_err(|e| e.to_string())?));
        }
    }
    let keys: Vec<String> = images.into_iter().map(|(key, _)| key).collect();
    let png = encode_png(image).map_err(|e| e.to_string())?;
    let thumbnail = encode_png(&thumbnail(image)).map_err(|e| e.to_string())?;
    let saved = Saved { size: image.size, thumbnail: &thumbnail, png: &png, drawing: Some((&json, &keys)), new_images: &new_images };
    store.save(id, now, &saved).map_err(|e| e.to_string())
}

fn load_entry(store: &Store, id: i64) -> Result<Loaded, String> {
    let (png, json) = store.load(id).map_err(|e| e.to_string())?.ok_or("it's no longer there")?;
    let image = decode_png(&png).map_err(|e| e.to_string())?;
    // A drawing that can't be read (say, from a newer Deeraw) still opens flattened.
    let drawing = json.and_then(|json| serde_json::from_str::<SavedDrawing>(&json).ok()).filter(|d| d.version <= drawing::VERSION);
    let drawing = match drawing {
        Some(drawing) => {
            let mut images = HashMap::new();
            for key in drawing::image_keys(&drawing) {
                // A missing image leaves it out (`drawing::restore`).
                if let Some(png) = store.image(key).map_err(|e| e.to_string())? {
                    images.insert(key.to_string(), Arc::new(decode_png(&png).map_err(|e| e.to_string())?));
                }
            }
            Some((drawing, images))
        }
        None => None,
    };
    Ok(Loaded { image, drawing })
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
                    Job::Save { drawing, image, objects, background, now } => {
                        let saved = save_entry(&store, entries.get(&drawing).copied(), now, &image, &objects, background)
                            .and_then(|id| {
                                entries.insert(drawing, id);
                                store.entries().map_err(|e| e.to_string())
                            });
                        reply(saved.map_or_else(|e| Reply::Failed(format!("Couldn't save to history: {e}")), Reply::Entries));
                    }
                    Job::Load(id) => {
                        let loaded = load_entry(&store, id);
                        reply(loaded.map_or_else(|e| Reply::Failed(format!("Couldn't open from history: {e}")), Reply::Loaded));
                    }
                    Job::Delete(id) => {
                        // A later save of its drawing adds a new entry.
                        entries.retain(|_, entry| *entry != id);
                        let listed = store.delete(id).and_then(|()| store.entries());
                        reply(listed.map_or_else(|e| Reply::Failed(format!("Couldn't delete from history: {e}")), Reply::Entries));
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

    /// Saves drawing number `drawing`, `objects` on `background`, flattened as `image`: its first save adds
    /// an entry, later ones replace it.
    pub fn save(&self, drawing: u64, image: ColorImage, objects: Vec<Object>, background: Background) {
        let now = chrono::Utc::now().timestamp_millis();
        self.send(Job::Save { drawing, image, objects, background, now });
    }

    /// Loads entry `id`; it arrives as `Reply::Loaded`.
    pub fn load(&self, id: i64) {
        self.send(Job::Load(id));
    }

    /// Loads entry `id`'s thumbnail; it arrives as `Reply::Thumbnail`.
    pub fn thumbnail(&self, id: i64) {
        self.send(Job::Thumbnail(id));
    }

    /// Deletes entry `id`; the list without it arrives as `Reply::Entries`.
    pub fn delete(&self, id: i64) {
        self.send(Job::Delete(id));
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

    /// An entry without a drawing, as version 0 saved them.
    fn flat(size: [usize; 2], thumbnail: &'static [u8], png: &'static [u8]) -> Saved<'static> {
        Saved { size, thumbnail, png, drawing: None, new_images: &[] }
    }

    fn png(store: &Store, id: i64) -> Option<Vec<u8>> {
        store.load(id).unwrap().map(|(png, _)| png)
    }

    fn image_count(store: &Store) -> i64 {
        store.conn.query_row("SELECT COUNT(*) FROM image", [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn saving_adds_then_replaces_an_entry() {
        let store = Store::in_memory().unwrap();
        let a = store.save(None, 1000, &flat([10, 20], b"t1", b"p1")).unwrap();
        let b = store.save(None, 2000, &flat([30, 40], b"t2", b"p2")).unwrap();
        assert_ne!(a, b);
        assert_eq!(store.save(Some(a), 3000, &flat([50, 60], b"t3", b"p3")).unwrap(), a);
        assert_eq!(
            store.entries().unwrap(),
            vec![Entry { id: a, updated: 3000, size: [50, 60] }, Entry { id: b, updated: 2000, size: [30, 40] }]
        );
        assert_eq!(store.load(a).unwrap(), Some((b"p3".to_vec(), None)));
        assert_eq!(store.thumbnail(a).unwrap(), Some((3000, b"t3".to_vec())));
        assert_eq!(store.thumbnail(b + 100).unwrap(), None);
        assert_eq!(png(&store, b + 100), None);
        // An entry deleted meanwhile is added again.
        store.delete(b).unwrap();
        assert_eq!(png(&store, b), None);
        let c = store.save(Some(b), 4000, &flat([1, 1], b"t", b"p")).unwrap();
        assert_eq!(store.entries().unwrap().len(), 2);
        assert_eq!(store.entries().unwrap()[0].id, c);
    }

    #[test]
    fn images_are_stored_once_and_deleted_once_unused() {
        let store = Store::in_memory().unwrap();
        let keys = |keys: &[&str]| keys.iter().map(|k| k.to_string()).collect::<Vec<_>>();
        let images = |keys: &[&str]| keys.iter().map(|k| (k.to_string(), format!("png of {k}").into_bytes())).collect::<Vec<_>>();
        let (k12, new12) = (keys(&["k1", "k2"]), images(&["k1", "k2"]));
        let a = store.save(None, 1, &Saved { drawing: Some(("{a}", &k12)), new_images: &new12, ..flat([1, 1], b"t", b"p") }).unwrap();
        assert!(store.has_image("k2").unwrap() && !store.has_image("k3").unwrap());
        // B uses k2 too, which is stored already.
        let k2 = keys(&["k2"]);
        let b = store.save(None, 2, &Saved { drawing: Some(("{b}", &k2)), ..flat([1, 1], b"t", b"p") }).unwrap();
        assert_eq!(image_count(&store), 2);
        assert_eq!(store.load(a).unwrap(), Some((b"p".to_vec(), Some("{a}".into()))));
        assert_eq!(store.image("k1").unwrap(), Some(b"png of k1".to_vec()));
        // A now uses only k3: k1 goes, k2 stays for B.
        let (k3, new3) = (keys(&["k3"]), images(&["k3"]));
        store.save(Some(a), 3, &Saved { drawing: Some(("{a2}", &k3)), new_images: &new3, ..flat([1, 1], b"t", b"p") }).unwrap();
        assert_eq!((store.has_image("k1").unwrap(), store.has_image("k2").unwrap(), store.has_image("k3").unwrap()), (false, true, true));
        store.delete(b).unwrap();
        assert!(!store.has_image("k2").unwrap());
        assert_eq!(store.prune(0).unwrap(), 1);
        assert_eq!(image_count(&store), 0);
    }

    /// A database from before drawings were kept: its entries stay and open flattened, and new ones keep drawings.
    #[test]
    fn version_0_databases_are_brought_up_to_date() {
        let path = std::env::temp_dir().join(format!("deeraw-history-v0-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let old = Connection::open(&path).unwrap();
        old.execute_batch(
            "CREATE TABLE entry (id INTEGER PRIMARY KEY, created INTEGER NOT NULL, updated INTEGER NOT NULL,
                width INTEGER NOT NULL, height INTEGER NOT NULL, thumbnail BLOB NOT NULL, png BLOB NOT NULL);
            CREATE INDEX entry_updated ON entry (updated);
            INSERT INTO entry (created, updated, width, height, thumbnail, png) VALUES (5, 5, 3, 4, x'01', x'02');",
        )
        .unwrap();
        drop(old);
        for _ in 0..2 {
            let store = Store::open(&path).unwrap();
            assert_eq!(store.entries().unwrap()[0], Entry { id: 1, updated: 5, size: [3, 4] });
            assert_eq!(store.load(1).unwrap(), Some((vec![2], None)));
            let version: i64 = store.conn.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap();
            assert_eq!(version, 1);
        }
        let store = Store::open(&path).unwrap();
        let keys = vec!["k".to_string()];
        let id = store.save(None, 9, &Saved { drawing: Some(("{}", &keys)), new_images: &[("k".into(), vec![3])], ..flat([1, 1], b"t", b"p") }).unwrap();
        assert_eq!(store.load(id).unwrap(), Some((b"p".to_vec(), Some("{}".into()))));
        drop(store);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    #[test]
    fn pruning_keeps_the_most_recently_saved() {
        let store = Store::in_memory().unwrap();
        let ids: Vec<i64> = (0..5).map(|i| store.save(None, i, &flat([1, 1], b"t", b"p")).unwrap()).collect();
        // The oldest, saved again, is now the newest.
        store.save(Some(ids[0]), 10, &flat([1, 1], b"t", b"p")).unwrap();
        assert_eq!(store.prune(10).unwrap(), 0);
        assert_eq!(store.prune(3).unwrap(), 2);
        let kept: Vec<i64> = store.entries().unwrap().iter().map(|e| e.id).collect();
        assert_eq!(kept, [ids[0], ids[4], ids[3]]);
        assert_eq!(png(&store, ids[1]), None);
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
        worker.save(1, pattern([64, 48]), vec![], Background::White);
        let first = recent(next(&worker));
        assert_eq!(first.len(), 1);
        // Drawing 1 again replaces its entry; drawing 2 gets its own.
        worker.save(1, pattern([80, 60]), vec![], Background::White);
        assert_eq!(recent(next(&worker))[0].id, first[0].id);
        worker.save(2, pattern([10, 10]), vec![], Background::White);
        let entries = recent(next(&worker));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1], Entry { size: [80, 60], ..entries[1].clone() });
        worker.finish();

        let mut worker = Worker::start(path.clone(), 10, egui::Context::default());
        let reopened = recent(next(&worker));
        assert_eq!(reopened.iter().map(|e| e.id).collect::<Vec<_>>(), entries.iter().map(|e| e.id).collect::<Vec<_>>());
        worker.load(first[0].id);
        match next(&worker) {
            Reply::Loaded(Loaded { image, drawing }) => {
                assert_eq!(image, pattern([80, 60]));
                let (drawing, images) = drawing.expect("the drawing");
                assert!(drawing.objects.is_empty() && images.is_empty());
            }
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
        worker.save(1, pattern([5, 5]), vec![], Background::White);
        assert_eq!(recent(next(&worker)).len(), 3);
        worker.finish();

        // Deleting a drawing's entry: its next save adds it again, and later ones replace that.
        let mut worker = Worker::start(path.clone(), 10, egui::Context::default());
        let before = recent(next(&worker));
        worker.save(1, pattern([7, 7]), vec![], Background::White);
        let added = recent(next(&worker))[0].clone();
        worker.delete(added.id);
        assert_eq!(recent(next(&worker)), before);
        worker.save(1, pattern([8, 8]), vec![], Background::White);
        assert_eq!(recent(next(&worker)).len(), before.len() + 1);
        worker.save(1, pattern([9, 9]), vec![], Background::White);
        let saved = recent(next(&worker));
        assert_eq!((saved.len(), saved[0].size), (before.len() + 1, [9, 9]));
        worker.finish();

        // A drawing with an image: the image is stored once, however often it's saved, and comes back.
        let mut worker = Worker::start(path.clone(), 10, egui::Context::default());
        recent(next(&worker));
        let ctx = egui::Context::default();
        let pixels = Arc::new(pattern([40, 30]));
        let texture = ctx.load_texture("image", (*pixels).clone(), Default::default());
        let mosaics = crate::mosaic::load_mosaics(&ctx, "mosaic", &pixels, 12);
        let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(0.5, 1.0));
        let kind = crate::object::Kind::Image { texture, uv, pixels: crate::object::Pixels(pixels.clone()), mosaics };
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(20.0, 30.0));
        let objects = vec![Object { id: 1, rect, aspect: 1.0, kind }];
        worker.save(1, pattern([20, 30]), objects.clone(), Background::Black);
        recent(next(&worker));
        worker.save(1, pattern([20, 30]), objects.clone(), Background::Black);
        worker.save(2, pattern([20, 30]), objects, Background::Black);
        recent(next(&worker));
        let entries = recent(next(&worker));
        worker.load(entries[0].id);
        match next(&worker) {
            Reply::Loaded(Loaded { drawing: Some((drawing, images)), .. }) => {
                assert_eq!(drawing.background, Background::Black);
                assert_eq!(drawing.objects.len(), 1);
                assert!(matches!(&drawing.objects[0].kind, drawing::SavedKind::Image { uv: u, .. } if *u == uv));
                assert_eq!(images.values().map(|i| (**i).clone()).collect::<Vec<_>>(), [pattern([40, 30])]);
            }
            _ => panic!("expected the drawing"),
        }
        worker.finish();
        assert_eq!(image_count(&Store::open(&path).unwrap()), 1);

        // Opened with a smaller limit, only the newest are left.
        let mut worker = Worker::start(path.clone(), 2, egui::Context::default());
        let pruned = recent(next(&worker));
        assert_eq!(pruned.len(), 2);
        assert_eq!(pruned[0].size, [20, 30]);
        worker.finish();
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }
}
