<script lang="ts">
  // "Make Chameleon your default player?" Shown once per launch when the
  // installed app isn't the default for MP3, until the user says never.
  import { app } from "$lib/state.svelte";
  import Icon from "./Icon.svelte";

  function notNow() {
    app.defaultPromptDone = true;
  }
  function never() {
    app.defaultPromptDone = true;
    app.setUi("defaultPrompt", "never");
  }
</script>

<div class="card" role="dialog" aria-label="Default Player">
  <div class="head">
    <span class="mark"><Icon name="play" size={14} /></span>
    <div>
      <div class="t">Make Chameleon Your Default Player?</div>
      <div class="s">Open MP3, FLAC and other music files with Chameleon.</div>
    </div>
  </div>
  <div class="row">
    <button class="btn primary sm" onclick={() => app.setAsDefault()}>Set as Default</button>
    <button class="btn ghost sm" onclick={notNow}>Not Now</button>
    <button class="link" onclick={never}>Don't Ask Again</button>
  </div>
</div>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px;
    border-radius: 8px;
    background: var(--ch-surface-2);
    color: var(--ch-text);
    box-shadow:
      0 0 0 1px var(--ch-hairline),
      0 14px 40px rgb(0 0 0 / 0.45);
    animation: chFadeIn 0.24s var(--ease-out) both;
  }
  .head {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }
  .mark {
    width: 28px;
    height: 28px;
    flex: none;
    border-radius: 4px;
    background: var(--ch-accent);
    color: var(--ch-on-accent);
    display: grid;
    place-items: center;
  }
  .t {
    font-size: 13px;
    font-weight: 600;
  }
  .s {
    font-size: 12px;
    color: var(--ch-text-subtle);
    margin-top: 2px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .link {
    margin-left: auto;
    border: 0;
    background: transparent;
    color: var(--ch-text-subtle);
    font-size: 11.5px;
    cursor: pointer;
    padding: 4px;
  }
  .link:hover {
    color: var(--ch-text);
  }
</style>
