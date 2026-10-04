// Typed bridge to the Rust backend (src-tauri/src/commands.rs).

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---- Types (mirror the Rust structs) ----------------------------------------

export interface Swatch {
  hex: string;
  l: number;
  c: number;
  h: number;
}

export interface Sides {
  top: Swatch;
  right: Swatch;
  bottom: Swatch;
  left: Swatch;
}

export interface Palette {
  scheme: "dark" | "light";
  dominant: Swatch;
  vibrant: Swatch;
  muted: Swatch;
  edges: Sides;
  glow: Sides;
  /** Three stops per side: top/bottom run left to right, left/right top to bottom. */
  glowStops: { top: Swatch[]; right: Swatch[]; bottom: Swatch[]; left: Swatch[] };
  roles: {
    background: Swatch;
    surface: Swatch;
    surfaceRaised: Swatch;
    border: Swatch;
    text: Swatch;
    textSubtle: Swatch;
    accent: Swatch;
    onAccent: Swatch;
  };
  swatches: (Swatch & { population: number })[];
  fromCover: boolean;
}

export interface Cover {
  hash: string;
  full: string;
  thumb: string;
  width: number;
  height: number;
}

export interface TrackRow {
  id: number;
  path: string;
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  albumKey: string;
  genre: string;
  year: number | null;
  trackNo: number | null;
  discNo: number | null;
  duration: number;
  bitrate: number | null;
  sampleRate: number | null;
  channels: number | null;
  bitDepth: number | null;
  format: string;
  cover: Cover | null;
  coverSource: "embedded" | "folder" | null;
}

export interface AlbumRow {
  key: string;
  title: string;
  artist: string;
  year: number | null;
  trackCount: number;
  duration: number;
  cover: Cover | null;
}

export interface NamedCount {
  name: string;
  trackCount: number;
  albumCount: number;
  cover: Cover | null;
}

export interface Stats {
  tracks: number;
  albums: number;
  artists: number;
  missingCovers: number;
  folders: number;
}

export interface Folder {
  id: number;
  path: string;
  trackCount: number;
}

export interface TrackFilter {
  albumKey?: string;
  artist?: string;
  genre?: string;
  search?: string;
  missingCover?: boolean;
  limit?: number;
}

export interface Track {
  id: number | null;
  path: string;
  duration: number | null;
}

export type Repeat = "off" | "all" | "one";
export type PlayState = "stopped" | "playing" | "paused";

export interface Status {
  state: PlayState;
  index: number | null;
  track: Track | null;
  position: number;
  duration: number | null;
  volume: number;
  repeat: Repeat;
  queueLen: number;
}

export type PlayerEvent =
  | ({ type: "status" } & Status)
  | { type: "position"; position: number; duration: number | null }
  | { type: "level"; value: number }
  | { type: "error"; path: string | null; message: string };

export interface ScanProgress {
  folder: string;
  phase: "discovering" | "reading" | "cleaning";
  found: number;
  total: number;
  processed: number;
}

export interface ScanSummary {
  added: number;
  updated: number;
  removed: number;
  unchanged: number;
  unsupported: number;
  failedCount: number;
  failures: { path: string; message: string }[];
  missingFolders: string[];
  cancelled: boolean;
}

export interface ScanDone {
  summary: ScanSummary | null;
  error: string | null;
}

export interface CoverInfo {
  cover: Cover | null;
  palette: Palette;
}

export interface TagInfo {
  path: string;
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  genre: string;
  year: number | null;
  trackNo: number | null;
  discNo: number | null;
  hasEmbeddedCover: boolean;
  readOnly: boolean;
}

export type CoverEdit = { action: "keep" } | { action: "remove" } | { action: "replace"; path: string };

export interface TagEdit {
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  genre: string;
  year: number | null;
  trackNo: number | null;
  discNo: number | null;
  cover: CoverEdit;
}

export interface Settings {
  volume: number;
  repeat: Repeat;
  ui: Record<string, unknown>;
}

export type PlayerCommand =
  | { type: "play" | "pause" | "toggle" | "next" | "prev" | "stop" }
  | { type: "playIndex"; index: number }
  | { type: "seek"; position: number }
  | { type: "setVolume"; volume: number }
  | { type: "setRepeat"; repeat: Repeat }
  | { type: "setMeter"; enabled: boolean };

// ---- Commands -----------------------------------------------------------------

export const api = {
  stats: () => invoke<Stats>("library_stats"),
  folders: () => invoke<Folder[]>("library_folders"),
  addFolder: (path: string) => invoke<Folder>("library_add_folder", { path }),
  removeFolder: (id: number) => invoke<void>("library_remove_folder", { id }),
  rescan: () => invoke<boolean>("library_rescan"),
  cancelScan: () => invoke<void>("library_cancel_scan"),
  isScanning: () => invoke<boolean>("library_is_scanning"),
  albums: (artist?: string, search?: string) => invoke<AlbumRow[]>("library_albums", { artist, search }),
  artists: (search?: string) => invoke<NamedCount[]>("library_artists", { search }),
  genres: (search?: string) => invoke<NamedCount[]>("library_genres", { search }),
  tracks: (filter: TrackFilter = {}) => invoke<TrackRow[]>("library_tracks", { filter }),
  trackByPath: (path: string) => invoke<TrackRow | null>("library_track_by_path", { path }),

  coverForPath: (path: string) => invoke<CoverInfo>("cover_for_path", { path }),
  coverPalette: (hash: string) => invoke<Palette>("cover_palette", { hash }),
  paletteFallback: () => invoke<Palette>("palette_fallback"),
  imagePreview: (path: string) => invoke<string>("image_preview", { path }),

  readTags: (path: string) => invoke<TagInfo>("tags_read", { path }),
  writeTags: (path: string, edit: TagEdit) => invoke<void>("tags_write", { path, edit }),

  load: (queue: Track[], index: number, autoplay = true) => invoke<void>("player_load", { queue, index, autoplay }),
  player: (command: PlayerCommand) => invoke<void>("player", { command }),
  status: () => invoke<Status>("player_status"),
  openPaths: (paths: string[]) => invoke<number>("open_paths", { paths }),

  settings: () => invoke<Settings>("settings_get"),
  setUiSettings: (ui: Record<string, unknown>) => invoke<void>("settings_set_ui", { ui }),
};

// ---- Events -------------------------------------------------------------------

export const events = {
  player: (cb: (e: PlayerEvent) => void): Promise<UnlistenFn> => listen<PlayerEvent>("player", (e) => cb(e.payload)),
  scanProgress: (cb: (p: ScanProgress) => void) => listen<ScanProgress>("scan-progress", (e) => cb(e.payload)),
  scanDone: (cb: (d: ScanDone) => void) => listen<ScanDone>("scan-done", (e) => cb(e.payload)),
  libraryChanged: (cb: (paths: string[]) => void) =>
    listen<{ paths: string[] }>("library-changed", (e) => cb(e.payload.paths)),
  appError: (cb: (message: string) => void) => listen<string>("app-error", (e) => cb(e.payload)),
};

// ---- Helpers ------------------------------------------------------------------

/** URL for a cached cover file on disk. */
export const coverUrl = (path: string) => convertFileSrc(path);

export const toTrack = (t: TrackRow): Track => ({ id: t.id, path: t.path, duration: t.duration || null });

export function formatTime(secs: number | null | undefined): string {
  if (secs == null || !isFinite(secs)) return "-:--";
  const s = Math.max(0, Math.floor(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;
