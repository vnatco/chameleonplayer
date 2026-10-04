<script lang="ts">
  // Seek bar (spec H): hairline track, accent fill, 12 px thumb ringed in
  // `ring`. Driven by the shared progress ticker; times update once a second.
  import { onMount } from "svelte";
  import { formatTime } from "$lib/api";
  import { onProgress } from "$lib/progress";
  import { app } from "$lib/state.svelte";

  let { ring = "var(--ch-scrim)", layout = "inline" }: { ring?: string; layout?: "inline" | "below" } = $props();

  let bar: HTMLDivElement;
  let fill: HTMLDivElement;
  let knob: HTMLDivElement;
  let dragging: number | null = null;
  let label = $state("0:00");
  let nowSec = $state(0);
  const total = $derived(formatTime(app.duration));

  function render(f: number, pos: number) {
    if (dragging != null) return;
    draw(f);
    const l = formatTime(pos);
    if (l !== label) {
      label = l;
      nowSec = Math.floor(pos);
    }
  }
  function draw(f: number) {
    fill.style.transform = `scaleX(${f})`;
    knob.style.translate = `${f * 100}% 0`;
  }

  onMount(() => onProgress(render));

  function frac(e: PointerEvent) {
    const r = bar.getBoundingClientRect();
    return Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
  }
  function down(e: PointerEvent) {
    if (!app.duration || e.button !== 0) return;
    e.stopPropagation();
    bar.setPointerCapture(e.pointerId);
    dragging = frac(e);
    draw(dragging);
    label = formatTime(dragging * app.duration);
  }
  function move(e: PointerEvent) {
    if (dragging == null) return;
    dragging = frac(e);
    draw(dragging);
    label = formatTime(dragging * app.duration);
  }
  function up(e: PointerEvent) {
    if (dragging == null) return;
    const f = frac(e);
    dragging = null;
    app.seek(f * app.duration);
  }
  function key(e: KeyboardEvent) {
    const step = e.shiftKey ? 15 : 5;
    if (e.key === "ArrowRight") app.seek(app.positionNow() + step);
    else if (e.key === "ArrowLeft") app.seek(app.positionNow() - step);
    else if (e.key === "Home") app.seek(0);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<div class="wrap {layout}">
  {#if layout === "inline"}<span class="time">{label}</span>{/if}
  <div
    class="seek"
    bind:this={bar}
    role="slider"
    tabindex="0"
    aria-label="Seek"
    aria-valuemin={0}
    aria-valuemax={Math.round(app.duration)}
    aria-valuenow={nowSec}
    aria-valuetext={label}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={() => (dragging = null)}
    onkeydown={key}
  >
    <div class="track"></div>
    <div class="fill" bind:this={fill}></div>
    <div class="knob" bind:this={knob}><span style:box-shadow="0 0 0 3px {ring}"></span></div>
  </div>
  {#if layout === "inline"}
    <span class="time">{total}</span>
  {:else}
    <div class="times"><span>{label}</span><span>{total}</span></div>
  {/if}
</div>

<style>
  .wrap {
    font-variant-numeric: tabular-nums;
    color: var(--ch-text-subtle);
  }
  .inline {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: inherit;
  }
  .below .times {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    margin-top: -2px;
  }
  .seek {
    position: relative;
    flex: 1;
    height: 16px;
    cursor: pointer;
    touch-action: none;
  }
  .below .seek {
    height: 14px;
  }
  .track,
  .fill {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(50% - 2px);
    height: 4px;
    border-radius: 2px;
  }
  .track {
    background: var(--ch-hairline);
  }
  .fill {
    background: var(--ch-accent);
    transform-origin: left center;
    transform: scaleX(0);
    will-change: transform;
    transition: background-color 0.4s;
  }
  .knob {
    position: absolute;
    inset: 0;
    pointer-events: none;
    will-change: translate;
  }
  .knob span {
    position: absolute;
    top: calc(50% - 6px);
    left: -6px;
    width: 12px;
    height: 12px;
    border-radius: 6px;
    background: var(--ch-accent);
  }
</style>
