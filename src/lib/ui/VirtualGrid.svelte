<script lang="ts" generics="T">
  // Windowed grid/list: only rows near the viewport are in the DOM, so a
  // library of tens of thousands of items scrolls like a short one.
  // `minCol` = minimum column width (1 column when 0); rows have fixed
  // height (`rowHeight`, or derived from the column width via `rowFor`).
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";

  let {
    items,
    minCol = 0,
    gapX = 0,
    gapY = 0,
    rowHeight = 44,
    rowFor,
    overscan = 3,
    key,
    item,
    scroller,
  }: {
    items: T[];
    minCol?: number;
    gapX?: number;
    gapY?: number;
    rowHeight?: number;
    /** Row height from the computed column width (for square tiles). */
    rowFor?: (colWidth: number) => number;
    overscan?: number;
    key: (t: T) => string | number;
    item: Snippet<[T, number]>;
    /** The scrolling ancestor. */
    scroller: HTMLElement;
  } = $props();

  let host: HTMLDivElement;
  let width = $state(0);
  let top = $state(0);
  let viewH = $state(800);

  const cols = $derived(minCol > 0 ? Math.max(1, Math.floor((width + gapX) / (minCol + gapX))) : 1);
  const colW = $derived(cols > 0 ? (width - gapX * (cols - 1)) / cols : width);
  const rowH = $derived(rowFor ? rowFor(colW) : rowHeight);
  const rows = $derived(Math.ceil(items.length / cols));
  const total = $derived(rows > 0 ? rows * rowH + (rows - 1) * gapY : 0);
  const first = $derived(Math.max(0, Math.floor(top / (rowH + gapY)) - overscan));
  const last = $derived(Math.min(rows, Math.ceil((top + viewH) / (rowH + gapY)) + overscan));
  const visible = $derived(items.slice(first * cols, last * cols));

  function measure() {
    if (!host || !scroller) return;
    const sr = scroller.getBoundingClientRect();
    const hr = host.getBoundingClientRect();
    top = Math.max(0, sr.top - hr.top);
    viewH = sr.height;
  }

  onMount(() => {
    const ro = new ResizeObserver(() => {
      width = host.clientWidth;
      measure();
    });
    ro.observe(host);
    ro.observe(scroller);
    const onScroll = () => measure();
    scroller.addEventListener("scroll", onScroll, { passive: true });
    width = host.clientWidth;
    measure();
    return () => {
      ro.disconnect();
      scroller.removeEventListener("scroll", onScroll);
    };
  });
</script>

<div class="vg" bind:this={host} style:height="{total}px">
  {#each visible as t, i (key(t))}
    {@const idx = first * cols + i}
    {@const r = Math.floor(idx / cols)}
    {@const c = idx % cols}
    <div
      class="cell"
      style:width="{colW}px"
      style:height="{rowH}px"
      style:transform="translate({c * (colW + gapX)}px, {r * (rowH + gapY)}px)"
    >
      {@render item(t, idx)}
    </div>
  {/each}
</div>

<style>
  .vg {
    position: relative;
    width: 100%;
    /* No paint containment: tiles lift and glow past their cells. */
    contain: layout style;
  }
  .cell {
    position: absolute;
    left: 0;
    top: 0;
  }
</style>
