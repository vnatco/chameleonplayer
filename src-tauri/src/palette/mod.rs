//! Cover -> palette extraction: the "chameleon" in Chameleon Player.
//!
//! The original app averaged the cover's edge pixels into one border color,
//! which tends to produce mud (red + green edges -> brown). Here we:
//!
//! 1. cluster the whole cover in OKLab (k-means) to find real swatches,
//! 2. cluster each edge band separately so the glow can differ per side,
//! 3. derive UI roles (background, text, accent, ...) from those swatches and
//!    nudge their lightness until they meet WCAG contrast targets.
//!
//! Field names are provisional until the design's token names land; the
//! frontend maps these onto CSS custom properties.

pub mod color;

use color::{Lab, Lch, Rgb, Swatch};
use image::{imageops::FilterType, DynamicImage};
use serde::Serialize;

/// Bump whenever extraction or the output shape changes; cached palettes
/// with an older version are recomputed on startup.
pub const VERSION: i64 = 4;

/// Longest side the cover is reduced to before analysis. Plenty for color,
/// and keeps extraction to a few milliseconds.
const ANALYSIS_SIZE: u32 = 96;
/// Edge bands are sampled from a larger copy; see [`extract`].
const EDGE_ANALYSIS_SIZE: u32 = 256;
/// Fraction of the cover's width/height that counts as an "edge band".
const EDGE_BAND: f32 = 0.08;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    Dark,
    Light,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sides {
    pub top: Swatch,
    pub right: Swatch,
    pub bottom: Swatch,
    pub left: Swatch,
}

#[derive(Debug, Clone, Serialize)]
pub struct SideStops {
    pub top: [Swatch; 3],
    pub right: [Swatch; 3],
    pub bottom: [Swatch; 3],
    pub left: [Swatch; 3],
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Roles {
    pub background: Swatch,
    pub surface: Swatch,
    pub surface_raised: Swatch,
    pub border: Swatch,
    pub text: Swatch,
    pub text_subtle: Swatch,
    pub accent: Swatch,
    pub on_accent: Swatch,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeightedSwatch {
    #[serde(flatten)]
    pub swatch: Swatch,
    /// Share of the cover's pixels, 0..1.
    pub population: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    pub scheme: Scheme,
    pub dominant: Swatch,
    pub vibrant: Swatch,
    pub muted: Swatch,
    /// True edge colors, for the crisp thin border.
    pub edges: Sides,
    /// Edge colors lifted so they read as light on a dark desktop.
    pub glow: Sides,
    /// The glow sampled in three segments along each side, so it can be
    /// drawn as a gradient (top/bottom run left to right, left/right run top
    /// to bottom).
    pub glow_stops: SideStops,
    pub roles: Roles,
    /// All clusters, largest first. Handy for debugging and for designers.
    pub swatches: Vec<WeightedSwatch>,
    /// False when this is the neutral no-cover fallback.
    pub from_cover: bool,
}

/// Extract a palette from decoded cover art.
pub fn extract(img: &DynamicImage) -> Palette {
    let all = lab_pixels(&img.resize(ANALYSIS_SIZE, ANALYSIS_SIZE, FilterType::Triangle));

    // Edges come from a sharper copy so thin bright details (neon lines,
    // lettering) keep their color instead of blurring into the background.
    let edge_img = img.resize(EDGE_ANALYSIS_SIZE, EDGE_ANALYSIS_SIZE, FilterType::Triangle).to_rgb8();
    let (w, h) = edge_img.dimensions();
    let px = |x: u32, y: u32| {
        let p = edge_img.get_pixel(x, y);
        Rgb::from_u8(p[0], p[1], p[2]).to_lab()
    };
    let bw = ((w as f32 * EDGE_BAND).round() as u32).max(1);
    let bh = ((h as f32 * EDGE_BAND).round() as u32).max(1);
    let band = |xs: std::ops::Range<u32>, ys: std::ops::Range<u32>| -> Vec<Lab> {
        ys.flat_map(|y| xs.clone().map(move |x| (x, y))).map(|(x, y)| px(x, y)).collect()
    };
    let edge = |pts: Vec<Lab>| -> Lch { biggest_cluster(&pts, 3).to_lch() };
    let emit = |pts: Vec<Lab>| -> Lch { emissive_cluster(&pts).to_lch() };
    let sides_px = [band(0..w, 0..bh), band(w - bw..w, 0..h), band(0..w, h - bh..h), band(0..bw, 0..h)];
    let edges_lch = sides_px.clone().map(edge);
    let glow_lch = sides_px.map(emit);
    // Thirds along each side.
    let third = |n: u32, i: u32| (n * i / 3)..(n * (i + 1) / 3).max(n * i / 3 + 1);
    let stops = [
        [0, 1, 2].map(|i| emit(band(third(w, i), 0..bh))),
        [0, 1, 2].map(|i| emit(band(w - bw..w, third(h, i)))),
        [0, 1, 2].map(|i| emit(band(third(w, i), h - bh..h))),
        [0, 1, 2].map(|i| emit(band(0..bw, third(h, i)))),
    ];

    let clusters = kmeans(&all, 8, 12);
    build(clusters, edges_lch, glow_lch, stops, mean_lightness(&all), true)
}

fn lab_pixels(img: &DynamicImage) -> Vec<Lab> {
    img.to_rgb8().pixels().map(|p| Rgb::from_u8(p[0], p[1], p[2]).to_lab()).collect()
}

/// Neutral palette used when a track has no cover. Deliberately cool and
/// slightly tinted so it still feels "alive" rather than broken.
pub fn fallback() -> Palette {
    let base = Lch { l: 0.32, c: 0.035, h: 265.0 };
    let accent = Lch { l: 0.72, c: 0.11, h: 280.0 };
    let clusters = vec![
        Cluster { centroid: base.to_lab(), population: 0.8 },
        Cluster { centroid: accent.to_lab(), population: 0.2 },
    ];
    // A soft lavender halo, so "no cover" still feels alive.
    let glow = Lch { l: 0.6, c: 0.075, h: 278.0 };
    build(clusters, [base; 4], [glow; 4], [[glow; 3]; 4], 0.3, false)
}

fn build(mut clusters: Vec<Cluster>, edges: [Lch; 4], glow: [Lch; 4], stops: [[Lch; 3]; 4], mean_l: f32, from_cover: bool) -> Palette {
    clusters.sort_by(|a, b| b.population.total_cmp(&a.population));
    let lch: Vec<(Lch, f32)> = clusters.iter().map(|c| (c.centroid.to_lch(), c.population)).collect();

    let dominant = lch[0].0;
    let vibrant = lch
        .iter()
        .filter(|(c, p)| (0.35..=0.88).contains(&c.l) && c.c > 0.05 && *p > 0.01)
        .max_by(|(a, pa), (b, pb)| (a.c * pa.powf(0.3)).total_cmp(&(b.c * pb.powf(0.3))))
        .map(|(c, _)| *c)
        .unwrap_or(dominant);
    let muted = lch
        .iter()
        .filter(|(c, p)| c.c < 0.08 && (0.2..=0.85).contains(&c.l) && *p > 0.02)
        .max_by(|(_, pa), (_, pb)| pa.total_cmp(pb))
        .map(|(c, _)| *c)
        .unwrap_or_else(|| dominant.with_c(dominant.c * 0.35));

    // Only near-white covers get a light UI; dark is the better stage for art.
    let scheme = if mean_l > 0.78 { Scheme::Light } else { Scheme::Dark };
    let roles = roles(scheme, dominant, vibrant);

    let glow = glow.map(lift);
    let [t, r, b, l] = stops.map(|side| side.map(|e| Swatch::from(lift(e))));
    let glow_stops = SideStops { top: t, right: r, bottom: b, left: l };

    Palette {
        scheme,
        dominant: dominant.into(),
        vibrant: vibrant.into(),
        muted: muted.into(),
        edges: sides(edges),
        glow: sides(glow),
        glow_stops,
        roles,
        swatches: lch.iter().map(|(c, p)| WeightedSwatch { swatch: (*c).into(), population: *p }).collect(),
        from_cover,
    }
}

/// Lift dark/dull edge colors so the glow is visible; keep the hue honest.
fn lift(e: Lch) -> Lch {
    let l = e.l.clamp(0.55, 0.85);
    let c = if e.c < 0.02 { e.c } else { (e.c * 1.3).min(0.26) };
    Lch { l, c, h: e.h }
}

fn sides([top, right, bottom, left]: [Lch; 4]) -> Sides {
    Sides { top: top.into(), right: right.into(), bottom: bottom.into(), left: left.into() }
}

fn roles(scheme: Scheme, dominant: Lch, vibrant: Lch) -> Roles {
    let hue = dominant.h;
    // Keep neutrals only gently tinted; a fully saturated background is loud.
    let tint = dominant.c.min(0.05);
    let neutral = |l: f32, c_scale: f32| Lch { l, c: tint * c_scale, h: hue };

    let (background, surface, surface_raised, border, text_l, subtle_l) = match scheme {
        Scheme::Dark => (neutral(0.17, 0.7), neutral(0.215, 0.75), neutral(0.26, 0.8), neutral(0.34, 1.0), 0.96, 0.76),
        Scheme::Light => (neutral(0.975, 0.3), neutral(0.94, 0.4), neutral(0.905, 0.5), neutral(0.83, 0.8), 0.22, 0.45),
    };
    let bg = background.to_rgb();

    let text = ensure_contrast(Lch { l: text_l, c: tint.min(0.02), h: hue }, bg, 7.0, scheme);
    let text_subtle = ensure_contrast(Lch { l: subtle_l, c: tint.min(0.035), h: hue }, bg, 4.5, scheme);

    let mut accent = if vibrant.c > 0.04 { vibrant } else { dominant.with_l(match scheme { Scheme::Dark => 0.78, Scheme::Light => 0.45 }) };
    accent = ensure_contrast(accent, bg, 3.0, scheme);

    let accent_rgb = accent.to_rgb();
    let light_on = Lch { l: 0.98, c: 0.01, h: accent.h };
    let dark_on = Lch { l: 0.18, c: 0.02, h: accent.h };
    let light_wins = light_on.to_rgb().contrast(accent_rgb) >= dark_on.to_rgb().contrast(accent_rgb);
    let mut on_accent = if light_wins { light_on } else { dark_on };
    // Tinted near-white/near-black can fall short on mid-lightness accents;
    // pure white or black always reaches at least 4.58:1 against any color.
    if on_accent.to_rgb().contrast(accent_rgb) < 4.5 {
        let white = Lch { l: 1.0, c: 0.0, h: accent.h };
        let black = Lch { l: 0.0, c: 0.0, h: accent.h };
        on_accent = if white.to_rgb().contrast(accent_rgb) >= black.to_rgb().contrast(accent_rgb) { white } else { black };
    }

    Roles {
        background: background.into(),
        surface: surface.into(),
        surface_raised: surface_raised.into(),
        border: border.into(),
        text: text.into(),
        text_subtle: text_subtle.into(),
        accent: accent.into(),
        on_accent: on_accent.into(),
    }
}

/// Move `c` away from the background's lightness until it reaches `min`
/// contrast. Dark schemes push lighter, light schemes push darker.
fn ensure_contrast(mut c: Lch, bg: Rgb, min: f32, scheme: Scheme) -> Lch {
    let step = match scheme {
        Scheme::Dark => 0.01,
        Scheme::Light => -0.01,
    };
    for _ in 0..100 {
        if c.to_rgb().contrast(bg) >= min {
            break;
        }
        c = c.with_l(c.l + step);
    }
    c
}

fn mean_lightness(pts: &[Lab]) -> f32 {
    if pts.is_empty() {
        return 0.0;
    }
    pts.iter().map(|p| p.l).sum::<f32>() / pts.len() as f32
}

#[derive(Debug, Clone, Copy)]
struct Cluster {
    centroid: Lab,
    population: f32,
}

/// The edge color that would "emit" light: favours bright, saturated
/// clusters over a large dark background (neon lines on navy glow cyan, not
/// grey), while a uniform edge still wins on sheer size.
fn emissive_cluster(pts: &[Lab]) -> Lab {
    kmeans(pts, 4, 10)
        .into_iter()
        .map(|c| {
            let lch = c.centroid.to_lch();
            // Roughly the light the cluster gives off: area x brightness^2 x color.
            (c.centroid, c.population * lch.l * lch.l * (0.05 + lch.c))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(c, _)| c)
        .unwrap_or_default()
}

fn biggest_cluster(pts: &[Lab], k: usize) -> Lab {
    kmeans(pts, k, 10)
        .into_iter()
        .max_by(|a, b| a.population.total_cmp(&b.population))
        .map(|c| c.centroid)
        .unwrap_or_default()
}

/// Deterministic k-means in OKLab with farthest-point seeding, so the same
/// cover always yields the same palette.
fn kmeans(pts: &[Lab], k: usize, iters: usize) -> Vec<Cluster> {
    if pts.is_empty() {
        return vec![Cluster { centroid: Lab::default(), population: 1.0 }];
    }
    let mean = Lab {
        l: mean_lightness(pts),
        a: pts.iter().map(|p| p.a).sum::<f32>() / pts.len() as f32,
        b: pts.iter().map(|p| p.b).sum::<f32>() / pts.len() as f32,
    };
    let first = *pts.iter().min_by(|a, b| a.dist2(mean).total_cmp(&b.dist2(mean))).unwrap();
    let mut centroids = vec![first];
    while centroids.len() < k {
        let far = pts
            .iter()
            .max_by(|a, b| {
                let da = centroids.iter().map(|c| a.dist2(*c)).fold(f32::MAX, f32::min);
                let db = centroids.iter().map(|c| b.dist2(*c)).fold(f32::MAX, f32::min);
                da.total_cmp(&db)
            })
            .copied()
            .unwrap();
        // Image has fewer distinct colors than k.
        if centroids.iter().any(|c| c.dist2(far) < 1e-6) {
            break;
        }
        centroids.push(far);
    }

    let mut assign = vec![0usize; pts.len()];
    for _ in 0..iters {
        for (i, p) in pts.iter().enumerate() {
            assign[i] = (0..centroids.len()).min_by(|&a, &b| p.dist2(centroids[a]).total_cmp(&p.dist2(centroids[b]))).unwrap();
        }
        let mut sums = vec![(Lab::default(), 0usize); centroids.len()];
        for (p, &ci) in pts.iter().zip(&assign) {
            let s = &mut sums[ci];
            s.0.l += p.l;
            s.0.a += p.a;
            s.0.b += p.b;
            s.1 += 1;
        }
        let mut moved = false;
        for (c, (s, n)) in centroids.iter_mut().zip(&sums) {
            if *n > 0 {
                let next = Lab { l: s.l / *n as f32, a: s.a / *n as f32, b: s.b / *n as f32 };
                moved |= next.dist2(*c) > 1e-8;
                *c = next;
            }
        }
        if !moved {
            break;
        }
    }

    let mut counts = vec![0usize; centroids.len()];
    for &ci in &assign {
        counts[ci] += 1;
    }
    centroids
        .into_iter()
        .zip(counts)
        .filter(|(_, n)| *n > 0)
        .map(|(centroid, n)| Cluster { centroid, population: n as f32 / pts.len() as f32 })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb as Px, RgbImage};

    fn hex_rgb(hex: &str) -> Rgb {
        let v = u32::from_str_radix(&hex[1..], 16).unwrap();
        Rgb::from_u8((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    fn solid(r: u8, g: u8, b: u8) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 64, Px([r, g, b])))
    }

    fn assert_contrast(p: &Palette) {
        let bg = hex_rgb(&p.roles.background.hex);
        assert!(hex_rgb(&p.roles.text.hex).contrast(bg) >= 7.0, "text contrast");
        assert!(hex_rgb(&p.roles.text_subtle.hex).contrast(bg) >= 4.5, "subtle contrast");
        assert!(hex_rgb(&p.roles.accent.hex).contrast(bg) >= 3.0, "accent contrast");
        let acc = hex_rgb(&p.roles.accent.hex);
        assert!(hex_rgb(&p.roles.on_accent.hex).contrast(acc) >= 4.5, "on-accent contrast");
    }

    #[test]
    fn edge_cases_meet_contrast() {
        for img in [solid(0, 0, 0), solid(255, 255, 255), solid(128, 128, 128), solid(230, 20, 30), solid(255, 240, 0)] {
            assert_contrast(&extract(&img));
        }
        assert_contrast(&fallback());
    }

    #[test]
    fn white_cover_is_light_scheme() {
        assert_eq!(extract(&solid(250, 250, 250)).scheme, Scheme::Light);
        assert_eq!(extract(&solid(10, 10, 30)).scheme, Scheme::Dark);
    }

    #[test]
    fn edges_are_sampled_per_side() {
        // Blue top half, orange bottom half.
        let img = RgbImage::from_fn(64, 64, |_, y| if y < 32 { Px([20, 60, 220]) } else { Px([240, 130, 20]) });
        let p = extract(&DynamicImage::ImageRgb8(img));
        let top = p.edges.top.h;
        let bottom = p.edges.bottom.h;
        assert!((240.0..290.0).contains(&top), "top hue {top}");
        assert!((40.0..80.0).contains(&bottom), "bottom hue {bottom}");
        // The left side runs blue (top third) to orange (bottom third).
        assert!((240.0..290.0).contains(&p.glow_stops.left[0].h), "left start {}", p.glow_stops.left[0].h);
        assert!((40.0..80.0).contains(&p.glow_stops.left[2].h), "left end {}", p.glow_stops.left[2].h);
    }

    #[test]
    fn glow_follows_bright_lines_not_dark_background() {
        // Navy cover with thin cyan lines: the border stays navy, the glow is cyan.
        let img = RgbImage::from_fn(512, 512, |x, y| {
            if x % 32 < 3 || y % 32 < 3 { Px([40, 220, 255]) } else { Px([12, 14, 40]) }
        });
        let p = extract(&DynamicImage::ImageRgb8(img));
        assert!(p.edges.top.l < 0.3, "border keeps the true dominant edge color");
        let g = &p.glow.top;
        assert!((190.0..240.0).contains(&g.h) && g.c > 0.08, "glow is cyan, got {g:?}");
    }

    #[test]
    fn vibrant_beats_dominant_grey() {
        // Mostly grey with a red stripe: accent should be the red, not grey.
        let img = RgbImage::from_fn(64, 64, |x, _| if x < 12 { Px([220, 30, 40]) } else { Px([120, 120, 120]) });
        let p = extract(&DynamicImage::ImageRgb8(img));
        assert!(p.vibrant.c > 0.1);
        assert!(p.roles.accent.c > 0.08);
    }
}
