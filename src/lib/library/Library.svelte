<script lang="ts">
  // The library window (spec D). The playing cover sits in the left rail
  // (positioned by the stage); this draws everything around it.
  import { audioDir } from "@tauri-apps/api/path";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, coverUrl, formatTime, mono, type AlbumRow, type NamedCount, type TrackRow } from "$lib/api";
  import { lib } from "$lib/library.svelte";
  import { app, win, type LibView } from "$lib/state.svelte";
  import Icon from "$lib/ui/Icon.svelte";
  import Seek from "$lib/ui/Seek.svelte";
  import Slider from "$lib/ui/Slider.svelte";
  import AlbumsView from "./AlbumsView.svelte";
  import AlbumView from "./AlbumView.svelte";
  import ArtistsView from "./ArtistsView.svelte";
  import GenresView from "./GenresView.svelte";
  import LibSettings from "./LibSettings.svelte";
  import MissingView from "./MissingView.svelte";
  import SongsView from "./SongsView.svelte";

  let { oncollapse, onedit }: { oncollapse: () => void; onedit: () => void } = $props();

  let scroller: HTMLDivElement | undefined = $state();
  let artistFilter = $state<string | null>(null);
  let genreFilter = $state<string | null>(null);

  const tabs: [LibView, string][] = [
    ["albums", "Albums"],
    ["artists", "Artists"],
    ["songs", "Songs"],
    ["genres", "Genres"],
    ["missing", "Missing Covers"],
    ["settings", "Settings"],
  ];
  function setTab(v: LibView) {
    app.libView = v;
    artistFilter = null;
    genreFilter = null;
    lib.album = null;
    scroller?.scrollTo({ top: 0 });
  }
  const sel = (v: LibView) => app.libView === v || (v === "albums" && app.libView === "album");

  // First run / initial scan take over the panel.
  const firstRun = $derived(!!app.stats && app.stats.tracks === 0 && app.folders.length === 0 && !app.scanning);
  const initialScan = $derived(app.scanning && (app.stats?.tracks ?? 0) === 0);
  const missingCount = $derived(app.stats?.missingCovers ? lib.missingAlbums.length || null : null);
  $effect(() => {
    void app.libraryVersion;
    lib.ensure("albums");
    api.folders().then((f) => { app.folders = f; }, () => {});
  });

  // ---- Search -------------------------------------------------------------
  let query = $state("");
  let searchOpen = $state(false);
  let results = $state<{ songs: TrackRow[]; albums: AlbumRow[]; artists: NamedCount[] } | null>(null);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let searchSeq = 0;
  function onQuery() {
    searchOpen = true;
    clearTimeout(searchTimer);
    const q = query.trim();
    if (!q) {
      results = null;
      return;
    }
    searchTimer = setTimeout(async () => {
      const seq = ++searchSeq;
      const [songs, albums, artists] = await Promise.all([
        api.tracks({ search: q, limit: 4 }).catch(() => []),
        api.albums(undefined, q).catch(() => []),
        api.artists(q).catch(() => []),
      ]);
      if (seq === searchSeq) results = { songs, albums: albums.slice(0, 4), artists: artists.slice(0, 3) };
    }, 80);
  }
  function closeSearch() {
    searchOpen = false;
  }
  export function escape(): boolean {
    if (searchOpen && query) {
      searchOpen = false;
      return true;
    }
    return false;
  }
  function openAlbum(key: string) {
    app.libView = "album";
    app.albumKey = key;
    lib.openAlbum(key);
    closeSearch();
  }
  function openArtist(name: string) {
    app.libView = "albums";
    lib.album = null;
    artistFilter = name;
    closeSearch();
  }

  // ---- Caption buttons & snap flyout ---------------------------------------
  let snapOpen = $state(false);
  let snapTimer: ReturnType<typeof setTimeout> | undefined;
  const layouts: [number, number, number, number][][] = [
    [[0, 0, 0.5, 1], [0.5, 0, 0.5, 1]],
    [[0, 0, 2 / 3, 1], [2 / 3, 0, 1 / 3, 1]],
    [[0, 0, 1 / 3, 1], [1 / 3, 0, 1 / 3, 1], [2 / 3, 0, 1 / 3, 1]],
    [[0, 0, 0.5, 0.5], [0.5, 0, 0.5, 0.5], [0, 0.5, 0.5, 0.5], [0.5, 0.5, 0.5, 0.5]],
    [[0, 0, 1 / 3, 1], [1 / 3, 0, 2 / 3, 1]],
    [[0, 0, 0.5, 1], [0.5, 0, 0.5, 0.5], [0.5, 0.5, 0.5, 0.5]],
  ];
  async function snap(z: [number, number, number, number]) {
    snapOpen = false;
    const f = await app.guard(api.windowFrame());
    if (!f) return;
    if (f.maximized) await app.guard(api.windowToggleMaximize());
    const w = f.work;
    await app.guard(api.windowSetFrame({ x: w.x + z[0] * w.w, y: w.y + z[1] * w.h, w: z[2] * w.w, h: z[3] * w.h }));
  }

  // ---- Title bar drag ------------------------------------------------------
  function barDown(e: MouseEvent) {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button, input, .results, .snap")) return;
    if (e.detail === 2) {
      app.guard(api.windowToggleMaximize());
      return;
    }
    win.startDragging().catch(() => {});
  }

  // ---- First run -------------------------------------------------------------
  let suggested = $state("");
  $effect(() => {
    if (firstRun && !suggested) audioDir().then((d) => { suggested = d; }, () => {});
  });
  async function addFolder(path?: string) {
    const p = path ?? (await open({ directory: true, multiple: false, title: "Add Music Folder" }));
    if (typeof p !== "string") return;
    const f = await app.guard(api.addFolder(p));
    if (f) {
      app.folders = [...app.folders, f];
      app.scanning = true;
    }
  }

  const upNext = $derived.by(() => {
    const i = app.status?.index;
    if (i == null || app.queue.length < 2) return [];
    return [1, 2, 3, 4].map((k) => (i + k) % app.queue.length).filter((j, n, arr) => j !== i && arr.indexOf(j) === n).map((j) => ({ q: app.queue[j], j }));
  });
  const scanPct = $derived(app.progress && app.progress.total ? Math.round((app.progress.processed / app.progress.total) * 100) : 0);
</script>

<div class="lib">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="bar" onmousedown={barDown}>
    <button class="ib solid col" onclick={oncollapse} aria-label="Collapse to Player" title="Collapse to Player (Esc)"><Icon name="collapse" /></button>
    <div class="brand subtle">Chameleon</div>
    <div class="search">
      <div class="sbox">
        <span class="si"><Icon name="search" stroke={2} /></span>
        <input
          bind:value={query}
          oninput={onQuery}
          onfocus={() => (searchOpen = true)}
          onblur={() => setTimeout(closeSearch, 150)}
          placeholder="Search Songs, Albums, Artists"
          aria-label="Search Library"
        />
      </div>
      {#if searchOpen && query.trim() && results}
        <div class="results">
          {#if !results.songs.length && !results.albums.length && !results.artists.length}
            <div class="none subtle">Nothing matches “{query.trim()}”.</div>
          {/if}
          {#if results.songs.length}
            <div class="eyebrow g">Songs</div>
            {#each results.songs as t (t.id)}
              <button class="res" onmousedown={(e) => e.preventDefault()} onclick={() => (app.playOne(t), closeSearch())}>
                <span class="rt">{#if t.cover}<img src={coverUrl(t.cover.thumb)} alt="" />{:else}{mono(t.title)}{/if}</span>
                <span class="rw"><span class="ellipsis">{t.title}</span><span class="ellipsis subtle">Song · {t.artist}</span></span>
              </button>
            {/each}
          {/if}
          {#if results.albums.length}
            <div class="eyebrow g">Albums</div>
            {#each results.albums as a (a.key)}
              <button class="res" onmousedown={(e) => e.preventDefault()} onclick={() => openAlbum(a.key)}>
                <span class="rt">{#if a.cover}<img src={coverUrl(a.cover.thumb)} alt="" />{:else}{mono(a.title)}{/if}</span>
                <span class="rw"><span class="ellipsis">{a.title}</span><span class="ellipsis subtle">Album · {a.artist}</span></span>
              </button>
            {/each}
          {/if}
          {#if results.artists.length}
            <div class="eyebrow g">Artists</div>
            {#each results.artists as r (r.name)}
              <button class="res" onmousedown={(e) => e.preventDefault()} onclick={() => openArtist(r.name)}>
                <span class="rt">{#if r.cover}<img src={coverUrl(r.cover.thumb)} alt="" />{:else}{mono(r.name)}{/if}</span>
                <span class="rw"><span class="ellipsis">{r.name}</span><span class="ellipsis subtle">Artist · {r.albumCount} {r.albumCount === 1 ? "Album" : "Albums"}</span></span>
              </button>
            {/each}
          {/if}
        </div>
      {/if}
    </div>
    <div class="caps">
      <button class="cap" onclick={() => win.minimize()} aria-label="Minimize"><Icon name="minimize" size={14} /></button>
      <div
        class="snapwrap"
        role="group"
        onmouseenter={() => {
          clearTimeout(snapTimer);
          snapTimer = setTimeout(() => (snapOpen = true), 350);
        }}
        onmouseleave={() => {
          clearTimeout(snapTimer);
          snapOpen = false;
        }}
      >
        <button class="cap" onclick={() => app.guard(api.windowToggleMaximize())} aria-label="Maximize"><Icon name="maximize" size={12} stroke={1.8} /></button>
        {#if snapOpen}
          <div class="snap">
            {#each layouts as zones, li (li)}
              <div class="layout">
                {#each zones as z, zi (zi)}
                  <button
                    class="zone"
                    style:left="{z[0] * 100}%"
                    style:top="{z[1] * 100}%"
                    style:width="calc({z[2] * 100}% - 3px)"
                    style:height="calc({z[3] * 100}% - 3px)"
                    onclick={() => snap(z)}
                    aria-label="Snap Window"
                  ></button>
                {/each}
              </div>
            {/each}
          </div>
        {/if}
      </div>
      <button class="cap close" onclick={oncollapse} aria-label="Close Library"><Icon name="close" size={14} /></button>
    </div>
  </div>

  <div class="rail">
    <div class="who">
      <div class="title ellipsis">{app.now?.title ?? "Nothing Playing"}</div>
      <div class="sub subtle ellipsis">{app.now ? [app.now.artist, app.now.album].filter(Boolean).join(" · ") : "Pick Something to Play"}</div>
    </div>
    <Seek layout="below" ring="var(--ch-bg)" />
    <div class="transport">
      <button class="ib solid t" onclick={() => app.prev()} aria-label="Previous"><Icon name="prev" stroke={1.8} /></button>
      <button class="play" onclick={() => app.toggle()} aria-label={app.playing ? "Pause" : "Play"}><Icon name={app.playing ? "pause" : "play"} /></button>
      <button class="ib solid t" onclick={() => app.next()} aria-label="Next"><Icon name="next" stroke={1.8} /></button>
    </div>
    <div class="vol">
      <button class="vi" onclick={() => app.toggleMute()} aria-label={(app.status?.volume ?? 0) > 0 ? "Mute" : "Unmute"}>
        <Icon name={(app.status?.volume ?? 0) > 0 ? "volume" : "mute"} />
      </button>
      <Slider value={app.status?.volume ?? 0} label="Volume" oninput={(v) => app.setVolume(v)} />
    </div>
    {#if upNext.length}
      <div class="eyebrow upn">Up Next</div>
      <div class="next">
        {#each upNext as { q, j } (j)}
          <button class="nrow" onclick={() => app.cmd({ type: "playIndex", index: j })}>
            <span class="nt" style:box-shadow={q.cover ? `0 0 0 1px ${q.cover.edge}e6, 0 2px 10px -2px ${q.cover.glow}b3` : "0 0 0 1px var(--ch-hairline)"}>
              {#if q.cover}<img src={coverUrl(q.cover.thumb)} alt="" />{:else}{mono(q.title)}{/if}
            </span>
            <span class="nw"><span class="ellipsis">{q.title}</span><span class="ellipsis subtle">{q.artist}</span></span>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <div class="panel">
    {#if !firstRun && !initialScan}
      <div class="tabs" role="tablist">
        {#each tabs as [v, label] (v)}
          <button role="tab" aria-selected={sel(v)} class:sel={sel(v)} class:right={v === "settings"} onclick={() => setTab(v)}>
            <span>{label}</span>
            {#if v === "missing" && missingCount}<span class="badge">{missingCount}</span>{/if}
            <span class="tbar"></span>
          </button>
        {/each}
      </div>
    {/if}
    <div class="content" bind:this={scroller}>
      {#if firstRun}
        <div class="center">
          <div class="first">
            <div class="dashes"><span></span><span></span><span></span></div>
            <div class="big">Show Me Your Music</div>
            <div class="subtle p">Pick the folders you keep music in. Chameleon reads tags and artwork, and keeps watching for new files.</div>
            <button class="btn primary" onclick={() => addFolder()}>Add Music Folder…</button>
            {#if suggested}
              <button class="link subtle" onclick={() => addFolder(suggested)}>Suggested: <span class="mono">{suggested}</span></button>
            {/if}
          </div>
        </div>
      {:else if initialScan}
        <div class="center">
          <div class="scan">
            <div class="big ellipsis">Scanning {app.progress?.folder ?? ""}</div>
            <div class="sbar"><div style:width="{scanPct}%"></div></div>
            <div class="cards">
              <div class="card"><div class="num">{(app.progress?.found ?? 0).toLocaleString()}</div><div class="subtle">Files Found</div></div>
              <div class="card"><div class="num">{(app.progress?.withCover ?? 0).toLocaleString()}</div><div class="subtle">With Artwork</div></div>
              <div class="card"><div class="num accent">{(app.progress?.noCover ?? 0).toLocaleString()}</div><div class="subtle">No Cover Yet</div></div>
            </div>
            <div class="mono file subtle ellipsis">{app.progress?.file ?? ""}</div>
          </div>
        </div>
      {:else if scroller}
        {#if app.libView === "albums"}
          <AlbumsView {scroller} artist={artistFilter} onclear={() => (artistFilter = null)} />
        {:else if app.libView === "album"}
          <AlbumView {onedit} />
        {:else if app.libView === "songs"}
          <SongsView {scroller} genre={genreFilter} onclear={() => (genreFilter = null)} />
        {:else if app.libView === "artists"}
          <ArtistsView onopen={openArtist} />
        {:else if app.libView === "genres"}
          <GenresView
            onopen={(g) => {
              app.libView = "songs";
              genreFilter = g;
            }}
          />
        {:else if app.libView === "missing"}
          <MissingView />
        {:else if app.libView === "settings"}
          <LibSettings />
        {/if}
      {/if}
    </div>
  </div>
</div>

<style>
  .lib {
    position: absolute;
    inset: 0;
    animation: chFadeIn 0.32s 0.2s var(--ease-out) both;
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: 40px;
    display: flex;
    align-items: center;
  }
  .col {
    margin-left: 6px;
    width: 36px;
    padding: 8px 10px;
  }
  .brand {
    font-size: 12px;
    margin-left: 4px;
  }
  .search {
    position: relative;
    margin: 0 auto;
    width: 360px;
  }
  .sbox {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 30px;
    padding: 0 10px;
    border-radius: 4px;
    background: var(--ch-panel);
    border: 1px solid var(--ch-hairline);
    border-bottom-color: var(--ch-text-subtle);
  }
  .si {
    width: 14px;
    height: 14px;
    color: var(--ch-text-subtle);
  }
  .sbox input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    font-size: 13px;
    height: 28px;
    color: var(--ch-text);
  }
  .sbox input:focus-visible {
    outline: none;
  }
  .sbox:focus-within {
    border-bottom: 2px solid var(--ch-accent);
  }
  .results {
    position: absolute;
    top: 36px;
    left: -40px;
    right: -40px;
    z-index: 20;
    padding: 8px;
    border-radius: 8px;
    background: var(--ch-surface);
    border: 1px solid var(--ch-hairline);
    box-shadow: 0 18px 50px rgb(0 0 0 / 0.4);
    animation: chFadeIn 0.14s ease-out both;
  }
  .none {
    padding: 14px;
    font-size: 13px;
  }
  .g {
    padding: 8px 8px 4px;
  }
  .res {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px 8px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    text-align: left;
    cursor: pointer;
    color: var(--ch-text);
  }
  .res:hover {
    background: var(--ch-hover);
  }
  .rt {
    width: 32px;
    height: 32px;
    flex: none;
    background: var(--ch-surface-2);
    display: grid;
    place-items: center;
    font-size: 12px;
    color: var(--ch-text-subtle);
    overflow: hidden;
  }
  .rt img {
    width: 32px;
    height: 32px;
    object-fit: cover;
  }
  .rw {
    min-width: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    font-size: 13px;
  }
  .rw .subtle {
    font-size: 12px;
  }
  .caps {
    display: flex;
    align-self: stretch;
  }
  .cap {
    width: 46px;
    border: 0;
    background: transparent;
    padding: 0 16px;
    cursor: pointer;
    color: var(--ch-text);
    display: grid;
    place-items: center;
  }
  .cap:hover {
    background: var(--ch-hover);
  }
  .cap.close {
    border-top-right-radius: 8px;
  }
  .cap.close:hover {
    background: #c42b1c;
    color: #fff;
  }
  .snapwrap {
    position: relative;
    display: flex;
  }
  .snap {
    position: absolute;
    top: 40px;
    right: -30px;
    z-index: 30;
    padding: 12px;
    border-radius: 8px;
    background: var(--ch-surface);
    border: 1px solid var(--ch-hairline);
    box-shadow: 0 16px 40px rgb(0 0 0 / 0.4);
    display: grid;
    grid-template-columns: repeat(3, 64px);
    gap: 10px;
    animation: chFadeIn 0.14s ease-out both;
  }
  .layout {
    position: relative;
    height: 44px;
  }
  .zone {
    position: absolute;
    border: 0;
    padding: 0;
    border-radius: 3px;
    background: var(--ch-surface-2);
    cursor: pointer;
  }
  .zone:hover {
    background: var(--ch-accent);
  }

  .rail {
    position: absolute;
    left: 24px;
    top: 328px;
    width: 252px;
    bottom: 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .who {
    min-width: 0;
  }
  .title {
    font-size: 17px;
    font-weight: 600;
    line-height: 1.25;
  }
  .sub {
    font-size: 13px;
    margin-top: 2px;
  }
  .transport {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
  }
  .t {
    width: 36px;
    height: 36px;
    padding: 9px;
  }
  .play {
    width: 44px;
    height: 44px;
    border: 0;
    border-radius: 4px;
    background: var(--ch-accent);
    color: var(--ch-on-accent);
    padding: 12px;
    cursor: pointer;
  }
  .play:hover {
    filter: brightness(1.08);
  }
  .vol {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .vi {
    width: 16px;
    height: 16px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--ch-text-subtle);
    cursor: pointer;
  }

  .upn {
    margin-top: 8px;
  }
  .next {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-height: 0;
    overflow: hidden;
  }
  .nrow {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 6px;
    margin: 0 -6px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    text-align: left;
    cursor: pointer;
    color: var(--ch-text);
  }
  .nrow:hover {
    background: var(--ch-hover);
  }
  .nt {
    width: 30px;
    height: 30px;
    flex: none;
    background: var(--ch-surface-2);
    overflow: hidden;
    display: grid;
    place-items: center;
    font-size: 11px;
    color: var(--ch-text-subtle);
  }
  .nt img {
    width: 30px;
    height: 30px;
    object-fit: cover;
  }
  .nw {
    min-width: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    font-size: 12.5px;
  }
  .nw .subtle {
    font-size: 11.5px;
  }

  .panel {
    position: absolute;
    left: 300px;
    right: 0;
    top: 40px;
    bottom: 0;
    border-top-left-radius: 8px;
    background: var(--ch-panel);
    border-top: 1px solid var(--ch-hairline);
    border-left: 1px solid var(--ch-hairline);
    display: flex;
    flex-direction: column;
  }
  .tabs {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 12px 20px 0;
    flex: none;
  }
  .tabs button {
    position: relative;
    height: 34px;
    padding: 0 12px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    cursor: pointer;
    font-size: 13.5px;
    color: var(--ch-text-subtle);
    display: flex;
    align-items: center;
    white-space: nowrap;
  }
  .tabs button:hover {
    background: var(--ch-hover);
  }
  .tabs button.sel {
    color: var(--ch-text);
    font-weight: 600;
  }
  .tabs .right {
    margin-left: auto;
  }
  .badge {
    margin-left: 6px;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 9px;
    background: var(--ch-accent);
    color: var(--ch-on-accent);
    font-size: 11px;
    font-weight: 600;
    display: inline-grid;
    place-items: center;
  }
  .tbar {
    position: absolute;
    left: 50%;
    bottom: 2px;
    width: 0;
    height: 3px;
    border-radius: 2px;
    background: var(--ch-accent);
    transition:
      width 0.2s var(--ease-out),
      margin-left 0.2s var(--ease-out);
  }
  .sel .tbar {
    width: 16px;
    margin-left: -8px;
  }
  .content {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 20px 24px 28px;
    overscroll-behavior: contain;
  }
  .center {
    height: 100%;
    display: grid;
    place-items: center;
  }
  .first {
    width: 440px;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
  }
  .dashes {
    display: grid;
    grid-template-columns: repeat(3, 44px);
    gap: 6px;
    opacity: 0.9;
  }
  .dashes span {
    height: 44px;
    border: 1px dashed var(--ch-text-subtle);
  }
  .big {
    font-family: var(--font-display);
    font-size: 26px;
    font-weight: 300;
    max-width: 100%;
  }
  .p {
    font-size: 13.5px;
    line-height: 1.5;
    text-wrap: pretty;
  }
  .link {
    border: 0;
    background: transparent;
    font-size: 12px;
    cursor: pointer;
  }
  .link:hover {
    color: var(--ch-text);
  }
  .scan {
    width: 480px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .scan .big {
    font-size: 24px;
  }
  .sbar {
    height: 4px;
    border-radius: 2px;
    background: var(--ch-hairline);
    overflow: hidden;
  }
  .sbar div {
    height: 4px;
    background: var(--ch-accent);
    transition: width 0.2s linear;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
  }
  .card {
    padding: 14px;
    background: var(--ch-surface);
    border: 1px solid var(--ch-hairline);
    border-radius: 4px;
    font-size: 12px;
  }
  .num {
    font-size: 28px;
    font-weight: 300;
    font-variant-numeric: tabular-nums;
  }
  .num.accent {
    color: var(--ch-accent);
  }
  .file {
    font-size: 11.5px;
  }
</style>
