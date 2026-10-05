<script lang="ts">
  // Settings → Library (spec D7).
  import { open } from "@tauri-apps/plugin-dialog";
  import DefaultPlayerRow from "$lib/ui/DefaultPlayerRow.svelte";
  import { api, type CoverSource } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import Segmented from "$lib/ui/Segmented.svelte";
  import Toggle from "$lib/ui/Toggle.svelte";

  $effect(() => {
    void app.libraryVersion;
    api.folders().then((f) => { app.folders = f; }, (e) => app.notify(String(e), true));
  });

  async function add() {
    const p = await open({ directory: true, multiple: false, title: "Add Music Folder" });
    if (typeof p !== "string") return;
    const f = await app.guard(api.addFolder(p));
    if (f) {
      app.folders = [...app.folders, f];
      app.scanning = true;
    }
  }
  async function remove(id: number) {
    if ((await app.guard(api.removeFolder(id))) !== undefined) app.folders = app.folders.filter((f) => f.id !== id);
  }
  async function rescan() {
    const ok = await app.guard(api.rescan());
    if (ok === false) app.notify("A scan is already running.");
    else if (ok) app.scanning = true;
  }
  async function setSource(s: CoverSource) {
    if ((await app.guard(api.setCoverSource(s))) !== undefined) {
      app.coverSource = s;
      app.scanning = true;
    }
  }
  const useFolder = $derived(app.coverSource !== "embeddedOnly");
  const missingFolders = $derived(new Set(app.summary?.missingFolders ?? []));
</script>

<div class="wrap">
  <section>
    <div class="h">Watched Folders</div>
    <div class="box">
      {#each app.folders as f (f.id)}
        <div class="frow">
          <span class="fp">
            <span class="mono path">{f.path}</span>
            <span class="subtle sub">
              {f.trackCount.toLocaleString()}
              {f.trackCount === 1 ? "File" : "Files"} · {missingFolders.has(f.path) ? "Not Reachable" : app.scanning ? "Scanning" : "Watching"}
            </span>
          </span>
          <button class="btn ghost sm" onclick={() => remove(f.id)} disabled={app.scanning}>Remove</button>
        </div>
      {:else}
        <div class="frow subtle">No Folders Yet.</div>
      {/each}
      <div class="actions">
        <button class="btn primary sm" onclick={add}>Add Folder…</button>
        <button class="btn ghost sm" onclick={rescan} disabled={app.scanning || !app.folders.length}>{app.scanning ? "Rescanning…" : "Rescan Now"}</button>
        {#if app.scanning}<button class="btn ghost sm" onclick={() => api.cancelScan()}>Stop</button>{/if}
      </div>
    </div>
    {#if app.summary && (app.summary.unsupported || app.summary.failedCount)}
      <p class="subtle note">
        {#if app.summary.unsupported}{app.summary.unsupported.toLocaleString()} files were skipped because their format (Opus, APE, WMA, WavPack, Musepack, DSD) can't be played yet.{/if}
        {#if app.summary.failedCount}
          {app.summary.failedCount.toLocaleString()} files couldn't be read{app.summary.failures[0] ? `, for example ${app.summary.failures[0].path.split(/[\\/]/).pop()}: ${app.summary.failures[0].message}` : ""}. The log file lists them all.
        {/if}
      </p>
    {/if}
  </section>

  <section>
    <div class="h">Where Covers Come From</div>
    <div class="subtle sub2">When a file has both, which one wins.</div>
    <Segmented
      label="Cover Source"
      options={[
        ["embedded", "Embedded First"],
        ["folder", "folder.jpg First"],
        ["embeddedOnly", "Embedded Only"],
      ] as [CoverSource, string][]}
      value={app.coverSource}
      onchange={setSource}
    />
  </section>

  <section>
    <div class="h">Default Player</div>
    <div class="subtle sub2">Windows asks you to confirm this on Chameleon's page in its Settings.</div>
    <DefaultPlayerRow />
  </section>

  <section class="toggles">
    <div class="trow">
      <span class="tl">Write Found Covers Into Files<span class="subtle">Embeds artwork in the tag and saves folder.jpg.</span></span>
      <Toggle on={app.ui.writeCovers} label="Write Found Covers Into Files" onchange={(v) => app.setUi("writeCovers", v)} />
    </div>
    <div class="trow">
      <span class="tl">Use folder.jpg / cover.png<span class="subtle">When a file has no embedded artwork.</span></span>
      <Toggle on={useFolder} label="Use Folder Images" onchange={(v) => setSource(v ? "embedded" : "embeddedOnly")} />
    </div>
  </section>
</div>

<style>
  .wrap {
    max-width: 640px;
    display: flex;
    flex-direction: column;
    gap: 22px;
  }
  .h {
    font-size: 15px;
    font-weight: 600;
    margin-bottom: 10px;
  }
  .sub2 {
    font-size: 12.5px;
    margin: -6px 0 10px;
  }
  .box {
    border: 1px solid var(--ch-hairline);
    border-radius: 4px;
    background: var(--ch-surface);
  }
  .frow {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--ch-hairline);
    font-size: 13px;
  }
  .fp {
    flex: 1;
    min-width: 0;
  }
  .path {
    display: block;
    font-size: 12.5px;
    overflow-wrap: anywhere;
    user-select: text;
  }
  .sub {
    display: block;
    font-size: 12px;
    margin-top: 2px;
  }
  .actions {
    display: flex;
    gap: 8px;
    padding: 10px 12px;
  }
  .note {
    font-size: 12px;
    line-height: 1.5;
    margin: 8px 0 0;
  }
  .toggles {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .trow {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 0;
    border-bottom: 1px solid var(--ch-hairline);
  }
  .tl {
    flex: 1;
    font-size: 13.5px;
  }
  .tl span {
    display: block;
    font-size: 12px;
    margin-top: 2px;
  }
</style>
