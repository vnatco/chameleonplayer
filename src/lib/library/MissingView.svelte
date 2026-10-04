<script lang="ts">
  // Missing covers (spec D6): every empty tile is a drop target; batch
  // search fills tiles one by one; accepting blooms the tile in its new
  // palette.
  import { coverUrl } from "$lib/api";
  import { lib } from "$lib/library.svelte";
  import { app } from "$lib/state.svelte";
  import Icon from "$lib/ui/Icon.svelte";

  $effect(() => {
    lib.ensure("albums");
  });
  const albums = $derived(lib.missingAlbums);
  const found = $derived(albums.filter((a) => lib.missing[a.key]?.st === "found").length);
  // Albums assigned in this session stay visible (marked done) until reload.
  const left = $derived(albums.filter((a) => lib.missing[a.key]?.st !== "assigned").length);
</script>

<div class="head">
  <div class="intro">
    <div class="h">{left ? `${left} ${left === 1 ? "Album" : "Albums"} Without Artwork` : "Every Album Has a Cover"}</div>
    <div class="subtle p">
      Drop an image from Explorer onto any tile, or let Chameleon look them up.
      {app.ui.writeCovers ? "Found covers are written into the files and the folder." : "Found covers are saved in your library only (see Settings)."}
    </div>
  </div>
  {#if found > 0}<button class="btn secondary" onclick={() => lib.acceptAll()}>Accept All Matches</button>{/if}
  <button class="btn primary" onclick={() => lib.findAll()} disabled={lib.finding || left === 0}>
    <Icon name="search" size={14} stroke={2} />{lib.finding ? "Searching…" : "Find All Online"}
  </button>
</div>

<div class="grid">
  {#each albums as a (a.key)}
    {@const m = lib.missing[a.key] ?? { st: "idle" }}
    {@const hot = lib.dropTile === a.key}
    <div class="tile" data-album-key={a.key}>
      <div class="box" class:hot class:flash={lib.flash === a.key}>
        {#if m.st === "found"}
          <img src={m.cand.thumb} alt="" style:opacity={0.92} />
          <div class="use">
            <button class="btn primary sm grow" onclick={() => lib.accept(a)}>Use · {m.cand.score}%</button>
            <button class="skip" onclick={() => lib.skip(a)} aria-label="Skip Match"><Icon name="close" stroke={2} /></button>
          </div>
        {:else if m.st === "assigned"}
          <div class="done"><Icon name="check" size={28} stroke={2} /></div>
        {:else if m.st === "searching" || m.st === "assigning"}
          <div class="center"><span class="spin"></span></div>
        {:else}
          <div class="empty" title={m.st === "none" ? (m.reason ?? "No cover found online.") : undefined}>
            <span class="ic"><Icon name="image" stroke={1.5} /></span>
            <span>{hot ? "Release to Assign" : m.st === "none" ? (m.reason ? "Can't Search · Drop Image" : "Nothing Found · Drop Image") : "Drop Image"}</span>
          </div>
        {/if}
      </div>
      <div class="meta">
        <div class="t ellipsis">{a.title}</div>
        <div class="s subtle ellipsis">
          {m.st === "assigned" ? "Cover Set ✓" : m.st === "found" ? "Match Found" : m.st === "searching" ? "Searching…" : `${a.artist} · ${a.trackCount} ${a.trackCount === 1 ? "Song" : "Songs"}`}
        </div>
      </div>
    </div>
  {/each}
</div>

<style>
  .head {
    display: flex;
    align-items: flex-end;
    gap: 16px;
    margin-bottom: 20px;
  }
  .intro {
    flex: 1;
    min-width: 0;
  }
  .h {
    font-family: var(--font-display);
    font-size: 24px;
    font-weight: 300;
  }
  .p {
    font-size: 13px;
    margin-top: 4px;
    text-wrap: pretty;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 22px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .box {
    position: relative;
    width: 100%;
    aspect-ratio: 1;
    background: var(--ch-surface);
    overflow: hidden;
    box-shadow: 0 0 0 1px var(--ch-hairline);
    transition:
      box-shadow 0.3s,
      background-color 0.2s;
  }
  .box.hot {
    background: var(--ch-accent-soft);
    box-shadow: 0 0 0 2px var(--ch-accent);
  }
  .box.flash {
    animation: bloom 0.9s var(--ease-out) both;
  }
  @keyframes bloom {
    0% {
      box-shadow: 0 0 0 1px var(--ch-edge), 0 0 0 0 var(--ch-accent);
    }
    40% {
      box-shadow: 0 0 0 1px var(--ch-edge), 0 0 40px 10px var(--ch-accent);
    }
    100% {
      box-shadow: 0 0 0 1px var(--ch-edge), 0 8px 24px -8px var(--ch-accent);
    }
  }
  .box img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .empty {
    position: absolute;
    inset: 10px;
    border: 1px dashed var(--ch-text-subtle);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    color: var(--ch-text-subtle);
    font-size: 12px;
    opacity: 0.85;
    text-align: center;
    padding: 6px;
  }
  .ic {
    width: 22px;
    height: 22px;
  }
  .center,
  .done {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--ch-accent);
  }
  .spin {
    width: 22px;
    height: 22px;
    border-radius: 11px;
    border: 2px solid var(--ch-hairline);
    border-top-color: var(--ch-accent);
    animation: chSpin 0.8s linear infinite;
  }
  .use {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    padding: 8px;
    display: flex;
    gap: 6px;
    background: linear-gradient(to top, rgb(0 0 0 / 0.7), transparent);
  }
  .grow {
    flex: 1;
    justify-content: center;
  }
  .skip {
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 4px;
    background: rgb(255 255 255 / 0.16);
    color: #fff;
    padding: 7px;
    cursor: pointer;
  }
  .t {
    font-size: 13px;
    font-weight: 500;
  }
  .s {
    font-size: 12px;
  }
</style>
