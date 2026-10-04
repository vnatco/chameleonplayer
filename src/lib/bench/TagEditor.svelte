<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, coverUrl, fileName, type Cover, type CoverEdit, type TagInfo } from "$lib/api";

  let { path, cover, onSaved }: { path: string; cover: Cover | null; onSaved?: () => void } = $props();

  let info = $state<TagInfo | null>(null);
  let form = $state({ title: "", artist: "", album: "", albumArtist: "", genre: "", year: "", trackNo: "", discNo: "" });
  let coverEdit = $state<CoverEdit>({ action: "keep" });
  let busy = $state(false);
  let message = $state<{ kind: "ok" | "error"; text: string } | null>(null);
  let loadError = $state<string | null>(null);
  let replacePreview = $state<string | null>(null);

  const dirty = $derived(
    info != null &&
      (coverEdit.action !== "keep" ||
        form.title !== info.title ||
        form.artist !== info.artist ||
        form.album !== info.album ||
        form.albumArtist !== info.albumArtist ||
        form.genre !== info.genre ||
        form.year !== (info.year?.toString() ?? "") ||
        form.trackNo !== (info.trackNo?.toString() ?? "") ||
        form.discNo !== (info.discNo?.toString() ?? "")),
  );

  async function load(p: string) {
    info = null;
    loadError = null;
    message = null;
    coverEdit = { action: "keep" };
    try {
      const t = await api.readTags(p);
      if (p !== path) return; // Track changed while loading.
      info = t;
      reset();
    } catch (e) {
      loadError = String(e);
    }
  }

  function reset() {
    if (!info) return;
    form = {
      title: info.title,
      artist: info.artist,
      album: info.album,
      albumArtist: info.albumArtist,
      genre: info.genre,
      year: info.year?.toString() ?? "",
      trackNo: info.trackNo?.toString() ?? "",
      discNo: info.discNo?.toString() ?? "",
    };
    coverEdit = { action: "keep" };
    message = null;
  }

  $effect(() => {
    load(path);
  });

  const num = (s: string) => {
    const n = parseInt(s.trim(), 10);
    return Number.isFinite(n) && n > 0 ? n : null;
  };

  async function chooseCover() {
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "bmp", "gif"] }],
    });
    if (typeof picked !== "string") return;
    try {
      replacePreview = await api.imagePreview(picked);
      coverEdit = { action: "replace", path: picked };
      message = null;
    } catch (e) {
      message = { kind: "error", text: String(e) };
    }
  }

  async function save() {
    busy = true;
    message = null;
    try {
      await api.writeTags(path, {
        title: form.title,
        artist: form.artist,
        album: form.album,
        albumArtist: form.albumArtist,
        genre: form.genre,
        year: num(form.year),
        trackNo: num(form.trackNo),
        discNo: num(form.discNo),
        cover: coverEdit,
      });
      message = { kind: "ok", text: "Saved." };
      await load(path);
      message = { kind: "ok", text: "Saved." };
      onSaved?.();
    } catch (e) {
      message = { kind: "error", text: String(e) };
    } finally {
      busy = false;
    }
  }

  const previewSrc = $derived(
    coverEdit.action === "replace"
      ? replacePreview
      : coverEdit.action === "remove"
        ? null
        : cover
          ? coverUrl(cover.thumb)
          : null,
  );
</script>

<section class="editor">
  <h3>Edit Tags <span class="file">{fileName(path)}</span></h3>

  {#if loadError}
    <p class="error">{loadError}</p>
  {:else if !info}
    <p class="subtle">Reading Tags...</p>
  {:else}
    {#if info.readOnly}
      <p class="error">This file is read-only, so changes can't be saved. Clear the read-only flag in its properties first.</p>
    {/if}
    <div class="grid">
      <div class="art">
        {#if previewSrc}
          <img src={previewSrc} alt="" />
        {:else}
          <div class="none">No Cover</div>
        {/if}
        <div class="row">
          <button onclick={chooseCover} disabled={busy || info.readOnly}>Choose New Cover</button>
          <button onclick={() => (coverEdit = { action: "remove" })} disabled={busy || info.readOnly || (!info.hasEmbeddedCover && coverEdit.action !== "replace")}>Remove Cover</button>
        </div>
        {#if coverEdit.action === "replace"}
          <p class="subtle small">New cover: {fileName(coverEdit.path)}. Images that aren't JPEG or PNG are converted to JPEG.</p>
        {:else if !info.hasEmbeddedCover && cover}
          <p class="subtle small">This cover comes from an image in the folder, not from the file.</p>
        {/if}
      </div>
      <div class="fields">
        <label>Title <input bind:value={form.title} disabled={info.readOnly} /></label>
        <label>Artist <input bind:value={form.artist} disabled={info.readOnly} /></label>
        <div class="two">
          <label>Album <input bind:value={form.album} disabled={info.readOnly} /></label>
          <label>Album Artist <input bind:value={form.albumArtist} disabled={info.readOnly} /></label>
        </div>
        <label>Genre <input bind:value={form.genre} disabled={info.readOnly} /></label>
        <div class="three">
          <label>Year <input bind:value={form.year} inputmode="numeric" disabled={info.readOnly} /></label>
          <label>Track <input bind:value={form.trackNo} inputmode="numeric" disabled={info.readOnly} /></label>
          <label>Disc <input bind:value={form.discNo} inputmode="numeric" disabled={info.readOnly} /></label>
        </div>
      </div>
    </div>
    <div class="actions">
      {#if message}
        <span class={message.kind}>{message.text}</span>
      {:else if dirty}
        <span class="subtle">Unsaved Changes</span>
      {/if}
      <button onclick={reset} disabled={busy || !dirty}>Revert</button>
      <button class="primary" onclick={save} disabled={busy || !dirty || info.readOnly}>{busy ? "Saving..." : "Save Changes"}</button>
    </div>
  {/if}
</section>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .file {
    font-weight: 400;
    color: var(--c-text-subtle);
    margin-left: 6px;
    font-size: 12px;
  }
  .grid {
    display: grid;
    grid-template-columns: 180px 1fr;
    gap: 16px;
  }
  .art img,
  .none {
    width: 180px;
    height: 180px;
    object-fit: cover;
    border-radius: 3px;
    display: grid;
    place-items: center;
    background: var(--c-surface-raised);
    color: var(--c-text-subtle);
  }
  .row {
    display: flex;
    gap: 6px;
    margin-top: 8px;
    flex-wrap: wrap;
  }
  .fields {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .three {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    color: var(--c-text-subtle);
  }
  .actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    align-items: center;
  }
  .actions > span {
    margin-right: auto;
    font-size: 12px;
  }
  .small {
    font-size: 11px;
    max-width: 180px;
  }
  .ok {
    color: var(--c-accent);
  }
  .error {
    color: #ff8a80;
  }
  .subtle {
    color: var(--c-text-subtle);
  }
</style>
