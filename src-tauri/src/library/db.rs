//! SQLite storage for the music library.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use std::path::Path;

pub type Result<T> = rusqlite::Result<T>;

const SCHEMA_VERSION: i64 = 3;

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    init(&conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    init(&conn)?;
    Ok(conn)
}

fn init(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch(
            "
            CREATE TABLE folders (
                id    INTEGER PRIMARY KEY,
                path  TEXT NOT NULL UNIQUE
            );
            CREATE TABLE covers (
                hash     TEXT PRIMARY KEY,
                full     TEXT NOT NULL,
                thumb    TEXT NOT NULL,
                width    INTEGER NOT NULL,
                height   INTEGER NOT NULL,
                palette  TEXT NOT NULL
            );
            CREATE TABLE tracks (
                id            INTEGER PRIMARY KEY,
                path          TEXT NOT NULL UNIQUE,
                folder_id     INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
                mtime         INTEGER NOT NULL,
                size          INTEGER NOT NULL,
                title         TEXT NOT NULL,
                artist        TEXT NOT NULL DEFAULT '',
                album         TEXT NOT NULL DEFAULT '',
                album_artist  TEXT NOT NULL DEFAULT '',
                album_key     TEXT NOT NULL,
                genre         TEXT NOT NULL DEFAULT '',
                year          INTEGER,
                track_no      INTEGER,
                disc_no       INTEGER,
                duration      REAL NOT NULL DEFAULT 0,
                bitrate       INTEGER,
                sample_rate   INTEGER,
                channels      INTEGER,
                bit_depth     INTEGER,
                format        TEXT NOT NULL,
                cover_hash    TEXT REFERENCES covers(hash) ON DELETE SET NULL,
                cover_source  TEXT,
                added_at      INTEGER NOT NULL
            );
            CREATE INDEX tracks_album_key ON tracks(album_key);
            CREATE INDEX tracks_artist ON tracks(artist COLLATE NOCASE);
            CREATE INDEX tracks_genre ON tracks(genre COLLATE NOCASE);
            CREATE INDEX tracks_folder ON tracks(folder_id);
            ",
        )?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    if version < 2 {
        conn.execute_batch("CREATE TABLE meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);")?;
        conn.pragma_update(None, "user_version", 2)?;
    }
    if version < 3 {
        // Compact tint so lists can glow without loading whole palettes.
        // Filled by the palette upgrade that runs on startup.
        conn.execute_batch(
            "ALTER TABLE covers ADD COLUMN edge TEXT NOT NULL DEFAULT '';
             ALTER TABLE covers ADD COLUMN glow TEXT NOT NULL DEFAULT '';",
        )?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    Ok(())
}

pub fn meta_get(conn: &Connection, key: &str) -> Result<Option<i64>> {
    conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()
}

pub fn meta_set(conn: &Connection, key: &str, value: i64) -> Result<()> {
    conn.execute("INSERT INTO meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value])?;
    Ok(())
}

/// (hash, full image path) of every cached cover.
pub fn cover_files(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT hash, full FROM covers")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

pub fn set_palette(conn: &Connection, hash: &str, palette_json: &str, edge: &str, glow: &str) -> Result<()> {
    conn.execute("UPDATE covers SET palette = ?2, edge = ?3, glow = ?4 WHERE hash = ?1", params![hash, palette_json, edge, glow])?;
    Ok(())
}

/// Grouping key for albums: same album title by the same album artist.
pub fn album_key(album_artist: &str, artist: &str, album: &str) -> String {
    let who = if album_artist.trim().is_empty() { artist } else { album_artist };
    format!("{}\u{1f}{}", who.trim().to_lowercase(), album.trim().to_lowercase())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: i64,
    pub path: String,
    pub track_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cover {
    pub hash: String,
    pub full: String,
    pub thumb: String,
    pub width: u32,
    pub height: u32,
    /// The cover's 1 px edge color and bottom glow color (`#rrggbb`).
    pub edge: String,
    pub glow: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackRow {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub album_key: String,
    pub genre: String,
    pub year: Option<i64>,
    pub track_no: Option<i64>,
    pub disc_no: Option<i64>,
    pub duration: f64,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub bit_depth: Option<i64>,
    pub format: String,
    pub cover: Option<Cover>,
    pub cover_source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumRow {
    pub key: String,
    pub title: String,
    pub artist: String,
    pub year: Option<i64>,
    pub track_count: i64,
    pub duration: f64,
    pub cover: Option<Cover>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedCount {
    pub name: String,
    pub track_count: i64,
    pub album_count: i64,
    pub cover: Option<Cover>,
    /// Up to four distinct album covers, most-used first (artist stacks,
    /// genre mosaics).
    pub covers: Vec<Cover>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub tracks: i64,
    pub albums: i64,
    pub artists: i64,
    pub missing_covers: i64,
    pub folders: i64,
}

/// Everything the scanner learned about one file.
#[derive(Debug, Clone)]
pub struct NewTrack {
    pub path: String,
    pub folder_id: i64,
    pub mtime: i64,
    pub size: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub year: Option<i64>,
    pub track_no: Option<i64>,
    pub disc_no: Option<i64>,
    pub duration: f64,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub bit_depth: Option<i64>,
    pub format: String,
    pub cover_hash: Option<String>,
    pub cover_source: Option<String>,
}

pub struct NewCover {
    pub hash: String,
    pub full: String,
    pub thumb: String,
    pub width: u32,
    pub height: u32,
    pub palette_json: String,
    pub edge: String,
    pub glow: String,
}

// ---- Folders ---------------------------------------------------------------

pub fn add_folder(conn: &Connection, path: &str) -> Result<i64> {
    conn.execute("INSERT OR IGNORE INTO folders(path) VALUES (?1)", [path])?;
    conn.query_row("SELECT id FROM folders WHERE path = ?1", [path], |r| r.get(0))
}

pub fn remove_folder(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM folders WHERE id = ?1", [id])?;
    Ok(())
}

pub fn folders(conn: &Connection) -> Result<Vec<Folder>> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.path, (SELECT COUNT(*) FROM tracks t WHERE t.folder_id = f.id)
         FROM folders f ORDER BY f.path COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| Ok(Folder { id: r.get(0)?, path: r.get(1)?, track_count: r.get(2)? }))?;
    rows.collect()
}

// ---- Scanner support -------------------------------------------------------

/// (path -> (mtime, size)) for one folder, to skip unchanged files.
pub fn known_files(conn: &Connection, folder_id: i64) -> Result<std::collections::HashMap<String, (i64, i64)>> {
    let mut stmt = conn.prepare("SELECT path, mtime, size FROM tracks WHERE folder_id = ?1")?;
    let rows = stmt.query_map([folder_id], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?))))?;
    rows.collect()
}

pub fn has_cover(conn: &Connection, hash: &str) -> Result<bool> {
    conn.query_row("SELECT 1 FROM covers WHERE hash = ?1", [hash], |_| Ok(())).optional().map(|o| o.is_some())
}

pub fn insert_cover(conn: &Connection, c: &NewCover) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO covers(hash, full, thumb, width, height, palette, edge, glow) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![c.hash, c.full, c.thumb, c.width, c.height, c.palette_json, c.edge, c.glow],
    )?;
    Ok(())
}

pub fn upsert_track(conn: &Connection, t: &NewTrack, now: i64) -> Result<()> {
    let key = album_key(&t.album_artist, &t.artist, &t.album);
    conn.execute(
        "INSERT INTO tracks(path, folder_id, mtime, size, title, artist, album, album_artist, album_key, genre,
                            year, track_no, disc_no, duration, bitrate, sample_rate, channels, bit_depth, format,
                            cover_hash, cover_source, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
         ON CONFLICT(path) DO UPDATE SET
            folder_id = excluded.folder_id, mtime = excluded.mtime, size = excluded.size, title = excluded.title,
            artist = excluded.artist, album = excluded.album, album_artist = excluded.album_artist,
            album_key = excluded.album_key, genre = excluded.genre, year = excluded.year,
            track_no = excluded.track_no, disc_no = excluded.disc_no, duration = excluded.duration,
            bitrate = excluded.bitrate, sample_rate = excluded.sample_rate, channels = excluded.channels,
            bit_depth = excluded.bit_depth, format = excluded.format, cover_hash = excluded.cover_hash,
            cover_source = excluded.cover_source",
        params![
            t.path, t.folder_id, t.mtime, t.size, t.title, t.artist, t.album, t.album_artist, key, t.genre, t.year,
            t.track_no, t.disc_no, t.duration, t.bitrate, t.sample_rate, t.channels, t.bit_depth, t.format,
            t.cover_hash, t.cover_source, now
        ],
    )?;
    Ok(())
}

pub fn remove_track_path(conn: &Connection, path: &str) -> Result<()> {
    conn.execute("DELETE FROM tracks WHERE path = ?1", [path])?;
    Ok(())
}

/// Covers no track references any more; returns their (full, thumb) files so
/// the caller can delete them from the cache.
pub fn prune_covers(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt =
        conn.prepare("SELECT full, thumb FROM covers WHERE hash NOT IN (SELECT cover_hash FROM tracks WHERE cover_hash IS NOT NULL)")?;
    let files: Vec<(String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_>>()?;
    conn.execute("DELETE FROM covers WHERE hash NOT IN (SELECT cover_hash FROM tracks WHERE cover_hash IS NOT NULL)", [])?;
    Ok(files)
}

// ---- Queries ---------------------------------------------------------------

const TRACK_COLS: &str = "t.id, t.path, t.title, t.artist, t.album, t.album_artist, t.album_key, t.genre, t.year,
    t.track_no, t.disc_no, t.duration, t.bitrate, t.sample_rate, t.channels, t.bit_depth, t.format,
    c.hash, c.full, c.thumb, c.width, c.height, c.edge, c.glow, t.cover_source";

fn cover_at(r: &Row, i: usize) -> Result<Option<Cover>> {
    let hash: Option<String> = r.get(i)?;
    Ok(match hash {
        Some(hash) => Some(Cover {
            hash,
            full: r.get(i + 1)?,
            thumb: r.get(i + 2)?,
            width: r.get(i + 3)?,
            height: r.get(i + 4)?,
            edge: r.get(i + 5)?,
            glow: r.get(i + 6)?,
        }),
        None => None,
    })
}

fn track_row(r: &Row) -> Result<TrackRow> {
    Ok(TrackRow {
        id: r.get(0)?,
        path: r.get(1)?,
        title: r.get(2)?,
        artist: r.get(3)?,
        album: r.get(4)?,
        album_artist: r.get(5)?,
        album_key: r.get(6)?,
        genre: r.get(7)?,
        year: r.get(8)?,
        track_no: r.get(9)?,
        disc_no: r.get(10)?,
        duration: r.get(11)?,
        bitrate: r.get(12)?,
        sample_rate: r.get(13)?,
        channels: r.get(14)?,
        bit_depth: r.get(15)?,
        format: r.get(16)?,
        cover: cover_at(r, 17)?,
        cover_source: r.get(24)?,
    })
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackFilter {
    pub album_key: Option<String>,
    pub artist: Option<String>,
    pub genre: Option<String>,
    pub search: Option<String>,
    #[serde(default)]
    pub missing_cover: bool,
    pub limit: Option<i64>,
}

pub fn tracks(conn: &Connection, f: &TrackFilter) -> Result<Vec<TrackRow>> {
    let mut sql = format!("SELECT {TRACK_COLS} FROM tracks t LEFT JOIN covers c ON c.hash = t.cover_hash WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(k) = &f.album_key {
        args.push(Box::new(k.clone()));
        sql += &format!(" AND t.album_key = ?{}", args.len());
    }
    if let Some(a) = &f.artist {
        args.push(Box::new(a.clone()));
        let n = args.len();
        sql += &format!(" AND (t.artist = ?{n} COLLATE NOCASE OR t.album_artist = ?{n} COLLATE NOCASE)");
    }
    if let Some(g) = &f.genre {
        args.push(Box::new(g.clone()));
        sql += &format!(" AND t.genre = ?{} COLLATE NOCASE", args.len());
    }
    if let Some(s) = f.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        args.push(Box::new(format!("%{}%", escape_like(s))));
        let n = args.len();
        sql += &format!(
            " AND (t.title LIKE ?{n} ESCAPE '\\' OR t.artist LIKE ?{n} ESCAPE '\\' OR t.album LIKE ?{n} ESCAPE '\\'
                   OR t.album_artist LIKE ?{n} ESCAPE '\\')"
        );
    }
    if f.missing_cover {
        sql += " AND t.cover_hash IS NULL";
    }
    sql += if f.album_key.is_some() {
        " ORDER BY COALESCE(t.disc_no, 1), COALESCE(t.track_no, 9999), t.title COLLATE NOCASE"
    } else {
        " ORDER BY t.artist COLLATE NOCASE, t.album COLLATE NOCASE, COALESCE(t.disc_no, 1), COALESCE(t.track_no, 9999), t.title COLLATE NOCASE"
    };
    if let Some(limit) = f.limit {
        args.push(Box::new(limit));
        sql += &format!(" LIMIT ?{}", args.len());
    }
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter().map(|b| b.as_ref())), track_row)?;
    rows.collect()
}

pub fn track(conn: &Connection, id: i64) -> Result<Option<TrackRow>> {
    let sql = format!("SELECT {TRACK_COLS} FROM tracks t LEFT JOIN covers c ON c.hash = t.cover_hash WHERE t.id = ?1");
    conn.query_row(&sql, [id], track_row).optional()
}

pub fn track_by_path(conn: &Connection, path: &str) -> Result<Option<TrackRow>> {
    let sql = format!("SELECT {TRACK_COLS} FROM tracks t LEFT JOIN covers c ON c.hash = t.cover_hash WHERE t.path = ?1");
    conn.query_row(&sql, [path], track_row).optional()
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

/// Album cover = the cover used by most of its tracks (ties: any).
const ALBUM_COVER: &str = "(SELECT t2.cover_hash FROM tracks t2 WHERE t2.album_key = t.album_key AND t2.cover_hash IS NOT NULL
      GROUP BY t2.cover_hash ORDER BY COUNT(*) DESC LIMIT 1)";

pub fn albums(conn: &Connection, artist: Option<&str>, search: Option<&str>) -> Result<Vec<AlbumRow>> {
    let mut sql = format!(
        "SELECT a.album_key, a.title, a.artist, a.year, a.n, a.dur, c.hash, c.full, c.thumb, c.width, c.height, c.edge, c.glow FROM (
            SELECT t.album_key, MAX(t.album) AS title,
                   COALESCE(NULLIF(MAX(t.album_artist), ''), MAX(t.artist)) AS artist,
                   MIN(t.year) AS year, COUNT(*) AS n, SUM(t.duration) AS dur, {ALBUM_COVER} AS cover_hash
            FROM tracks t WHERE t.album != ''"
    );
    let mut args: Vec<String> = Vec::new();
    if let Some(a) = artist {
        args.push(a.to_string());
        sql += " AND (t.artist = ?1 COLLATE NOCASE OR t.album_artist = ?1 COLLATE NOCASE)";
    }
    if let Some(s) = search.map(str::trim).filter(|s| !s.is_empty()) {
        args.push(format!("%{}%", escape_like(s)));
        let n = args.len();
        sql += &format!(" AND (t.album LIKE ?{n} ESCAPE '\\' OR t.album_artist LIKE ?{n} ESCAPE '\\' OR t.artist LIKE ?{n} ESCAPE '\\')");
    }
    sql += " GROUP BY t.album_key) a LEFT JOIN covers c ON c.hash = a.cover_hash
             ORDER BY a.artist COLLATE NOCASE, a.year, a.title COLLATE NOCASE";
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        Ok(AlbumRow {
            key: r.get(0)?,
            title: r.get(1)?,
            artist: r.get(2)?,
            year: r.get(3)?,
            track_count: r.get(4)?,
            duration: r.get(5)?,
            cover: cover_at(r, 6)?,
        })
    })?;
    rows.collect()
}

fn named_counts(conn: &Connection, column: &str, search: Option<&str>) -> Result<Vec<NamedCount>> {
    let mut sql = format!(
        "SELECT a.name, a.n, a.albums, c.hash, c.full, c.thumb, c.width, c.height, c.edge, c.glow FROM (
            SELECT t.{column} AS name, COUNT(*) AS n, COUNT(DISTINCT t.album_key) AS albums,
                   (SELECT t2.cover_hash FROM tracks t2 WHERE t2.{column} = t.{column} COLLATE NOCASE
                      AND t2.cover_hash IS NOT NULL GROUP BY t2.cover_hash ORDER BY COUNT(*) DESC LIMIT 1) AS cover_hash
            FROM tracks t WHERE t.{column} != ''"
    );
    let mut args: Vec<String> = Vec::new();
    if let Some(s) = search.map(str::trim).filter(|s| !s.is_empty()) {
        args.push(format!("%{}%", escape_like(s)));
        sql += &format!(" AND t.{column} LIKE ?1 ESCAPE '\\'");
    }
    sql += &format!(" GROUP BY t.{column} COLLATE NOCASE) a LEFT JOIN covers c ON c.hash = a.cover_hash ORDER BY a.name COLLATE NOCASE");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        Ok(NamedCount { name: r.get(0)?, track_count: r.get(1)?, album_count: r.get(2)?, cover: cover_at(r, 3)?, covers: Vec::new() })
    })?;
    let mut out: Vec<NamedCount> = rows.collect::<Result<_>>()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT c.hash, c.full, c.thumb, c.width, c.height, c.edge, c.glow FROM covers c JOIN (
            SELECT t.cover_hash AS h, COUNT(*) AS n FROM tracks t
            WHERE t.{column} = ?1 COLLATE NOCASE AND t.cover_hash IS NOT NULL
            GROUP BY t.album_key, t.cover_hash
         ) x ON x.h = c.hash GROUP BY c.hash ORDER BY MAX(x.n) DESC LIMIT 4"
    ))?;
    for n in &mut out {
        n.covers = stmt.query_map([&n.name], |r| cover_at(r, 0).map(|c| c.expect("hash is not null")))?.collect::<Result<_>>()?;
    }
    Ok(out)
}

pub fn artists(conn: &Connection, search: Option<&str>) -> Result<Vec<NamedCount>> {
    named_counts(conn, "artist", search)
}

pub fn genres(conn: &Connection, search: Option<&str>) -> Result<Vec<NamedCount>> {
    named_counts(conn, "genre", search)
}

pub fn stats(conn: &Connection) -> Result<Stats> {
    conn.query_row(
        "SELECT (SELECT COUNT(*) FROM tracks),
                (SELECT COUNT(DISTINCT album_key) FROM tracks WHERE album != ''),
                (SELECT COUNT(DISTINCT artist COLLATE NOCASE) FROM tracks WHERE artist != ''),
                (SELECT COUNT(*) FROM tracks WHERE cover_hash IS NULL),
                (SELECT COUNT(*) FROM folders)",
        [],
        |r| Ok(Stats { tracks: r.get(0)?, albums: r.get(1)?, artists: r.get(2)?, missing_covers: r.get(3)?, folders: r.get(4)? }),
    )
}

pub fn palette_json(conn: &Connection, hash: &str) -> Result<Option<String>> {
    conn.query_row("SELECT palette FROM covers WHERE hash = ?1", [hash], |r| r.get(0)).optional()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(path: &str, folder_id: i64, artist: &str, album: &str, cover: Option<&str>) -> NewTrack {
        NewTrack {
            path: path.into(),
            folder_id,
            mtime: 1,
            size: 1,
            title: path.into(),
            artist: artist.into(),
            album: album.into(),
            album_artist: String::new(),
            genre: "House".into(),
            year: Some(2016),
            track_no: None,
            disc_no: None,
            duration: 200.0,
            bitrate: Some(320),
            sample_rate: Some(44100),
            channels: Some(2),
            bit_depth: None,
            format: "MPEG".into(),
            cover_hash: cover.map(Into::into),
            cover_source: cover.map(|_| "embedded".into()),
        }
    }

    fn cover(hash: &str) -> NewCover {
        NewCover {
            hash: hash.into(),
            full: format!("{hash}.jpg"),
            thumb: format!("{hash}_t.jpg"),
            width: 10,
            height: 10,
            palette_json: "{}".into(),
            edge: "#000000".into(),
            glow: "#000000".into(),
        }
    }

    #[test]
    fn albums_group_and_pick_cover() {
        let c = open_in_memory().unwrap();
        let f = add_folder(&c, "C:\\Music").unwrap();
        insert_cover(&c, &cover("aa")).unwrap();
        upsert_track(&c, &track("1.mp3", f, "D.O.D", "Ping", Some("aa")), 0).unwrap();
        upsert_track(&c, &track("2.mp3", f, "d.o.d", "PING", None), 0).unwrap();
        upsert_track(&c, &track("3.mp3", f, "Other", "Else", None), 0).unwrap();

        let albums = albums(&c, None, None).unwrap();
        assert_eq!(albums.len(), 2);
        let ping = albums.iter().find(|a| a.title.eq_ignore_ascii_case("ping")).unwrap();
        assert_eq!(ping.track_count, 2);
        assert_eq!(ping.cover.as_ref().unwrap().hash, "aa");

        let s = stats(&c).unwrap();
        assert_eq!((s.tracks, s.albums, s.missing_covers), (3, 2, 2));
    }

    #[test]
    fn upsert_updates_in_place_and_search_escapes() {
        let c = open_in_memory().unwrap();
        let f = add_folder(&c, "C:\\Music").unwrap();
        upsert_track(&c, &track("a.mp3", f, "100% Pure", "X", None), 0).unwrap();
        upsert_track(&c, &track("a.mp3", f, "100% Pure", "Y", None), 0).unwrap();
        assert_eq!(tracks(&c, &TrackFilter::default()).unwrap().len(), 1);
        assert_eq!(tracks(&c, &TrackFilter::default()).unwrap()[0].album, "Y");

        let hit = TrackFilter { search: Some("100%".into()), ..Default::default() };
        assert_eq!(tracks(&c, &hit).unwrap().len(), 1);
        let miss = TrackFilter { search: Some("1_0".into()), ..Default::default() };
        assert_eq!(tracks(&c, &miss).unwrap().len(), 0);
    }

    #[test]
    fn removing_folder_cascades_and_prunes_covers() {
        let c = open_in_memory().unwrap();
        let f = add_folder(&c, "C:\\Music").unwrap();
        insert_cover(&c, &cover("bb")).unwrap();
        upsert_track(&c, &track("a.mp3", f, "A", "B", Some("bb")), 0).unwrap();
        remove_folder(&c, f).unwrap();
        assert_eq!(stats(&c).unwrap().tracks, 0);
        let pruned = prune_covers(&c).unwrap();
        assert_eq!(pruned, vec![("bb.jpg".to_string(), "bb_t.jpg".to_string())]);
    }
}
