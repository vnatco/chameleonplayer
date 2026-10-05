<script lang="ts">
  // The one slider used everywhere (volume, glow, palette strength): the same
  // language as the seek bar. Hairline track, accent fill, a ringed thumb
  // that grows a little under the pointer. Drag, click, wheel and arrow keys.
  let {
    value,
    label,
    oninput,
    onchange,
    ring = "var(--ch-bg)",
    step = 0.05,
    wheel = true,
    valueText,
    vertical = false,
  }: {
    value: number;
    label: string;
    /** Live, while dragging. */
    oninput: (v: number) => void;
    /** Once, when the gesture ends. */
    onchange?: (v: number) => void;
    ring?: string;
    step?: number;
    wheel?: boolean;
    valueText?: string;
    /** Bottom = 0, top = 1. */
    vertical?: boolean;
  } = $props();

  let el: HTMLDivElement;
  let dragging = $state(false);
  const v = $derived(Math.min(1, Math.max(0, value)));

  function at(e: PointerEvent) {
    const r = el.getBoundingClientRect();
    const f = vertical ? (r.bottom - e.clientY) / r.height : (e.clientX - r.left) / r.width;
    return Math.min(1, Math.max(0, f));
  }
  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    e.stopPropagation();
    el.setPointerCapture(e.pointerId);
    dragging = true;
    oninput(at(e));
  }
  function move(e: PointerEvent) {
    if (dragging) oninput(at(e));
  }
  function up(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    const x = at(e);
    oninput(x);
    onchange?.(x);
  }
  function nudge(d: number) {
    const x = Math.min(1, Math.max(0, Math.round((v + d) / step) * step));
    oninput(x);
    onchange?.(x);
  }
  function key(e: KeyboardEvent) {
    if (e.key === "ArrowRight" || e.key === "ArrowUp") nudge(step);
    else if (e.key === "ArrowLeft" || e.key === "ArrowDown") nudge(-step);
    else if (e.key === "Home") nudge(-1);
    else if (e.key === "End") nudge(1);
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
  function onwheel(e: WheelEvent) {
    if (!wheel) return;
    e.preventDefault();
    e.stopPropagation();
    nudge(e.deltaY < 0 ? step : -step);
  }
</script>

<div
  class="slider"
  class:vertical
  class:dragging
  bind:this={el}
  role="slider"
  tabindex="0"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={Math.round(v * 100)}
  aria-valuetext={valueText ?? `${Math.round(v * 100)}%`}
  aria-orientation={vertical ? "vertical" : "horizontal"}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={() => (dragging = false)}
  onkeydown={key}
  {onwheel}
>
  <div class="track"></div>
  <div class="fill" style:transform={vertical ? `scaleY(${v})` : `scaleX(${v})`}></div>
  <div class="knob" style:translate={vertical ? `0 ${-v * 100}%` : `${v * 100}% 0`}><span style:box-shadow="0 0 0 3px {ring}"></span></div>
</div>

<style>
  .slider {
    position: relative;
    height: 16px;
    flex: 1;
    min-width: 0;
    cursor: pointer;
    touch-action: none;
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
  }
  .knob {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .knob span {
    position: absolute;
    top: calc(50% - 6px);
    left: -6px;
    width: 12px;
    height: 12px;
    border-radius: 6px;
    background: var(--ch-accent);
    transition: transform 0.12s var(--ease-out);
  }
  .vertical {
    width: 16px;
    height: auto;
    flex: none;
  }
  .vertical .track,
  .vertical .fill {
    left: calc(50% - 2px);
    right: auto;
    top: 0;
    bottom: 0;
    width: 4px;
    height: auto;
  }
  .vertical .fill {
    transform-origin: center bottom;
  }
  .vertical .knob span {
    top: auto;
    bottom: -6px;
    left: calc(50% - 6px);
  }
  .slider:hover .knob span,
  .slider:focus-visible .knob span,
  .dragging .knob span {
    transform: scale(1.17);
  }
</style>
