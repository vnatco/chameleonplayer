<script lang="ts">
  import { api, type AlbumRow } from "$lib/api";
  import { lib } from "$lib/library.svelte";
  import { app } from "$lib/state.svelte";
  import VirtualGrid from "$lib/ui/VirtualGrid.svelte";
  import Icon from "$lib/ui/Icon.svelte";
  import AlbumTile from "./AlbumTile.svelte";

  let { scroller, artist = null, onclear }: { scroller: HTMLElement; artist?: string | null; onclear: () => void } = $props();

  let byArtist = $state<AlbumRow[] | null>(null);
  $effect(() => {
    void app.libraryVersion;
    if (artist) {
      byArtist = null;
      api.albums(artist).then((a) => { byArtist = a; }, (e) => app.notify(String(e), true));
    } else {
      lib.ensure("albums");
    }
  });
  const items = $derived(artist ? byArtist : lib.albums);

  function open(a: AlbumRow) {
    app.albumKey = a.key;
    app.libView = "album";
    lib.openAlbum(a.key);
  }
</script>

{#if artist}
  <div class="filter">
    <button class="back" onclick={onclear}><Icon name="back" size={14} stroke={2} />All Albums</button>
    <span class="subtle">Albums by</span> <strong>{artist}</strong>
  </div>
{/if}

{#if items == null}
  <div class="skel">{#each Array(8) as _, i (i)}<span></span>{/each}</div>
{:else if items.length === 0}
  <p class="subtle">No Albums Yet.</p>
{:else}
  <VirtualGrid {items} {scroller} minCol={150} gapX={22} gapY={26} rowFor={(w) => w + 44} key={(a) => a.key}>
    {#snippet item(a)}
      <AlbumTile album={a} onopen={() => open(a)} />
    {/snippet}
  </VirtualGrid>
{/if}

<style>
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
  .skel {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 26px 22px;
  }
  .skel span {
    aspect-ratio: 1;
    background: var(--ch-surface-2);
    opacity: 0.5;
  }
</style>
