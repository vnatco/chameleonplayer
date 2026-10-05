// App state and actions. One reactive object, shared by every component.

import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  api,
  coverUrl,
  type Cover,
  type DefaultStatus,
  type Folder,
  type Palette,
  type QueueItem,
  type ScanProgress,
  type ScanSummary,
  type Stats,
  type Status,
  type Track,
  type TrackRow,
} from "./api";
import { PaletteDriver, prepare } from "./palette";

export type Size = "mini" | "normal" | "large";
export type Mode = "player" | "library";
export type BackTab = "info" | "queue" | "edit" | "settings";
export type LibView = "albums" | "album" | "artists" | "songs" | "genres" | "missing" | "settings";

/** Cover edge length and glow margin per size (spec B). */
export const SIZES: Record<Size, number> = { mini: 160, normal: 360, large: 640 };
export const MARGINS: Record<Size, number> = { mini: 40, normal: 80, large: 110 };
/** Glow reach multiplier per size (--gs). */
export const REACH: Record<Size, number> = { mini: 0.42, normal: 1, large: 1.45 };
/** The player window is fixed at the largest sleeve plus its margin, so size
 * changes are pure CSS and never resize the OS window. */
export const PLAYER_WIN = SIZES.large + 2 * MARGINS.large;
export const LIB_DEFAULT = { w: 1200, h: 800 };
/** Where the cover sits in the library rail. */
export const RAIL_COVER = { x: 24, y: 56, s: 252 };

export interface UiSettings {
  glow: number;
  strength: number;
  size: Size;
  onTop: boolean;
  remember: boolean;
  pulse: boolean;
  anim: boolean;
  writeCovers: boolean;
  /** Last player window position (logical px, top-left) and library frame. */
  playerPos?: { x: number; y: number };
  libFrame?: { x: number; y: number; w: number; h: number };
  premute?: number;
  /** "never" once the user picks Don't Ask Again on the default-player prompt. */
  defaultPrompt?: "ask" | "never";
  /** Settings layout version, for one-time migrations. */
  v?: number;
}

const DEFAULT_UI: UiSettings = {
  glow: 0.75,
  strength: 1,
  size: "normal",
  onTop: false,
  remember: true,
  pulse: false,
  anim: true,
  writeCovers: true,
};

export interface NowPlaying {
  path: string;
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  genre: string;
  year: number | null;
  format: string;
  bitrate: number | null;
  duration: number;
  cover: Cover | null;
  inLibrary: boolean;
}

class AppState {
  ready = $state(false);
  status = $state<Status | null>(null);
  /** Last known position and when it was reported, for smooth progress. */
  posAt = $state({ pos: 0, t: 0 });
  level = $state(0);
  queue = $state<QueueItem[]>([]);
  now = $state<NowPlaying | null>(null);
  palette = $state<Palette | null>(null);
  /** A palette being previewed (e.g. a candidate cover in the editor). */
  preview = $state<Palette | null>(null);
  fallback: Palette | null = null;
  /** Two image slots, cross-faded on track change. */
  slots = $state<{ a: string | null; b: string | null; front: "a" | "b" }>({ a: null, b: null, front: "a" });
  loading = $state(false);
  pendingTitle = $state("");

  ui = $state<UiSettings>({ ...DEFAULT_UI });
  coverSource = $state<"embedded" | "folder" | "embeddedOnly">("embedded");
  mode = $state<Mode>("player");
  /** True while the window morphs between player and library. */
  morphing = $state(false);
  flipped = $state(false);
  backTab = $state<BackTab>("info");
  libView = $state<LibView>("albums");
  albumKey = $state<string | null>(null);
  reduceMotion = $state(false);

  stats = $state<Stats | null>(null);
  folders = $state<Folder[]>([]);
  scanning = $state(false);
  progress = $state<ScanProgress | null>(null);
  summary = $state<ScanSummary | null>(null);
  libraryVersion = $state(0);

  toast = $state<{ id: number; text: string; error: boolean } | null>(null);
  /** Default-player status; null until checked. */
  defaults = $state<DefaultStatus | null>(null);
  /** The prompt was answered (or dismissed) this session. */
  defaultPromptDone = $state(false);
  private awaitingDefault = false;

  async refreshDefaults() {
    const before = this.defaults?.isDefault;
    const s = await api.defaultStatus().catch((e) => {
      console.warn("default status failed", e);
      return null;
    });
    if (!s) return;
    this.defaults = s;
    if (this.awaitingDefault && s.isDefault && !before) {
      this.awaitingDefault = false;
      this.notify("Chameleon Is Now Your Default Player");
    }
  }

  /** Open Chameleon's page in Windows Settings; we recheck when the user
   * comes back to the app. */
  async setAsDefault() {
    this.defaultPromptDone = true;
    if ((await this.guard(api.openDefaultApps())) !== undefined) this.awaitingDefault = true;
  }

  /** The volume capsule beside the sleeve. */
  volOpen = $state(false);
  private volTimer: ReturnType<typeof setTimeout> | undefined;

  volIn() {
    clearTimeout(this.volTimer);
    this.volOpen = true;
  }
  volOut(delay = 260) {
    clearTimeout(this.volTimer);
    this.volTimer = setTimeout(() => (this.volOpen = false), delay);
  }

  driver: PaletteDriver | null = null;
  private toastTimer: ReturnType<typeof setTimeout> | undefined;
  private toastId = 0;
  private shownPath: string | null = null;
  private trackSeq = 0;
  private prefetched = new Map<string, Promise<{ cover: Cover | null; palette: Palette } | null>>();

  get anim() {
    return this.ui.anim && !this.reduceMotion;
  }
  get playing() {
    return this.status?.state === "playing";
  }
  get duration() {
    return this.status?.duration ?? this.now?.duration ?? 0;
  }
  get size(): Size {
    return this.ui.size;
  }

  // ---- Feedback -------------------------------------------------------------

  notify(text: string, error = false) {
    clearTimeout(this.toastTimer);
    this.toast = { id: ++this.toastId, text, error };
    this.toastTimer = setTimeout(() => (this.toast = null), error ? 5000 : 2600);
  }

  /** Run a backend call; surface failures as an error toast. */
  async guard<T>(p: Promise<T>): Promise<T | undefined> {
    try {
      return await p;
    } catch (e) {
      this.notify(String(e), true);
      return undefined;
    }
  }

  // ---- Settings -------------------------------------------------------------

  private saveTimer: ReturnType<typeof setTimeout> | undefined;
  setUi<K extends keyof UiSettings>(key: K, value: UiSettings[K]) {
    this.ui[key] = value;
    clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => this.guard(api.setUiSettings($state.snapshot(this.ui) as Record<string, unknown>)), 250);
    if (key === "strength") this.applyPalette(260);
    if (key === "onTop") this.guard(api.windowOnTop(!!value));
    if (key === "pulse") this.guard(api.player({ type: "setMeter", enabled: !!value }));
  }

  loadUi(raw: Record<string, unknown>) {
    const ui = { ...DEFAULT_UI, ...(raw as Partial<UiSettings>) };
    if (!["mini", "normal", "large"].includes(ui.size)) ui.size = "normal";
    // v2: Always on Top became opt-in.
    if ((ui.v ?? 1) < 2) {
      ui.onTop = false;
      ui.v = 2;
    }
    this.ui = ui;
  }

  // ---- Palette & art --------------------------------------------------------

  /** Push the current palette (with strength and flip) to the root tokens. */
  applyPalette(duration?: number) {
    const p = this.preview ?? this.palette ?? this.fallback;
    if (!p || !this.driver) return;
    const d = duration ?? (this.anim ? 420 : 200);
    this.driver.set(prepare(p, this.ui.strength, this.flipped && this.mode === "player"), d);
  }

  private async coverInfo(path: string) {
    let p = this.prefetched.get(path);
    if (!p) {
      p = api.coverForPath(path).catch((e) => {
        console.warn("cover lookup failed", path, e);
        return null;
      });
      this.prefetched.set(path, p);
      if (this.prefetched.size > 24) this.prefetched.delete(this.prefetched.keys().next().value!);
    }
    return p;
  }

  private async decode(url: string) {
    const img = new Image();
    img.decoding = "async";
    img.src = url;
    try {
      await img.decode();
    } catch {
      // A broken image still swaps in; the slot just shows nothing.
    }
  }

  /** Swap the art and palette together once the new image is decoded. */
  private async showTrack(path: string | null, force = false) {
    if (!force && path === this.shownPath) return;
    this.shownPath = path;
    const seq = ++this.trackSeq;
    if (!path) {
      this.now = null;
      this.setSlot(null);
      this.palette = null;
      this.applyPalette();
      return;
    }
    const pending = this.queue.find((q) => q.path === path);
    this.pendingTitle = pending?.title ?? "";
    // Show "Decoding" only if this takes long enough to notice.
    const slow = setTimeout(() => seq === this.trackSeq && (this.loading = true), 250);

    const [info, row] = await Promise.all([this.coverInfo(path), api.trackByPath(path).catch(() => null)]);
    let now: NowPlaying;
    if (row) {
      now = fromRow(row);
    } else {
      const tags = await api.readTags(path).catch(() => null);
      now = {
        path,
        title: tags?.title || pending?.title || path.split(/[\\/]/).pop() || path,
        artist: tags?.artist ?? "",
        album: tags?.album ?? "",
        albumArtist: tags?.albumArtist ?? "",
        genre: tags?.genre ?? "",
        year: tags?.year ?? null,
        format: (path.split(".").pop() ?? "").toUpperCase(),
        bitrate: null,
        duration: this.status?.duration ?? 0,
        cover: info?.cover ?? null,
        inLibrary: false,
      };
    }
    const url = info?.cover ? coverUrl(info.cover.full) : null;
    if (url) await this.decode(url);
    clearTimeout(slow);
    if (seq !== this.trackSeq) return;
    this.loading = false;
    this.now = now;
    this.setSlot(url);
    this.palette = info?.palette ?? null;
    this.applyPalette();
    this.prefetchNext();
  }

  private setSlot(url: string | null) {
    const s = this.slots;
    const shown = s.front === "a" ? s.a : s.b;
    if (shown === url) return;
    this.slots = s.front === "a" ? { a: s.a, b: url, front: "b" } : { a: url, b: s.b, front: "a" };
  }

  /** Warm the next track's cover so the hand-over is instant. */
  private prefetchNext() {
    const i = this.status?.index;
    if (i == null || !this.queue.length) return;
    const next = this.queue[(i + 1) % this.queue.length];
    if (!next || next.path === this.shownPath) return;
    this.coverInfo(next.path).then((info) => info?.cover && this.decode(coverUrl(info.cover.full)));
  }

  /** Preview another palette app-wide (null restores the cover's own). */
  previewPalette(p: Palette | null) {
    this.preview = p;
    this.applyPalette();
  }

  /** Re-read the current track after its tags or cover changed. */
  refreshNow() {
    if (this.shownPath) {
      this.prefetched.delete(this.shownPath);
      this.showTrack(this.shownPath, true);
    }
  }

  // ---- Player ---------------------------------------------------------------

  private lastQueueId = -1;
  onStatus(s: Status) {
    this.status = s;
    this.posAt = { pos: s.position, t: performance.now() };
    if (s.state !== "playing") this.level = 0;
    if (s.queueId !== this.lastQueueId) {
      this.lastQueueId = s.queueId;
      this.prefetched.clear();
      api.queue().then((q) => {
        this.queue = q;
        this.prefetchNext();
      }, (e) => this.notify(String(e), true));
    }
    this.showTrack(s.track?.path ?? null);
  }

  onPosition(position: number, duration: number | null) {
    this.posAt = { pos: position, t: performance.now() };
    if (this.status && duration != null && this.status.duration !== duration) this.status.duration = duration;
  }

  /** Position now, extrapolated between backend reports while playing. */
  positionNow(now = performance.now()) {
    const { pos, t } = this.posAt;
    if (!this.playing) return pos;
    return Math.min(this.duration || Infinity, pos + (now - t) / 1000);
  }

  cmd(c: Parameters<typeof api.player>[0]) {
    return this.guard(api.player(c));
  }
  toggle() {
    if (!this.status?.queueLen) return;
    this.cmd({ type: "toggle" });
  }
  next() {
    this.cmd({ type: "next" });
  }
  prev() {
    this.cmd({ type: "prev" });
  }
  seek(position: number) {
    const p = Math.max(0, Math.min(this.duration || 0, position));
    this.posAt = { pos: p, t: performance.now() };
    this.cmd({ type: "seek", position: p });
  }
  setVolume(v: number) {
    v = Math.max(0, Math.min(1, v));
    if (this.status) this.status.volume = v;
    this.cmd({ type: "setVolume", volume: v });
  }
  toggleMute() {
    const v = this.status?.volume ?? 0;
    if (v > 0) {
      this.setUi("premute", v);
      this.setVolume(0);
    } else {
      this.setVolume(this.ui.premute && this.ui.premute > 0 ? this.ui.premute : 0.7);
    }
  }

  /** Play a list (album, search results...) from `index`. */
  playList(list: TrackRow[], index: number) {
    this.guard(api.load(list.map(trackOf), index, true));
  }

  /** Play one song: jump to it if queued, else insert it after the current one. */
  playOne(t: TrackRow) {
    const i = this.queue.findIndex((q) => q.path === t.path);
    if (i >= 0) {
      this.cmd({ type: "playIndex", index: i });
      return;
    }
    const at = (this.status?.index ?? -1) + 1;
    this.guard(api.insert([trackOf(t)], at, true));
  }

  // ---- Sleeve ---------------------------------------------------------------

  flip(tab?: BackTab) {
    if (this.mode !== "player") return;
    if (tab) this.backTab = tab;
    if (this.ui.size === "mini") {
      // Grows to Normal first, then flips (spec B).
      this.setUi("size", "normal");
      setTimeout(() => this.flip(), this.anim ? 480 : 0);
      return;
    }
    this.flipped = !this.flipped;
    this.applyPalette(this.anim ? 560 : 200);
  }

  /** Step through Mini, Normal, Large (dir +1 grows, -1 shrinks). */
  stepSize(dir: 1 | -1, wrap = false) {
    const order: Size[] = ["mini", "normal", "large"];
    let i = order.indexOf(this.ui.size) + dir;
    if (wrap) i = (i + order.length) % order.length;
    if (i < 0 || i >= order.length) return;
    this.setSize(order[i]);
  }

  setSize(size: Size) {
    if (size === "mini" && this.flipped) {
      this.flipped = false;
      this.applyPalette();
    }
    this.setUi("size", size);
  }
}

function fromRow(r: TrackRow): NowPlaying {
  const parts = [r.format];
  if (r.bitDepth && r.sampleRate) parts.push(`${r.bitDepth}-bit / ${(r.sampleRate / 1000).toFixed(1)} kHz`);
  else if (r.sampleRate) parts.push(`${(r.sampleRate / 1000).toFixed(1)} kHz`);
  return {
    path: r.path,
    title: r.title,
    artist: r.artist,
    album: r.album,
    albumArtist: r.albumArtist,
    genre: r.genre,
    year: r.year,
    format: parts.join(" · "),
    bitrate: r.bitrate,
    duration: r.duration,
    cover: r.cover,
    inLibrary: true,
  };
}

export const trackOf = (t: TrackRow): Track => ({ id: t.id, path: t.path, duration: t.duration || null });

export const app = new AppState();

export const win = getCurrentWindow();
