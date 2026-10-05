<script lang="ts">
  // Hover controls on the front of the sleeve (spec A2). Opacity-only fade:
  // 160 ms in, 240 ms out. Scrims use the cover's own bg, so the overlay
  // reads as a shadow cast by the art.
  import Icon from "$lib/ui/Icon.svelte";
  import Seek from "$lib/ui/Seek.svelte";
  import Slider from "$lib/ui/Slider.svelte";
  import { app, win } from "$lib/state.svelte";

  let { show, onlibrary }: { show: boolean; onlibrary: () => void } = $props();

  const vol = $derived(app.status?.volume ?? 0);
  const large = $derived(app.ui.size === "large");

  function wheel(e: WheelEvent) {
    e.preventDefault();
    app.volIn();
    app.setVolume(vol - Math.sign(e.deltaY) * 0.05);
  }
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

  <!-- Edit and volume sit in their own side columns; the seek bar and the
       transport sit between them, so the volume capsule can grow straight up
       out of its button without touching any other control. -->
  <div class="foot">
    <div class="side">
      <button class="ib" onclick={() => app.flip("edit")} aria-label="Edit Tags and Artwork" title="Edit Tags"><Icon name="edit" /></button>
    </div>
    <div class="mid">
      <Seek />
      <div class="transport">
        <button class="ib t" onclick={() => app.prev()} aria-label="Previous" title="Previous"><Icon name="prev" stroke={1.8} /></button>
        <button class="play" onclick={() => app.toggle()} aria-label={app.playing ? "Pause" : "Play"} title="Play / Pause (Space)">
          <Icon name={app.playing ? "pause" : "play"} />
        </button>
        <button class="ib t" onclick={() => app.next()} aria-label="Next" title="Next (N)"><Icon name="next" stroke={1.8} /></button>
      </div>
    </div>
    <div class="side">
      <div class="vol" class:open={app.volOpen} role="group" aria-label="Volume" onmouseenter={() => app.volIn()} onmouseleave={() => app.volOut()} onwheel={wheel}>
        <div class="cap">
          <span class="pct">{Math.round(vol * 100)}</span>
          <Slider vertical value={vol} label="Volume" ring="var(--ch-surface-2)" wheel={false} oninput={(v) => app.setVolume(v)} />
        </div>
        <button class="ib vb" onclick={() => app.toggleMute()} onfocus={() => app.volIn()} onblur={() => app.volOut()} aria-label={vol > 0 ? "Mute" : "Unmute"} title="Volume (Scroll to Adjust)">
          <Icon name={vol > 0 ? "volume" : "mute"} />
        </button>
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
    --play: clamp(40px, 13.5cqw, 64px);
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    padding: 0 clamp(12px, 4cqw, 22px) clamp(8px, 2.6cqw, 16px);
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) 32px;
    column-gap: clamp(6px, 2cqw, 12px);
    align-items: end;
    text-shadow: var(--ch-text-shadow);
    font-size: clamp(11px, 3.2cqw, 14px);
  }
  .mid {
    display: flex;
    flex-direction: column;
    gap: clamp(4px, 1.4cqw, 10px);
    min-width: 0;
  }
  /* Side buttons line up with the middle of the transport row. */
  .side {
    padding-bottom: calc((var(--play) - 32px) / 2);
  }
  .transport {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: clamp(4px, 2cqw, 14px);
  }
  .t {
    width: clamp(32px, 10cqw, 44px);
    height: clamp(32px, 10cqw, 44px);
    padding: clamp(7px, 2.6cqw, 11px);
  }
  .play {
    width: var(--play);
    height: var(--play);
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
  /* The speaker button stretches up into a capsule: one shape, anchored on
     the icon. */
  .vol {
    position: relative;
    width: 32px;
    height: 32px;
  }
  .cap {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 32px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 0;
    border-radius: 16px;
    overflow: hidden;
    background: transparent;
    text-shadow: none;
    transition:
      height 0.2s var(--ease-out),
      background-color 0.16s,
      box-shadow 0.16s;
  }
  .cap > :global(*) {
    opacity: 0;
    visibility: hidden;
    transition:
      opacity 0.12s,
      visibility 0s 0.12s;
  }
  .vol.open .cap {
    height: clamp(120px, 42cqw, 170px);
    padding: 12px 0 40px;
    background: var(--ch-surface-2);
    box-shadow:
      0 0 0 1px var(--ch-hairline),
      0 10px 26px rgb(0 0 0 / 0.35);
  }
  .vol.open .cap > :global(*) {
    opacity: 1;
    visibility: visible;
    transition: opacity 0.16s 0.06s;
  }
  .cap :global(.slider.vertical) {
    flex: 1 1 auto;
  }
  .pct {
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
    color: var(--ch-text-subtle);
  }
  .vb {
    position: relative;
    padding: 7px;
    border-radius: 16px;
  }
  .vol.open .vb:hover {
    background: transparent;
  }
</style>
