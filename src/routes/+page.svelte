<script lang="ts">
  // Developer test bench: exercises the real backend (library, playback,
  // palettes, tag editing) until the final design is implemented.
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import {
    api,
    events,
    coverUrl,
    formatTime,
    fileName,
    toTrack,
    type AlbumRow,
    type Cover,
    type Folder,
    type NamedCount,
    type Palette,
    type Repeat,
    type ScanProgress,
    type ScanSummary,
    type Stats,
    type Status,
    type TrackRow,
  } from "$lib/api";
  import { applyPalette } from "$lib/palette";
  import CoverStage from "$lib/bench/CoverStage.svelte";
  import TagEditor from "$lib/bench/TagEditor.svelte";

  type Tab = "albums" | "artists" | "genres" | "songs" | "missing";
  const SONG_LIMIT = 1000;

  // ---- Player state --------------------------------------------------------
  let status = $state<Status | null>(null);
  let position = $state(0);
  let seeking = $state(false);
  let level = $state(0);
  let nowCover = $state<Cover | null>(null);
  let palette = $state<Palette | null>(null);
  let nowInfo = $state<{ title: string; artist: string; album: string; format: string } | null>(null);
  let queue = $state<TrackRow[]>([]);

  // UI preferences (persisted in settings.ui)
  let glow = $state(0.8);
  let pulse = $state(false);

  // ---- Library state -------------------------------------------------------
  let stats = $state<Stats | null>(null);
  let folders = $state<Folder[]>([]);
  let tab = $state<Tab>("albums");
  let search = $state("");
  let albums = $state<AlbumRow[]>([]);
  let named = $state<NamedCount[]>([]);
  let songs = $state<TrackRow[]>([]);
  let openAlbum = $state<AlbumRow | null>(null);
  let albumTracks = $state<TrackRow[]>([]);
  let artistFilter = $state<string | null>(null);
  let genreFilter = $state<string | null>(null);
  let scanning = $state(false);
  let progress = $state<ScanProgress | null>(null);
  let summary = $state<ScanSummary | null>(null);
  let showFailures = $state(false);
  let editing = $state(false);
  let errors = $state<{ id: number; text: string }[]>([]);
  let errorId = 0;

  function notify(text: string) {
    const id = ++errorId;
    errors = [...errors, { id, text }];
    setTimeout(() => (errors = errors.filter((e) => e.id !== id)), 8000);
  }

  async function guard<T>(p: Promise<T>): Promise<T | undefined> {
    try {
      return await p;
    } catch (e) {
      notify(String(e));
      return undefined;
    }
  }

  // ---- Now playing ---------------------------------------------------------
  let shownPath: string | null = null;

  async function refreshNowPlaying(force = false) {
    const path = status?.track?.path ?? null;
    if (!force && path === shownPath) return;
    shownPath = path;
    if (!path) {
      nowCover = null;
      nowInfo = null;
      const fallback = await guard(api.paletteFallback());
      if (fallback) {
        palette = fallback;
        applyPalette(fallback);
      }
      return;
    }
    const [info, row] = await Promise.all([guard(api.coverForPath(path)), guard(api.trackByPath(path))]);
    if (path !== shownPath) return;
    if (info) {
      nowCover = info.cover;
      palette = info.palette;
      applyPalette(info.palette);
    }
    if (row) {
      nowInfo = { title: row.title, artist: row.artist, album: row.album, format: describeFormat(row) };
    } else {
      const tags = await guard(api.readTags(path));
      nowInfo = { title: tags?.title || fileName(path), artist: tags?.artist ?? "", album: tags?.album ?? "", format: "" };
    }
  }

  function describeFormat(t: TrackRow) {
    const parts = [t.format];
    if (t.bitrate) parts.push(`${t.bitrate} kbps`);
    if (t.sampleRate) parts.push(`${(t.sampleRate / 1000).toFixed(1)} kHz`);
    if (t.bitDepth) parts.push(`${t.bitDepth}-bit`);
    return parts.join(" · ");
  }

  function play(list: TrackRow[], index: number) {
    queue = list;
    guard(api.load(list.map(toTrack), index, true));
  }

  function shuffle(list: TrackRow[]) {
    const a = [...list];
    for (let i = a.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [a[i], a[j]] = [a[j], a[i]];
    }
    play(a, 0);
  }

  const cmd = (c: Parameters<typeof api.player>[0]) => guard(api.player(c));

  function cycleRepeat() {
    const order: Repeat[] = ["off", "all", "one"];
    const next = order[(order.indexOf(status?.repeat ?? "off") + 1) % order.length];
    cmd({ type: "setRepeat", repeat: next });
  }

  function saveUi() {
    guard(api.setUiSettings({ glow, pulse }));
  }

  // ---- Library -------------------------------------------------------------
  let searchTimer: ReturnType<typeof setTimeout> | undefined;

  async function refreshLibrary() {
    const [s, f] = await Promise.all([guard(api.stats()), guard(api.folders())]);
    if (s) stats = s;
    if (f) folders = f;
    await loadTab();
    if (openAlbum) {
      albumTracks = (await guard(api.tracks({ albumKey: openAlbum.key }))) ?? [];
    }
  }

  async function loadTab() {
    const q = search.trim() || undefined;
    if (tab === "albums") albums = (await guard(api.albums(artistFilter ?? undefined, q))) ?? [];
    else if (tab === "artists") named = (await guard(api.artists(q))) ?? [];
    else if (tab === "genres") named = (await guard(api.genres(q))) ?? [];
    else if (tab === "songs")
      songs = (await guard(api.tracks({ search: q, genre: genreFilter ?? undefined, limit: SONG_LIMIT }))) ?? [];
    else if (tab === "missing") songs = (await guard(api.tracks({ search: q, missingCover: true, limit: SONG_LIMIT }))) ?? [];
  }

  function setTab(t: Tab) {
    tab = t;
    openAlbum = null;
    if (t !== "albums") artistFilter = null;
    if (t !== "songs") genreFilter = null;
    loadTab();
  }

  function onSearch() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(loadTab, 200);
  }

  async function showAlbum(a: AlbumRow) {
    openAlbum = a;
    albumTracks = (await guard(api.tracks({ albumKey: a.key }))) ?? [];
    if (a.cover) {
      const p = await guard(api.coverPalette(a.cover.hash));
      if (p) applyPalette(p);
    }
  }

  function closeAlbum() {
    openAlbum = null;
    if (palette) applyPalette(palette);
  }

  async function addFolder() {
    const picked = await open({ directory: true, multiple: false, title: "Add Music Folder" });
    if (typeof picked !== "string") return;
    const f = await guard(api.addFolder(picked));
    if (f) {
      scanning = true;
      folders = (await guard(api.folders())) ?? folders;
    }
  }

  async function removeFolder(f: Folder) {
    if (await guard(api.removeFolder(f.id)) !== undefined) refreshLibrary();
  }

  async function rescan() {
    const started = await guard(api.rescan());
    if (started === false) notify("A scan is already running.");
    else if (started) scanning = true;
  }

  // ---- Lifecycle -----------------------------------------------------------
  onMount(() => {
    const unsubs: Promise<() => void>[] = [];

    (async () => {
      const settings = await guard(api.settings());
      if (settings) {
        if (typeof settings.ui.glow === "number") glow = settings.ui.glow;
        if (typeof settings.ui.pulse === "boolean") pulse = settings.ui.pulse;
        if (pulse) cmd({ type: "setMeter", enabled: true });
      }
      const fallback = await guard(api.paletteFallback());
      if (fallback && !shownPath) {
        palette = fallback;
        applyPalette(fallback);
      }
      status = (await guard(api.status())) ?? null;
      position = status?.position ?? 0;
      scanning = (await guard(api.isScanning())) ?? false;
      await refreshNowPlaying(true);
      await refreshLibrary();
    })();

    unsubs.push(
      events.player((e) => {
        if (e.type === "status") {
          const { type: _, ...s } = e;
          status = s;
          if (!seeking) position = s.position;
          if (s.state !== "playing") level = 0;
          refreshNowPlaying();
        } else if (e.type === "position") {
          if (!seeking) position = e.position;
          if (status) status = { ...status, duration: e.duration ?? status.duration };
        } else if (e.type === "level") {
          level = e.value;
        } else if (e.type === "error") {
          notify(e.path ? `${fileName(e.path)}: ${e.message}` : e.message);
        }
      }),
      events.scanProgress((p) => {
        scanning = true;
        progress = p;
      }),
      events.scanDone((d) => {
        scanning = false;
        progress = null;
        if (d.error) notify(`Scan failed: ${d.error}`);
        if (d.summary) summary = d.summary;
      }),
      events.libraryChanged((paths) => {
        refreshLibrary();
        if (paths.length === 0 || (shownPath && paths.includes(shownPath))) refreshNowPlaying(true);
      }),
      events.appError(notify),
      getCurrentWebview().onDragDropEvent((e) => {
        if (e.payload.type === "drop" && e.payload.paths.length) {
          queue = [];
          guard(api.openPaths(e.payload.paths));
        }
      }),
    );

    return () => unsubs.forEach((u) => u.then((f) => f()));
  });

  const duration = $derived(status?.duration ?? 0);
  const playing = $derived(status?.state === "playing");
</script>

<div class="app">
  <aside class="library">
    <header>
      <h1>Chameleon Player <span class="badge">Test Bench</span></h1>
      <p class="subtle small">Temporary developer screen. The real interface comes with the new design.</p>
      {#if stats}
        <p class="subtle small">
          {stats.tracks} Songs · {stats.albums} Albums · {stats.artists} Artists · {stats.missingCovers} Without Cover
        </p>
      {/if}
    </header>

    <section class="folders">
      <div class="row">
        <h2>Music Folders</h2>
        <button onclick={addFolder}>Add Music Folder</button>
        <button onclick={rescan} disabled={scanning || folders.length === 0}>Rescan</button>
        {#if scanning}<button onclick={() => api.cancelScan()}>Cancel Scan</button>{/if}
      </div>
      {#if folders.length === 0}
        <p class="subtle">No Music Folders Yet. Add one to build your library, or drop files on the window to play them.</p>
      {/if}
      {#each folders as f (f.id)}
        <div class="folder">
          <span title={f.path}>{f.path}</span>
          <span class="subtle small">{f.trackCount} Songs</span>
          <button class="link" onclick={() => removeFolder(f)} disabled={scanning}>Remove</button>
        </div>
      {/each}
      {#if scanning}
        <p class="small">
          {#if progress}
            {progress.phase === "discovering" ? "Finding Files" : progress.phase === "reading" ? "Reading Tags and Covers" : "Cleaning Up"}:
            {progress.phase === "reading" ? `${progress.processed} / ${progress.total}` : `${progress.found} Found`}
            <span class="subtle">in {progress.folder}</span>
          {:else}
            Scanning...
          {/if}
        </p>
      {:else if summary}
        <p class="small subtle">
          Last Scan: {summary.added} Added, {summary.updated} Updated, {summary.removed} Removed, {summary.unchanged} Unchanged
          {#if summary.cancelled}(Cancelled){/if}
        </p>
        {#if summary.unsupported > 0}
          <p class="small warn">{summary.unsupported} files were skipped because their format (Opus, APE, WMA, WavPack, Musepack, DSD) can't be played yet.</p>
        {/if}
        {#if summary.missingFolders.length}
          <p class="small warn">Not reachable, kept as-is: {summary.missingFolders.join(", ")}</p>
        {/if}
        {#if summary.failedCount > 0}
          <button class="link small warn" onclick={() => (showFailures = !showFailures)}>
            {summary.failedCount} Files Couldn't Be Read {showFailures ? "▴" : "▾"}
          </button>
          {#if showFailures}
            <ul class="failures">
              {#each summary.failures as f}
                <li><span>{fileName(f.path)}</span> <span class="subtle">{f.message}</span></li>
              {/each}
              {#if summary.failedCount > summary.failures.length}
                <li class="subtle">And {summary.failedCount - summary.failures.length} more (see the log file).</li>
              {/if}
            </ul>
          {/if}
        {/if}
      {/if}
    </section>

    <nav class="tabs">
      {#each [["albums", "Albums"], ["artists", "Artists"], ["genres", "Genres"], ["songs", "Songs"], ["missing", "Missing Covers"]] as [t, label]}
        <button class:active={tab === t} onclick={() => setTab(t as Tab)}>{label}</button>
      {/each}
      <input class="search" placeholder="Search" bind:value={search} oninput={onSearch} />
    </nav>

    <div class="content">
      {#if openAlbum}
        <div class="album-head">
          <button class="link" onclick={closeAlbum}>← Back</button>
          {#if openAlbum.cover}<img src={coverUrl(openAlbum.cover.thumb)} alt="" />{/if}
          <div>
            <h2>{openAlbum.title}</h2>
            <p class="subtle">{openAlbum.artist}{openAlbum.year ? ` · ${openAlbum.year}` : ""} · {openAlbum.trackCount} Songs · {formatTime(openAlbum.duration)}</p>
            <div class="row">
              <button class="primary" onclick={() => play(albumTracks, 0)}>Play</button>
              <button onclick={() => shuffle(albumTracks)}>Shuffle</button>
            </div>
          </div>
        </div>
        {@render songList(albumTracks)}
      {:else if tab === "albums"}
        {#if artistFilter}
          <p class="small">Albums by {artistFilter} <button class="link" onclick={() => { artistFilter = null; loadTab(); }}>Show All</button></p>
        {/if}
        <div class="grid">
          {#each albums as a (a.key)}
            <button class="tile" onclick={() => showAlbum(a)} title="{a.title} - {a.artist}">
              {#if a.cover}<img src={coverUrl(a.cover.thumb)} alt="" loading="lazy" />{:else}<div class="no-art">No Cover</div>{/if}
              <span class="t">{a.title}</span>
              <span class="subtle small">{a.artist}</span>
            </button>
          {:else}
            <p class="subtle">No Albums Yet.</p>
          {/each}
        </div>
      {:else if tab === "artists" || tab === "genres"}
        <ul class="named">
          {#each named as n (n.name)}
            <li>
              <button
                onclick={() => {
                  if (tab === "artists") { artistFilter = n.name; tab = "albums"; }
                  else { genreFilter = n.name; tab = "songs"; }
                  loadTab();
                }}
              >
                {#if n.cover}<img src={coverUrl(n.cover.thumb)} alt="" loading="lazy" />{:else}<div class="no-art sm"></div>{/if}
                <span>{n.name}</span>
                <span class="subtle small">{n.albumCount} Albums · {n.trackCount} Songs</span>
              </button>
            </li>
          {:else}
            <p class="subtle">Nothing Here Yet.</p>
          {/each}
        </ul>
      {:else}
        {#if genreFilter}
          <p class="small">Genre: {genreFilter} <button class="link" onclick={() => { genreFilter = null; loadTab(); }}>Show All</button></p>
        {/if}
        {#if tab === "missing"}
          <p class="small subtle">Songs with no embedded cover and no cover image in their folder. Play one and use Edit Tags to add a cover.</p>
        {/if}
        {@render songList(songs)}
        {#if songs.length >= SONG_LIMIT}
          <p class="small subtle">Showing the first {SONG_LIMIT} songs. Use search to narrow it down.</p>
        {/if}
      {/if}
    </div>
  </aside>

  <main class="player">
    <CoverStage cover={nowCover} size={300} {glow} level={pulse ? level : 0} />

    <div class="meta">
      {#if nowInfo}
        <h2>{nowInfo.title}</h2>
        <p>{nowInfo.artist}{nowInfo.album ? ` · ${nowInfo.album}` : ""}</p>
        {#if nowInfo.format}<p class="subtle small">{nowInfo.format}</p>{/if}
      {:else}
        <h2 class="subtle">Nothing Playing</h2>
        <p class="subtle small">Pick something from the library, or drop music files or a folder here.</p>
      {/if}
    </div>

    <div class="seek">
      <span class="small">{formatTime(position)}</span>
      <input
        type="range"
        min="0"
        max={duration || 1}
        step="0.1"
        value={position}
        disabled={!status?.track}
        oninput={(e) => { seeking = true; position = +e.currentTarget.value; }}
        onchange={(e) => { seeking = false; cmd({ type: "seek", position: +e.currentTarget.value }); }}
      />
      <span class="small">{formatTime(duration)}</span>
    </div>

    <div class="controls">
      <button onclick={() => cmd({ type: "prev" })} disabled={!status?.track}>Previous</button>
      <button class="primary big" onclick={() => cmd({ type: "toggle" })} disabled={!status?.queueLen}>{playing ? "Pause" : "Play"}</button>
      <button onclick={() => cmd({ type: "next" })} disabled={!status?.track}>Next</button>
      <button onclick={cycleRepeat}>Repeat: {status?.repeat === "one" ? "One" : status?.repeat === "all" ? "All" : "Off"}</button>
    </div>

    <div class="sliders">
      <label>Volume
        <input type="range" min="0" max="1" step="0.01" value={status?.volume ?? 0.8}
          onchange={(e) => cmd({ type: "setVolume", volume: +e.currentTarget.value })} />
      </label>
      <label>Glow <span class="subtle small">Crisp ↔ Full Glow</span>
        <input type="range" min="0" max="1" step="0.01" bind:value={glow} onchange={saveUi} />
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={pulse}
          onchange={() => { cmd({ type: "setMeter", enabled: pulse }); saveUi(); }} />
        Music Pulse
      </label>
    </div>

    {#if palette}
      <div class="swatches" title="Extracted palette (debug)">
        {#each Object.entries(palette.roles) as [name, s]}
          <span style:background={s.hex} title="{name} {s.hex}"></span>
        {/each}
        <span class="sep"></span>
        {#each palette.swatches as s}
          <span style:background={s.hex} style:flex-grow={Math.max(0.3, s.population * 10)} title="{s.hex} {(s.population * 100).toFixed(0)}%"></span>
        {/each}
      </div>
    {/if}

    {#if status?.track}
      <button onclick={() => (editing = !editing)}>{editing ? "Close Tag Editor" : "Edit Tags"}</button>
      {#if editing}
        <TagEditor path={status.track.path} cover={nowCover} />
      {/if}
    {/if}

    {#if queue.length > 1}
      <section class="queue">
        <h3>Up Next</h3>
        {#each queue.slice((status?.index ?? 0) + 1, (status?.index ?? 0) + 8) as t, i (t.id + "-" + i)}
          <button class="qrow" onclick={() => cmd({ type: "playIndex", index: (status?.index ?? 0) + 1 + i })}>
            {#if t.cover}<img src={coverUrl(t.cover.thumb)} alt="" />{:else}<div class="no-art xs"></div>{/if}
            <span>{t.title}</span><span class="subtle small">{t.artist}</span>
          </button>
        {/each}
      </section>
    {/if}
  </main>

  <div class="toasts">
    {#each errors as e (e.id)}
      <div class="toast">{e.text}</div>
    {/each}
  </div>
</div>

{#snippet songList(list: TrackRow[])}
  <table class="songs">
    <thead><tr><th></th><th>Title</th><th>Artist</th><th>Album</th><th>Time</th></tr></thead>
    <tbody>
      {#each list as t, i (t.id)}
        <tr class:current={status?.track?.path === t.path} ondblclick={() => play(list, i)}>
          <td>{#if t.cover}<img src={coverUrl(t.cover.thumb)} alt="" loading="lazy" />{:else}<div class="no-art xs"></div>{/if}</td>
          <td>{t.title}</td>
          <td>{t.artist}</td>
          <td>{t.album}</td>
          <td class="num">{formatTime(t.duration)}</td>
        </tr>
      {:else}
        <tr><td colspan="5" class="subtle">No Songs Here.</td></tr>
      {/each}
    </tbody>
  </table>
  {#if list.length}<p class="small subtle">Double-click a song to play it.</p>{/if}
{/snippet}

<style>
  /* Registered so palette changes cross-fade instead of snapping. */
  @property --c-background { syntax: "<color>"; inherits: true; initial-value: #1d1f2b; }
  @property --c-surface { syntax: "<color>"; inherits: true; initial-value: #262838; }
  @property --c-surface-raised { syntax: "<color>"; inherits: true; initial-value: #2f3244; }
  @property --c-border { syntax: "<color>"; inherits: true; initial-value: #3d4157; }
  @property --c-text { syntax: "<color>"; inherits: true; initial-value: #f2f2f7; }
  @property --c-text-subtle { syntax: "<color>"; inherits: true; initial-value: #b3b5c8; }
  @property --c-accent { syntax: "<color>"; inherits: true; initial-value: #a99cff; }
  @property --c-on-accent { syntax: "<color>"; inherits: true; initial-value: #111111; }
  @property --c-glow-top { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-right { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-bottom { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-left { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-top-0 { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-top-1 { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-top-2 { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-right-0 { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-right-1 { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-right-2 { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-bottom-0 { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-bottom-1 { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-bottom-2 { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-left-0 { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-left-1 { syntax: "<color>"; inherits: true; initial-value: #6f7390; }
  @property --c-glow-left-2 { syntax: "<color>"; inherits: true; initial-value: #6f7390; }

  :global(:root) {
    transition:
      --c-background 400ms ease,
      --c-surface 400ms ease,
      --c-surface-raised 400ms ease,
      --c-border 400ms ease,
      --c-text 400ms ease,
      --c-text-subtle 400ms ease,
      --c-accent 400ms ease,
      --c-on-accent 400ms ease,
      --c-glow-top 400ms ease,
      --c-glow-right 400ms ease,
      --c-glow-bottom 400ms ease,
      --c-glow-left 400ms ease,
      --c-glow-top-0 400ms ease,
      --c-glow-top-1 400ms ease,
      --c-glow-top-2 400ms ease,
      --c-glow-right-0 400ms ease,
      --c-glow-right-1 400ms ease,
      --c-glow-right-2 400ms ease,
      --c-glow-bottom-0 400ms ease,
      --c-glow-bottom-1 400ms ease,
      --c-glow-bottom-2 400ms ease,
      --c-glow-left-0 400ms ease,
      --c-glow-left-1 400ms ease,
      --c-glow-left-2 400ms ease;
    color-scheme: dark;
  }
  :global(:root[data-scheme="light"]) {
    color-scheme: light;
  }
  @media (prefers-reduced-motion: reduce) {
    :global(:root) {
      transition: none;
    }
  }
  :global(body) {
    margin: 0;
    font-family: "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif;
    font-size: 13px;
    background: var(--c-background);
    color: var(--c-text);
    overflow: hidden;
  }
  :global(button) {
    font: inherit;
    color: var(--c-text);
    background: var(--c-surface-raised);
    border: 1px solid var(--c-border);
    border-radius: 4px;
    padding: 5px 10px;
    cursor: pointer;
  }
  :global(button:hover:not(:disabled)) {
    border-color: var(--c-accent);
  }
  :global(button:disabled) {
    opacity: 0.45;
    cursor: default;
  }
  :global(button.primary) {
    background: var(--c-accent);
    color: var(--c-on-accent);
    border-color: transparent;
  }
  :global(button.link) {
    background: none;
    border: none;
    padding: 0 4px;
    color: var(--c-accent);
  }
  :global(input:not([type])),
  :global(input[inputmode]) {
    font: inherit;
    color: var(--c-text);
    background: var(--c-surface);
    border: 1px solid var(--c-border);
    border-radius: 4px;
    padding: 5px 8px;
  }
  :global(input[type="range"]) {
    accent-color: var(--c-accent);
  }
  :global(:focus-visible) {
    outline: 2px solid var(--c-accent);
    outline-offset: 2px;
  }

  .app {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 420px;
    height: 100vh;
  }
  .library {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--c-border);
    background: var(--c-surface);
  }
  header,
  .folders,
  .tabs {
    padding: 10px 14px;
    border-bottom: 1px solid var(--c-border);
  }
  h1 {
    font-size: 16px;
    margin: 0 0 4px;
    font-weight: 600;
  }
  h2 {
    font-size: 14px;
    margin: 0;
    font-weight: 600;
  }
  h3 {
    font-size: 13px;
    margin: 0 0 6px;
  }
  .badge {
    font-size: 10px;
    font-weight: 600;
    background: var(--c-accent);
    color: var(--c-on-accent);
    border-radius: 3px;
    padding: 1px 5px;
    vertical-align: middle;
  }
  p {
    margin: 4px 0;
  }
  .subtle {
    color: var(--c-text-subtle);
  }
  .small {
    font-size: 11.5px;
  }
  .warn {
    color: #ffcc80;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .folder {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 3px 0;
  }
  .folder > span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .failures {
    max-height: 140px;
    overflow: auto;
    margin: 4px 0;
    padding-left: 16px;
    font-size: 11.5px;
  }
  .tabs {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .tabs button {
    background: none;
    border-color: transparent;
  }
  .tabs button.active {
    background: var(--c-surface-raised);
    border-color: var(--c-border);
  }
  .search {
    margin-left: auto;
    width: 200px;
  }
  .content {
    overflow: auto;
    padding: 12px 14px;
    flex: 1;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 14px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 3px;
    text-align: left;
    background: none;
    border: none;
    padding: 0;
  }
  .tile img,
  .no-art {
    width: 100%;
    aspect-ratio: 1;
    object-fit: cover;
    border-radius: 3px;
    background: var(--c-surface-raised);
    display: grid;
    place-items: center;
    color: var(--c-text-subtle);
    font-size: 11px;
  }
  .tile .t {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .no-art.sm {
    width: 36px;
    height: 36px;
  }
  .no-art.xs {
    width: 28px;
    height: 28px;
  }
  .named {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .named button {
    display: grid;
    grid-template-columns: 36px 1fr auto;
    gap: 10px;
    align-items: center;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    padding: 4px;
  }
  .named img {
    width: 36px;
    height: 36px;
    object-fit: cover;
    border-radius: 2px;
  }
  .album-head {
    display: flex;
    gap: 14px;
    align-items: flex-start;
    margin-bottom: 12px;
  }
  .album-head img {
    width: 120px;
    height: 120px;
    object-fit: cover;
    border-radius: 3px;
  }
  .songs {
    width: 100%;
    border-collapse: collapse;
  }
  .songs th {
    text-align: left;
    font-weight: 600;
    color: var(--c-text-subtle);
    font-size: 11.5px;
    padding: 4px 6px;
  }
  .songs td {
    padding: 3px 6px;
    border-top: 1px solid color-mix(in oklab, var(--c-border) 50%, transparent);
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: default;
  }
  .songs img {
    width: 28px;
    height: 28px;
    object-fit: cover;
    border-radius: 2px;
    display: block;
  }
  .songs tr.current td {
    color: var(--c-accent);
  }
  .songs tbody tr:hover td {
    background: var(--c-surface-raised);
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .player {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 56px 24px 24px;
    overflow: auto;
    background: var(--c-background);
  }
  .player > :global(*) {
    flex: none;
  }
  .meta {
    text-align: center;
    margin-top: 18px;
  }
  .meta h2 {
    font-size: 17px;
  }
  .seek {
    display: grid;
    grid-template-columns: 44px 1fr 44px;
    gap: 8px;
    align-items: center;
    width: 100%;
    font-variant-numeric: tabular-nums;
  }
  .seek span:last-child {
    text-align: right;
  }
  .controls {
    display: flex;
    gap: 8px;
  }
  .big {
    min-width: 80px;
  }
  .sliders {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
  }
  .sliders label {
    display: grid;
    grid-template-columns: 70px 1fr;
    align-items: center;
    gap: 8px;
  }
  .sliders label span {
    grid-column: 2;
    grid-row: 2;
  }
  .sliders .check {
    display: flex;
  }
  .swatches {
    display: flex;
    width: 100%;
    height: 14px;
    border-radius: 3px;
    overflow: hidden;
  }
  .swatches span {
    flex: 1;
  }
  .swatches .sep {
    flex: 0 0 4px;
    background: transparent;
  }
  .queue {
    width: 100%;
  }
  .qrow {
    display: grid;
    grid-template-columns: 28px 1fr auto;
    gap: 8px;
    align-items: center;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    padding: 3px 0;
  }
  .qrow img {
    width: 28px;
    height: 28px;
    object-fit: cover;
    border-radius: 2px;
  }
  .toasts {
    position: fixed;
    bottom: 14px;
    left: 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    z-index: 10;
  }
  .toast {
    background: #3b1f22;
    color: #ffd7d4;
    border: 1px solid #7a3a3f;
    border-radius: 4px;
    padding: 8px 12px;
    max-width: 520px;
  }
</style>
