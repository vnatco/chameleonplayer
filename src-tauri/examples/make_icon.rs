//! Renders the app icon (1024 px PNG) from geometry: a graphite tile with a
//! glowing, gradient "sleeve" mark (a framed square with a filled center,
//! the same mark as the tray icon).
//!
//! Usage: cargo run --example make_icon -- <out.png>
//! Then:  npx tauri icon <out.png>

use image::{Rgba, RgbaImage};

const N: u32 = 1024;
const SS: u32 = 4;

fn main() {
    let out = std::env::args().nth(1).expect("usage: make_icon <out.png>");
    let mut img = RgbaImage::new(N, N);
    let f = N as f32;

    let tile_r = 0.19 * f; // tile corner radius
    let inset = 0.035 * f; // tile margin
    let (m0, m1) = (0.27 * f, 0.73 * f); // sleeve outer square
    let frame = 0.065 * f; // frame thickness
    let (c0, c1) = (0.415 * f, 0.585 * f); // filled center

    for py in 0..N {
        for px in 0..N {
            let mut acc = [0f32; 4];
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = px as f32 + (sx as f32 + 0.5) / SS as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / SS as f32;
                    let c = sample(x, y, f, tile_r, inset, m0, m1, frame, c0, c1);
                    // Premultiplied accumulation.
                    acc[0] += c[0] * c[3];
                    acc[1] += c[1] * c[3];
                    acc[2] += c[2] * c[3];
                    acc[3] += c[3];
                }
            }
            let n = (SS * SS) as f32;
            let a = acc[3] / n;
            let px_col = if a > 0.0 {
                [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3], a]
            } else {
                [0.0; 4]
            };
            img.put_pixel(px, py, Rgba(px_col.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)));
        }
    }
    img.save(&out).unwrap();
    println!("wrote {out}");
}

/// Linear-ish gradient along the diagonal: magenta -> amber -> teal.
fn gradient(t: f32) -> [f32; 3] {
    let stops = [(0.0, [0.91, 0.30, 0.86]), (0.5, [1.0, 0.62, 0.25]), (1.0, [0.20, 0.86, 0.80])];
    let t = t.clamp(0.0, 1.0);
    for w in stops.windows(2) {
        let ((t0, a), (t1, b)) = (w[0], w[1]);
        if t <= t1 {
            let k = (t - t0) / (t1 - t0);
            return [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k];
        }
    }
    stops[2].1
}

fn rounded_inside(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    if x < x0 || y < y0 || x > x1 || y > y1 {
        return false;
    }
    let cx = x.clamp(x0 + r, x1 - r);
    let cy = y.clamp(y0 + r, y1 - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

#[allow(clippy::too_many_arguments)]
fn sample(x: f32, y: f32, f: f32, tile_r: f32, inset: f32, m0: f32, m1: f32, frame: f32, c0: f32, c1: f32) -> [f32; 4] {
    if !rounded_inside(x, y, inset, inset, f - inset, f - inset, tile_r) {
        return [0.0; 4];
    }
    let diag = (x + y) / (2.0 * f);
    let grad = gradient((diag - 0.25) / 0.5);

    let in_outer = x >= m0 && x <= m1 && y >= m0 && y <= m1;
    let in_hole = x >= m0 + frame && x <= m1 - frame && y >= m0 + frame && y <= m1 - frame;
    let in_center = x >= c0 && x <= c1 && y >= c0 && y <= c1;
    if (in_outer && !in_hole) || in_center {
        return [grad[0], grad[1], grad[2], 1.0];
    }

    // Graphite tile with a soft glow around the sleeve.
    let base = [0.075, 0.075, 0.09];
    let dx = (m0 - x).max(x - m1).max(0.0);
    let dy = (m0 - y).max(y - m1).max(0.0);
    let d = (dx * dx + dy * dy).sqrt();
    let inside_frame = in_outer; // the dark gap between frame and center
    let glow = if inside_frame { 0.10 } else { (-(d / (0.075 * f)).powi(2)).exp() * 0.55 };
    [base[0] + (grad[0] - base[0]) * glow, base[1] + (grad[1] - base[1]) * glow, base[2] + (grad[2] - base[2]) * glow, 1.0]
}
