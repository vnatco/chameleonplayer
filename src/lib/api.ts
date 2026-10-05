// Typed bridge to the Rust backend (src-tauri/src/commands.rs, window.rs).

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Rgb = [number, number, number];
export interface Sides<T> {
  t: T;
  r: T;
  b: T;
  l: T;
}

/** A finished palette, computed once per cover by the backend (spec F). */
export interface Palette {
  scheme: "dark" | "light";
  gray: boolean;
  fallback: boolean;
  roles: {
    bg: Rgb;
    surface: Rgb;
    surface2: Rgb;
    accent: Rgb;
    onAccent: Rgb;
    text: Rgb;
    subtle: Rgb;
    edge: Rgb;
  };
  glow: Sides<Rgb>;
  glowA: Sides<number>;
  /** Three segments per side: top/bottom run left to right, left/right top to bottom. */
  glowStops: Sides<[Rgb, Rgb, Rgb]>;
  dominant: Rgb;
  vibrant: Rgb;
  muted: Rgb;
  swatches: Rgb[];
  ratios: { text: number; subtle: number; accent: number; onAccent: number };
  fixes: string[];
}

export interface Cover {
  hash: string;
  full: string;
  thumb: string;
  width: number;
  height: number;
  /** `#rrggbb` edge border and bottom glow, for small thumbnails. */
  edge: string;
  glow: string;
  /** A built-in fallback shown for music without art, not the music's own. */
  placeholder: boolean;
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
  coverSource: "embedded" | "folder" | "library" | null;
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
  /** Up to four distinct album covers. */
  covers: Cover[];
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

export interface QueueItem {
  path: string;
  id: number | null;
  title: string;
  artist: string;
  album: string;
  duration: number;
  cover: Cover | null;
}

export type Repeat = "off" | "all" | "one";
export type PlayState = "stopped" | "playing" | "paused";
export type CoverSource = "embedded" | "folder" | "embeddedOnly";

export interface Status {
  state: PlayState;
  index: number | null;
  track: Track | null;
  position: number;
  duration: number | null;
  volume: number;
  repeat: Repeat;
  queueLen: number;
  queueId: number;
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
  withCover: number;
  noCover: number;
  file: string;
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

export interface Candidate {
  release: string;
  artist: string;
  year: string | null;
  score: number;
  thumb: string;
  image: string;
  label: string;
}

export interface AlbumCoverResult {
  written: number;
  libraryOnly: number;
  failures: string[];
  folderImage: string | null;
}

export interface DefaultStatus {
  /** Registered with Windows by the installer (false for dev builds). */
  registered: boolean;
  scope: "machine" | "user" | null;
  ours: string[];
  others: string[];
  /** Chameleon opens MP3 files. */
  isDefault: boolean;
}

export interface Settings {
  volume: number;
  repeat: Repeat;
  coverSource: CoverSource;
  ui: Record<string, unknown>;
}

export interface Bootstrap {
  settings: Settings;
  status: Status;
  stats: Stats;
  scanning: boolean;
  fallback: Palette;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Frame extends Rect {
  scale: number;
  work: Rect;
  maximized: boolean;
}

export type PlayerCommand =
  | { type: "play" | "pause" | "toggle" | "next" | "prev" | "stop" }
  | { type: "playIndex"; index: number }
  | { type: "seek"; position: number }
  | { type: "setVolume"; volume: number }
  | { type: "setRepeat"; repeat: Repeat }
  | { type: "setMeter"; enabled: boolean };

export const api = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),

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
  setCoverSource: (source: CoverSource) => invoke<void>("library_set_cover_source", { source }),
  updateTrackOnly: (path: string, edit: TagEdit) => invoke<void>("library_update_track", { path, edit }),

  coverForPath: (path: string) => invoke<CoverInfo>("cover_for_path", { path }),
  coverPalette: (hash: string) => invoke<Palette>("cover_palette", { hash }),
  imagePreview: (path: string) => invoke<string>("image_preview", { path }),
  imagePalette: (path: string) => invoke<Palette>("image_palette", { path }),
  findCovers: (artist: string, album: string, title: string) => invoke<Candidate[]>("covers_find", { artist, album, title }),
  downloadCover: (url: string) => invoke<string>("covers_download", { url }),
  albumSetCover: (albumKey: string, image: string, writeFiles: boolean) =>
    invoke<AlbumCoverResult>("album_set_cover", { albumKey, image, writeFiles }),

  readTags: (path: string) => invoke<TagInfo>("tags_read", { path }),
  writeTags: (path: string, edit: TagEdit) => invoke<void>("tags_write", { path, edit }),

  load: (queue: Track[], index: number, autoplay = true) => invoke<void>("player_load", { queue, index, autoplay }),
  insert: (tracks: Track[], at: number, play: boolean) => invoke<void>("player_insert", { tracks, at, play }),
  queue: () => invoke<QueueItem[]>("player_queue"),
  player: (command: PlayerCommand) => invoke<void>("player", { command }),
  status: () => invoke<Status>("player_status"),
  openPaths: (paths: string[]) => invoke<number>("open_paths", { paths }),

  setUiSettings: (ui: Record<string, unknown>) => invoke<void>("settings_set_ui", { ui }),
  defaultStatus: () => invoke<DefaultStatus>("default_status"),
  openDefaultApps: () => invoke<void>("open_default_apps"),

  windowHit: (rects: Rect[]) => invoke<void>("window_hit", { rects }),
  windowFrame: () => invoke<Frame>("window_frame"),
  windowSetFrame: (r: Rect) => invoke<Frame>("window_set_frame", { ...r }),
  windowChrome: (library: boolean, mica: boolean) => invoke<void>("window_chrome", { library, mica }),
  windowOnTop: (on: boolean) => invoke<void>("window_set_on_top", { on }),
  windowToggleMaximize: () => invoke<Frame>("window_toggle_maximize"),
  windowShow: () => invoke<void>("window_show"),
};

export const events = {
  player: (cb: (e: PlayerEvent) => void): Promise<UnlistenFn> => listen<PlayerEvent>("player", (e) => cb(e.payload)),
  scanProgress: (cb: (p: ScanProgress) => void) => listen<ScanProgress>("scan-progress", (e) => cb(e.payload)),
  scanDone: (cb: (d: ScanDone) => void) => listen<ScanDone>("scan-done", (e) => cb(e.payload)),
  libraryChanged: (cb: (paths: string[]) => void) =>
    listen<{ paths: string[] }>("library-changed", (e) => cb(e.payload.paths)),
  appError: (cb: (message: string) => void) => listen<string>("app-error", (e) => cb(e.payload)),
  pointerInside: (cb: (inside: boolean) => void) =>
    listen<{ inside: boolean }>("pointer-inside", (e) => cb(e.payload.inside)),
};

/** URL for a cached cover file on disk (memoized; called for every thumbnail). */
const urlCache = new Map<string, string>();
export function coverUrl(path: string): string {
  let u = urlCache.get(path);
  if (!u) {
    u = convertFileSrc(path);
    urlCache.set(path, u);
  }
  return u;
}

export const toTrack = (t: { id: number | null; path: string; duration: number }): Track => ({
  id: t.id,
  path: t.path,
  duration: t.duration || null,
});

export function formatTime(secs: number | null | undefined): string {
  if (secs == null || !isFinite(secs)) return "-:--";
  const s = Math.max(0, Math.floor(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** First letter or digit, for typographic placeholders. */
export const mono = (t: string | null | undefined) =>
  (t || "?").replace(/[^\p{L}\p{N}]/gu, "").charAt(0).toUpperCase() || "?";
