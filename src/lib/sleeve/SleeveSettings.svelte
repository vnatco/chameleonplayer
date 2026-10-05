<script lang="ts">
  // Back of the sleeve, Settings tab (spec C4).
  import Segmented from "$lib/ui/Segmented.svelte";
  import Slider from "$lib/ui/Slider.svelte";
  import Toggle from "$lib/ui/Toggle.svelte";
  import DefaultPlayerRow from "$lib/ui/DefaultPlayerRow.svelte";
  import { app, type Size } from "$lib/state.svelte";

  const glowLabel = $derived(app.ui.glow < 0.04 ? "Crisp" : app.ui.glow > 0.9 ? "Full Glow" : `${Math.round(app.ui.glow * 100)}%`);
  const strengthLabel = $derived(
    app.ui.strength > 0.9 ? "Full Chameleon" : app.ui.strength < 0.1 ? "Subtle" : `${Math.round(app.ui.strength * 100)}%`,
  );
  const toggles = [
    ["onTop", "Always on Top", "Keep the sleeve above other windows."],
    ["remember", "Remember Position & Size", "Reopen where you left it."],
    ["pulse", "Music Pulse", "The glow breathes with the bass."],
    ["anim", "Animations", "Flip, morph and fades. Also off when Windows reduces motion."],
  ] as const;
</script>

<div class="settings">
  <div>
    <div class="head"><span>Glow</span><span class="subtle">{glowLabel}</span></div>
    <Slider value={app.ui.glow} label="Glow Intensity" valueText={glowLabel} oninput={(v) => app.setUi("glow", v)} />
    <div class="ends subtle"><span>Crisp</span><span>Full Glow</span></div>
  </div>
  <div>
    <div class="head"><span>Palette Strength</span><span class="subtle">{strengthLabel}</span></div>
    <Slider value={app.ui.strength} label="Palette Strength" valueText={strengthLabel} oninput={(v) => app.setUi("strength", v)} />
    <div class="ends subtle"><span>Subtle</span><span>Full Chameleon</span></div>
  </div>
  <div>
    <div class="label">Window Size</div>
    <div class="seg">
      <Segmented
        label="Window Size"
        options={[
          ["mini", "Mini"],
          ["normal", "Normal"],
          ["large", "Large"],
        ] as [Size, string][]}
        value={app.ui.size}
        onchange={(v) => app.setSize(v)}
      />
    </div>
  </div>
  <div class="toggles">
    {#each toggles as [key, label, sub] (key)}
      <div class="trow">
        <span class="tl">{label}<span class="sub subtle">{sub}</span></span>
        <Toggle on={!!app.ui[key]} {label} onchange={(v) => app.setUi(key, v)} />
      </div>
    {/each}
    <DefaultPlayerRow />
  </div>
</div>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    font-size: 12.5px;
    margin-bottom: 4px;
  }
  .settings :global(.slider) {
    width: 100%;
  }
  .ends {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
  }
  .label {
    font-size: 12.5px;
    margin-bottom: 6px;
  }
  .seg :global(div[role="radiogroup"]) {
    display: flex;
  }
  .toggles {
    display: flex;
    flex-direction: column;
  }
  .trow {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 0;
    border-top: 1px solid var(--ch-hairline);
  }
  .tl {
    flex: 1;
    font-size: 12.5px;
  }
  .sub {
    display: block;
    font-size: 11px;
    margin-top: 1px;
  }
</style>
