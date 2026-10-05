<script lang="ts">
  // Back of the sleeve (spec C): Now Playing, Up Next, Edit, Settings.
  import { coverUrl, formatTime, mono } from "$lib/api";
  import Icon from "$lib/ui/Icon.svelte";
  import { app, type BackTab } from "$lib/state.svelte";
  import Editor from "./Editor.svelte";
  import SleeveSettings from "./SleeveSettings.svelte";

  const tabs: [BackTab, string][] = [
    ["info", "Now Playing"],
    ["queue", "Up Next"],
    ["edit", "Edit"],
    ["settings", "Settings"],
  ];

  const now = $derived(app.now);
  const info = $derived(
    now
      ? [
          ["Album", now.album || "-"],
          ["Album Artist", now.albumArtist || "-"],
          ["Genre", now.genre || "-"],
          ["Year", now.year ? String(now.year) : "-"],
          ["Format", now.format || "-"],
          ["Bitrate", now.bitrate ? `${now.bitrate} kbps` : "-"],
          ["Duration", formatTime(app.duration)],
        ]
      : [],
  );

  let list: HTMLDivElement | undefined = $state();
  // Keep the playing row in view when the queue opens.
  $effect(() => {
    if (app.backTab === "queue" && list && app.status?.index != null) {
      const row = list.children[app.status.index] as HTMLElement | undefined;
      row?.scrollIntoView({ block: "nearest" });
    }
  });
</script>

<div class="back">
  <div class="head" data-drag>
    <div class="tabs" role="tablist">
      {#each tabs as [k, label] (k)}
        <button role="tab" aria-selected={app.backTab === k} class:sel={app.backTab === k} onclick={() => (app.backTab = k)}>
          {label}<span class="bar"></span>
        </button>
      {/each}
    </div>
    <button class="ib solid flipb" onclick={() => app.flip()} aria-label="Flip Back to Cover" title="Flip Back (F)"><Icon name="flip" /></button>
  </div>

  {#if app.backTab === "edit"}
    <Editor />
  {:else}
    <div class="body">
      {#if app.backTab === "info"}
        {#if now}
          <div class="info">
            <div>
              <div class="title">{now.title}</div>
              <div class="artist subtle">{now.artist}</div>
            </div>
            <div class="grid">
              {#each info as [k, v] (k)}
                <span class="subtle">{k}</span><span class="ellipsis">{v}</span>
              {/each}
            </div>
            <div class="path mono subtle">{now.path}</div>
            <div>
              <div class="eyebrow">Palette From This Cover</div>
              <div class="strip">
                <span title="Background" style="background:var(--ch-bg)"></span>
                <span title="Surface" style="background:var(--ch-surface-2)"></span>
                <span title="Accent" style="background:var(--ch-accent);flex:2"></span>
                <span title="Text" style="background:var(--ch-text)"></span>
                <span title="Glow Top" style="background:var(--ch-glow-t)"></span>
                <span title="Glow Right" style="background:var(--ch-glow-r)"></span>
                <span title="Glow Bottom" style="background:var(--ch-glow-b)"></span>
                <span title="Glow Left" style="background:var(--ch-glow-l)"></span>
              </div>
            </div>
          </div>
        {:else}
          <p class="subtle empty">Nothing is playing. Drop music on the sleeve, or open the library.</p>
        {/if}
      {:else if app.backTab === "queue"}
        {#if app.queue.length}
          <div class="queue" bind:this={list}>
            {#each app.queue as q, i (i)}
              {@const active = i === app.status?.index}
              <button class="row" class:active onclick={() => app.cmd({ type: "playIndex", index: i })}>
                <span
                  class="thumb"
                  style:box-shadow={q.cover ? `0 0 0 1px ${q.cover.edge}e6, 0 2px 10px -2px ${q.cover.glow}b3` : "0 0 0 1px var(--ch-hairline)"}
                >
                  {#if q.cover}<img src={coverUrl(q.cover.thumb)} alt="" loading="lazy" decoding="async" />{:else}{mono(q.title)}{/if}
                </span>
                <span class="who">
                  <span class="t ellipsis">{q.title}</span>
                  <span class="a ellipsis">{q.artist}</span>
                </span>
                <span class="d">{formatTime(q.duration)}</span>
              </button>
            {/each}
          </div>
        {:else}
          <p class="subtle empty">The queue is empty.</p>
        {/if}
      {:else}
        <SleeveSettings />
      {/if}
    </div>
  {/if}
</div>

<style>
  .back {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--ch-bg);
    color: var(--ch-text);
    box-shadow: inset 0 0 0 1px var(--ch-edge);
    container-type: size;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 8px 8px 0 12px;
    flex: none;
  }
  .tabs {
    display: flex;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }
  .tabs button {
    position: relative;
    height: 30px;
    padding: 0 9px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    cursor: pointer;
    font-size: 12.5px;
    color: var(--ch-text-subtle);
    white-space: nowrap;
  }
  .tabs button:hover {
    background: var(--ch-hover);
  }
  .tabs button.sel {
    color: var(--ch-text);
    font-weight: 600;
  }
  .bar {
    position: absolute;
    left: 50%;
    bottom: 2px;
    width: 0;
    height: 3px;
    border-radius: 2px;
    background: var(--ch-accent);
    transition:
      width 0.2s var(--ease-out),
      margin-left 0.2s var(--ease-out);
  }
  .sel .bar {
    width: 16px;
    margin-left: -8px;
  }
  .flipb {
    padding: 7px;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    scrollbar-color: transparent transparent;
    transition: scrollbar-color 0.2s;
    padding: 12px 16px 14px;
  }
  .body:hover {
    scrollbar-color: var(--ch-hairline) transparent;
  }
  .empty {
    font-size: 13px;
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .title {
    font-family: var(--font-display);
    font-size: clamp(17px, 5.4cqw, 28px);
    font-weight: 600;
    line-height: 1.15;
    text-wrap: balance;
  }
  .artist {
    font-size: 13px;
    margin-top: 4px;
  }
  .grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 14px;
    font-size: 12.5px;
  }
  .path {
    font-size: 10.5px;
    word-break: break-all;
    line-height: 1.5;
    user-select: text;
  }
  .strip {
    display: flex;
    height: 22px;
    margin-top: 6px;
    border: 1px solid var(--ch-hairline);
  }
  .strip span {
    flex: 1;
  }
  .queue {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 8px;
    margin: 0 -8px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    text-align: left;
    cursor: pointer;
    color: var(--ch-text);
  }
  .row:hover {
    background: var(--ch-hover);
  }
  .row.active {
    background: var(--ch-accent-soft);
  }
  .row.active .t {
    color: var(--ch-accent);
  }
  .thumb {
    width: 34px;
    height: 34px;
    flex: none;
    background: var(--ch-surface-2);
    overflow: hidden;
    display: grid;
    place-items: center;
    font-size: 12px;
    color: var(--ch-text-subtle);
  }
  .thumb img {
    width: 34px;
    height: 34px;
    object-fit: cover;
    display: block;
  }
  .who {
    min-width: 0;
    flex: 1;
  }
  .t {
    display: block;
    font-size: 13px;
  }
  .a {
    display: block;
    font-size: 11.5px;
    color: var(--ch-text-subtle);
  }
  .d {
    font-size: 11.5px;
    color: var(--ch-text-subtle);
    font-variant-numeric: tabular-nums;
  }
</style>
