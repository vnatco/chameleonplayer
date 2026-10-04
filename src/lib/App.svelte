<script lang="ts">
  // App shell: bootstrap, events, keyboard, drag & drop, and the window
  // morph between the square player and the library.
  //
  // The player window has a fixed size (the largest sleeve plus its glow
  // margin), so size changes are pure CSS. Player <-> library keeps the
  // window's top-left fixed, so the cover never jumps on screen when the OS
  // window resizes; the frame and cover then move with GPU transforms only.
  import { onMount, tick } from "svelte";
  import { availableMonitors } from "@tauri-apps/api/window";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { api, events, type Rect } from "$lib/api";
  import { lib } from "$lib/library.svelte";
  import { PaletteDriver, prepare } from "$lib/palette";
  import { kick } from "$lib/progress";
  import { app, LIB_DEFAULT, MARGINS, PLAYER_WIN, RAIL_COVER, SIZES, win } from "$lib/state.svelte";
  import Library from "$lib/library/Library.svelte";
  import Sleeve, { type DropState } from "$lib/sleeve/Sleeve.svelte";
  import Icon from "$lib/ui/Icon.svelte";
  import { renderFallback, updateShell } from "$lib/shell";

  const MORPH_MS = 460;
  const IMAGE_EXT = /\.(jpe?g|png|webp|bmp|gif)$/i;
  const AUDIO_EXT = /\.(mp3|m4a|m4b|mp4|aac|flac|ogg|oga|wav|aiff?|aifc)$/i;

  let pointerInside = $state(true);
  let drop = $state<DropState>(null);
  let libFrame = $state({ w: LIB_DEFAULT.w, h: LIB_DEFAULT.h });
  let mica = $state(false);
  /** Morph phase: "start" = old geometry, no transition; "run" = animating to the new one. */
  let morph = $state<{ dir: "expand" | "collapse"; phase: "start" | "run"; from: Rect } | null>(null);
  let library: Library | undefined = $state();

  const S = $derived(SIZES[app.ui.size]);
  const off = $derived((PLAYER_WIN - S) / 2);
  const playerRect = $derived<Rect>({ x: off, y: off, w: S, h: S });
  const showLibrary = $derived(app.mode === "library" && morph?.dir !== "collapse");

  // ---- Sleeve geometry ------------------------------------------------------
  const sleeveStyle = $derived.by(() => {
    const lib = app.mode === "library" && morph?.dir !== "collapse";
    const r = lib ? { x: RAIL_COVER.x, y: RAIL_COVER.y, w: RAIL_COVER.s, h: RAIL_COVER.s } : playerRect;
    let transform = "none";
    let transition = app.anim && !morph ? `left ${MORPH_MS}ms var(--ease-out), top ${MORPH_MS}ms var(--ease-out), width ${MORPH_MS}ms var(--ease-out), height ${MORPH_MS}ms var(--ease-out)` : "none";
    if (morph) {
      // FLIP: start where the cover was, animate to where it is.
      const from = morph.dir === "expand" ? morph.from : { x: RAIL_COVER.x, y: RAIL_COVER.y, w: RAIL_COVER.s, h: RAIL_COVER.s };
      if (morph.phase === "start") transform = `translate(${from.x - r.x}px, ${from.y - r.y}px) scale(${from.w / r.w})`;
      transition = morph.phase === "run" && app.anim ? `transform ${MORPH_MS}ms var(--ease-out)` : "none";
    }
    return `left:${r.x}px;top:${r.y}px;width:${r.w}px;height:${r.h}px;transform:${transform};transition:${transition}`;
  });

  // The library frame grows from the cover (expand) or shrinks into it
  // (collapse) by animating a clip, so its content is never scaled or
  // distorted, only revealed, like a window opening.
  const frameStyle = $derived.by(() => {
    const full = !morph;
    const w = full ? "100vw" : `${libFrame.w}px`;
    const h = full ? "100vh" : `${libFrame.h}px`;
    let clip = "inset(0 round 8px)";
    let transition = "none";
    if (morph) {
      const atCover = (morph.dir === "expand") === (morph.phase === "start");
      const c = morph.dir === "expand" ? morph.from : playerRect;
      if (atCover) clip = `inset(${c.y}px ${libFrame.w - c.x - c.w}px ${libFrame.h - c.y - c.h}px ${c.x}px round 0px)`;
      if (morph.phase === "run" && app.anim) transition = `clip-path ${MORPH_MS}ms var(--ease-out)`;
    }
    return `width:${w};height:${h};clip-path:${clip};transition:${transition}`;
  });

  // ---- Click-through regions --------------------------------------------------
  $effect(() => {
    const rects: Rect[] = app.mode === "library" || morph ? [{ x: 0, y: 0, w: 10000, h: 10000 }] : [playerRect];
    api.windowHit(rects).catch(() => {});
  });

  // ---- Expand / collapse -----------------------------------------------------
  async function expand() {
    if (app.mode === "library" || morph) return;
    if (app.flipped) {
      app.flipped = false;
      app.applyPalette();
    }
    const f = await app.guard(api.windowFrame());
    if (!f) return;
    const saved = app.ui.remember ? app.ui.libFrame : undefined;
    const w = Math.min(saved?.w ?? LIB_DEFAULT.w, f.work.w);
    const h = Math.min(saved?.h ?? LIB_DEFAULT.h, f.work.h);
    // Keep the top-left, nudged onto the screen if needed.
    const x = Math.max(f.work.x, Math.min(f.x, f.work.x + f.work.w - w));
    const y = Math.max(f.work.y, Math.min(f.y, f.work.y + f.work.h - h));
    libFrame = { w, h };
    morph = { dir: "expand", phase: "start", from: { x: playerRect.x + f.x - x, y: playerRect.y + f.y - y, w: S, h: S } };
    app.mode = "library";
    app.applyPalette();
    await tick();
    await app.guard(api.windowSetFrame({ x, y, w, h }));
    requestAnimationFrame(() => requestAnimationFrame(() => morph && (morph = { ...morph, phase: "run" })));
    setTimeout(
      async () => {
        morph = null;
        try {
          await api.windowChrome(true, true);
          mica = true;
        } catch (e) {
          // Windows 10 or transparency off: solid background instead of Mica.
          mica = false;
          api.windowChrome(true, false).catch(() => {});
          console.warn("mica unavailable", e);
        }
      },
      app.anim ? MORPH_MS + 40 : 0,
    );
  }

  async function collapse() {
    if (app.mode !== "library" || morph) return;
    let f = await app.guard(api.windowFrame());
    if (!f) return;
    if (f.maximized) f = (await app.guard(api.windowToggleMaximize())) ?? f;
    else if (app.ui.remember) app.setUi("libFrame", { x: f.x, y: f.y, w: f.w, h: f.h });
    mica = false;
    await app.guard(api.windowChrome(false, false));
    libFrame = { w: f.w, h: f.h };
    morph = { dir: "collapse", phase: "start", from: playerRect };
    app.applyPalette();
    await tick();
    requestAnimationFrame(() => requestAnimationFrame(() => morph && (morph = { ...morph, phase: "run" })));
    setTimeout(
      async () => {
        await app.guard(api.windowSetFrame({ x: f!.x, y: f!.y, w: PLAYER_WIN, h: PLAYER_WIN }));
        app.mode = "player";
        morph = null;
        app.applyPalette();
      },
      app.anim ? MORPH_MS + 20 : 0,
    );
  }

  function toggleLibrary() {
    if (app.mode === "library") collapse();
    else expand();
  }

  function editFromLibrary() {
    collapse();
    setTimeout(() => app.flip("edit"), (app.anim ? MORPH_MS : 0) + 80);
  }

  // ---- Keyboard (spec H) ----------------------------------------------------
  function onKey(e: KeyboardEvent) {
    const t = e.target as HTMLElement;
    const typing = t.tagName === "INPUT" && (t as HTMLInputElement).type !== "range";
    if (e.key === "Escape") {
      if (library?.escape()) return;
      if (typing) {
        t.blur();
        return;
      }
      if (app.flipped) return app.flip();
      if (app.mode === "library") return collapse();
      return;
    }
    if (typing || e.ctrlKey || e.altKey || e.metaKey) return;
    switch (e.key) {
      case " ":
        if (t.tagName === "BUTTON") return;
        e.preventDefault();
        app.toggle();
        break;
      case "ArrowRight":
        if (t.getAttribute("role") === "slider" || t.tagName === "INPUT") return;
        app.seek(app.positionNow() + 5);
        break;
      case "ArrowLeft":
        if (t.getAttribute("role") === "slider" || t.tagName === "INPUT") return;
        app.seek(app.positionNow() - 5);
        break;
      case "ArrowUp":
        e.preventDefault();
        app.setVolume((app.status?.volume ?? 0) + 0.05);
        break;
      case "ArrowDown":
        e.preventDefault();
        app.setVolume((app.status?.volume ?? 0) - 0.05);
        break;
      case "n":
      case "N":
        app.next();
        break;
      case "f":
      case "F":
        app.flip();
        break;
      case "l":
      case "L":
        toggleLibrary();
        break;
    }
  }

  // ---- Drag & drop ------------------------------------------------------------
  function dropTarget(paths: string[], x: number, y: number) {
    const single = paths.length === 1 && IMAGE_EXT.test(paths[0]);
    if (app.mode === "library") {
      lib.dropTile = null;
      if (single && app.libView === "missing") {
        const el = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-album-key]");
        lib.dropTile = el?.dataset.albumKey ?? null;
      }
      return;
    }
    const r = playerRect;
    const inside = x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h;
    if (!inside || app.flipped) {
      drop = null;
      return;
    }
    drop = { kind: single ? "image" : "files", zone: app.ui.size === "mini" ? 0 : y < r.y + r.h / 2 ? 0 : 1 };
  }

  async function addToLibrary(folders: string[]) {
    for (const f of folders) {
      const added = await app.guard(api.addFolder(f));
      if (added) {
        app.scanning = true;
        app.notify(`Added ${added.path} to Your Library`);
      }
    }
  }

  async function setCoverFromImage(image: string) {
    const now = app.now;
    if (!now) {
      app.notify("Play a song first, then drop its cover on the sleeve.", true);
      return;
    }
    if (!app.ui.writeCovers && now.inLibrary) {
      const tags = await app.guard(api.readTags(now.path));
      if (!tags) return;
      await app.guard(api.updateTrackOnly(now.path, { ...tags, cover: { action: "replace", path: image } }));
      app.notify(`Cover Set for ${now.album || now.title} in Your Library`);
      return;
    }
    const tags = await app.guard(api.readTags(now.path));
    if (!tags) return;
    if (tags.readOnly) {
      if (now.inLibrary) {
        await app.guard(api.updateTrackOnly(now.path, { ...tags, cover: { action: "replace", path: image } }));
        app.notify("The file is read-only, so the cover was saved to your library only.");
      } else {
        app.notify("This file is read-only, so its cover can't be changed.", true);
      }
      return;
    }
    const ok = await app.guard(api.writeTags(now.path, { ...tags, cover: { action: "replace", path: image } }));
    if (ok !== undefined) app.notify(`Cover Set for ${now.album || now.title}`);
  }

  async function onDrop(paths: string[], x: number, y: number) {
    const single = paths.length === 1 && IMAGE_EXT.test(paths[0]);
    const z = drop;
    drop = null;
    const folders = paths.filter((p) => !/\.[a-z0-9]{2,5}$/i.test(p.split(/[\\/]/).pop() ?? ""));
    const audio = paths.filter((p) => AUDIO_EXT.test(p));

    if (app.mode === "library") {
      const key = lib.dropTile;
      lib.dropTile = null;
      if (single && key) {
        const a = lib.missingAlbums.find((m) => m.key === key);
        if (a) lib.assign(a, paths[0]);
        return;
      }
      if (folders.length) addToLibrary(folders);
      if (audio.length) app.guard(api.openPaths(audio));
      return;
    }
    if (app.flipped) {
      if (single && app.backTab === "edit") window.dispatchEvent(new CustomEvent("editor-image", { detail: paths[0] }));
      return;
    }
    if (!z) return;
    if (z.kind === "image") return setCoverFromImage(paths[0]);
    // Folders always go to the library.
    if (folders.length) addToLibrary(folders);
    if (!audio.length) return;
    if (z.zone === 0) {
      const n = await app.guard(api.openPaths(audio));
      if (n) app.notify(`Playing ${n} ${n === 1 ? "File" : "Files"}`);
    } else {
      const dirs = [...new Set(audio.map((p) => p.replace(/[\\/][^\\/]+$/, "")))];
      addToLibrary(dirs);
    }
  }

  // ---- Boot -----------------------------------------------------------------
  onMount(() => {
    const root = document.documentElement;
    app.driver = new PaletteDriver(root);
    const mq = matchMedia("(prefers-reduced-motion: reduce)");
    app.reduceMotion = mq.matches;
    mq.onchange = () => (app.reduceMotion = mq.matches);

    let lastPaths: string[] = [];
    const unsubs: Promise<() => void>[] = [
      events.player((e) => {
        if (e.type === "status") {
          const { type: _t, ...s } = e;
          app.onStatus(s);
          kick();
        } else if (e.type === "position") {
          app.onPosition(e.position, e.duration);
          kick();
        } else if (e.type === "level") {
          app.level = e.value;
        } else if (e.type === "error") {
          app.notify(e.path ? `${e.path.split(/[\\/]/).pop()}: ${e.message}` : e.message, true);
        }
      }),
      events.scanProgress((p) => {
        app.scanning = true;
        app.progress = p;
      }),
      events.scanDone((d) => {
        app.scanning = false;
        app.progress = null;
        if (d.error) app.notify(`Scan failed: ${d.error}`, true);
        if (d.summary) {
          app.summary = d.summary;
          const s = d.summary;
          if (s.added || s.removed) app.notify(`Library Updated: ${s.added} New, ${s.removed} Removed`);
        }
      }),
      events.libraryChanged((paths) => {
        app.libraryVersion++;
        app.guard(api.stats()).then((s) => { if (s) app.stats = s; });
        const cur = app.now?.path;
        if (!paths.length || (cur && paths.includes(cur))) app.refreshNow();
        if (lib.album) lib.refreshAlbum();
      }),
      events.appError((m) => app.notify(m, true)),
      events.pointerInside((inside) => (pointerInside = inside)),
      getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        const dpr = window.devicePixelRatio || 1;
        if (p.type === "enter") {
          lastPaths = p.paths;
          dropTarget(p.paths, p.position.x / dpr, p.position.y / dpr);
        } else if (p.type === "over") dropTarget(lastPaths, p.position.x / dpr, p.position.y / dpr);
        else if (p.type === "drop") onDrop(p.paths, p.position.x / dpr, p.position.y / dpr);
        else {
          drop = null;
          lib.dropTile = null;
        }
      }),
    ];

    (async () => {
      const b = await app.guard(api.bootstrap());
      if (!b) {
        await api.windowShow().catch(() => {});
        return;
      }
      app.fallback = b.fallback;
      app.loadUi(b.settings.ui);
      app.coverSource = b.settings.coverSource;
      app.stats = b.stats;
      app.scanning = b.scanning;
      root.dataset.motion = app.anim ? "on" : "off";
      app.driver!.set(prepare(b.fallback), 0);

      // Always start as the square player, whatever state the window was
      // left in (e.g. a reload while the library was open).
      await api.windowChrome(false, false).catch(() => {});
      const start = await app.guard(api.windowFrame());
      if (start && (Math.round(start.w) !== PLAYER_WIN || Math.round(start.h) !== PLAYER_WIN)) {
        await app.guard(api.windowSetFrame({ x: start.x, y: start.y, w: PLAYER_WIN, h: PLAYER_WIN }));
      }

      // Restore the window where it was, if that spot is still on a screen.
      if (app.ui.remember && app.ui.playerPos) {
        const { x, y } = app.ui.playerPos;
        const mons = await availableMonitors().catch(() => []);
        const cx = x + PLAYER_WIN / 2;
        const cy = y + PLAYER_WIN / 2;
        const onScreen = mons.some((m) => {
          const s = m.scaleFactor;
          const wx = m.workArea.position.x / s;
          const wy = m.workArea.position.y / s;
          return cx >= wx && cy >= wy && cx < wx + m.workArea.size.width / s && cy < wy + m.workArea.size.height / s;
        });
        if (onScreen) await app.guard(api.windowSetFrame({ x, y, w: PLAYER_WIN, h: PLAYER_WIN }));
      }
      await app.guard(api.windowOnTop(app.ui.onTop));
      if (app.ui.pulse) app.cmd({ type: "setMeter", enabled: true });
      app.onStatus(b.status);
      app.guard(api.folders()).then((f) => { if (f) app.folders = f; });
      app.ready = true;
      // Show the window only once the first frame is painted (no flash).
      requestAnimationFrame(() => requestAnimationFrame(() => api.windowShow().catch(() => {})));
    })();

    // Remember where the player sits.
    let moveTimer: ReturnType<typeof setTimeout> | undefined;
    unsubs.push(
      win.onMoved(() => {
        clearTimeout(moveTimer);
        moveTimer = setTimeout(async () => {
          if (!app.ui.remember || morph) return;
          const f = await api.windowFrame().catch(() => null);
          if (!f) return;
          if (app.mode === "player") app.setUi("playerPos", { x: f.x, y: f.y });
          else if (!f.maximized) app.setUi("libFrame", { x: f.x, y: f.y, w: f.w, h: f.h });
        }, 400);
      }),
      win.onResized(() => {
        if (app.mode === "library" && !morph) libFrame = { w: window.innerWidth, h: window.innerHeight };
      }),
    );

    return () => unsubs.forEach((u) => u.then((f) => f()));
  });

  $effect(() => {
    document.documentElement.dataset.motion = app.anim ? "on" : "off";
  });

  // Tray, taskbar thumbnail and media overlay follow the track.
  $effect(() => {
    if (!app.ready) return;
    const now = app.now;
    const pal = app.palette ?? app.fallback;
    if (!pal) return;
    const inLib = app.mode === "library";
    const coverRect = inLib ? { x: RAIL_COVER.x, y: RAIL_COVER.y, w: RAIL_COVER.s, h: RAIL_COVER.s } : playerRect;
    updateShell({
      path: now?.path ?? null,
      title: now?.title ?? "",
      artist: now?.artist ?? "",
      accent: pal.roles.accent,
      playing: app.playing,
      cover: now?.cover?.full ?? null,
      fallbackPng: now && !now.cover ? renderFallback(now.title, now.artist) : null,
      coverRect,
    });
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="stage" class:ready={app.ready}>
  {#if app.mode === "library" || morph}
    <div class="frame" class:mica class:round={app.mode === "library" && !morph} style={frameStyle}>
      {#if showLibrary}
        <Library bind:this={library} oncollapse={collapse} onedit={editFromLibrary} />
      {/if}
    </div>
  {/if}

  <div class="sleeve-pos" style={sleeveStyle}>
    <Sleeve inLibrary={app.mode === "library" && morph?.dir !== "collapse"} {drop} {pointerInside} onlibrary={expand} />
  </div>

  {#if app.toast}
    {#key app.toast.id}
      <div
        class="toast"
        class:err={app.toast.error}
        role="status"
        style:bottom={app.mode === "library" ? "24px" : `${PLAYER_WIN - (off + S) + 14}px`}
      >
        {#if app.toast.error}<span class="dot"></span>{:else}<span class="ok"><Icon name="check" size={14} stroke={2.4} /></span>{/if}
        <span>{app.toast.text}</span>
      </div>
    {/key}
  {/if}
</div>

<style>
  .stage {
    position: fixed;
    inset: 0;
    overflow: hidden;
    opacity: 0;
  }
  .stage.ready {
    opacity: 1;
  }
  .frame {
    position: absolute;
    left: 0;
    top: 0;
    background: var(--ch-bg);
    box-shadow: inset 0 0 0 1px var(--ch-hairline);
    overflow: hidden;
    will-change: clip-path;
  }
  .frame.round {
    border-radius: 8px;
  }
  .frame.mica {
    background: var(--ch-mica);
    transition: background-color 0.2s;
  }
  .sleeve-pos {
    position: absolute;
    transform-origin: 0 0;
    z-index: 2;
  }
  .toast {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: min(560px, 90vw);
    padding: 8px 14px;
    border-radius: 4px;
    background: var(--ch-surface-2);
    color: var(--ch-text);
    font-size: 12.5px;
    box-shadow:
      0 0 0 1px var(--ch-hairline),
      0 10px 30px rgb(0 0 0 / 0.35);
    animation: toastIn 0.2s var(--ease-out) both;
    pointer-events: none;
    z-index: 10;
  }
  .toast.err {
    box-shadow:
      0 0 0 1px var(--ch-danger),
      0 10px 30px rgb(0 0 0 / 0.35);
  }
  .toast .ok {
    color: var(--ch-accent);
    display: grid;
  }
  .toast .dot {
    width: 8px;
    height: 8px;
    border-radius: 4px;
    background: var(--ch-danger);
    flex: none;
  }
  @keyframes toastIn {
    from {
      opacity: 0;
      transform: translate(-50%, 8px);
    }
    to {
      opacity: 1;
      transform: translate(-50%, 0);
    }
  }
</style>
