//! Normalizes images into the built-in fallback covers: square, 1200 px,
//! JPEG. Re-encoding drops all embedded metadata (EXIF, color profiles,
//! provenance chunks).
//!
//! Usage: cargo run --example make_fallbacks -- <source dir> <output dir>

use image::{codecs::jpeg::JpegEncoder, imageops::FilterType};
use std::fs;
use std::path::PathBuf;

const SIZE: u32 = 1200;

fn main() {
    let mut args = std::env::args().skip(1);
    let src = PathBuf::from(args.next().expect("usage: make_fallbacks <source dir> <output dir>"));
    let out = PathBuf::from(args.next().expect("usage: make_fallbacks <source dir> <output dir>"));
    fs::create_dir_all(&out).unwrap();
    let mut files: Vec<PathBuf> = fs::read_dir(&src).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    files.sort();
    let mut n = 0;
    for f in files {
        let Ok(img) = image::open(&f) else {
            eprintln!("skip (not an image): {}", f.display());
            continue;
        };
        n += 1;
        let sq = img.resize_to_fill(SIZE, SIZE, FilterType::Lanczos3).to_rgb8();
        let mut bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut bytes, 90).encode_image(&sq).unwrap();
        let dst = out.join(format!("{n:02}.jpg"));
        fs::write(&dst, &bytes).unwrap();
        println!("{} -> {} ({} KB)", f.display(), dst.display(), bytes.len() / 1024);
    }
}
