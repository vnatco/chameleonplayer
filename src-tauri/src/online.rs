//! Online cover lookup: MusicBrainz finds the release, the Cover Art Archive
//! serves its front cover.
//!
//! MusicBrainz allows one request per second per client and requires a
//! descriptive User-Agent; both are enforced here. Downloads land in the
//! cover cache folder so the webview can show them and the tag editor can
//! embed them like any other picked image.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const USER_AGENT: &str = concat!("ChameleonPlayer/", env!("CARGO_PKG_VERSION"), " ( https://github.com/vnatco/chameleon_player )");
const MB_GAP: Duration = Duration::from_millis(1100);
const MAX_IMAGE_BYTES: u64 = 30 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// Release title and artist as MusicBrainz has them.
    pub release: String,
    pub artist: String,
    pub year: Option<String>,
    /// MusicBrainz search score, 0-100.
    pub score: u32,
    /// Small preview image URL (250 px).
    pub thumb: String,
    /// Image to download when picked (1200 px when available).
    pub image: String,
    /// Short label for the UI, e.g. "Cover Art Archive · 1200 px".
    pub label: String,
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .user_agent(USER_AGENT)
            .build()
            .into()
    })
}

/// Wait so MusicBrainz requests are at least `MB_GAP` apart.
fn mb_throttle() {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let mut last = LAST.lock();
    if let Some(t) = *last {
        let since = t.elapsed();
        if since < MB_GAP {
            std::thread::sleep(MB_GAP - since);
        }
    }
    *last = Some(Instant::now());
}

#[derive(Deserialize)]
struct ReleaseSearch {
    #[serde(default)]
    releases: Vec<Release>,
}

#[derive(Deserialize)]
struct RecordingSearch {
    #[serde(default)]
    recordings: Vec<Recording>,
}

#[derive(Deserialize)]
struct Recording {
    #[serde(default)]
    releases: Vec<Release>,
    #[serde(default)]
    score: u32,
}

#[derive(Deserialize, Clone)]
struct Release {
    id: String,
    title: String,
    #[serde(default)]
    score: u32,
    #[serde(default)]
    date: Option<String>,
    #[serde(rename = "artist-credit", default)]
    artist_credit: Vec<ArtistCredit>,
}

#[derive(Deserialize, Clone)]
struct ArtistCredit {
    name: String,
    #[serde(default)]
    joinphrase: String,
}

#[derive(Deserialize)]
struct CaaListing {
    #[serde(default)]
    images: Vec<CaaImage>,
}

#[derive(Deserialize)]
struct CaaImage {
    #[serde(default)]
    front: bool,
    image: String,
    #[serde(default)]
    thumbnails: CaaThumbs,
}

#[derive(Deserialize, Default)]
struct CaaThumbs {
    #[serde(rename = "250")]
    t250: Option<String>,
    #[serde(rename = "500")]
    t500: Option<String>,
    #[serde(rename = "1200")]
    t1200: Option<String>,
}

/// Lucene query escaping for MusicBrainz search.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "+-&|!(){}[]^\"~*?:\\/".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// GET from MusicBrainz. 503 means "slow down" there, so it is retried
/// with growing pauses before giving up.
fn mb_get<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, String> {
    let mut wait = Duration::from_millis(1500);
    for attempt in 0..4 {
        mb_throttle();
        match agent().get(url).call() {
            Ok(mut r) => return r.body_mut().read_json::<T>().map_err(|e| format!("Unexpected answer from MusicBrainz: {e}")),
            Err(ureq::Error::StatusCode(503)) if attempt < 3 => {
                log::info!("musicbrainz busy (503); retrying in {wait:?}");
                std::thread::sleep(wait);
                wait *= 2;
            }
            Err(e) => return Err(format!("MusicBrainz didn't answer: {e}")),
        }
    }
    Err("MusicBrainz is busy right now. Try again in a minute.".into())
}

/// Find up to `max` front covers for a release. With an album name the
/// release is searched directly; for untagged files the recording (title +
/// artist) is searched and its releases used as best guesses.
pub fn find(artist: &str, album: &str, title: &str, max: usize) -> Result<Vec<Candidate>, String> {
    let (artist, album, title) = (meaningful(artist), meaningful(album), meaningful(title));
    // Without a real artist, a search for "Untitled" or "Track 01" only
    // returns confident-looking nonsense.
    if artist.is_empty() {
        return Err("Add the artist first, so the search has something real to go on.".into());
    }
    let releases: Vec<Release> = if !album.is_empty() {
        let mut q = format!("release:\"{}\"", esc(album));
        if !artist.is_empty() {
            q += &format!(" AND artist:\"{}\"", esc(artist));
        }
        let url = format!("https://musicbrainz.org/ws/2/release/?fmt=json&limit=10&query={}", urlencoding::encode(&q));
        mb_get::<ReleaseSearch>(&url)?.releases
    } else if !title.is_empty() {
        let mut q = format!("recording:\"{}\"", esc(title));
        if !artist.is_empty() {
            q += &format!(" AND artist:\"{}\"", esc(artist));
        }
        let url = format!("https://musicbrainz.org/ws/2/recording/?fmt=json&limit=10&query={}", urlencoding::encode(&q));
        mb_get::<RecordingSearch>(&url)?
            .recordings
            .into_iter()
            .flat_map(|r| {
                let s = r.score;
                r.releases.into_iter().map(move |mut rel| {
                    rel.score = rel.score.max(s);
                    rel
                })
            })
            .collect()
    } else {
        return Err("Add an album, title or artist first, so there's something to search for.".into());
    };

    let mut out = Vec::new();
    let mut seen_images = std::collections::HashSet::new();
    for rel in releases.iter().filter(|r| r.score >= 50).take(8) {
        if out.len() >= max {
            break;
        }
        // The archive answers 404 for releases without art; that's normal.
        let url = format!("https://coverartarchive.org/release/{}", rel.id);
        let listing = match agent().get(&url).call() {
            Ok(mut r) => match r.body_mut().read_json::<CaaListing>() {
                Ok(l) => l,
                Err(e) => {
                    log::warn!("cover art archive: bad listing for {}: {e}", rel.id);
                    continue;
                }
            },
            Err(ureq::Error::StatusCode(404)) => continue,
            Err(e) => return Err(format!("Cover Art Archive didn't answer: {e}")),
        };
        let Some(img) = listing.images.into_iter().find(|i| i.front) else { continue };
        if !seen_images.insert(img.image.clone()) {
            continue;
        }
        let (image, size) = match &img.thumbnails.t1200 {
            Some(u) => (u.clone(), "1200 px"),
            None => (img.image.clone(), "Original"),
        };
        let thumb = img.thumbnails.t250.clone().or(img.thumbnails.t500.clone()).unwrap_or_else(|| image.clone());
        let artist_name: String = rel.artist_credit.iter().map(|a| format!("{}{}", a.name, a.joinphrase)).collect();
        let year = rel.date.as_ref().map(|d| d.chars().take(4).collect::<String>()).filter(|y| !y.is_empty());
        out.push(Candidate {
            label: format!("{} · {size}", if rel.score >= 95 { "Best Match" } else { "Alternative" }),
            release: rel.title.clone(),
            artist: artist_name,
            year,
            score: rel.score,
            thumb: https(thumb),
            image: https(image),
        });
    }
    Ok(out)
}

/// Empty for placeholder tags ("Unknown Artist", "Untitled", "Track 01"...).
fn meaningful(s: &str) -> &str {
    let t = s.trim();
    let l = t.to_lowercase();
    const PLACEHOLDERS: &[&str] = &["unknown", "unknown artist", "unknown album", "[unknown]", "various", "various artists", "untitled", "no title", "n/a", "none"];
    let generic_track = l.strip_prefix("track").map(|r| r.trim().chars().all(|c| c.is_ascii_digit())).unwrap_or(false);
    if PLACEHOLDERS.contains(&l.as_str()) || generic_track {
        ""
    } else {
        t
    }
}

fn https(u: String) -> String {
    match u.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => u,
    }
}

/// Download an image into `dir`, verify it decodes, and return its path.
pub fn download(url: &str, dir: &Path) -> Result<PathBuf, String> {
    if !url.starts_with("https://") {
        return Err("Only secure (https) image links are downloaded.".into());
    }
    let bytes = agent()
        .get(url)
        .call()
        .map_err(|e| format!("Download failed: {e}"))?
        .body_mut()
        .with_config()
        .limit(MAX_IMAGE_BYTES)
        .read_to_vec()
        .map_err(|e| format!("Download failed: {e}"))?;
    let format = image::guess_format(&bytes).map_err(|_| "The download isn't an image.".to_string())?;
    image::load_from_memory_with_format(&bytes, format).map_err(|e| format!("The downloaded image is damaged: {e}"))?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let ext = format.extensions_str().first().copied().unwrap_or("img");
    let path = dir.join(format!("{}.{ext}", crate::library::covers::hash(&bytes)));
    crate::library::covers::write_atomic(&path, &bytes)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lucene_escaping() {
        assert_eq!(esc("AC/DC: Live!"), "AC\\/DC\\: Live\\!");
        assert_eq!(esc("plain"), "plain");
    }

    #[test]
    fn placeholders_are_not_searched() {
        assert_eq!(meaningful(" Unknown Artist "), "");
        assert_eq!(meaningful("Untitled"), "");
        assert_eq!(meaningful("Track 07"), "");
        assert_eq!(meaningful("Daft Punk"), "Daft Punk");
        assert_eq!(meaningful("Track Star"), "Track Star");
        assert!(find("Unknown Artist", "Untitled", "", 3).is_err());
    }

    #[test]
    fn upgrades_to_https() {
        assert_eq!(https("http://a/b".into()), "https://a/b");
        assert_eq!(https("https://a/b".into()), "https://a/b");
    }

    #[test]
    fn refuses_insecure_download() {
        assert!(download("http://example.com/a.jpg", Path::new(".")).is_err());
    }
}

#[cfg(test)]
mod live {
    /// Hits the real services; run with `cargo test -- --ignored live_`.
    #[test]
    #[ignore]
    fn live_lookup() {
        let c = super::find("Daft Punk", "Discovery", "", 3).unwrap();
        assert!(!c.is_empty(), "no candidates");
        for x in &c {
            println!("{} - {} ({:?}) score {} {}", x.artist, x.release, x.year, x.score, x.label);
        }
        let dir = std::env::temp_dir().join("chameleon-live");
        let p = super::download(&c[0].image, &dir).unwrap();
        println!("downloaded {}", p.display());
        assert!(image::open(&p).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
