// One animation-frame ticker for every progress display. The backend reports
// position 4x a second; between reports the position is extrapolated so bars
// glide instead of stepping. Subscribers write transforms directly (no
// reactive state per frame), and the loop only runs while playing.

import { app } from "./state.svelte";

type Sub = (fraction: number, position: number) => void;
const subs = new Set<Sub>();
let raf = 0;

function tick() {
  raf = 0;
  if (!subs.size) return;
  const pos = app.positionNow();
  const d = app.duration;
  const f = d > 0 ? Math.min(1, Math.max(0, pos / d)) : 0;
  for (const s of subs) s(f, pos);
  if (app.playing) raf = requestAnimationFrame(tick);
}

/** Re-render now (after a seek, pause or track change) and resume if playing. */
export function kick() {
  if (!raf) raf = requestAnimationFrame(tick);
}

export function onProgress(fn: Sub): () => void {
  subs.add(fn);
  kick();
  return () => {
    subs.delete(fn);
  };
}
