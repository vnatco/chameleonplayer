<script lang="ts">
  // Songs (spec D4a): a dense, windowed table.
  import { api, coverUrl, formatTime, mono, type TrackRow } from "$lib/api";
  import { lib } from "$lib/library.svelte";
  import { app } from "$lib/state.svelte";
  import Icon from "$lib/ui/Icon.svelte";
  import VirtualGrid from "$lib/ui/VirtualGrid.svelte";

  let { scroller, genre = null, onclear }: { scroller: HTMLElement; genre?: string | null; onclear: () => void } = $props();

  let byGenre = $state<TrackRow[] | null>(null);
  $effect(() => {
    void app.libraryVersion;
    if (genre) {
      byGenre = null;
      api.tracks({ genre }).then((t) => { byGenre = t; }, (e) => app.notify(String(e), true));
    } else {
      lib.ensure("songs");
    }
  });
  const items = $derived(genre ? byGenre : lib.songs);
  const playing = $derived(app.status?.track?.path);
</script>

{#if genre}
  <div class="filter">
    <button class="back" onclick={onclear}><Icon name="back" size={14} stroke={2} />All Songs</button>
    <span class="subtle">Genre</span> <strong>{genre}</strong>
  </div>
{/if}

<div class="hdr eyebrow"><span></span><span>Title</span><span>Artist</span><span>Album</span><span>Genre</span><span class="r">Time</span></div>
{#if items == null}
  <p class="subtle pad">Loading…</p>
{:else if items.length === 0}
  <p class="subtle pad">No Songs Yet.</p>
{:else}
  <VirtualGrid {items} {scroller} rowHeight={44} key={(t) => t.id}>
    {#snippet item(t)}
      {@const on = t.path === playing}
      <button class="row" class:on onclick={() => app.playOne(t)}>
        <span class="thumb" style:box-shadow={t.cover ? `0 0 0 1px ${t.cover.edge}e6, 0 2px 10px -2px ${t.cover.glow}b3` : "0 0 0 1px var(--ch-hairline)"}>
          {#if t.cover}<img src={coverUrl(t.cover.thumb)} alt="" loading="lazy" decoding="async" />{:else}{mono(t.album || t.title)}{/if}
        </span>
        <span class="ellipsis title">{t.title}</span>
        <span class="ellipsis subtle">{t.artist}</span>
        <span class="ellipsis subtle">{t.album}</span>
        <span class="ellipsis subtle">{t.genre}</span>
        <span class="r subtle num">{formatTime(t.duration)}</span>
      </button>
    {/snippet}
  </VirtualGrid>
{/if}

<style>
  .hdr,
  .row {
    display: grid;
    grid-template-columns: 40px minmax(0, 2fr) minmax(0, 1.3fr) minmax(0, 1.3fr) minmax(0, 0.9fr) 52px;
    gap: 0 14px;
    align-items: center;
  }
  .hdr {
    padding: 0 8px 8px;
    border-bottom: 1px solid var(--ch-hairline);
    margin-bottom: 4px;
  }
  .r {
    text-align: right;
  }
  .row {
    width: 100%;
    height: 44px;
    padding: 0 8px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    color: var(--ch-text);
  }
  .row:hover {
    background: var(--ch-hover);
  }
  .row.on {
    background: var(--ch-accent-soft);
  }
  .row.on .title {
    color: var(--ch-accent);
  }
  .thumb {
    width: 32px;
    height: 32px;
    background: var(--ch-surface-2);
    overflow: hidden;
    display: grid;
    place-items: center;
    font-size: 11px;
    color: var(--ch-text-subtle);
  }
  .thumb img {
    width: 32px;
    height: 32px;
    object-fit: cover;
    display: block;
  }
  .num {
    font-variant-numeric: tabular-nums;
  }
  .pad {
    padding: 8px;
  }
  .filter {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    margin: -6px 0 16px;
  }
  .back {
    height: 28px;
    padding: 0 10px 0 6px;
    margin-left: -6px;
    margin-right: 8px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--ch-text-subtle);
    font-size: 12.5px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .back:hover {
    background: var(--ch-hover);
    color: var(--ch-text);
  }
</style>
