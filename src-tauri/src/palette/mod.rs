//! Cover -> palette: the "chameleon" in Chameleon Player.
//!
//! Implements the design's color system (Chameleon Spec, section F):
//!
//! 1. Sample: downscale to 48x48, average a 5 px strip per edge in linear
//!    light for the glow sources, bucket colors (512 buckets) and measure mean
//!    luminance and chroma.
//! 2. Pick dominant / vibrant / muted swatches.
//! 3. Build roles from the dominant hue at fixed lightness steps.
//! 4. Enforce contrast by walking lightness only (text >= 7:1, subtle and
//!    accent >= 4.5:1), never hue.
//!
//! On top of the spec's four glow colors, each edge is also sampled in three
//! segments so the glow can shift color along an edge.
//!
//! The palette is computed once per cover (at scan time) and cached; the UI
//! only interpolates between finished palettes.

pub mod color;

use color::{contrast, hsl_to_rgb, lin, mix_lin, rgb_to_hsl, unlin, Rgb};
use image::{imageops::FilterType, DynamicImage};
use serde::Serialize;

/// Bump whenever extraction or the output shape changes; cached palettes
/// with an older version are recomputed on startup.
pub const VERSION: i64 = 6;

const N: u32 = 48;
/// Edge strip depth in pixels of the 48x48 sample.
const D: u32 = 5;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    Dark,
    Light,
}

#[derive(Debug, Clone, Serialize)]
pub struct Roles {
    pub bg: Rgb,
    pub surface: Rgb,
    pub surface2: Rgb,
    pub accent: Rgb,
    #[serde(rename = "onAccent")]
    pub on_accent: Rgb,
    pub text: Rgb,
    pub subtle: Rgb,
    /// Mean of the four glow colors: the 1 px sleeve border.
    pub edge: Rgb,
}

/// Per-side values, keyed like the spec's tokens (t/r/b/l).
#[derive(Debug, Clone, Serialize)]
pub struct Sides<T> {
    pub t: T,
    pub r: T,
    pub b: T,
    pub l: T,
}

#[derive(Debug, Clone, Serialize)]
pub struct Ratios {
    pub text: f32,
    pub subtle: f32,
    pub accent: f32,
    #[serde(rename = "onAccent")]
    pub on_accent: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    pub scheme: Scheme,
    pub gray: bool,
    /// True for the neutral no-cover palette.
    pub fallback: bool,
    pub roles: Roles,
    pub glow: Sides<Rgb>,
    #[serde(rename = "glowA")]
    pub glow_a: Sides<f32>,
    /// Three segments per side: top/bottom run left to right, left/right run
    /// top to bottom.
    pub glow_stops: Sides<[Rgb; 3]>,
    pub dominant: Rgb,
    pub vibrant: Rgb,
    pub muted: Rgb,
    pub swatches: Vec<Rgb>,
    pub ratios: Ratios,
    /// Contrast corrections applied, for the spec's debug readout.
    pub fixes: Vec<String>,
}

/// A color bucket: mean color, pixel count, and its HSL.
type Entry = (Rgb, u32, (f32, f32, f32));

struct Analysis {
    edges: Sides<Rgb>,
    segments: Sides<[Rgb; 3]>,
    dominant: Rgb,
    vibrant: Rgb,
    muted: Rgb,
    mean_lum: f32,
    chroma: f32,
    swatches: Vec<Rgb>,
}

/// Extract a palette from decoded cover art.
pub fn extract(img: &DynamicImage) -> Palette {
    derive(&analyze(img))
}

fn analyze(img: &DynamicImage) -> Analysis {
    let small = img.resize_exact(N, N, FilterType::Triangle).to_rgb8();
    let px = |x: u32, y: u32| {
        let p = small.get_pixel(x, y);
        [p[0], p[1], p[2]]
    };

    // Edge strips: whole side, plus three segments along it.
    let strip = |side: usize, from: u32, to: u32| -> Rgb {
        let mut acc = [0f32; 3];
        let mut n = 0f32;
        for d in 0..D {
            for i in from..to {
                let p = match side {
                    0 => px(i, d),
                    1 => px(N - 1 - d, i),
                    2 => px(i, N - 1 - d),
                    _ => px(d, i),
                };
                for (a, v) in acc.iter_mut().zip(p) {
                    *a += lin(v);
                }
                n += 1.0;
            }
        }
        [unlin(acc[0] / n), unlin(acc[1] / n), unlin(acc[2] / n)]
    };
    let thirds = |side: usize| [strip(side, 0, N / 3), strip(side, N / 3, 2 * N / 3), strip(side, 2 * N / 3, N)];
    let edges = Sides { t: strip(0, 0, N), r: strip(1, 0, N), b: strip(2, 0, N), l: strip(3, 0, N) };
    let segments = Sides { t: thirds(0), r: thirds(1), b: thirds(2), l: thirds(3) };

    // 3 bits per channel buckets.
    #[derive(Clone, Copy, Default)]
    struct Bucket {
        n: u32,
        sum: [u32; 3],
    }
    let mut buckets = vec![Bucket::default(); 512];
    let (mut total_lum, mut sat_w) = (0f32, 0f32);
    for p in small.pixels() {
        let p = [p[0], p[1], p[2]];
        let k = ((p[0] >> 5) as usize) << 6 | ((p[1] >> 5) as usize) << 3 | (p[2] >> 5) as usize;
        let b = &mut buckets[k];
        b.n += 1;
        for (s, v) in b.sum.iter_mut().zip(p) {
            *s += v as u32;
        }
        total_lum += color::luminance(p);
        let (_, s, l) = rgb_to_hsl(p);
        sat_w += s * (1.0 - (l - 0.5).abs() * 2.0);
    }
    let total = (N * N) as f32;
    let mut list: Vec<Entry> = buckets
        .iter()
        .filter(|b| b.n > 0)
        .map(|b| {
            let c = [0, 1, 2].map(|i| (b.sum[i] as f32 / b.n as f32).round() as u8);
            (c, b.n, rgb_to_hsl(c))
        })
        .collect();
    list.sort_by_key(|e| std::cmp::Reverse(e.1));

    let frac = |n: u32| n as f32 / total;
    let best = |score: &dyn Fn(&Entry) -> Option<f32>| -> Rgb {
        list.iter()
            .filter_map(|e| score(e).map(|s| (e.0, s)))
            .fold((list[0].0, -1.0f32), |b, (c, s)| if s > b.1 { (c, s) } else { b })
            .0
    };
    let vibrant = best(&|e| {
        let (_, s, l) = e.2;
        (frac(e.1) >= 0.004).then(|| s * (1.0 - (l - 0.55).abs() * 1.6) * frac(e.1).powf(0.25))
    });
    let muted = best(&|e| {
        let (_, s, l) = e.2;
        (frac(e.1) >= 0.01).then(|| (0.5 - (s - 0.25).abs()) * (1.0 - (l - 0.4).abs()) * frac(e.1).powf(0.3))
    });

    Analysis {
        edges,
        segments,
        dominant: list[0].0,
        vibrant,
        muted,
        mean_lum: total_lum / total,
        chroma: sat_w / total,
        swatches: list.iter().take(6).map(|e| e.0).collect(),
    }
}

/// Walk lightness in `dir` until contrast against `bg` reaches `target`.
fn ensure(hsl: (f32, f32, f32), bg: Rgb, target: f32, dir: f32, fixes: &mut Vec<String>, role: &str) -> Rgb {
    let (h, s, start) = hsl;
    let mut l = start;
    for _ in 0..80 {
        if contrast(hsl_to_rgb(h, s, l), bg) >= target {
            break;
        }
        l = (l + dir * 0.012).clamp(0.0, 1.0);
    }
    if (l - start).abs() > 0.001 {
        fixes.push(format!("{role} L {}->{}%", (start * 100.0).round(), (l * 100.0).round()));
    }
    hsl_to_rgb(h, s, l)
}

fn derive(a: &Analysis) -> Palette {
    let mut fixes = Vec::new();
    let gray = a.chroma < 0.07;
    let light = a.mean_lum > 0.42;
    let (dh, ds, _) = rgb_to_hsl(a.dominant);
    let (vh, vsat, vl) = rgb_to_hsl(a.vibrant);
    let hue = if ds > 0.15 { dh } else { vh };
    let tint = if gray { 0.02 } else { ds.max(vsat * 0.5).min(0.42) };

    let (bg, surface, surface2, text, subtle, accent);
    if !light {
        bg = hsl_to_rgb(hue, tint, 0.10);
        surface = hsl_to_rgb(hue, tint, 0.15);
        surface2 = hsl_to_rgb(hue, tint * 0.9, 0.21);
        text = ensure((hue, tint * 0.35, 0.94), bg, 7.0, 1.0, &mut fixes, "text");
        subtle = ensure((hue, tint * 0.3, 0.66), bg, 4.5, 1.0, &mut fixes, "textSubtle");
        accent = if gray {
            ensure((hue, 0.03, 0.8), bg, 4.5, 1.0, &mut fixes, "accent")
        } else {
            ensure((vh, vsat.clamp(0.55, 0.95), vl.clamp(0.52, 0.66)), bg, 4.5, 1.0, &mut fixes, "accent")
        };
    } else {
        bg = hsl_to_rgb(hue, tint * 0.7, 0.95);
        surface = hsl_to_rgb(hue, tint * 0.6, 0.985);
        surface2 = hsl_to_rgb(hue, tint * 0.7, 0.89);
        text = ensure((hue, tint * 0.5, 0.13), bg, 7.0, -1.0, &mut fixes, "text");
        subtle = ensure((hue, tint * 0.4, 0.4), bg, 4.5, -1.0, &mut fixes, "textSubtle");
        accent = if gray {
            ensure((hue, 0.03, 0.28), bg, 4.5, -1.0, &mut fixes, "accent")
        } else {
            ensure((vh, vsat.clamp(0.55, 0.95), vl.clamp(0.32, 0.5)), bg, 4.5, -1.0, &mut fixes, "accent")
        };
    }
    const W: Rgb = [255, 255, 255];
    const K: Rgb = [14, 14, 16];
    let on_accent = if contrast(W, accent) >= contrast(K, accent) { W } else { K };

    // Glow: saturate, clamp lightness; near-black edges cast no light, so mix
    // them with the accent and dim them.
    let mut dark_mixed = false;
    let mut glow_of = |c: Rgb| -> (Rgb, f32) {
        let (h, s, l) = rgb_to_hsl(c);
        let dark = color::luminance(c) < 0.02;
        let s = if gray { s } else { (s * 1.3).clamp(0.0, 1.0) };
        let mut g = hsl_to_rgb(h, s, l.clamp(0.3, 0.72));
        if dark {
            g = mix_lin(g, accent, 0.5);
            dark_mixed = true;
        }
        (g, if dark { 0.55 } else { 0.85 })
    };
    let (gt, at) = glow_of(a.edges.t);
    let (gr, ar) = glow_of(a.edges.r);
    let (gb, ab) = glow_of(a.edges.b);
    let (gl, al) = glow_of(a.edges.l);
    let glow_stops = Sides {
        t: a.segments.t.map(|c| glow_of(c).0),
        r: a.segments.r.map(|c| glow_of(c).0),
        b: a.segments.b.map(|c| glow_of(c).0),
        l: a.segments.l.map(|c| glow_of(c).0),
    };
    if dark_mixed {
        fixes.push("glow: dark edge -> accent mix".into());
    }
    let edge = mix_lin(mix_lin(gt, gb, 0.5), mix_lin(gl, gr, 0.5), 0.5);

    Palette {
        scheme: if light { Scheme::Light } else { Scheme::Dark },
        gray,
        fallback: false,
        ratios: Ratios {
            text: contrast(text, bg),
            subtle: contrast(subtle, bg),
            accent: contrast(accent, bg),
            on_accent: contrast(on_accent, accent),
        },
        roles: Roles { bg, surface, surface2, accent, on_accent, text, subtle, edge },
        glow: Sides { t: gt, r: gr, b: gb, l: gl },
        glow_a: Sides { t: at, r: ar, b: ab, l: al },
        glow_stops,
        dominant: a.dominant,
        vibrant: a.vibrant,
        muted: a.muted,
        swatches: a.swatches.clone(),
        fixes,
    }
}

/// `#rrggbb` for a color.
pub fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

impl Palette {
    /// Compact tint for small thumbnails: (edge border, bottom glow).
    pub fn tint(&self) -> (String, String) {
        (hex(self.roles.edge), hex(self.glow.b))
    }
}

/// The neutral graphite palette for tracks without art (spec A5).
pub fn fallback() -> Palette {
    let roles = Roles {
        bg: [19, 19, 22],
        surface: [28, 28, 32],
        surface2: [40, 40, 46],
        accent: [214, 214, 224],
        on_accent: [14, 14, 16],
        text: [238, 238, 242],
        subtle: [158, 158, 170],
        edge: [96, 96, 108],
    };
    let glow = Sides { t: [150, 150, 162], r: [140, 144, 160], b: [120, 120, 134], l: [160, 156, 166] };
    Palette {
        scheme: Scheme::Dark,
        gray: true,
        fallback: true,
        ratios: Ratios {
            text: contrast(roles.text, roles.bg),
            subtle: contrast(roles.subtle, roles.bg),
            accent: contrast(roles.accent, roles.bg),
            on_accent: contrast(roles.on_accent, roles.accent),
        },
        glow_stops: Sides { t: [glow.t; 3], r: [glow.r; 3], b: [glow.b; 3], l: [glow.l; 3] },
        glow,
        glow_a: Sides { t: 0.4, r: 0.4, b: 0.4, l: 0.4 },
        dominant: roles.bg,
        vibrant: roles.accent,
        muted: roles.surface2,
        swatches: vec![],
        fixes: vec![],
        roles,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb as Px, RgbImage};

    fn img(f: impl Fn(u32, u32) -> [u8; 3]) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(300, 300, |x, y| Px(f(x, y))))
    }

    fn assert_contrast(p: &Palette) {
        let r = &p.roles;
        assert!(contrast(r.text, r.bg) >= 7.0, "text {}", p.ratios.text);
        assert!(contrast(r.subtle, r.bg) >= 4.5, "subtle {}", p.ratios.subtle);
        assert!(contrast(r.accent, r.bg) >= 4.5, "accent {}", p.ratios.accent);
        assert!(contrast(r.on_accent, r.accent) >= 4.5, "on-accent {}", p.ratios.on_accent);
    }

    #[test]
    fn edge_cases_meet_contrast() {
        for c in [[0, 0, 0], [255, 255, 255], [128, 128, 128], [214, 28, 40], [255, 240, 0], [20, 40, 200], [250, 180, 220]] {
            assert_contrast(&extract(&img(|_, _| c)));
        }
        assert_contrast(&fallback());
    }

    #[test]
    fn scheme_and_gray_detection() {
        let white = extract(&img(|_, _| [246, 244, 240]));
        assert_eq!(white.scheme, Scheme::Light);
        assert!(white.gray);
        let red = extract(&img(|_, _| [214, 28, 40]));
        assert_eq!(red.scheme, Scheme::Dark);
        assert!(!red.gray);
    }

    #[test]
    fn edges_and_segments_follow_the_art() {
        // Blue top half, orange bottom half.
        let p = extract(&img(|_, y| if y < 150 { [20, 60, 220] } else { [240, 130, 20] }));
        let (th, _, _) = rgb_to_hsl(p.glow.t);
        let (bh, _, _) = rgb_to_hsl(p.glow.b);
        assert!((0.55..0.72).contains(&th), "top hue {th}");
        assert!((0.04..0.14).contains(&bh), "bottom hue {bh}");
        // The left edge runs blue -> orange top to bottom.
        let (l0, _, _) = rgb_to_hsl(p.glow_stops.l[0]);
        let (l2, _, _) = rgb_to_hsl(p.glow_stops.l[2]);
        assert!((0.55..0.72).contains(&l0) && (0.04..0.14).contains(&l2), "{l0} {l2}");
    }

    #[test]
    fn black_edges_take_the_accent() {
        // Black sleeve with an orange center: edges glow orange, dimmed.
        let p = extract(&img(|x, y| if (90..210).contains(&x) && (90..210).contains(&y) { [255, 120, 0] } else { [0, 0, 0] }));
        assert_eq!(p.glow_a.t, 0.55);
        let (h, s, _) = rgb_to_hsl(p.glow.t);
        assert!(s > 0.3 && (0.03..0.14).contains(&h), "glow takes the orange accent: h {h} s {s}");
        assert!(p.fixes.iter().any(|f| f.contains("accent mix")));
    }

    #[test]
    fn deterministic() {
        let a = extract(&img(|x, y| [(x % 256) as u8, (y % 256) as u8, 90]));
        let b = extract(&img(|x, y| [(x % 256) as u8, (y % 256) as u8, 90]));
        assert_eq!(serde_json::to_string(&a).unwrap(), serde_json::to_string(&b).unwrap());
    }
}
