<script lang="ts">
  // Mini tile hover (spec B1b): play/pause, grow, 3 px progress.
  import { onMount } from "svelte";
  import Icon from "$lib/ui/Icon.svelte";
  import { onProgress } from "$lib/progress";
  import { app } from "$lib/state.svelte";

  let { show }: { show: boolean } = $props();
  let fill: HTMLDivElement;
  onMount(() => onProgress((f) => (fill.style.transform = `scaleX(${f})`)));
</script>

<div class="ovl" class:show aria-hidden={!show}>
  <div class="scrim"></div>
  <div class="drag" data-drag></div>
  <button class="ib grow" onclick={() => app.setSize("normal")} aria-label="Grow to Normal Size" title="Grow"><Icon name="grow" stroke={1.8} /></button>
  <button class="play" onclick={() => app.toggle()} aria-label={app.playing ? "Pause" : "Play"}>
    <Icon name={app.playing ? "pause" : "play"} />
  </button>
  <div class="bar"><div class="fill" bind:this={fill}></div></div>
</div>

<style>
  .ovl {
    position: absolute;
    inset: 0;
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.24s ease-in;
  }
  .ovl.show {
    opacity: 1;
    pointer-events: auto;
    transition: opacity 0.16s ease-out;
  }
  .scrim {
    position: absolute;
    inset: 0;
    background: var(--ch-scrim);
    opacity: 0.55;
    pointer-events: none;
  }
  .drag {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: 30px;
  }
  .grow {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 26px;
    height: 26px;
    padding: 6px;
  }
  .play {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 44px;
    height: 44px;
    margin: -22px 0 0 -22px;
    border: 0;
    border-radius: 4px;
    background: var(--ch-accent);
    color: var(--ch-on-accent);
    padding: 12px;
    cursor: pointer;
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 3px;
    background: var(--ch-hairline);
  }
  .fill {
    height: 3px;
    background: var(--ch-accent);
    transform-origin: left;
    transform: scaleX(0);
    will-change: transform;
  }
</style>
