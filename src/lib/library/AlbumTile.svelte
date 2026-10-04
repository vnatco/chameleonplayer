<script lang="ts">
  // Album tile (spec D2): crisp square; hover lifts 3 px, lights the tile's
  // own four-side glow and shows a five-swatch palette strip.
  import { coverUrl, mono, type AlbumRow, type Palette } from "$lib/api";
  import { lib } from "$lib/library.svelte";
  import { rgbCss } from "$lib/palette";

  let { album, onopen }: { album: AlbumRow; onopen: () => void } = $props();

  let pal = $state<Palette | null>(null);
  let hot = $state(false);

  function enter() {
    hot = true;
    if (album.cover && !pal) lib.palette(album.cover.hash).then((p) => { pal = p; });
  }

  const shadow = $derived.by(() => {
    if (!hot || !pal) return "0 0 0 1px var(--ch-hairline)";
    const g = pal.glow;
    return `0 0 0 1px ${rgbCss(pal.roles.edge)}, 0 -10px 26px -10px ${rgbCss(g.t, 0.9)}, 10px 0 26px -10px ${rgbCss(g.r, 0.9)}, 0 14px 32px -10px ${rgbCss(g.b, 0.95)}, -10px 0 26px -10px ${rgbCss(g.l, 0.9)}`;
  });
  const swatches = $derived(pal ? [pal.roles.bg, pal.roles.surface2, pal.roles.accent, pal.roles.text, pal.roles.edge].map((c) => rgbCss(c)) : []);
  const flash = $derived(lib.flash === album.key);
</script>

<button class="tile" onclick={onopen} onmouseenter={enter} onmouseleave={() => (hot = false)} onfocus={enter} onblur={() => (hot = false)} title="{album.title} - {album.artist}">
  <span class="art" class:hot class:flash style:box-shadow={flash ? undefined : shadow} style:--flash-edge={album.cover?.edge} style:--flash-glow={album.cover?.glow}>
    {#if album.cover}
      <img src={coverUrl(album.cover.thumb)} alt="" loading="lazy" decoding="async" />
    {:else}
      <span class="mono-big">{mono(album.title)}</span>
    {/if}
    {#if hot && swatches.length}
      <span class="sw">{#each swatches as c, i (i)}<span style:background={c}></span>{/each}</span>
    {/if}
  </span>
  <span class="meta">
    <span class="t ellipsis">{album.title}</span>
    <span class="a ellipsis">{album.artist}</span>
  </span>
</button>

<style>
  .tile {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
    height: 100%;
    padding: 0;
    border: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
    color: var(--ch-text);
  }
  .art {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 1;
    background: var(--ch-surface-2);
    transition:
      box-shadow 0.3s var(--ease-out),
      transform 0.3s var(--ease-out);
  }
  .art.hot {
    transform: translateY(-3px);
  }
  .art img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .mono-big {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-family: var(--font-display);
    font-weight: 300;
    font-size: 56px;
    color: var(--ch-text-subtle);
    opacity: 0.5;
  }
  .sw {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 8px;
    display: flex;
    animation: chFadeIn 0.2s ease-out both;
  }
  .sw span {
    flex: 1;
  }
  /* A cover was just assigned: bloom in its new palette (900 ms). */
  .art.flash {
    animation: bloom 0.9s var(--ease-out) both;
  }
  @keyframes bloom {
    0% {
      box-shadow:
        0 0 0 1px var(--flash-edge),
        0 0 0 0 var(--flash-glow);
    }
    40% {
      box-shadow:
        0 0 0 1px var(--flash-edge),
        0 0 40px 10px var(--flash-glow);
    }
    100% {
      box-shadow:
        0 0 0 1px var(--flash-edge),
        0 8px 24px -8px var(--flash-glow);
    }
  }
  .meta {
    min-width: 0;
  }
  .t {
    display: block;
    font-size: 13px;
    font-weight: 500;
  }
  .a {
    display: block;
    font-size: 12px;
    color: var(--ch-text-subtle);
  }
</style>
