<script lang="ts" module>
  export type DropState = { kind: "files" | "image"; zone: number } | null;
</script>

<script lang="ts">
  // The record sleeve: glow, art, hover controls, and the flip to its back.
  import { onMount } from "svelte";
  import { mono } from "$lib/api";
  import { onProgress } from "$lib/progress";
  import { app, REACH, win } from "$lib/state.svelte";
  import Glow from "$lib/ui/Glow.svelte";
  import Icon from "$lib/ui/Icon.svelte";
  import Back from "./Back.svelte";
  import MiniOverlay from "./MiniOverlay.svelte";
  import Overlay from "./Overlay.svelte";

  let {
    inLibrary = false,
    drop = null,
    pointerInside = true,
    onlibrary,
  }: { inLibrary?: boolean; drop?: DropState; pointerInside?: boolean; onlibrary: () => void } = $props();

  // ---- Hover & idle (controls hide after 2.6 s without movement) ----------
  let hover = $state(false);
  let idle = $state(false);
  let focusIn = $state(false);
  let idleTimer: ReturnType<typeof setTimeout> | undefined;
  function poke() {
    if (!hover || idle) {
      hover = true;
      idle = false;
    }
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => (idle = true), 2600);
  }
  function leave() {
    clearTimeout(idleTimer);
    hover = false;
    app.volOut();
  }
  // A click-through window gets no pointerleave; the backend tells us.
  $effect(() => {
    if (!pointerInside) {
      leave();
      app.volOut(0);
    }
  });

  const flipped = $derived(app.flipped && !inLibrary);
  const mini = $derived(app.ui.size === "mini" && !inLibrary);
  const showCtl = $derived(!inLibrary && !flipped && !drop && !app.loading && ((hover && !idle) || focusIn || app.volOpen));

  // Switching animations on/off must not replay the flip: drop transitions
  // for a couple of frames while the styles change.
  let instant = $state(false);
  let lastAnim: boolean | null = null;
  $effect(() => {
    const a = app.anim;
    if (lastAnim !== null && a !== lastAnim) {
      instant = true;
      requestAnimationFrame(() => requestAnimationFrame(() => (instant = false)));
    }
    lastAnim = a;
  });

  // Keep the back mounted through the flip-back turn.
  let backMounted = $state(false);
  let backTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    if (flipped) {
      clearTimeout(backTimer);
      backMounted = true;
    } else if (backMounted) {
      backTimer = setTimeout(() => (backMounted = false), app.anim ? 600 : 220);
    }
  });

  // ---- Glow state (spec G, "Glow behaviour by state") ----------------------
  const k = $derived(app.ui.glow);
  const hasTrack = $derived(!!app.status?.track);
  const pausedLook = $derived(hasTrack && !app.playing);
  const glowOpacity = $derived.by(() => {
    if (k < 0.04) return 0;
    let go = 0.35 + 0.65 * k;
    if (app.loading) go *= 0.5;
    else if (pausedLook) go *= 0.55;
    if (inLibrary) go *= 0.7;
    return go;
  });
  const glowReach = $derived.by(() => {
    if (k < 0.04) return 0;
    let gk = 0.45 + 0.55 * k;
    if (inLibrary) gk *= 0.5;
    return gk * (inLibrary ? REACH.normal : REACH[app.ui.size]);
  });
  let dip = $state(0);
  let lastFlip = false;
  $effect(() => {
    if (flipped !== lastFlip) {
      lastFlip = flipped;
      dip++;
    }
  });

  // ---- Paused progress line ----------------------------------------------
  let line: HTMLDivElement | undefined = $state();
  onMount(() => onProgress((f) => line && (line.style.transform = `scaleX(${f})`)));

  // ---- Dragging the window by the art -------------------------------------
  let downAt: { x: number; y: number } | null = null;
  function mousedown(e: MouseEvent) {
    if (e.button !== 0 || inLibrary || flipped) return;
    const t = e.target as HTMLElement;
    if (t.closest("button, input, [role=slider], a")) return;
    downAt = { x: e.screenX, y: e.screenY };
  }
  function mousemove(e: MouseEvent) {
    poke();
    if (downAt && (e.buttons & 1) && Math.hypot(e.screenX - downAt.x, e.screenY - downAt.y) > 3) {
      downAt = null;
      win.startDragging().catch((err) => console.warn("drag failed", err));
    }
  }
  function dblclick(e: MouseEvent) {
    if (inLibrary || flipped) return;
    if ((e.target as HTMLElement).closest("button, input, [role=slider]")) return;
    // Mini -> Normal -> Large -> Mini.
    app.stepSize(1, true);
  }
  // Ctrl + wheel resizes the sleeve.
  function wheel(e: WheelEvent) {
    if (!e.ctrlKey || inLibrary) return;
    e.preventDefault();
    const now = performance.now();
    if (now - lastWheel < 300) return;
    lastWheel = now;
    app.stepSize(e.deltaY < 0 ? 1 : -1);
  }
  let lastWheel = 0;

  const zones = $derived(
    drop?.kind === "image"
      ? [{ label: "Set as Cover", sub: app.now ? `For ${app.now.title}` : "No song is playing" }]
      : mini
        ? [{ label: "Play", sub: "" }]
        : [
            { label: "Play Now", sub: "Replaces the Queue" },
            { label: "Add to Library", sub: "Folders are watched for new files." },
          ],
  );
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="sleeve"
  class:lib={inLibrary}
  onpointerenter={poke}
  onpointerleave={leave}
  onmousedown={mousedown}
  onmousemove={mousemove}
  onmouseup={() => (downAt = null)}
  ondblclick={dblclick}
  onwheel={wheel}
  onfocusin={(e) => {
    if ((e.target as HTMLElement).matches?.(":focus-visible")) focusIn = true;
  }}
  onfocusout={() => (focusIn = false)}
>
  <Glow reach={glowReach} opacity={glowOpacity} breathing={pausedLook && !app.loading} pulse={app.ui.pulse && app.playing} level={() => app.level} {dip} anim={app.anim} />

  <div class="persp">
    <div class="flipper" class:flipped class:still={!app.anim} class:instant>
      <div class="face front" class:hidden={flipped}>
        <div class="bg"></div>
        {#if app.slots.a}<img class="art" class:on={app.slots.front === "a"} src={app.slots.a} alt="" draggable="false" />{/if}
        {#if app.slots.b}<img class="art" class:on={app.slots.front === "b"} src={app.slots.b} alt="" draggable="false" />{/if}
        <div class="nocover" class:on={!app.now?.cover}>
          <div class="initial">{mono(app.now?.title ?? "C")}</div>
          <div class="label">
            <div class="eyebrow-mono">No Artwork</div>
            <div class="nt">{app.now?.title ?? "Chameleon Player"}</div>
            <div class="na">{app.now ? app.now.artist : "Drop music here, or open the library."}</div>
          </div>
        </div>
        <div class="edge"></div>

        {#if pausedLook && !showCtl && !inLibrary && !mini}
          <div class="pline"><div class="pfill" bind:this={line}></div></div>
        {/if}

        {#if app.loading}
          <div class="loading">
            <div class="lt">Decoding…</div>
            <div class="ls ellipsis">{app.pendingTitle}</div>
            <div class="indet"><span></span></div>
          </div>
        {/if}

        {#if !inLibrary}
          {#if mini}
            <MiniOverlay show={showCtl} />
          {:else}
            <Overlay show={showCtl} {onlibrary} />
          {/if}
        {/if}

        {#if drop && !inLibrary}
          <div class="drop">
            {#each zones as z, i (z.label)}
              <div class="zone" class:hot={drop.kind === "image" || drop.zone === i}>
                <span class="zi"><Icon name="drop" /></span>
                <span class="zl">{z.label}</span>
                {#if z.sub}<span class="zs">{z.sub}</span>{/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <div class="face backface" class:hidden={!flipped}>
        {#if backMounted}<Back />{/if}
      </div>
    </div>
  </div>
</div>

<style>
  .sleeve {
    position: absolute;
    inset: 0;
    container-type: size;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.3);
  }
  .persp {
    position: absolute;
    inset: 0;
    perspective: 1400px;
  }
  .flipper {
    position: absolute;
    inset: 0;
    transform-style: preserve-3d;
    transition: transform 0.56s var(--ease-io);
  }
  .flipper.flipped {
    transform: rotateY(180deg);
  }
  .flipper.instant,
  .flipper.instant .face {
    transition: none !important;
  }
  .face {
    position: absolute;
    inset: 0;
    overflow: hidden;
    backface-visibility: hidden;
    visibility: visible;
    transition: visibility 0s linear 0.28s;
  }
  .face.hidden {
    visibility: hidden;
  }
  .backface {
    transform: rotateY(180deg);
  }
  /* Animations off: a 200 ms cross-fade instead of the turn. */
  .flipper.still,
  .flipper.still.flipped {
    transform: none;
    transition: none;
  }
  .flipper.still .face {
    backface-visibility: visible;
    transform: none;
    opacity: 1;
    transition:
      opacity 0.2s,
      visibility 0s linear 0.2s;
  }
  .flipper.still .face.hidden {
    opacity: 0;
    pointer-events: none;
  }
  .bg {
    position: absolute;
    inset: 0;
    background: var(--ch-bg);
  }
  .art {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    opacity: 0;
    transition: opacity 0.42s cubic-bezier(0.4, 0, 0.2, 1);
    will-change: opacity;
  }
  .art.on {
    opacity: 1;
  }
  .nocover {
    position: absolute;
    inset: 0;
    background: radial-gradient(130% 120% at 28% 18%, #2b2b31 0%, #17171a 62%, #111113 100%);
    opacity: 0;
    transition: opacity 0.42s cubic-bezier(0.4, 0, 0.2, 1);
    pointer-events: none;
  }
  .nocover.on {
    opacity: 1;
  }
  .initial {
    position: absolute;
    left: 7%;
    top: 2%;
    font-family: var(--font-display);
    font-weight: 200;
    font-size: 62cqw;
    line-height: 1;
    color: rgb(255 255 255 / 0.07);
    letter-spacing: -0.04em;
  }
  .label {
    position: absolute;
    left: 7%;
    right: 7%;
    bottom: 7%;
    display: flex;
    flex-direction: column;
    gap: clamp(2px, 1cqw, 6px);
  }
  .eyebrow-mono {
    font-family: var(--font-mono);
    font-size: clamp(8px, 2.6cqw, 12px);
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgb(255 255 255 / 0.55);
  }
  .nt {
    font-size: clamp(11px, 5.2cqw, 30px);
    font-weight: 300;
    color: #ececf0;
    line-height: 1.15;
  }
  .na {
    font-size: clamp(9px, 3.4cqw, 17px);
    color: #a0a0ab;
  }
  .edge {
    position: absolute;
    inset: 0;
    box-shadow: inset 0 0 0 1px var(--ch-edge);
    opacity: 0.55;
    pointer-events: none;
  }
  .pline {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 2px;
  }
  .pfill {
    height: 2px;
    background: var(--ch-accent);
    box-shadow: 0 0 6px var(--ch-accent);
    transform-origin: left;
    transform: scaleX(0);
  }
  .loading {
    position: absolute;
    inset: 0;
    background: var(--ch-scrim);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    animation: chFade 0.2s ease-out both;
  }
  .lt {
    font-size: clamp(10px, 3.6cqw, 16px);
  }
  .ls {
    font-size: clamp(9px, 3cqw, 13px);
    color: var(--ch-text-subtle);
    max-width: 80%;
  }
  .indet {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 2px;
    overflow: hidden;
    background: var(--ch-hairline);
  }
  .indet span {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 35%;
    background: var(--ch-accent);
    animation: chIndet 1.1s cubic-bezier(0.4, 0, 0.2, 1) infinite;
  }
  .drop {
    position: absolute;
    inset: 0;
    background: var(--ch-scrim);
    display: flex;
    flex-direction: column;
    padding: clamp(6px, 3cqw, 16px);
    gap: clamp(6px, 2.4cqw, 12px);
    animation: chFade 0.14s ease-out both;
    pointer-events: none;
  }
  .zone {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    border: 1.5px dashed var(--ch-text-subtle);
    border-radius: 2px;
    text-align: center;
    padding: 6px;
    color: var(--ch-text);
    transition:
      background-color 0.12s,
      border-color 0.12s;
  }
  .zone.hot {
    border-color: var(--ch-accent);
    background: var(--ch-accent-soft);
  }
  .zi {
    width: clamp(16px, 6cqw, 28px);
    height: clamp(16px, 6cqw, 28px);
  }
  .zl {
    font-size: clamp(11px, 4cqw, 18px);
    font-weight: 600;
  }
  .zs {
    font-size: clamp(9px, 3cqw, 13px);
    color: var(--ch-text-subtle);
  }
</style>
