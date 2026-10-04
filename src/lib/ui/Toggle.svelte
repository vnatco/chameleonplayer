<script lang="ts">
  let { on, label, onchange }: { on: boolean; label: string; onchange: (v: boolean) => void } = $props();
</script>

<button role="switch" aria-checked={on} aria-label={label} class:on onclick={() => onchange(!on)}>
  <span></span>
</button>

<style>
  button {
    position: relative;
    width: 40px;
    height: 20px;
    flex: none;
    border-radius: 10px;
    padding: 0;
    cursor: pointer;
    border: 1px solid var(--ch-text-subtle);
    background: transparent;
    transition:
      background-color 0.16s,
      border-color 0.16s;
  }
  button.on {
    border-color: var(--ch-accent);
    background: var(--ch-accent);
  }
  span {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 12px;
    height: 12px;
    border-radius: 6px;
    background: var(--ch-text-subtle);
    transition: transform 0.16s var(--ease-out);
  }
  .on span {
    background: var(--ch-on-accent);
    transform: translateX(20px);
  }
</style>
