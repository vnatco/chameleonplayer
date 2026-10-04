<script lang="ts">
  // Tag editor on the back of the sleeve (spec C3). A picked cover previews
  // its palette app-wide right away; Cancel or leaving restores it.
  import { onDestroy, onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, coverUrl, type Candidate, type CoverEdit, type TagInfo } from "$lib/api";
  import Icon from "$lib/ui/Icon.svelte";
  import { app } from "$lib/state.svelte";

  type Draft = { title: string; artist: string; album: string; albumArtist: string; genre: string; year: string; trackNo: string; discNo: string };
  const empty: Draft = { title: "", artist: "", album: "", albumArtist: "", genre: "", year: "", trackNo: "", discNo: "" };

  let info = $state<TagInfo | null>(null);
  let base = $state<Draft>({ ...empty });
  let draft = $state<Draft>({ ...empty });
  let cover = $state<CoverEdit>({ action: "keep" });
  let artUrl = $state<string | null>(null);
  let candidates = $state<Candidate[] | null>(null);
  let finding = $state(false);
  let picking = $state<string | null>(null);
  let saving = $state(false);
  let saved = $state(false);
  let problem = $state<{ kind: "readonly" | "failed"; text: string } | null>(null);
  let loadError = $state<string | null>(null);
  let loadedPath: string | null = null;
  let savedTimer: ReturnType<typeof setTimeout> | undefined;

  const path = $derived(app.now?.path ?? null);
  const dirty = $derived(cover.action !== "keep" || (Object.keys(empty) as (keyof Draft)[]).some((k) => draft[k] !== base[k]));
  const currentArt = $derived(app.now?.cover ? coverUrl(app.now.cover.full) : null);
  const shownArt = $derived(cover.action === "keep" ? currentArt : cover.action === "remove" ? null : artUrl);

  async function load(p: string) {
    loadedPath = p;
    loadError = null;
    problem = null;
    try {
      const t = await api.readTags(p);
      if (p !== loadedPath) return;
      info = t;
      base = {
        title: t.title,
        artist: t.artist,
        album: t.album,
        albumArtist: t.albumArtist,
        genre: t.genre,
        year: t.year ? String(t.year) : "",
        trackNo: t.trackNo ? String(t.trackNo) : "",
        discNo: t.discNo ? String(t.discNo) : "",
      };
      draft = { ...base };
      resetCover();
    } catch (e) {
      loadError = String(e);
    }
  }

  $effect(() => {
    if (path && path !== loadedPath && !dirty) load(path);
  });
  onMount(() => {
    const onDrop = (e: Event) => pickFile((e as CustomEvent<string>).detail);
    window.addEventListener("editor-image", onDrop);
    return () => window.removeEventListener("editor-image", onDrop);
  });
  onDestroy(() => {
    clearTimeout(savedTimer);
    app.previewPalette(null);
  });

  function resetCover() {
    cover = { action: "keep" };
    artUrl = null;
    candidates = null;
    app.previewPalette(null);
  }

  async function pickFile(file: string) {
    problem = null;
    try {
      const [preview, pal] = await Promise.all([api.imagePreview(file), api.imagePalette(file)]);
      cover = { action: "replace", path: file };
      artUrl = preview;
      saved = false;
      app.previewPalette(pal);
    } catch (e) {
      app.notify(String(e), true);
    }
  }

  async function replace() {
    const f = await open({ multiple: false, directory: false, title: "Choose Cover Art", filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "bmp", "gif"] }] });
    if (typeof f === "string") pickFile(f);
  }

  async function findOnline() {
    finding = true;
    candidates = [];
    try {
      candidates = await api.findCovers(draft.artist || draft.albumArtist, draft.album, draft.title);
    } catch (e) {
      candidates = null;
      app.notify(String(e), true);
    } finally {
      finding = false;
    }
  }

  async function pickCandidate(c: Candidate) {
    picking = c.image;
    try {
      const local = await api.downloadCover(c.image);
      const pal = await api.imagePalette(local);
      cover = { action: "replace", path: local };
      artUrl = coverUrl(local);
      saved = false;
      app.previewPalette(pal);
    } catch (e) {
      app.notify(String(e), true);
    } finally {
      picking = null;
    }
  }

  function remove() {
    cover = { action: "remove" };
    artUrl = null;
    saved = false;
    app.previewPalette(app.fallback);
  }

  const num = (s: string) => {
    const n = parseInt(s.trim(), 10);
    return Number.isFinite(n) && n > 0 ? n : null;
  };
  const yearBad = $derived(draft.year.trim() !== "" && !/^\d{1,4}$/.test(draft.year.trim()));
  const edit = () => ({
    title: draft.title,
    artist: draft.artist,
    album: draft.album,
    albumArtist: draft.albumArtist,
    genre: draft.genre,
    year: num(draft.year),
    trackNo: num(draft.trackNo),
    discNo: num(draft.discNo),
    cover: $state.snapshot(cover) as CoverEdit,
  });

  function afterSave() {
    base = { ...draft };
    cover = { action: "keep" };
    candidates = null;
    app.preview = null; // the refreshed cover brings its own palette
    saved = true;
    clearTimeout(savedTimer);
    savedTimer = setTimeout(() => (saved = false), 2400);
  }

  async function save() {
    if (!path || !dirty || saving || yearBad) return;
    if (info?.readOnly) {
      problem = { kind: "readonly", text: "" };
      return;
    }
    saving = true;
    problem = null;
    try {
      await api.writeTags(path, edit());
      afterSave();
    } catch (e) {
      problem = { kind: "failed", text: String(e) };
    } finally {
      saving = false;
    }
  }

  async function saveLibraryOnly() {
    if (!path) return;
    saving = true;
    try {
      await api.updateTrackOnly(path, edit());
      afterSave();
      problem = null;
      app.notify("Saved to library. The file is unchanged.");
    } catch (e) {
      app.notify(String(e), true);
    } finally {
      saving = false;
    }
  }

  function cancel() {
    draft = { ...base };
    resetCover();
    problem = null;
    app.flip();
  }

  const fields: [keyof Draft, string, boolean][] = [
    ["title", "Title", true],
    ["artist", "Artist", true],
    ["album", "Album", false],
    ["albumArtist", "Album Artist", false],
    ["genre", "Genre", false],
    ["year", "Year", false],
    ["trackNo", "Track", false],
    ["discNo", "Disc", false],
  ];
</script>

<div class="body">
  {#if loadError}
    <p class="err">{loadError}</p>
  {:else if !path}
    <p class="subtle">Nothing is playing, so there's nothing to edit.</p>
  {:else}
    <div class="top">
      <div class="art">
        {#if shownArt}
          <img src={shownArt} alt="" />
        {:else}
          <span class="none subtle">No Artwork<br />Drop an Image</span>
        {/if}
      </div>
      <div class="actions">
        <button class="act" onclick={replace}><Icon name="image" size={15} />Replace…</button>
        <button class="act" onclick={findOnline} disabled={finding}><Icon name="search" size={15} stroke={2} />{finding ? "Searching…" : "Find Online"}</button>
        <button class="act indent" onclick={remove}>Remove</button>
        <button class="act indent" onclick={resetCover} disabled={cover.action === "keep"}>Revert</button>
      </div>
    </div>

    {#if candidates}
      <div class="cands">
        {#if finding}
          {#each [0, 1, 2] as i (i)}<div class="cand skel"><span></span></div>{/each}
        {:else if candidates.length === 0}
          <p class="subtle small">No covers found online for this album. Try fixing the artist or album name first.</p>
        {:else}
          {#each candidates as c (c.image)}
            <button class="cand" onclick={() => pickCandidate(c)} disabled={picking != null} title="{c.artist} - {c.release}">
              <span class="cimg">
                <img src={c.thumb} alt="" loading="lazy" />
                {#if picking === c.image}<span class="spin"></span>{/if}
              </span>
              <span class="clabel ellipsis" title="{c.artist} - {c.release}">{c.label}{c.year ? ` · ${c.year}` : ""}</span>
            </button>
          {/each}
        {/if}
      </div>
    {/if}

    <div class="fields">
      {#each fields as [k, label, full] (k)}
        <label class:full>
          <span>{label}</span>
          <input
            class="field"
            class:error={k === "year" && yearBad}
            bind:value={draft[k]}
            oninput={() => (saved = false)}
            placeholder={`Add ${label}`}
            inputmode={k === "year" || k === "trackNo" || k === "discNo" ? "numeric" : undefined}
          />
        </label>
      {/each}
    </div>

    {#if problem}
      <div class="alert" role="alert">
        {#if problem.kind === "readonly"}
          <div><span class="danger">Can't Write to This File.</span> It's read-only or in a protected folder.</div>
        {:else}
          <div><span class="danger">Couldn't Save.</span> {problem.text} The file was left untouched.</div>
        {/if}
        <div class="row">
          {#if app.now?.inLibrary}
            <button class="btn primary sm" onclick={saveLibraryOnly} disabled={saving}>Save to Library Only</button>
          {/if}
          <button class="btn ghost sm" onclick={() => (problem = null)}>Dismiss</button>
        </div>
      </div>
    {/if}
  {/if}
</div>

<div class="foot">
  <div class="state">
    {#if saved}
      <span class="ok"><Icon name="check" size={14} stroke={2.4} /></span><span>Saved</span>
    {:else if dirty}
      <span class="dot"></span><span class="subtle">Unsaved Changes</span>
    {/if}
  </div>
  <button class="btn ghost sm" onclick={cancel}>Cancel</button>
  <button class="btn primary sm save" onclick={save} disabled={!dirty || saving || yearBad || !path}>{saving ? "Saving…" : "Save"}</button>
</div>

<style>
  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 12px 16px 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .top {
    display: flex;
    gap: 12px;
  }
  .art {
    position: relative;
    width: clamp(96px, 30cqw, 180px);
    height: clamp(96px, 30cqw, 180px);
    flex: none;
    background: var(--ch-surface-2);
    box-shadow: 0 0 0 1px var(--ch-edge);
    display: grid;
    place-items: center;
  }
  .art img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .none {
    font-size: 11px;
    text-align: center;
    padding: 8px;
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    flex: 1;
  }
  .act {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 8px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    font-size: 12.5px;
    cursor: pointer;
    text-align: left;
    color: var(--ch-text);
  }
  .act:hover:not(:disabled) {
    background: var(--ch-hover);
  }
  .act:disabled {
    color: var(--ch-text-subtle);
    cursor: default;
  }
  .act.indent {
    padding-left: 31px;
  }
  .cands {
    display: flex;
    gap: 8px;
  }
  .cand {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    background: transparent;
    cursor: pointer;
    text-align: left;
    color: var(--ch-text);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .cimg,
  .skel span {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 1;
    background: var(--ch-surface-2);
    box-shadow: 0 0 0 1px var(--ch-hairline);
  }
  .cimg img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .cand:hover .cimg {
    box-shadow: 0 0 0 2px var(--ch-accent);
  }
  .skel span {
    animation: pulse 1.1s ease-in-out infinite alternate;
  }
  @keyframes pulse {
    from {
      opacity: 0.5;
    }
    to {
      opacity: 1;
    }
  }
  .spin {
    position: absolute;
    left: calc(50% - 11px);
    top: calc(50% - 11px);
    width: 22px;
    height: 22px;
    border-radius: 11px;
    border: 2px solid var(--ch-hairline);
    border-top-color: var(--ch-accent);
    animation: chSpin 0.8s linear infinite;
  }
  .clabel {
    font-size: 10.5px;
    color: var(--ch-text-subtle);
  }
  .small {
    font-size: 12px;
    margin: 0;
  }
  .fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px 10px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 11.5px;
    color: var(--ch-text-subtle);
    min-width: 0;
  }
  label.full {
    grid-column: 1 / -1;
  }
  .alert {
    padding: 10px 12px;
    border-radius: 4px;
    background: var(--ch-surface-2);
    border: 1px solid var(--ch-danger);
    font-size: 12.5px;
    line-height: 1.45;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .danger {
    color: var(--ch-danger);
    font-weight: 600;
  }
  .err {
    color: var(--ch-danger);
    font-size: 12.5px;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .foot {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px 12px 16px;
    border-top: 1px solid var(--ch-hairline);
    background: var(--ch-bg);
  }
  .state {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 4px;
    background: var(--ch-accent);
    flex: none;
  }
  .ok {
    color: var(--ch-accent);
    display: grid;
  }
  .save {
    padding: 0 16px;
  }
</style>
