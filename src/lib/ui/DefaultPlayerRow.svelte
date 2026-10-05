<script lang="ts">
  // Default-player status and action, for the Settings screens.
  import { onMount } from "svelte";
  import { app } from "$lib/state.svelte";
  import Icon from "./Icon.svelte";

  onMount(() => {
    app.refreshDefaults();
  });
  const d = $derived(app.defaults);
  const partial = $derived(!!d && d.isDefault && d.others.length > 0);
</script>

<div class="row">
  <span class="text">
    <span class="t">Default Player</span>
    <span class="s">
      {#if !d}
        Checking…
      {:else if !d.registered}
        Install Chameleon with its installer to make it your default player.
      {:else if d.isDefault && !partial}
        Chameleon opens your music files.
      {:else if d.isDefault}
        Chameleon opens MP3. Other formats ({d.others.join(", ")}) open elsewhere.
      {:else}
        Another app opens your music files.
      {/if}
    </span>
  </span>
  {#if d?.isDefault && !partial}
    <span class="ok"><Icon name="check" size={14} stroke={2.4} />Default</span>
  {:else}
    <button class="btn sm {d?.isDefault ? 'ghost' : 'primary'}" disabled={!d?.registered} onclick={() => app.setAsDefault()}>
      {d?.isDefault ? "Change Formats…" : "Set as Default…"}
    </button>
  {/if}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 0;
    border-top: 1px solid var(--ch-hairline);
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .t {
    display: block;
    font-size: 12.5px;
  }
  .s {
    display: block;
    font-size: 11px;
    color: var(--ch-text-subtle);
    margin-top: 1px;
  }
  .ok {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--ch-accent);
    white-space: nowrap;
  }
</style>
