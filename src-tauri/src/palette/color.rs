//! Color math: sRGB <-> OKLab/OKLCH, gamut mapping and WCAG contrast.
//!
//! OKLab is used for all clustering and adjustment because distances and
//! lightness in it match perception, which avoids the muddy results of
//! averaging in RGB (the original player's main color problem).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Lab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lch {
    pub l: f32,
    pub c: f32,
    /// Hue in degrees, 0..360.
    pub h: f32,
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

impl Rgb {
    pub fn from_u8(r: u8, g: u8, b: u8) -> Self {
        Self { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0 }
    }

    pub fn to_lab(self) -> Lab {
        let (r, g, b) = (srgb_to_linear(self.r), srgb_to_linear(self.g), srgb_to_linear(self.b));
        let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_99 * b).cbrt();
        let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
        let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
        Lab {
            l: 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
            a: 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
            b: 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
        }
    }

    pub fn in_gamut(self) -> bool {
        const E: f32 = 1e-4;
        [self.r, self.g, self.b].iter().all(|c| (-E..=1.0 + E).contains(c))
    }

    pub fn clamped(self) -> Self {
        Self { r: self.r.clamp(0.0, 1.0), g: self.g.clamp(0.0, 1.0), b: self.b.clamp(0.0, 1.0) }
    }

    pub fn hex(self) -> String {
        let c = self.clamped();
        let to = |v: f32| (v * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}", to(c.r), to(c.g), to(c.b))
    }

    /// WCAG 2.x relative luminance.
    pub fn luminance(self) -> f32 {
        let c = self.clamped();
        0.2126 * srgb_to_linear(c.r) + 0.7152 * srgb_to_linear(c.g) + 0.0722 * srgb_to_linear(c.b)
    }

    /// WCAG 2.x contrast ratio, 1..21.
    pub fn contrast(self, other: Rgb) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }
}

impl Lab {
    pub fn to_rgb(self) -> Rgb {
        let l = (self.l + 0.396_337_78 * self.a + 0.215_803_76 * self.b).powi(3);
        let m = (self.l - 0.105_561_346 * self.a - 0.063_854_17 * self.b).powi(3);
        let s = (self.l - 0.089_484_18 * self.a - 1.291_485_5 * self.b).powi(3);
        Rgb {
            r: linear_to_srgb(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
            g: linear_to_srgb(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s),
            b: linear_to_srgb(-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s),
        }
    }

    pub fn to_lch(self) -> Lch {
        let h = self.b.atan2(self.a).to_degrees();
        Lch { l: self.l, c: (self.a * self.a + self.b * self.b).sqrt(), h: if h < 0.0 { h + 360.0 } else { h } }
    }

    pub fn dist2(self, o: Lab) -> f32 {
        let (dl, da, db) = (self.l - o.l, self.a - o.a, self.b - o.b);
        dl * dl + da * da + db * db
    }
}

impl Lch {
    pub fn to_lab(self) -> Lab {
        let h = self.h.to_radians();
        Lab { l: self.l, a: self.c * h.cos(), b: self.c * h.sin() }
    }

    pub fn with_l(self, l: f32) -> Self {
        Self { l: l.clamp(0.0, 1.0), ..self }
    }

    pub fn with_c(self, c: f32) -> Self {
        Self { c: c.max(0.0), ..self }
    }

    /// Map into sRGB by reducing chroma (keeping lightness and hue), which is
    /// far less jarring than per-channel clipping.
    pub fn to_rgb(self) -> Rgb {
        let direct = self.to_lab().to_rgb();
        if direct.in_gamut() {
            return direct.clamped();
        }
        let (mut lo, mut hi) = (0.0_f32, self.c);
        for _ in 0..20 {
            let mid = (lo + hi) / 2.0;
            if self.with_c(mid).to_lab().to_rgb().in_gamut() {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        self.with_c(lo).to_lab().to_rgb().clamped()
    }
}

/// A finished color as handed to the frontend.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Swatch {
    pub hex: String,
    /// OKLCH components so the UI can derive tints (e.g. `oklch(l c h / a)`).
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

impl From<Lch> for Swatch {
    fn from(lch: Lch) -> Self {
        let rgb = lch.to_rgb();
        // Report the gamut-mapped values so hex and oklch agree.
        let actual = rgb.to_lab().to_lch();
        let round = |v: f32, p: f32| (v * p).round() / p;
        Swatch { hex: rgb.hex(), l: round(actual.l, 1000.0), c: round(actual.c, 1000.0), h: round(actual.h, 10.0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_rgb_lab() {
        for &(r, g, b) in &[(255, 0, 0), (12, 200, 99), (0, 0, 0), (255, 255, 255), (30, 60, 200)] {
            let c = Rgb::from_u8(r, g, b);
            let back = c.to_lab().to_rgb();
            assert!((c.r - back.r).abs() < 1e-3 && (c.g - back.g).abs() < 1e-3 && (c.b - back.b).abs() < 1e-3);
        }
    }

    #[test]
    fn contrast_extremes() {
        let w = Rgb::from_u8(255, 255, 255);
        let k = Rgb::from_u8(0, 0, 0);
        assert!((w.contrast(k) - 21.0).abs() < 0.01);
        assert!((w.contrast(w) - 1.0).abs() < 0.001);
    }

    #[test]
    fn gamut_mapping_stays_in_gamut() {
        let wild = Lch { l: 0.7, c: 0.5, h: 140.0 };
        assert!(wild.to_rgb().in_gamut());
    }
}
