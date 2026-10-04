<script lang="ts">
  // The ambient glow behind the sleeve (spec A1 / G).
  //
  // Geometry follows the design's eight box-shadows: per side a "near" light
  // (offset 26, blur 46, spread -10) and a "far" light (50 / 100 / -24), all
  // scaled by the reach R. Each light is a cover-sized slab shifted outward
  // and filled with a gradient through that side's three sampled segments,
  // so the glow can change color along an edge. Each set of four is blurred
  // once as a group.
  //
  // Performance: the blurred layers only re-render when colors or reach
  // change. Pulse, breathing and the flip dip touch only opacity and
  // transform on wrapper layers, which the compositor handles on the GPU.
  import { onDestroy } from "svelte";

  let {
    reach,
    opacity,
    breathing = false,
    pulse = false,
    level = () => 0,
    dip = 0,
    anim = true,
  }: {
    /** Glow reach R (intensity x size scale). */
    reach: number;
    /** Resting opacity for the current state. */
    opacity: number;
    breathing?: boolean;
    pulse?: boolean;
    /** Latest bass level, 0..1. */
    level?: () => number;
    /** Increment to play the flip dip. */
    dip?: number;
    anim?: boolean;
  } = $props();

  let pulseEl: HTMLDivElement | undefined = $state();
  let dipEl: HTMLDivElement | undefined = $state();

  // Music pulse: instant attack, exponential decay (tau 110 ms); opacity
  // x0.9-1.1 and reach +6% at most.
  let raf = 0;
  let env = 0;
  let last = 0;
  function frame(now: number) {
    const dt = last ? (now - last) / 1000 : 0;
    last = now;
    env = Math.max(Math.min(1, level()), env * Math.exp(-dt / 0.11));
    if (pulseEl) {
      pulseEl.style.opacity = String(0.9 + 0.2 * env);
      pulseEl.style.transform = `scale(${1 + 0.06 * env})`;
    }
    raf = requestAnimationFrame(frame);
  }
  $effect(() => {
    if (pulse && anim) {
      last = 0;
      raf = requestAnimationFrame(frame);
      return () => {
        cancelAnimationFrame(raf);
        if (pulseEl) {
          pulseEl.style.opacity = "";
          pulseEl.style.transform = "";
        }
      };
    }
  });

  // Flip: the glow dips to 60% over the 560 ms turn.
  let lastDip: number | null = null;
  $effect(() => {
    if (lastDip === null) {
      lastDip = dip;
      return;
    }
    if (dip !== lastDip) {
      lastDip = dip;
      if (anim && dipEl) {
        dipEl.animate([{ opacity: 1 }, { opacity: 0.6, offset: 0.5 }, { opacity: 1 }], {
          duration: 560,
          easing: "cubic-bezier(.65,0,.35,1)",
        });
      }
    }
  });

  onDestroy(() => cancelAnimationFrame(raf));

  // Breathing is +-12% around the paused level; the outer layer carries the
  // peak and the inner one breathes down from it.
  const outer = $derived(Math.min(1, breathing && anim ? opacity * 1.12 : opacity));
</script>

{#if reach > 0 && opacity > 0}
  <div class="glow" style:opacity={outer} style:--R={reach}>
    <div class="breathe" class:on={breathing && anim}>
      <div class="dip" bind:this={dipEl}>
        <div class="pulse" bind:this={pulseEl}>
          <div class="set far">
            <div class="slab l"></div>
            <div class="slab b"></div>
            <div class="slab r"></div>
            <div class="slab t"></div>
          </div>
          <div class="set near">
            <div class="slab l"></div>
            <div class="slab b"></div>
            <div class="slab r"></div>
            <div class="slab t"></div>
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .glow {
    position: absolute;
    inset: 0;
    pointer-events: none;
    transition: opacity 0.4s;
    contain: layout style;
  }
  .breathe,
  .dip,
  .pulse {
    position: absolute;
    inset: 0;
    will-change: opacity, transform;
  }
  .breathe.on {
    animation: breathe 4s ease-in-out infinite;
  }
  @keyframes breathe {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.786;
    }
  }
  .set {
    position: absolute;
    inset: 0;
    transition: filter 0.46s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  /* Box-shadow blur b is a Gaussian with sigma b/2. */
  .near {
    --o: calc(var(--R) * 26px);
    --sp: calc(var(--R) * 10px);
    filter: blur(calc(var(--R) * 23px));
  }
  .far {
    --o: calc(var(--R) * 50px);
    --sp: calc(var(--R) * 24px);
    filter: blur(calc(var(--R) * 50px));
  }
  .slab {
    position: absolute;
    inset: var(--sp);
    transition:
      translate 0.46s cubic-bezier(0.2, 0.8, 0.2, 1),
      inset 0.46s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .t {
    translate: 0 calc(var(--o) * -1);
    background: linear-gradient(to right, var(--ch-glow-t-0), var(--ch-glow-t-1), var(--ch-glow-t-2));
  }
  .b {
    translate: 0 var(--o);
    background: linear-gradient(to right, var(--ch-glow-b-0), var(--ch-glow-b-1), var(--ch-glow-b-2));
  }
  .l {
    translate: calc(var(--o) * -1) 0;
    background: linear-gradient(to bottom, var(--ch-glow-l-0), var(--ch-glow-l-1), var(--ch-glow-l-2));
  }
  .r {
    translate: var(--o) 0;
    background: linear-gradient(to bottom, var(--ch-glow-r-0), var(--ch-glow-r-1), var(--ch-glow-r-2));
  }
  @media (prefers-reduced-motion: reduce) {
    .breathe.on {
      animation: none;
    }
  }
</style>
