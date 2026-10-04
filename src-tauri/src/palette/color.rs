//! sRGB, linear light, HSL and WCAG contrast helpers (mirrors the design's
//! palette.js so results match the spec).

pub type Rgb = [u8; 3];

pub fn lin(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn unlin(v: f32) -> u8 {
    let v = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (255.0 * v.clamp(0.0, 1.0)).round() as u8
}

/// WCAG relative luminance.
pub fn luminance(c: Rgb) -> f32 {
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Mix in linear light (avoids the muddy midpoints of sRGB mixing).
pub fn mix_lin(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [0, 1, 2].map(|i| unlin(lin(a[i]) * (1.0 - t) + lin(b[i]) * t))
}

/// (h, s, l), each 0..1.
pub fn rgb_to_hsl(c: Rgb) -> (f32, f32, f32) {
    let [r, g, b] = c.map(|v| v as f32 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    if mx == mn {
        return (0.0, 0.0, l);
    }
    let d = mx - mn;
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h = if mx == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Rgb {
    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return [v, v, v];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |t: f32| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    [f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_roundtrip() {
        for c in [[255, 0, 0], [12, 200, 99], [0, 0, 0], [255, 255, 255], [30, 60, 200], [214, 28, 40]] {
            let (h, s, l) = rgb_to_hsl(c);
            let back = hsl_to_rgb(h, s, l);
            for i in 0..3 {
                assert!((c[i] as i32 - back[i] as i32).abs() <= 1, "{c:?} -> {back:?}");
            }
        }
    }

    #[test]
    fn contrast_extremes() {
        assert!((contrast([255, 255, 255], [0, 0, 0]) - 21.0).abs() < 0.01);
        assert!((contrast([90, 90, 90], [90, 90, 90]) - 1.0).abs() < 0.001);
    }

    #[test]
    fn linear_mix_midpoint_is_brighter_than_srgb() {
        let m = mix_lin([0, 0, 0], [255, 255, 255], 0.5);
        assert!(m[0] > 180, "linear-light midpoint of black/white is ~188, got {}", m[0]);
    }
}
