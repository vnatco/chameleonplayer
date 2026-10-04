//! Reading and safely writing tags and embedded cover art.
//!
//! Writes never touch the original file until the new version is complete:
//! the file is copied, the copy is tagged and re-read to verify it, and only
//! then is it moved over the original.

use image::ImageFormat;
use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::{Accessor, ItemKey, TagExt, TaggedFileExt};
use lofty::tag::items::Timestamp;
use lofty::tag::Tag;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub year: Option<u32>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub has_embedded_cover: bool,
    pub read_only: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum CoverEdit {
    Keep,
    Remove,
    /// Replace with an image file chosen or dropped by the user.
    Replace { path: PathBuf },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagEdit {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub year: Option<u32>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub cover: CoverEdit,
}

pub fn read(path: &Path) -> Result<TagInfo, String> {
    let tagged = lofty::read_from_path(path).map_err(|e| format!("Can't read tags: {e}"))?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let s = |v: Option<std::borrow::Cow<'_, str>>| v.map(|x| x.into_owned()).unwrap_or_default();
    Ok(TagInfo {
        path: path.to_string_lossy().into_owned(),
        title: tag.map(|t| s(t.title())).unwrap_or_default(),
        artist: tag.map(|t| s(t.artist())).unwrap_or_default(),
        album: tag.map(|t| s(t.album())).unwrap_or_default(),
        album_artist: tag.and_then(|t| t.get_string(ItemKey::AlbumArtist)).unwrap_or_default().to_string(),
        genre: tag.map(|t| s(t.genre())).unwrap_or_default(),
        year: tag.and_then(|t| t.date()).map(|d| d.year as u32).filter(|y| *y > 0),
        track_no: tag.and_then(|t| t.track()),
        disc_no: tag.and_then(|t| t.disk()),
        has_embedded_cover: tag.is_some_and(|t| t.picture_count() > 0),
        read_only: is_read_only(path),
    })
}

fn is_read_only(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.permissions().readonly()).unwrap_or(true)
}

pub fn write(path: &Path, edit: &TagEdit) -> Result<(), String> {
    if is_read_only(path) {
        return Err(format!("{} is read-only. Clear the read-only flag in its properties to edit it.", file_name(path)));
    }
    // Prepare the cover first so a bad image fails before any file work.
    let picture = match &edit.cover {
        CoverEdit::Replace { path } => Some(load_picture(path)?),
        _ => None,
    };

    let tmp = temp_sibling(path);
    fs::copy(path, &tmp).map_err(|e| format!("Can't prepare {} for editing: {e}", file_name(path)))?;
    let result = (|| {
        apply(&tmp, edit, picture)?;
        // Verify the edited copy still parses before it replaces the original.
        lofty::read_from_path(&tmp).map_err(|e| format!("The edited file didn't verify ({e}); the original was left untouched."))?;
        fs::rename(&tmp, path).map_err(|e| format!("Can't save {} (is it open in another program?): {e}", file_name(path)))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn apply(file: &Path, edit: &TagEdit, picture: Option<Picture>) -> Result<(), String> {
    let mut tagged = lofty::read_from_path(file).map_err(|e| format!("Can't read tags: {e}"))?;
    if tagged.primary_tag().is_none() {
        let tag_type = tagged.primary_tag_type();
        tagged.insert_tag(Tag::new(tag_type));
    }
    let tag = tagged.primary_tag_mut().ok_or("This file type can't store tags")?;

    set_text(tag, edit.title.trim(), Tag::set_title, Tag::remove_title);
    set_text(tag, edit.artist.trim(), Tag::set_artist, Tag::remove_artist);
    set_text(tag, edit.album.trim(), Tag::set_album, Tag::remove_album);
    set_text(tag, edit.genre.trim(), Tag::set_genre, Tag::remove_genre);
    let album_artist = edit.album_artist.trim();
    if album_artist.is_empty() {
        tag.remove_key(ItemKey::AlbumArtist);
    } else {
        tag.insert_text(ItemKey::AlbumArtist, album_artist.to_string());
    }
    match edit.year.filter(|y| (1..=9999).contains(y)) {
        Some(year) => tag.set_date(Timestamp { year: year as u16, ..Timestamp::default() }),
        None => tag.remove_date(),
    }
    match edit.track_no.filter(|n| *n > 0) {
        Some(n) => tag.set_track(n),
        None => tag.remove_track(),
    }
    match edit.disc_no.filter(|n| *n > 0) {
        Some(n) => tag.set_disk(n),
        None => tag.remove_disk(),
    }

    match (&edit.cover, picture) {
        (CoverEdit::Remove, _) => remove_pictures(tag),
        (CoverEdit::Replace { .. }, Some(pic)) => {
            remove_pictures(tag);
            tag.push_picture(pic);
        }
        _ => {}
    }

    tag.save_to_path(file, WriteOptions::default()).map_err(|e| format!("Can't write tags: {e}"))
}

fn set_text(tag: &mut Tag, value: &str, set: fn(&mut Tag, String), remove: fn(&mut Tag)) {
    if value.is_empty() {
        remove(tag);
    } else {
        set(tag, value.to_string());
    }
}

fn remove_pictures(tag: &mut Tag) {
    while tag.picture_count() > 0 {
        tag.remove_picture(0);
    }
}

/// Load an image for embedding. JPEG and PNG go in as-is; anything else
/// (WebP, BMP, GIF, ...) is converted to JPEG, which every player can show.
fn load_picture(path: &Path) -> Result<Picture, String> {
    let bytes = fs::read(path).map_err(|e| format!("Can't read {}: {e}", file_name(path)))?;
    let format = image::guess_format(&bytes).map_err(|_| format!("{} isn't an image", file_name(path)))?;
    let img = image::load_from_memory_with_format(&bytes, format).map_err(|e| format!("Can't decode {}: {e}", file_name(path)))?;
    let (data, mime) = match format {
        ImageFormat::Jpeg => (bytes, MimeType::Jpeg),
        ImageFormat::Png => (bytes, MimeType::Png),
        _ => {
            let mut out = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92)
                .encode_image(&img.to_rgb8())
                .map_err(|e| format!("Can't convert {}: {e}", file_name(path)))?;
            (out, MimeType::Jpeg)
        }
    };
    Ok(Picture::unchecked(data).pic_type(PictureType::CoverFront).mime_type(mime).build())
}

/// `song.mp3` -> `~song.tagedit.mp3` in the same folder (same volume, so the
/// final rename is atomic; same extension, so the format is detected).
fn temp_sibling(path: &Path) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    path.with_file_name(format!("~{stem}.tagedit{ext}"))
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testutil::wav_bytes;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("chameleon-tags-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn edit(cover: CoverEdit) -> TagEdit {
        TagEdit {
            title: "Ping".into(),
            artist: "D.O.D & J-Trick".into(),
            album: "Speeeedy EDM".into(),
            album_artist: "Various".into(),
            genre: "Electro House".into(),
            year: Some(2016),
            track_no: Some(3),
            disc_no: None,
            cover,
        }
    }

    #[test]
    fn write_roundtrip_with_cover() {
        let dir = temp_dir("roundtrip");
        let song = dir.join("song.wav");
        fs::write(&song, wav_bytes()).unwrap();
        let png = dir.join("art.png");
        image::RgbImage::from_pixel(8, 8, image::Rgb([200, 10, 10])).save(&png).unwrap();

        write(&song, &edit(CoverEdit::Replace { path: png })).unwrap();
        let info = read(&song).unwrap();
        assert_eq!(info.title, "Ping");
        assert_eq!(info.artist, "D.O.D & J-Trick");
        assert_eq!(info.album_artist, "Various");
        assert_eq!(info.year, Some(2016));
        assert_eq!(info.track_no, Some(3));
        assert!(info.has_embedded_cover);
        assert!(!dir.join("~song.tagedit.wav").exists(), "temp file cleaned up");

        write(&song, &edit(CoverEdit::Remove)).unwrap();
        assert!(!read(&song).unwrap().has_embedded_cover);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_cover_leaves_file_untouched() {
        let dir = temp_dir("badcover");
        let song = dir.join("song.wav");
        fs::write(&song, wav_bytes()).unwrap();
        let before = fs::read(&song).unwrap();
        let fake = dir.join("fake.png");
        fs::write(&fake, b"not an image").unwrap();
        assert!(write(&song, &edit(CoverEdit::Replace { path: fake })).is_err());
        assert_eq!(fs::read(&song).unwrap(), before);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn can_save_while_file_is_playing() {
        use std::io::Read;
        let dir = temp_dir("playing");
        let song = dir.join("song.wav");
        fs::write(&song, wav_bytes()).unwrap();
        let mut playing = crate::audio::open_shared(&song).unwrap();
        let mut head = [0u8; 4];
        playing.read_exact(&mut head).unwrap();

        write(&song, &edit(CoverEdit::Keep)).unwrap();
        assert_eq!(read(&song).unwrap().title, "Ping");
        // The open handle still reads the original data.
        let mut rest = Vec::new();
        playing.read_to_end(&mut rest).unwrap();
        assert_eq!(&head, b"RIFF");
        drop(playing);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_only_is_refused() {
        let dir = temp_dir("readonly");
        let song = dir.join("song.wav");
        fs::write(&song, wav_bytes()).unwrap();
        let mut perms = fs::metadata(&song).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&song, perms.clone()).unwrap();
        let err = write(&song, &edit(CoverEdit::Keep)).unwrap_err();
        assert!(err.contains("read-only"));
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        fs::set_permissions(&song, perms).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }
}
