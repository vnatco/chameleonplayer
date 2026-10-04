<script lang="ts" generics="T extends string">
  let {
    options,
    value,
    label,
    onchange,
  }: { options: [T, string][]; value: T; label: string; onchange: (v: T) => void } = $props();
</script>

<div role="radiogroup" aria-label={label}>
  {#each options as [v, text] (v)}
    <button role="radio" aria-checked={value === v} class:sel={value === v} onclick={() => onchange(v)}>{text}</button>
  {/each}
</div>

<style>
  div {
    display: inline-flex;
    padding: 2px;
    border-radius: 5px;
    background: var(--ch-surface);
    border: 1px solid var(--ch-hairline);
  }
  button {
    flex: 1;
    height: 26px;
    padding: 0 12px;
    border: 0;
    border-radius: 4px;
    font-size: 12.5px;
    cursor: pointer;
    background: transparent;
    color: var(--ch-text);
    white-space: nowrap;
    transition:
      background-color 0.16s,
      color 0.16s;
  }
  button.sel {
    background: var(--ch-accent);
    color: var(--ch-on-accent);
    font-weight: 600;
  }
</style>
