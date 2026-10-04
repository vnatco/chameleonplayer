<script lang="ts" module>
  // Icon geometry from the design (24x24 viewBox).
  const ICONS = {
    play: '<path d="M7.5 5v14L19 12z" fill="currentColor" stroke="none"/>',
    pause: '<rect x="6.5" y="5" width="3.6" height="14" rx=".6" fill="currentColor" stroke="none"/><rect x="13.9" y="5" width="3.6" height="14" rx=".6" fill="currentColor" stroke="none"/>',
    prev: '<path d="M6 6v12"/><path d="M18 6l-9 6 9 6z" fill="currentColor"/>',
    next: '<path d="M18 6v12"/><path d="M6 6l9 6-9 6z" fill="currentColor"/>',
    library: '<rect x="4" y="4" width="6.5" height="6.5"/><rect x="13.5" y="4" width="6.5" height="6.5"/><rect x="4" y="13.5" width="6.5" height="6.5"/><rect x="13.5" y="13.5" width="6.5" height="6.5"/>',
    flip: '<path d="M4.5 12a7.5 7.5 0 0113.2-4.9"/><path d="M19.5 12a7.5 7.5 0 01-13.2 4.9"/><path d="M18 3.5v4h-4"/><path d="M6 20.5v-4h4"/>',
    minimize: '<path d="M5 12h14"/>',
    maximize: '<rect x="4" y="4" width="16" height="16" rx="2"/>',
    close: '<path d="M6 6l12 12M18 6L6 18"/>',
    edit: '<path d="M4 20h4L19 9l-4-4L4 16z"/>',
    volume: '<path d="M4 9.5h3.5L12 6v12l-4.5-3.5H4z" fill="currentColor"/><path d="M15.5 9a4 4 0 010 6"/><path d="M18 6.5a7.5 7.5 0 010 11"/>',
    mute: '<path d="M4 9.5h3.5L12 6v12l-4.5-3.5H4z" fill="currentColor"/><path d="M16 9.5l5 5M21 9.5l-5 5"/>',
    grow: '<path d="M14 4h6v6"/><path d="M10 20H4v-6"/><path d="M20 4l-6 6"/><path d="M4 20l6-6"/>',
    collapse: '<path d="M4 14h6v6"/><path d="M20 10h-6V4"/><path d="M14 10l6-6"/><path d="M10 14l-6 6"/>',
    search: '<circle cx="11" cy="11" r="6.5"/><path d="M20 20l-4.2-4.2"/>',
    image: '<rect x="4" y="4" width="16" height="16" rx="1"/><circle cx="9" cy="9.5" r="1.6"/><path d="M4 17l5-5 4 4 2.5-2.5L20 18"/>',
    check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
    back: '<path d="M15 5l-7 7 7 7"/>',
    drop: '<path d="M12 4v11"/><path d="M7 10l5 5 5-5"/><path d="M5 19h14"/>',
    repeat: '<path d="M17 3l3 3-3 3"/><path d="M4 11V9a3 3 0 013-3h13"/><path d="M7 21l-3-3 3-3"/><path d="M20 13v2a3 3 0 01-3 3H4"/>',
    shuffle: '<path d="M16 4h4v4"/><path d="M4 19L20 4"/><path d="M20 16v4h-4"/><path d="M14 14l6 6"/><path d="M4 5l5 5"/>',
  } as const;
  export type IconName = keyof typeof ICONS;
</script>

<script lang="ts">
  let { name, size = "100%", stroke = 1.6 }: { name: IconName; size?: string | number; stroke?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true">{@html ICONS[name]}</svg>

<style>
  svg {
    display: block;
    flex: none;
  }
</style>
