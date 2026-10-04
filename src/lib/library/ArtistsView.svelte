<script lang="ts">
  // Artists (spec D4b): cards with up to three stacked album covers.
  import { coverUrl, mono } from "$lib/api";
  import { lib } from "$lib/library.svelte";

  let { onopen }: { onopen: (name: string) => void } = $props();
  $effect(() => {
    lib.ensure("artists");
  });
</script>

{#if lib.artists == null}
  <p class="subtle">Loading…</p>
{:else if lib.artists.length === 0}
  <p class="subtle">No Artists Yet.</p>
{:else}
  <div class="grid">
    {#each lib.artists as r (r.name)}
      <button class="card" onclick={() => onopen(r.name)}>
        <span class="stack">
          {#if r.covers.length}
            {#each r.covers.slice(0, 3) as c, i (c.hash)}
              <span class="sq" style:left="{i * 10}px" style:z-index={3 - i}><img src={coverUrl(c.thumb)} alt="" loading="lazy" decoding="async" /></span>
            {/each}
          {:else}
            <span class="sq">{mono(r.name)}</span>
          {/if}
        </span>
        <span class="who">
          <span class="n ellipsis">{r.name}</span>
          <span class="s subtle">{r.albumCount} {r.albumCount === 1 ? "Album" : "Albums"} · {r.trackCount} {r.trackCount === 1 ? "Song" : "Songs"}</span>
        </span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 12px;
  }
  .card {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px;
    border: 1px solid var(--ch-hairline);
    border-radius: 4px;
    background: var(--ch-surface);
    text-align: left;
    cursor: pointer;
    color: var(--ch-text);
    content-visibility: auto;
    contain-intrinsic-size: auto 82px;
  }
  .card:hover {
    background: var(--ch-surface-2);
  }
  .stack {
    position: relative;
    width: 76px;
    height: 56px;
    flex: none;
  }
  .sq {
    position: absolute;
    top: 0;
    left: 0;
    width: 56px;
    height: 56px;
    background: var(--ch-surface-2);
    overflow: hidden;
    box-shadow:
      0 0 0 1px var(--ch-bg),
      2px 0 8px rgb(0 0 0 / 0.25);
    display: grid;
    place-items: center;
    font-size: 18px;
    color: var(--ch-text-subtle);
  }
  .sq img {
    width: 56px;
    height: 56px;
    object-fit: cover;
    display: block;
  }
  .who {
    min-width: 0;
  }
  .n {
    display: block;
    font-size: 14px;
    font-weight: 500;
  }
  .s {
    display: block;
    font-size: 12px;
  }
</style>
