//! Cover art: finding it (embedded or folder image), caching it on disk and
//! extracting its palette once.
//!
//! Covers are content-addressed by a hash of the image bytes, so an album's
//! twelve tracks with the same embedded art share one cached file.

use super::db::NewCover;
use crate::palette;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};
use lofty::picture::PictureType;
use lofty::tag::Tag;
use std::fs;
use std::path::{Path, PathBuf};

/// Covers are stored up to this size; plenty for a large player window.
const FULL_MAX: u32 = 1400;
const THUMB_SIZE: u32 = 320;

/// File names (without extension) treated as folder cover art, best first.
const FOLDER_NAMES: &[&str] = &["cover", "folder", "front", "album", "albumart", "artwork"];
const FOLDER_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp"];

#[derive(Clone)]
pub struct CoverStore {
    full_dir: PathBuf,
    thumb_dir: PathBuf,
}

impl CoverStore {
    pub fn new(root: &Path) -> std::io::Result<Self> {
        let full_dir = root.join("full");
        let thumb_dir = root.join("thumb");
        fs::create_dir_all(&full_dir)?;
        fs::create_dir_all(&thumb_dir)?;
        Ok(Self { full_dir, thumb_dir })
    }

    /// Decode, cache and analyse cover bytes. `hash` must be [`hash`] of `bytes`.
    pub fn process(&self, hash: &str, bytes: &[u8]) -> Result<NewCover, String> {
        let format = image::guess_format(bytes).map_err(|e| format!("Unknown image format: {e}"))?;
        let img = image::load_from_memory_with_format(bytes, format).map_err(|e| format!("Can't decode image: {e}"))?;
        let (width, height) = (img.width(), img.height());
        if width == 0 || height == 0 {
            return Err("Image has no pixels".into());
        }

        let palette_json = serde_json::to_string(&palette::extract(&img)).map_err(|e| e.to_string())?;

        // Keep the original bytes when they are already a sensible size and a
        // format the webview shows natively; otherwise re-encode.
        let keep_original = width.max(height) <= FULL_MAX && matches!(format, ImageFormat::Jpeg | ImageFormat::Png);
        let full = if keep_original {
            let ext = if format == ImageFormat::Png { "png" } else { "jpg" };
            let path = self.full_dir.join(format!("{hash}.{ext}"));
            write_atomic(&path, bytes)?;
            path
        } else {
            let path = self.full_dir.join(format!("{hash}.jpg"));
            let scaled = if width.max(height) > FULL_MAX { img.resize(FULL_MAX, FULL_MAX, FilterType::Lanczos3) } else { img.clone() };
            write_atomic(&path, &encode_jpeg(&scaled, 92)?)?;
            path
        };

        let thumb = self.thumb_dir.join(format!("{hash}.jpg"));
        write_atomic(&thumb, &encode_jpeg(&img.resize_to_fill(THUMB_SIZE, THUMB_SIZE, FilterType::Lanczos3), 85)?)?;

        Ok(NewCover {
            hash: hash.to_string(),
            full: full.to_string_lossy().into_owned(),
            thumb: thumb.to_string_lossy().into_owned(),
            width,
            height,
            palette_json,
        })
    }
}

impl CoverStore {
    /// Remove cached files whose hash (file stem) isn't in `keep`, plus any
    /// stray `.tmp` files from an interrupted write.
    pub fn sweep(&self, keep: &std::collections::HashSet<String>) {
        for dir in [&self.full_dir, &self.thumb_dir] {
            let Ok(entries) = fs::read_dir(dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let is_tmp = path.extension().is_some_and(|e| e == "tmp");
                if is_tmp || !keep.contains(stem) {
                    if let Err(e) = fs::remove_file(&path) {
                        log::warn!("Can't delete cached cover {}: {e}", path.display());
                    }
                }
            }
        }
    }
}

pub fn hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex()[..32].to_string()
}

/// Best embedded picture: the front cover, else whatever comes first.
pub fn embedded(tag: &Tag) -> Option<Vec<u8>> {
    tag.get_picture_type(PictureType::CoverFront)
        .or_else(|| tag.pictures().first())
        .map(|p| p.data().to_vec())
        .filter(|d| !d.is_empty())
}

/// Look for cover.jpg / folder.png / ... next to the audio file.
pub fn find_in_folder(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    let mut best: Option<(usize, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let (Some(stem), Some(ext)) = (path.file_stem().and_then(|s| s.to_str()), path.extension().and_then(|s| s.to_str())) else {
            continue;
        };
        if !FOLDER_EXTS.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
            continue;
        }
        if let Some(rank) = FOLDER_NAMES.iter().position(|n| n.eq_ignore_ascii_case(stem)) {
            if best.as_ref().is_none_or(|(r, _)| rank < *r) {
                best = Some((rank, path));
            }
        }
    }
    best.map(|(_, p)| p)
}

fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality).encode_image(&img.to_rgb8()).map_err(|e| format!("Can't encode JPEG: {e}"))?;
    Ok(out)
}

/// Write to a sibling temp file, then rename over the target, so a crash
/// never leaves a truncated cover in the cache.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|e| format!("Can't write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Can't write {}: {e}", path.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("chameleon-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(w, h, |x, _| if x < w / 2 { Rgb([200, 30, 30]) } else { Rgb([20, 20, 180]) });
        let mut out = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img).write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn process_writes_full_and_thumb() {
        let dir = temp_dir("covers");
        let store = CoverStore::new(&dir).unwrap();
        let bytes = png_bytes(600, 600);
        let c = store.process(&hash(&bytes), &bytes).unwrap();
        assert!(c.full.ends_with(".png"), "small PNG is kept as-is");
        assert!(Path::new(&c.full).exists() && Path::new(&c.thumb).exists());
        assert_eq!((c.width, c.height), (600, 600));
        let thumb = image::open(&c.thumb).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (THUMB_SIZE, THUMB_SIZE));
        assert!(c.palette_json.contains("\"edges\""));

        let big = png_bytes(2000, 2000);
        let c = store.process(&hash(&big), &big).unwrap();
        assert!(c.full.ends_with(".jpg"), "oversized cover is re-encoded");
        let full = image::open(&c.full).unwrap();
        assert_eq!(full.width(), FULL_MAX);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_garbage() {
        let dir = temp_dir("garbage");
        let store = CoverStore::new(&dir).unwrap();
        assert!(store.process("x", b"definitely not an image").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn folder_cover_prefers_cover_over_folder() {
        let dir = temp_dir("folder");
        fs::write(dir.join("Folder.JPG"), b"x").unwrap();
        fs::write(dir.join("cover.png"), b"x").unwrap();
        fs::write(dir.join("random.jpg"), b"x").unwrap();
        let found = find_in_folder(&dir).unwrap();
        assert_eq!(found.file_name().unwrap(), "cover.png");
        let _ = fs::remove_dir_all(&dir);
    }
}
