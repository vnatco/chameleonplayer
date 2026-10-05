<script lang="ts" module>
  export type MenuItem =
    | { kind: "item"; label: string; hint?: string; checked?: boolean; disabled?: boolean; action: () => void }
    | { kind: "sep" }
    | { kind: "label"; label: string };
</script>

<script lang="ts">
  // The app's right-click menu, in the cover's palette.
  import { onMount, tick } from "svelte";
  import type { Rect } from "$lib/api";
  import Icon from "./Icon.svelte";

  let {
    x,
    y,
    items,
    bounds,
    onclose,
    onrect,
  }: {
    x: number;
    y: number;
    items: MenuItem[];
    /** Area the menu must stay inside (the window). */
    bounds: { w: number; h: number };
    onclose: () => void;
    /** Reports the menu's rect so it can be made clickable. */
    onrect: (r: Rect) => void;
  } = $props();

  let el: HTMLDivElement;
  let pos = $state({ x: 0, y: 0 });
  let shown = $state(false);

  onMount(() => {
    (async () => {
      await tick();
      const w = el.offsetWidth;
      const h = el.offsetHeight;
      const px = x + w > bounds.w - 4 ? Math.max(4, x - w) : x;
      const py = y + h > bounds.h - 4 ? Math.max(4, bounds.h - h - 4) : y;
      pos = { x: px, y: py };
      onrect({ x: px, y: py, w, h });
      shown = true;
      (el.querySelector("button:not(:disabled)") as HTMLElement | null)?.focus({ preventScroll: true });
    })();
    const away = (e: PointerEvent) => {
      if (!el.contains(e.target as Node)) onclose();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onclose();
      } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        const btns = [...el.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")];
        const i = btns.indexOf(document.activeElement as HTMLButtonElement);
        btns[(i + (e.key === "ArrowDown" ? 1 : -1) + btns.length) % btns.length]?.focus();
      }
    };
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", key, true);
    window.addEventListener("blur", onclose);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", key, true);
      window.removeEventListener("blur", onclose);
    };
  });

  function run(a: () => void) {
    onclose();
    a();
  }
</script>

<div class="menu" class:shown bind:this={el} role="menu" style:left="{pos.x}px" style:top="{pos.y}px">
  {#each items as it, i (i)}
    {#if it.kind === "sep"}
      <div class="sep" role="separator"></div>
    {:else if it.kind === "label"}
      <div class="lbl">{it.label}</div>
    {:else}
      <button role={it.checked === undefined ? "menuitem" : "menuitemcheckbox"} aria-checked={it.checked} disabled={it.disabled} onclick={() => run(it.action)}>
        <span class="chk">{#if it.checked}<Icon name="check" size={14} stroke={2.2} />{/if}</span>
        <span class="t">{it.label}</span>
        {#if it.hint}<span class="hint">{it.hint}</span>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: absolute;
    z-index: 50;
    min-width: 210px;
    padding: 5px;
    border-radius: 8px;
    background: var(--ch-surface-2);
    color: var(--ch-text);
    box-shadow:
      0 0 0 1px var(--ch-hairline),
      0 14px 40px rgb(0 0 0 / 0.45);
    font-size: 12.5px;
    opacity: 0;
    transform: translateY(-4px);
  }
  .menu.shown {
    opacity: 1;
    transform: none;
    transition:
      opacity 0.12s ease-out,
      transform 0.14s var(--ease-out);
  }
  button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 30px;
    padding: 0 10px 0 6px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  button:hover:not(:disabled),
  button:focus-visible {
    background: var(--ch-hover);
    outline: none;
  }
  button:disabled {
    color: var(--ch-text-subtle);
    cursor: default;
  }
  .chk {
    width: 16px;
    display: grid;
    place-items: center;
    color: var(--ch-accent);
  }
  .t {
    flex: 1;
    white-space: nowrap;
  }
  .hint {
    color: var(--ch-text-subtle);
    font-size: 11.5px;
  }
  .sep {
    height: 1px;
    margin: 5px 6px;
    background: var(--ch-hairline);
  }
  .lbl {
    padding: 6px 10px 3px 30px;
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--ch-text-subtle);
  }
</style>
