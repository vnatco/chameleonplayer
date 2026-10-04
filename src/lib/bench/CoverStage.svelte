<script lang="ts">
  // Cover with the per-side ambient glow. Each edge of the artwork casts its
  // own color outward; `glow` 0 = crisp border only, 1 = full glow.
  import { coverUrl, type Cover } from "$lib/api";

  let {
    cover,
    size = 320,
    glow = 0.8,
    level = 0,
  }: { cover: Cover | null; size?: number; glow?: number; level?: number } = $props();

  // The pulse only ever adds light, and only a little.
  const strength = $derived(glow * (1 + level * 0.35));
</script>

<div class="stage" style:--size="{size}px" style:--glow={strength}>
  <!-- Blur is applied to the whole layer, after each side is masked. -->
  <div class="glow-layer">
    <div class="glow top"></div>
    <div class="glow right"></div>
    <div class="glow bottom"></div>
    <div class="glow left"></div>
  </div>
  <div class="cover">
    {#if cover}
      <img src={coverUrl(cover.full)} alt="" draggable="false" />
    {:else}
      <div class="no-cover">No Cover</div>
    {/if}
  </div>
</div>

<style>
  .stage {
    position: relative;
    width: var(--size);
    height: var(--size);
    flex: none;
  }
  .cover {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: 3px;
    /* Thin, crisp edge in the cover's own colors ("Crisp" mode). */
    box-shadow:
      0 0 0 1px oklch(var(--c-edge-top-lch) / 0.9),
      0 1px 2px rgb(0 0 0 / 0.4);
    background: var(--c-surface);
  }
  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .no-cover {
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--c-text-subtle);
    background: radial-gradient(circle at 30% 25%, var(--c-surface-raised), var(--c-background));
    letter-spacing: 0.08em;
  }
  .glow-layer {
    position: absolute;
    inset: 0;
    filter: blur(calc(var(--size) * 0.09));
    opacity: calc(var(--glow) * 0.9);
    pointer-events: none;
    transition: opacity 120ms linear;
  }
  .glow {
    position: absolute;
  }
  /* Each side is a gradient through its three sampled segments, and fades
     toward the cover's center so neighbouring sides don't swamp each other. */
  .top {
    left: 4%;
    right: 4%;
    top: -9%;
    height: 34%;
    background: linear-gradient(to right, var(--c-glow-top-0), var(--c-glow-top-1), var(--c-glow-top-2));
    mask-image: linear-gradient(to bottom, black 40%, transparent);
  }
  .bottom {
    left: 4%;
    right: 4%;
    bottom: -9%;
    height: 34%;
    background: linear-gradient(to right, var(--c-glow-bottom-0), var(--c-glow-bottom-1), var(--c-glow-bottom-2));
    mask-image: linear-gradient(to top, black 40%, transparent);
  }
  .left {
    top: 4%;
    bottom: 4%;
    left: -9%;
    width: 34%;
    background: linear-gradient(to bottom, var(--c-glow-left-0), var(--c-glow-left-1), var(--c-glow-left-2));
    mask-image: linear-gradient(to right, black 40%, transparent);
  }
  .right {
    top: 4%;
    bottom: 4%;
    right: -9%;
    width: 34%;
    background: linear-gradient(to bottom, var(--c-glow-right-0), var(--c-glow-right-1), var(--c-glow-right-2));
    mask-image: linear-gradient(to left, black 40%, transparent);
  }
</style>
