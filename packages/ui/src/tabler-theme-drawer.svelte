<script lang="ts">
  import type { ThemeSettings } from "./types.js";
  import { DEFAULT_THEME_SETTINGS } from "./defaults.js";
  import { dialogFocus } from "./dialog-focus.js";
  import {
    IconSun,
    IconMoon,
    IconMoonStars,
    IconX,
    IconDeviceFloppy,
    IconRotate2,
  } from "@tabler/icons-svelte";

  interface Props {
    open: boolean;
    themeSettings: ThemeSettings;
    onClose: () => void;
    onUpdateTheme: (updates: Partial<ThemeSettings>) => void;
  }

  let { open = false, themeSettings, onClose, onUpdateTheme }: Props = $props();
  let drawer: HTMLDialogElement | undefined;

  $effect(() => {
    if (open && !drawer?.open) drawer?.showModal();
    if (!open && drawer?.open) drawer.close();
  });

  const SCHEMES = [
    { id: "#066fd1", name: "Tabler Blue", hex: "#066fd1" },
    { id: "#d63939", name: "Red", hex: "#d63939" },
    { id: "#2fb344", name: "Green", hex: "#2fb344" },
    { id: "#f76707", name: "Orange", hex: "#f76707" },
    { id: "#ae3ec9", name: "Purple", hex: "#ae3ec9" },
    { id: "#0ca678", name: "Teal", hex: "#0ca678" },
    { id: "#17a2b8", name: "Cyan", hex: "#17a2b8" },
    { id: "#8B2E2A", name: "Fasti Oxblood", hex: "#8B2E2A" },
    { id: "#D4AF37", name: "Horological Gold", hex: "#D4AF37" },
  ];

  const BASES = [
    { id: "slate", name: "Slate" },
    { id: "gray", name: "Gray" },
    { id: "zinc", name: "Zinc" },
    { id: "neutral", name: "Neutral" },
    { id: "stone", name: "Stone" },
  ];

  const RADII = [
    { id: 0, label: "0" },
    { id: 0.5, label: ".5" },
    { id: 1, label: "1" },
    { id: 1.5, label: "1.5" },
    { id: 2, label: "2" },
  ];

  function handleReset(): void {
    onUpdateTheme(DEFAULT_THEME_SETTINGS);
  }
</script>

<dialog
  bind:this={drawer}
  use:dialogFocus
  class="theme-drawer-panel"
  aria-modal="true"
  aria-labelledby="theme-drawer-title"
  oncancel={onClose}
  onclick={(event) => {
    if (event.target === event.currentTarget) onClose();
  }}
>
  <header class="drawer-header">
    <h2 id="theme-drawer-title" class="drawer-title">Theme settings</h2>
    <button
      type="button"
      class="close-btn"
      onclick={onClose}
      aria-label="Close theme settings"
    >
      <IconX size={20} />
    </button>
  </header>

  <div class="drawer-body">
    <!-- 1. Color Mode -->
    <section class="drawer-section">
      <span class="section-label">Color mode</span>
      <div class="mode-grid">
        <button
          type="button"
          class="mode-btn"
          class:active={themeSettings.mode === "light"}
          onclick={() => onUpdateTheme({ mode: "light" })}
        >
          <IconSun size={18} />
          <span>Light</span>
        </button>

        <button
          type="button"
          class="mode-btn"
          class:active={themeSettings.mode === "dark"}
          onclick={() => onUpdateTheme({ mode: "dark" })}
        >
          <IconMoon size={18} />
          <span>Dark</span>
        </button>

        <button
          type="button"
          class="mode-btn"
          class:active={themeSettings.mode === "night"}
          onclick={() => onUpdateTheme({ mode: "night" })}
        >
          <IconMoonStars size={18} />
          <span>Night</span>
        </button>
      </div>
    </section>

    <!-- 2. Color Scheme Palette -->
    <section class="drawer-section">
      <span class="section-label">Scheme</span>
      <div class="swatches-grid">
        {#each SCHEMES as s}
          <button
            type="button"
            class="swatch-btn"
            class:selected={themeSettings.accentColor === s.hex}
            style="background-color: {s.hex};"
            title={s.name}
            onclick={() => onUpdateTheme({ accentColor: s.hex })}
            aria-label={s.name}
          ></button>
        {/each}
      </div>
    </section>

    <!-- 3. Font Family -->
    <section class="drawer-section">
      <label class="section-label" for="theme-font-select">Font family</label>
      <div class="select-wrapper">
        <select
          id="theme-font-select"
          class="theme-select"
          value={themeSettings.fontFamily ?? "sans-serif"}
          onchange={(e) =>
            onUpdateTheme({ fontFamily: e.currentTarget.value as any })}
        >
          <option value="sans-serif">Atkinson Hyperlegible (Sans)</option>
          <option value="serif">Newsreader (Editorial Serif)</option>
          <option value="monospace">IBM Plex Mono (Terminal Mono)</option>
        </select>
      </div>
    </section>

    <!-- 4. Theme Base (Gray Shade) -->
    <section class="drawer-section">
      <span class="section-label">Theme base</span>
      <div class="segmented-row">
        {#each BASES as b}
          <button
            type="button"
            class="segment-btn"
            class:active={(themeSettings.themeBase ?? "slate") === b.id}
            onclick={() => onUpdateTheme({ themeBase: b.id as any })}
          >
            {b.name}
          </button>
        {/each}
      </div>
    </section>

    <!-- 5. Corner Radius -->
    <section class="drawer-section">
      <span class="section-label">Corner radius</span>
      <div class="segmented-row">
        {#each RADII as r}
          <button
            type="button"
            class="segment-btn"
            class:active={(themeSettings.cornerRadius ?? 1) === r.id}
            onclick={() => onUpdateTheme({ cornerRadius: r.id })}
          >
            {r.label}
          </button>
        {/each}
      </div>
    </section>
  </div>

  <footer class="drawer-footer">
    <button type="button" class="btn-reset" onclick={handleReset}>
      <IconRotate2 size={16} /> Reset changes
    </button>
    <button type="button" class="btn-save" onclick={onClose}>
      <IconDeviceFloppy size={16} /> Save
    </button>
  </footer>
</dialog>

<style>
  .theme-drawer-panel::backdrop {
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(2px);
  }

  .theme-drawer-panel:not([open]) {
    display: none;
  }

  .theme-drawer-panel {
    position: fixed;
    top: 0;
    right: 0;
    left: auto;
    height: 100dvh;
    width: 320px;
    max-width: 90vw;
    max-height: none;
    margin: 0;
    padding: 0;
    background: var(--fasti-surface-paper);
    border-left: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    z-index: 1000;
    display: flex;
    flex-direction: column;
    box-shadow: -8px 0 24px rgba(0, 0, 0, 0.15);
    animation: slide-in-right 180ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes slide-in-right {
    from {
      transform: translateX(100%);
    }
    to {
      transform: translateX(0);
    }
  }

  .drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .drawer-title {
    font-size: 1.05rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
  }

  .close-btn {
    background: transparent;
    border: none;
    color: var(--fasti-text-muted);
    cursor: pointer;
    min-width: 44px;
    min-height: 44px;
    padding: 8px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    color: var(--fasti-text-primary);
    background: var(--fasti-surface-archive);
  }

  .drawer-body {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .drawer-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .section-label {
    font-size: 0.8rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--fasti-text-muted);
  }

  .mode-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  .mode-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-height: 64px;
    padding: 12px 8px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: var(--tblr-border-radius, 4px);
    color: var(--fasti-text-muted);
    font-size: 0.82rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 120ms ease;
  }

  .mode-btn:hover {
    color: var(--fasti-text-primary);
    border-color: var(--fasti-text-muted);
  }

  .mode-btn.active {
    background: color-mix(
      in srgb,
      var(--fasti-action-primary) 12%,
      transparent
    );
    border-color: var(--fasti-action-primary);
    color: var(--fasti-action-primary);
    font-weight: 600;
  }

  .swatches-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .swatch-btn {
    width: 34px;
    height: 34px;
    min-width: 34px;
    min-height: 34px;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
    padding: 0;
    margin: 2px;
    transition: transform 120ms ease;
  }

  .swatch-btn:hover {
    transform: scale(1.15);
  }

  .swatch-btn.selected {
    outline: 2px solid var(--fasti-text-primary);
    outline-offset: 2px;
  }

  .theme-select {
    width: 100%;
    min-height: 44px;
    padding: 10px 14px;
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: var(--tblr-border-radius, 4px);
  }

  .segmented-row {
    display: flex;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: var(--tblr-border-radius, 4px);
    overflow: hidden;
  }

  .segment-btn {
    flex: 1;
    min-height: 44px;
    background: var(--fasti-surface-archive);
    border: none;
    border-right: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    padding: 10px 6px;
    color: var(--fasti-text-primary);
    font-size: 0.82rem;
    font-weight: 500;
    cursor: pointer;
    text-align: center;
    transition: all 100ms ease;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .segment-btn:last-child {
    border-right: none;
  }

  .segment-btn.active {
    background: var(--fasti-action-primary);
    color: #ffffff;
  }

  .drawer-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    background: var(--fasti-surface-archive);
  }

  .btn-reset {
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: none;
    color: var(--fasti-text-muted);
    font-size: 0.86rem;
    cursor: pointer;
    padding: 8px 14px;
    border-radius: 4px;
  }

  .btn-reset:hover {
    color: var(--fasti-text-primary);
  }

  .btn-save {
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--fasti-action-primary);
    color: #ffffff;
    border: none;
    border-radius: var(--tblr-border-radius, 4px);
    padding: 10px 20px;
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
  }

  .btn-save:hover {
    filter: brightness(1.1);
  }
</style>
