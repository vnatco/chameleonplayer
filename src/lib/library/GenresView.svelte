<script lang="ts">
  // Genres (spec D4c): each tile takes the palette of its first cover.
  import { coverUrl, type Palette } from "$lib/api";
  import { lib } from "$lib/library.svelte";
  import { staticVars } from "$lib/palette";
  import { app } from "$lib/state.svelte";

  let { onopen }: { onopen: (name: string) => void } = $props();
  let pals = $state<Record<string, Palette | null>>({});

  $effect(() => {
    lib.ensure("genres");
  });
  $effect(() => {
    for (const g of lib.genres ?? []) {
      const c = g.covers[0];
      if (c && !(c.hash in pals)) {
        pals[c.hash] = null;
        lib.palette(c.hash).then((p) => { pals[c.hash] = p; });
      }
    }
  });
</script>

{#if lib.genres == null}
  <p class="subtle">Loading…</p>
{:else if lib.genres.length === 0}
  <p class="subtle">No Genres Yet.</p>
{:else}
  <div class="grid">
    {#each lib.genres as g (g.name)}
      {@const p = g.covers[0] ? pals[g.covers[0].hash] : null}
      <button class="tile" style={p ? staticVars(p, app.ui.strength) : ""} onclick={() => onopen(g.name)}>
        <span class="inner">
          <span class="mosaic">
            {#each [0, 1, 2, 3] as i (i)}
              {@const c = g.covers.length ? g.covers[i % g.covers.length] : null}
              <span>{#if c}<img src={coverUrl(c.thumb)} alt="" loading="lazy" decoding="async" />{/if}</span>
            {/each}
          </span>
          <span>
            <span class="n">{g.name}</span>
            <span class="s subtle">{g.trackCount} {g.trackCount === 1 ? "Song" : "Songs"}</span>
          </span>
        </span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 14px;
  }
  .tile {
    padding: 0;
    border: 0;
    background: transparent;
    cursor: pointer;
    display: block;
    width: 100%;
  }
  .inner {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 96px;
    padding: 0 14px;
    background: var(--ch-bg);
    border: 1px solid var(--ch-hairline);
    border-radius: 4px;
    color: var(--ch-text);
    text-align: left;
    width: 100%;
    transition: background-color 0.4s;
  }
  .tile:hover .inner {
    border-color: var(--ch-accent-line);
  }
  .mosaic {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2px;
    width: 64px;
    height: 64px;
    flex: none;
  }
  .mosaic > span {
    background: var(--ch-surface-2);
    overflow: hidden;
  }
  .mosaic img {
    width: 31px;
    height: 31px;
    object-fit: cover;
    display: block;
  }
  .n {
    display: block;
    font-size: 15px;
    font-weight: 600;
  }
  .s {
    display: block;
    font-size: 12px;
  }
</style>
