// Applies a cover palette to the document as CSS custom properties.
// Token names are provisional until the design's token list lands.

import type { Palette, Swatch } from "./api";

const kebab = (s: string) => s.replace(/[A-Z]/g, (m) => "-" + m.toLowerCase());

export function paletteVars(p: Palette): Record<string, string> {
  const vars: Record<string, string> = {};
  const put = (name: string, s: Swatch) => {
    vars[`--c-${name}`] = s.hex;
    // Raw OKLCH parts so styles can derive tints: oklch(var(--c-x-lch) / 0.4)
    vars[`--c-${name}-lch`] = `${s.l} ${s.c} ${s.h}`;
  };
  for (const [k, s] of Object.entries(p.roles)) put(kebab(k), s);
  for (const side of ["top", "right", "bottom", "left"] as const) {
    put(`edge-${side}`, p.edges[side]);
    put(`glow-${side}`, p.glow[side]);
  }
  for (const side of ["top", "right", "bottom", "left"] as const) {
    p.glowStops[side].forEach((s, i) => put(`glow-${side}-${i}`, s));
  }
  put("dominant", p.dominant);
  put("vibrant", p.vibrant);
  put("muted", p.muted);
  return vars;
}

export function applyPalette(p: Palette, el: HTMLElement = document.documentElement) {
  for (const [k, v] of Object.entries(paletteVars(p))) el.style.setProperty(k, v);
  el.dataset.scheme = p.scheme;
}
