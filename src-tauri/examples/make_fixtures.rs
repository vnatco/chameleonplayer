//! Generates a small test library: tone WAV files tagged with synthetic
//! covers chosen to stress palette extraction (dark, bright, white,
//! grayscale, busy, single-color, folder art, no art).
//!
//! Usage: cargo run --example make_fixtures -- <output dir>

use chameleon_lib::tags::{self, CoverEdit, TagEdit};
use image::{Rgb, RgbImage};
use std::fs;
use std::path::PathBuf;

const RATE: u32 = 44_100;
const SECONDS: u32 = 12;

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("usage: make_fixtures <output dir>"));
    fs::create_dir_all(&out).unwrap();
    let covers = out.join("_covers");
    fs::create_dir_all(&covers).unwrap();

    let albums: Vec<(&str, &str, &str, Option<RgbImage>, f32)> = vec![
        ("Neon Maze", "D.O.D & J-Trick", "Electro House", Some(neon()), 220.0),
        ("Sunset Drive", "Coastline", "Synthwave", Some(sunset()), 247.0),
        ("Paper", "Minimal Club", "Ambient", Some(white()), 262.0),
        ("Static", "Grey Area", "Techno", Some(mono()), 294.0),
        ("Confetti", "Party People", "Pop", Some(busy()), 330.0),
        ("Red", "Single Color", "Rock", Some(solid(214, 28, 40)), 349.0),
        ("Folder Art", "Old Rips", "House", None, 392.0),
        ("Untitled", "Unknown Artist", "Demo", None, 440.0),
    ];

    for (album, artist, genre, cover, base_hz) in albums {
        let dir = out.join(sanitize(artist)).join(sanitize(album));
        fs::create_dir_all(&dir).unwrap();
        let cover_path = cover.map(|img| {
            let p = covers.join(format!("{}.png", sanitize(album)));
            img.save(&p).unwrap();
            p
        });
        if album == "Folder Art" {
            folder_art().save(dir.join("cover.jpg")).unwrap();
        }
        for n in 1..=3u32 {
            let path = dir.join(format!("{n:02} - {album} {n}.wav"));
            fs::write(&path, tone(base_hz * (1.0 + (n - 1) as f32 * 0.25))).unwrap();
            let edit = TagEdit {
                title: format!("{album} Part {n}"),
                artist: artist.into(),
                album: album.into(),
                album_artist: String::new(),
                genre: genre.into(),
                year: Some(2010 + n),
                track_no: Some(n),
                disc_no: None,
                cover: match &cover_path {
                    Some(p) => CoverEdit::Replace { path: p.clone() },
                    None => CoverEdit::Keep,
                },
            };
            tags::write(&path, &edit).unwrap();
        }
        println!("{}", dir.display());
    }
}

fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect()
}

/// Stereo 16-bit tone with a gentle beat, so the pulse meter has something
/// to follow.
fn tone(hz: f32) -> Vec<u8> {
    let frames = RATE * SECONDS;
    let data_len = frames * 4;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&RATE.to_le_bytes());
    v.extend_from_slice(&(RATE * 4).to_le_bytes());
    v.extend_from_slice(&4u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        let t = i as f32 / RATE as f32;
        // 120 BPM kick-ish envelope on a low sine, plus the melody tone.
        let beat = (-((t * 2.0).fract() * 12.0)).exp();
        let kick = (2.0 * std::f32::consts::PI * 55.0 * t).sin() * beat * 0.5;
        let lead = (2.0 * std::f32::consts::PI * hz * t).sin() * 0.18;
        let s = ((kick + lead) * i16::MAX as f32) as i16;
        v.extend_from_slice(&s.to_le_bytes());
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

const S: u32 = 512;

fn neon() -> RgbImage {
    RgbImage::from_fn(S, S, |x, y| {
        let grid = (x / 32 + y / 32) % 2 == 0 && (x % 32 < 3 || y % 32 < 3);
        if grid {
            if x < S / 2 { Rgb([40, 220, 255]) } else { Rgb([230, 60, 255]) }
        } else {
            Rgb([12, 14, 40])
        }
    })
}

fn sunset() -> RgbImage {
    RgbImage::from_fn(S, S, |_, y| {
        let t = y as f32 / S as f32;
        let lerp = |a: f32, b: f32| (a + (b - a) * t) as u8;
        Rgb([lerp(255.0, 70.0), lerp(150.0, 20.0), lerp(40.0, 120.0)])
    })
}

fn white() -> RgbImage {
    RgbImage::from_fn(S, S, |x, y| {
        if (200..312).contains(&x) && (200..312).contains(&y) { Rgb([20, 20, 20]) } else { Rgb([246, 244, 240]) }
    })
}

fn mono() -> RgbImage {
    let mut seed = 7u32;
    RgbImage::from_fn(S, S, |_, _| {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let v = 60 + ((seed >> 16) % 120) as u8;
        Rgb([v, v, v])
    })
}

fn busy() -> RgbImage {
    let palette = [[255, 89, 94], [255, 202, 58], [138, 201, 38], [25, 130, 196], [106, 76, 147], [255, 255, 255]];
    RgbImage::from_fn(S, S, |x, y| {
        let i = ((x / 37) * 7 + (y / 29) * 13 + (x / 91) * (y / 53)) as usize % palette.len();
        Rgb(palette[i])
    })
}

fn solid(r: u8, g: u8, b: u8) -> RgbImage {
    RgbImage::from_pixel(S, S, Rgb([r, g, b]))
}

fn folder_art() -> RgbImage {
    RgbImage::from_fn(S, S, |x, y| {
        let d = ((x as f32 - 256.0).powi(2) + (y as f32 - 256.0).powi(2)).sqrt();
        if d < 180.0 { Rgb([30, 160, 120]) } else { Rgb([8, 40, 36]) }
    })
}
