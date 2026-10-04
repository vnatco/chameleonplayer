<script lang="ts">
  // Album page (spec D3): the album's own palette, inside the playing
  // cover's frame.
  import { coverUrl, formatTime, mono } from "$lib/api";
  import { lib } from "$lib/library.svelte";
  import { staticVars } from "$lib/palette";
  import { app, trackOf } from "$lib/state.svelte";
  import { api } from "$lib/api";
  import Icon from "$lib/ui/Icon.svelte";

  let { onedit }: { onedit: () => void } = $props();

  const a = $derived(lib.album);
  const vars = $derived(a?.palette ? staticVars(a.palette, app.ui.strength) : "");
  const genre = $derived(a?.tracks.find((t) => t.genre)?.genre ?? "");
  const total = $derived(a ? a.tracks.reduce((s, t) => s + t.duration, 0) : 0);

  function shuffle() {
    if (!a) return;
    const list = [...a.tracks];
    for (let i = list.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [list[i], list[j]] = [list[j], list[i]];
    }
    app.guard(api.load(list.map(trackOf), 0, true));
  }
</script>

<div class="page" style={vars}>
  <button class="back" onclick={() => ((app.libView = "albums"), (lib.album = null))}>
    <Icon name="back" size={14} stroke={2} />Albums
  </button>
  {#if a}
    <div class="head">
      <div class="cover">
        {#if a.row.cover}
          <img src={coverUrl(a.row.cover.full)} alt="" decoding="async" />
        {:else}
          <span class="mono-big">{mono(a.row.title)}</span>
        {/if}
      </div>
      <div class="meta">
        <div class="eyebrow">Album{genre ? ` · ${genre}` : ""}</div>
        <div class="title">{a.row.title}</div>
        <div class="sub">
          {a.row.artist}{a.row.year ? ` · ${a.row.year}` : ""} · {a.tracks.length}
          {a.tracks.length === 1 ? "Song" : "Songs"}, {formatTime(total)}
        </div>
        <div class="actions">
          <button class="btn primary" onclick={() => app.playList(a.tracks, 0)}><Icon name="play" size={14} />Play</button>
          <button class="btn secondary" onclick={shuffle}>Shuffle</button>
          <button class="ib ghostb" onclick={onedit} aria-label="Edit Album Tags" title="Edit Tags"><Icon name="edit" /></button>
        </div>
      </div>
    </div>
    <div class="tracks">
      {#each a.tracks as t, i (t.id)}
        {@const on = t.path === app.status?.track?.path}
        <button class="tr" class:on onclick={() => app.playList(a.tracks, i)}>
          <span class="n">{on ? "▶" : (t.trackNo ?? i + 1)}</span>
          <span class="tt ellipsis">{t.title}</span>
          <span class="d">{formatTime(t.duration)}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .page {
    margin: -20px -24px 0;
    padding: 20px 24px 28px;
    background: var(--ch-bg);
    color: var(--ch-text);
    min-height: 100%;
    transition: background-color 0.4s;
  }
  .back {
    height: 28px;
    padding: 0 10px 0 6px;
    margin-left: -6px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--ch-text-subtle);
    font-size: 12.5px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .back:hover {
    background: var(--ch-hover);
    color: var(--ch-text);
  }
  .head {
    display: flex;
    gap: 28px;
    align-items: flex-end;
    margin-top: 14px;
  }
  .cover {
    position: relative;
    width: 208px;
    height: 208px;
    flex: none;
    background: var(--ch-surface-2);
    box-shadow:
      0 0 0 1px var(--ch-edge),
      0 -14px 40px -14px var(--ch-glow-t),
      14px 0 40px -14px var(--ch-glow-r),
      0 18px 46px -14px var(--ch-glow-b),
      -14px 0 40px -14px var(--ch-glow-l);
  }
  .cover img {
    width: 208px;
    height: 208px;
    object-fit: cover;
    display: block;
  }
  .mono-big {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-size: 80px;
    font-weight: 300;
    color: var(--ch-text-subtle);
    opacity: 0.5;
  }
  .meta {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-bottom: 4px;
  }
  .eyebrow {
    letter-spacing: 0.08em;
  }
  .title {
    font-family: var(--font-display);
    font-size: 36px;
    font-weight: 600;
    line-height: 1.1;
    text-wrap: balance;
  }
  .sub {
    font-size: 14px;
    color: var(--ch-text-subtle);
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
  .actions .btn {
    height: 34px;
  }
  .actions .btn.primary {
    padding: 0 18px 0 14px;
  }
  .ghostb {
    width: 34px;
    height: 34px;
    border: 1px solid var(--ch-hairline);
  }
  .tracks {
    margin-top: 28px;
    display: flex;
    flex-direction: column;
  }
  .tr {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 40px;
    padding: 0 10px;
    border: 0;
    border-bottom: 1px solid var(--ch-hairline);
    background: transparent;
    color: var(--ch-text);
    font-size: 13.5px;
    text-align: left;
    cursor: pointer;
  }
  .tr:hover {
    background: var(--ch-hover);
  }
  .tr.on {
    background: var(--ch-accent-soft);
    color: var(--ch-accent);
    font-weight: 600;
  }
  .n {
    width: 28px;
    color: var(--ch-text-subtle);
    font-variant-numeric: tabular-nums;
  }
  .on .n {
    color: inherit;
  }
  .tt {
    flex: 1;
    min-width: 0;
  }
  .d {
    color: var(--ch-text-subtle);
    font-variant-numeric: tabular-nums;
  }
</style>
