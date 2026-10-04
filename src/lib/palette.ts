// Palette runtime: strength, mirroring, linear-light interpolation, and the
// CSS tokens (spec F). Palettes themselves are computed by the backend.

import type { Palette, Rgb } from "./api";

// sRGB <-> linear light, via lookup tables (this runs every frame of a fade).
const LIN = new Float32Array(256);
for (let i = 0; i < 256; i++) {
  const c = i / 255;
  LIN[i] = c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}
const UNLIN_N = 4096;
const UNLIN = new Uint8Array(UNLIN_N + 1);
for (let i = 0; i <= UNLIN_N; i++) {
  const v = i / UNLIN_N;
  UNLIN[i] = Math.round(255 * Math.min(1, Math.max(0, v <= 0.0031308 ? v * 12.92 : 1.055 * Math.pow(v, 1 / 2.4) - 0.055)));
}
const unlin = (v: number) => UNLIN[Math.round(Math.min(1, Math.max(0, v)) * UNLIN_N)];

type Lin = [number, number, number];
const toLin = (c: Rgb): Lin => [LIN[c[0]], LIN[c[1]], LIN[c[2]]];
const toRgb = (c: Lin): Rgb => [unlin(c[0]), unlin(c[1]), unlin(c[2])];
const mix = (a: Lin, b: Lin, t: number): Lin => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];

const ROLE_KEYS = ["bg", "surface", "surface2", "accent", "onAccent", "text", "subtle", "edge"] as const;
const SIDES = ["t", "r", "b", "l"] as const;
type RoleKey = (typeof ROLE_KEYS)[number];
type Side = (typeof SIDES)[number];

/** A palette prepared for interpolation: every color in linear light. */
export interface LinPalette {
  light: boolean;
  roles: Record<RoleKey, Lin>;
  glow: Record<Side, Lin>;
  glowA: Record<Side, number>;
  stops: Record<Side, [Lin, Lin, Lin]>;
}

const NEUTRAL_DARK = { bg: [22, 22, 25], surface: [32, 32, 36], surface2: [44, 44, 50] } as const;
const NEUTRAL_LIGHT = { bg: [242, 242, 244], surface: [251, 251, 252], surface2: [226, 226, 230] } as const;

/**
 * Prepare a palette: `strength` 0 = subtle (surfaces near-neutral, accent and
 * glow stay cover-true), 1 = full chameleon. `mirror` swaps the glow left and
 * right, for the flipped sleeve.
 */
export function prepare(p: Palette, strength = 1, mirror = false): LinPalette {
  const light = p.scheme === "light";
  const roles = {} as Record<RoleKey, Lin>;
  for (const k of ROLE_KEYS) roles[k] = toLin(p.roles[k]);
  if (strength < 0.999) {
    const n = light ? NEUTRAL_LIGHT : NEUTRAL_DARK;
    const t = 0.2 + 0.8 * Math.max(0, strength);
    for (const k of ["bg", "surface", "surface2"] as const) roles[k] = mix(toLin(n[k] as unknown as Rgb), roles[k], t);
  }
  const glow = {} as Record<Side, Lin>;
  const glowA = {} as Record<Side, number>;
  const stops = {} as Record<Side, [Lin, Lin, Lin]>;
  for (const s of SIDES) {
    const src: Side = mirror ? (s === "l" ? "r" : s === "r" ? "l" : s) : s;
    glow[s] = toLin(p.glow[src]);
    glowA[s] = p.glowA[src];
    const st = p.glowStops[src].map(toLin) as [Lin, Lin, Lin];
    // Mirrored, top and bottom also run the other way.
    stops[s] = mirror && (s === "t" || s === "b") ? [st[2], st[1], st[0]] : st;
  }
  return { light, roles, glow, glowA, stops };
}

export function lerp(a: LinPalette, b: LinPalette, t: number): LinPalette {
  const roles = {} as Record<RoleKey, Lin>;
  for (const k of ROLE_KEYS) roles[k] = mix(a.roles[k], b.roles[k], t);
  const glow = {} as Record<Side, Lin>;
  const glowA = {} as Record<Side, number>;
  const stops = {} as Record<Side, [Lin, Lin, Lin]>;
  for (const s of SIDES) {
    glow[s] = mix(a.glow[s], b.glow[s], t);
    glowA[s] = a.glowA[s] + (b.glowA[s] - a.glowA[s]) * t;
    stops[s] = [mix(a.stops[s][0], b.stops[s][0], t), mix(a.stops[s][1], b.stops[s][1], t), mix(a.stops[s][2], b.stops[s][2], t)];
  }
  // The scheme flips at the midpoint, while text is crossing mid-gray.
  return { light: t < 0.5 ? a.light : b.light, roles, glow, glowA, stops };
}

const css = (c: Lin, a?: number) => {
  const [r, g, b] = toRgb(c);
  return a == null ? `rgb(${r} ${g} ${b})` : `rgb(${r} ${g} ${b} / ${a.toFixed(3)})`;
};

/** The spec's token set (F), plus per-segment glow stops. */
export function toVars(p: LinPalette): Record<string, string> {
  const r = p.roles;
  const v: Record<string, string> = {
    "--ch-bg": css(r.bg),
    "--ch-surface": css(r.surface),
    "--ch-surface-2": css(r.surface2),
    "--ch-accent": css(r.accent),
    "--ch-on-accent": css(r.onAccent),
    "--ch-accent-soft": css(r.accent, 0.16),
    "--ch-accent-line": css(r.accent, 0.45),
    "--ch-text": css(r.text),
    "--ch-text-subtle": css(r.subtle),
    "--ch-hairline": css(r.text, 0.12),
    "--ch-hover": css(r.text, 0.07),
    "--ch-edge": css(r.edge),
    "--ch-scrim": css(r.bg, p.light ? 0.88 : 0.86),
    "--ch-scrim-0": css(r.bg, 0),
    "--ch-text-shadow": p.light ? "0 1px 2px rgb(255 255 255 / .55)" : "0 1px 2px rgb(0 0 0 / .45)",
    "--ch-mica": css(r.bg, p.light ? 0.8 : 0.84),
    "--ch-panel": css(r.surface, p.light ? 0.72 : 0.62),
    "--ch-danger": p.light ? "#b3261e" : "#ff99a4",
  };
  for (const s of SIDES) {
    v[`--ch-glow-${s}`] = css(p.glow[s], p.glowA[s]);
    for (let i = 0; i < 3; i++) v[`--ch-glow-${s}-${i}`] = css(p.stops[s][i], p.glowA[s]);
  }
  return v;
}

/** Cubic in-out, the spec's cubic-bezier(.65,0,.35,1). */
const ease = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);

/**
 * Drives the tokens on one element (the document root, or an album page that
 * wears its own palette). Writes only the variables that changed, once per
 * animation frame, and stops when the fade is done.
 */
export class PaletteDriver {
  private shown: LinPalette | null = null;
  private written = new Map<string, string>();
  private raf = 0;
  onScheme?: (light: boolean) => void;

  constructor(private el: HTMLElement) {}

  get current() {
    return this.shown;
  }

  set(target: LinPalette, duration: number) {
    cancelAnimationFrame(this.raf);
    const from = this.shown;
    if (!from || duration <= 0) {
      this.apply(target);
      return;
    }
    const t0 = performance.now();
    const step = (now: number) => {
      const t = Math.min(1, (now - t0) / duration);
      this.apply(lerp(from, target, ease(t)));
      if (t < 1) this.raf = requestAnimationFrame(step);
    };
    this.raf = requestAnimationFrame(step);
  }

  private apply(p: LinPalette) {
    const light = this.shown?.light;
    this.shown = p;
    const style = this.el.style;
    for (const [k, v] of Object.entries(toVars(p))) {
      if (this.written.get(k) !== v) {
        style.setProperty(k, v);
        this.written.set(k, v);
      }
    }
    if (light !== p.light) {
      this.el.dataset.scheme = p.light ? "light" : "dark";
      this.onScheme?.(p.light);
    }
  }

  dispose() {
    cancelAnimationFrame(this.raf);
  }
}

/** Static tokens for an element that doesn't animate (genre tiles, previews). */
export function staticVars(p: Palette, strength = 1): string {
  return Object.entries(toVars(prepare(p, strength)))
    .map(([k, v]) => `${k}:${v}`)
    .join(";");
}

export const rgbCss = (c: Rgb, a?: number) => (a == null ? `rgb(${c[0]} ${c[1]} ${c[2]})` : `rgb(${c[0]} ${c[1]} ${c[2]} / ${a})`);
