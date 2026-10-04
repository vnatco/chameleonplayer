// Keeps the Windows shell surfaces (tray, taskbar thumbnail, media overlay)
// in step with the player.

import { invoke } from "@tauri-apps/api/core";
import { mono, type Rect, type Rgb } from "./api";

export interface ShellUpdate {
  path: string | null;
  title: string;
  artist: string;
  accent: Rgb;
  playing: boolean;
  cover: string | null;
  fallbackPng: string | null;
  coverRect: Rect;
}

const fallbackCache = new Map<string, string>();

/** The typographic no-cover sleeve (spec A5) as a 600 px PNG, base64. */
export function renderFallback(title: string, artist: string): string {
  const key = `${title}\u0001${artist}`;
  const hit = fallbackCache.get(key);
  if (hit) return hit;
  const N = 600;
  const c = document.createElement("canvas");
  c.width = c.height = N;
  const g = c.getContext("2d")!;
  const grad = g.createRadialGradient(N * 0.28, N * 0.18, 0, N * 0.28, N * 0.18, N * 1.25);
  grad.addColorStop(0, "#2b2b31");
  grad.addColorStop(0.62, "#17171a");
  grad.addColorStop(1, "#111113");
  g.fillStyle = grad;
  g.fillRect(0, 0, N, N);
  const display = '"Segoe UI Variable Display", "Segoe UI", Figtree, sans-serif';
  g.fillStyle = "rgb(255 255 255 / 0.07)";
  g.font = `200 ${N * 0.62}px ${display}`;
  g.textBaseline = "top";
  g.fillText(mono(title), N * 0.07, N * 0.0);
  g.textBaseline = "alphabetic";
  const left = N * 0.07;
  const maxW = N * 0.86;
  g.fillStyle = "#a0a0ab";
  g.font = `${Math.round(N * 0.034)}px "Segoe UI", Figtree, sans-serif`;
  const artistY = N * 0.93;
  if (artist) g.fillText(artist, left, artistY, maxW);
  g.fillStyle = "#ececf0";
  g.font = `300 ${Math.round(N * 0.052)}px ${display}`;
  const titleY = artistY - N * 0.06;
  g.fillText(title || "Chameleon Player", left, titleY, maxW);
  g.fillStyle = "rgb(255 255 255 / 0.55)";
  g.font = `${Math.round(N * 0.022)}px "JetBrains Mono", ui-monospace, monospace`;
  g.fillText("NO ARTWORK", left, titleY - N * 0.075);
  const b64 = c.toDataURL("image/png").split(",")[1];
  fallbackCache.set(key, b64);
  if (fallbackCache.size > 32) fallbackCache.delete(fallbackCache.keys().next().value!);
  return b64;
}

let timer: ReturnType<typeof setTimeout> | undefined;
let lastJson = "";

/** Debounced: sends only when something actually changed. */
export function updateShell(u: ShellUpdate) {
  clearTimeout(timer);
  timer = setTimeout(() => {
    const json = JSON.stringify(u);
    if (json === lastJson) return;
    lastJson = json;
    invoke("shell_update", { update: u }).catch((e) => console.warn("shell update failed", e));
  }, 120);
}
