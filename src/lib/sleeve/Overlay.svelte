<script lang="ts">
  // Hover controls on the front of the sleeve (spec A2). Opacity-only fade:
  // 160 ms in, 240 ms out. Scrims use the cover's own bg, so the overlay
  // reads as a shadow cast by the art.
  import Icon from "$lib/ui/Icon.svelte";
  import Seek from "$lib/ui/Seek.svelte";
  import Slider from "$lib/ui/Slider.svelte";
  import { app, win } from "$lib/state.svelte";

  let { show, onlibrary }: { show: boolean; onlibrary: () => void } = $props();

  let volOpen = $state(false);
  let volTimer: ReturnType<typeof setTimeout> | undefined;
  const vol = $derived(app.status?.volume ?? 0);
  const large = $derived(app.ui.size === "large");

  function volIn() {
    clearTimeout(volTimer);
    volOpen = true;
  }
  function volOut() {
    clearTimeout(volTimer);
    volTimer = setTimeout(() => (volOpen = false), 220);
  }
  function wheel(e: WheelEvent) {
    e.preventDefault();
    volIn();
    app.setVolume(vol - Math.sign(e.deltaY) * 0.05);
  }
  $effect(() => {
    if (!show) volOpen = false;
  });
</script>

<div class="ovl" class:show aria-hidden={!show}>
  <div class="scrim top"></div>
  <div class="scrim bottom"></div>

  <div class="bar" data-drag>
    <div class="who">
      <div class="title ellipsis">{app.now?.title ?? "Nothing Playing"}</div>
      <div class="artist ellipsis">{app.now?.artist ?? ""}</div>
      {#if large && app.now?.album}
        <div class="album ellipsis">{app.now.album}{app.now.year ? ` · ${app.now.year}` : ""}</div>
      {/if}
    </div>
    <div class="caps">
      <button class="ib" onclick={onlibrary} aria-label="Open Library" title="Library (L)"><Icon name="library" /></button>
      <button class="ib flip" onclick={() => app.flip()} aria-label="Flip Cover" title="Flip (F)"><Icon name="flip" /></button>
      <button class="ib" onclick={() => win.minimize()} aria-label="Minimize" title="Minimize"><Icon name="minimize" /></button>
      <button class="ib close" onclick={() => win.close()} aria-label="Close" title="Close"><Icon name="close" /></button>
    </div>
  </div>

  <div class="foot">
    <Seek />
    <div class="row">
      <div class="left">
        <button class="ib" onclick={() => app.flip("edit")} aria-label="Edit Tags and Artwork" title="Edit Tags"><Icon name="edit" /></button>
      </div>
      <div class="transport">
        <button class="ib t" onclick={() => app.prev()} aria-label="Previous" title="Previous"><Icon name="prev" stroke={1.8} /></button>
        <button class="play" onclick={() => app.toggle()} aria-label={app.playing ? "Pause" : "Play"} title="Play / Pause (Space)">
          <Icon name={app.playing ? "pause" : "play"} />
        </button>
        <button class="ib t" onclick={() => app.next()} aria-label="Next" title="Next (N)"><Icon name="next" stroke={1.8} /></button>
      </div>
      <div class="right">
        <!-- Only the pill itself reacts: the icon, and the slider once it's out. -->
        <div class="vol" class:open={volOpen} role="group" aria-label="Volume" onmouseenter={volIn} onmouseleave={volOut} onwheel={wheel} onfocusin={volIn} onfocusout={volOut}>
          <div class="cap">
            <span class="pct">{Math.round(vol * 100)}</span>
            <Slider vertical value={vol} label="Volume" ring="var(--ch-surface-2)" oninput={(v) => app.setVolume(v)} />
          </div>
          <button class="ib vb" onclick={() => app.toggleMute()} aria-label={vol > 0 ? "Mute" : "Unmute"} title="Volume (Scroll to Adjust)">
            <Icon name={vol > 0 ? "volume" : "mute"} />
          </button>
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .ovl {
    position: absolute;
    inset: 0;
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.24s ease-in;
    color: var(--ch-text);
  }
  .ovl.show {
    opacity: 1;
    pointer-events: auto;
    transition: opacity 0.16s ease-out;
  }
  .scrim {
    position: absolute;
    left: 0;
    right: 0;
    pointer-events: none;
  }
  .top {
    top: 0;
    height: 46%;
    background: linear-gradient(to bottom, var(--ch-scrim) 0%, var(--ch-scrim) 32%, var(--ch-scrim-0) 100%);
  }
  .bottom {
    bottom: 0;
    height: 60%;
    background: linear-gradient(to top, var(--ch-scrim) 0%, var(--ch-scrim) 44%, var(--ch-scrim-0) 100%);
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: clamp(8px, 3cqw, 18px) clamp(8px, 2.4cqw, 14px) 0 clamp(12px, 4cqw, 22px);
    text-shadow: var(--ch-text-shadow);
  }
  .who {
    flex: 1;
    min-width: 0;
    padding-top: 4px;
  }
  .title {
    font-size: clamp(14px, 4.4cqw, 24px);
    font-weight: 600;
    line-height: 1.2;
  }
  .artist {
    font-size: clamp(12px, 3.7cqw, 17px);
    color: var(--ch-text-subtle);
    margin-top: 2px;
  }
  .album {
    font-size: 13px;
    color: var(--ch-text-subtle);
    margin-top: 2px;
  }
  .caps {
    display: flex;
    gap: 2px;
    flex: none;
  }
  .flip {
    padding: 7px;
  }
  .foot {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    padding: 0 clamp(12px, 4cqw, 22px) clamp(8px, 2.6cqw, 16px);
    display: flex;
    flex-direction: column;
    gap: clamp(4px, 1.4cqw, 10px);
    text-shadow: var(--ch-text-shadow);
    font-size: clamp(11px, 3.2cqw, 14px);
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
  }
  .left {
    display: flex;
  }
  .transport {
    display: flex;
    align-items: center;
    gap: clamp(4px, 2cqw, 14px);
  }
  .t {
    width: clamp(32px, 10cqw, 44px);
    height: clamp(32px, 10cqw, 44px);
    padding: clamp(7px, 2.6cqw, 11px);
  }
  .play {
    width: clamp(40px, 13.5cqw, 64px);
    height: clamp(40px, 13.5cqw, 64px);
    border: 0;
    border-radius: 4px;
    background: var(--ch-accent);
    color: var(--ch-on-accent);
    padding: clamp(11px, 3.8cqw, 19px);
    cursor: pointer;
    box-shadow: 0 4px 18px -4px var(--ch-accent);
    transition: filter 0.12s;
  }
  .play:hover {
    filter: brightness(1.08);
  }
  .right {
    position: relative;
    display: flex;
    justify-content: flex-end;
    align-items: center;
  }
  /* A capsule that rises out of the speaker button: button and slider are
     one shape, so the pointer never crosses a gap. */
  .vol {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 32px;
    border-radius: 16px;
    transition: background-color 0.16s;
  }
  .vol.open {
    background: var(--ch-surface-2);
    box-shadow: 0 0 0 1px var(--ch-hairline), 0 10px 24px rgb(0 0 0 / 0.3);
    text-shadow: none;
  }
  .cap {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 100%;
    height: clamp(96px, 34cqw, 132px);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 12px 0 8px;
    border-radius: 16px 16px 0 0;
    background: var(--ch-surface-2);
    box-shadow: 0 -1px 0 0 var(--ch-hairline), 1px 0 0 0 var(--ch-hairline), -1px 0 0 0 var(--ch-hairline);
    opacity: 0;
    transform: scaleY(0.6);
    transform-origin: center bottom;
    pointer-events: none;
    transition:
      opacity 0.14s ease-out,
      transform 0.18s var(--ease-out);
  }
  .vol.open .cap {
    opacity: 1;
    transform: none;
    pointer-events: auto;
  }
  .vol.open {
    border-top-left-radius: 0;
    border-top-right-radius: 0;
  }
  .cap :global(.slider.vertical) {
    flex: 1;
  }
  .pct {
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
    color: var(--ch-text-subtle);
  }
  .vb {
    padding: 7px;
    border-radius: 16px;
  }
  .vol.open .vb:hover {
    background: transparent;
  }
</style>
