<script lang="ts">
  import type { ThemeSettings } from "./types.js";
  import { DEFAULT_THEME_SETTINGS } from "./defaults.js";
  import { dialogFocus } from "./dialog-focus.js";
  import IconSun from "@tabler/icons-svelte/icons/sun";
  import IconMoon from "@tabler/icons-svelte/icons/moon";
  import IconMoonStars from "@tabler/icons-svelte/icons/moon-stars";
  import IconX from "@tabler/icons-svelte/icons/x";
  import IconRotate2 from "@tabler/icons-svelte/icons/rotate-2";

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
  ] as const;

  const BASES = [
    { id: "slate", name: "Slate" },
    { id: "gray", name: "Gray" },
    { id: "zinc", name: "Zinc" },
    { id: "neutral", name: "Neutral" },
    { id: "stone", name: "Stone" },
  ] as const;

  const RADII = [
    { id: 0, label: "0" },
    { id: 0.5, label: ".5" },
    { id: 1, label: "1" },
    { id: 1.5, label: "1.5" },
    { id: 2, label: "2" },
  ] as const;

  function handleReset(): void {
    onUpdateTheme(DEFAULT_THEME_SETTINGS);
  }
</script>

<dialog
  bind:this={drawer}
  use:dialogFocus
  class="offcanvas offcanvas-end offcanvas-narrow theme-drawer-panel show"
  aria-modal="true"
  aria-labelledby="theme-drawer-title"
  oncancel={onClose}
  onclick={(event) => {
    if (event.target === event.currentTarget) onClose();
  }}
>
  <header class="offcanvas-header">
    <h2 id="theme-drawer-title" class="offcanvas-title">Theme settings</h2>
    <button
      type="button"
      class="btn btn-icon btn-ghost-secondary close-button"
      onclick={onClose}
      aria-label="Close theme settings"
    >
      <IconX size={20} />
    </button>
  </header>

  <div class="offcanvas-body">
    <fieldset class="theme-group">
      <legend class="form-label">Color mode</legend>
      <div class="mode-grid">
        <button
          type="button"
          class="btn btn-outline-secondary mode-button"
          class:active={themeSettings.mode === "light"}
          aria-pressed={themeSettings.mode === "light"}
          onclick={() => onUpdateTheme({ mode: "light" })}
        >
          <IconSun size={18} />
          <span>Light</span>
        </button>

        <button
          type="button"
          class="btn btn-outline-secondary mode-button"
          class:active={themeSettings.mode === "dark"}
          aria-pressed={themeSettings.mode === "dark"}
          onclick={() => onUpdateTheme({ mode: "dark" })}
        >
          <IconMoon size={18} />
          <span>Dark</span>
        </button>

        <button
          type="button"
          class="btn btn-outline-secondary mode-button"
          class:active={themeSettings.mode === "night"}
          aria-pressed={themeSettings.mode === "night"}
          onclick={() => onUpdateTheme({ mode: "night" })}
        >
          <IconMoonStars size={18} />
          <span>Night</span>
        </button>
      </div>
    </fieldset>

    <fieldset class="theme-group">
      <legend class="form-label">Scheme</legend>
      <div class="swatches-grid">
        {#each SCHEMES as s}
          <button
            type="button"
            class="btn btn-icon swatch-button"
            class:selected={themeSettings.accentColor === s.hex}
            aria-pressed={themeSettings.accentColor === s.hex}
            style="background-color: {s.hex};"
            title={s.name}
            onclick={() => onUpdateTheme({ accentColor: s.hex })}
            aria-label={s.name}
          ></button>
        {/each}
      </div>
    </fieldset>

    <div class="theme-group">
      <label class="form-label" for="theme-font-select">Font family</label>
      <div>
        <select
          id="theme-font-select"
          class="form-select"
          value={themeSettings.fontFamily ?? "sans-serif"}
          onchange={(e) =>
            onUpdateTheme({
              fontFamily: e.currentTarget.value as NonNullable<
                ThemeSettings["fontFamily"]
              >,
            })}
        >
          <option value="sans-serif">Atkinson Hyperlegible (Sans)</option>
          <option value="serif">Newsreader (Editorial Serif)</option>
          <option value="monospace">IBM Plex Mono (Terminal Mono)</option>
        </select>
      </div>
    </div>

    <fieldset class="theme-group">
      <legend class="form-label">Theme base</legend>
      <div
        class="btn-group w-100 theme-segment-group"
        role="group"
        aria-label="Theme base"
      >
        {#each BASES as b}
          <button
            type="button"
            class="btn btn-outline-secondary segment-button"
            class:active={(themeSettings.themeBase ?? "slate") === b.id}
            aria-pressed={(themeSettings.themeBase ?? "slate") === b.id}
            onclick={() => onUpdateTheme({ themeBase: b.id })}
          >
            {b.name}
          </button>
        {/each}
      </div>
    </fieldset>

    <fieldset class="theme-group">
      <legend class="form-label">Corner radius</legend>
      <div
        class="btn-group w-100 theme-segment-group"
        role="group"
        aria-label="Corner radius"
      >
        {#each RADII as r}
          <button
            type="button"
            class="btn btn-outline-secondary segment-button"
            class:active={(themeSettings.cornerRadius ?? 1) === r.id}
            aria-pressed={(themeSettings.cornerRadius ?? 1) === r.id}
            onclick={() => onUpdateTheme({ cornerRadius: r.id })}
          >
            {r.label}
          </button>
        {/each}
      </div>
    </fieldset>
  </div>

  <footer class="offcanvas-footer d-flex justify-content-between gap-2">
    <button
      type="button"
      class="btn btn-outline-secondary theme-reset-button"
      onclick={handleReset}
    >
      <IconRotate2 size={16} /> Reset changes
    </button>
    <button type="button" class="btn btn-primary" onclick={onClose}>
      Done
    </button>
  </footer>
</dialog>

<style>
  .theme-drawer-panel::backdrop {
    background: rgba(0, 0, 0, 0.45);
  }

  .theme-drawer-panel:not([open]) {
    display: none;
  }

  .theme-drawer-panel {
    height: 100dvh;
    max-width: 100vw;
    max-height: none;
    margin: 0 0 0 auto;
    padding: 0;
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  .theme-drawer-panel :global(.btn) {
    transition: none;
  }

  .close-button {
    margin-left: auto;
    color: var(--fasti-text-muted);
    min-width: 44px;
    min-height: 44px;
  }

  .offcanvas-body {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .theme-group {
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }

  .form-label {
    float: none;
    width: auto;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--fasti-text-muted);
  }

  .mode-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  .mode-button {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-height: 64px;
  }

  .mode-button,
  .segment-button {
    --tblr-btn-active-color: var(--fasti-action-contrast);
    --tblr-btn-active-bg: var(--fasti-action-primary);
    --tblr-btn-active-border-color: var(--fasti-action-primary);
  }

  .mode-button.active,
  .segment-button.active {
    background: var(--fasti-action-primary);
    border-color: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
  }

  .swatches-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .swatch-button {
    width: 44px;
    height: 44px;
    min-width: 44px;
    min-height: 44px;
    border-radius: 50%;
    padding: 0;
  }

  .swatch-button.selected {
    outline: 3px solid var(--fasti-text-primary);
    outline-offset: 2px;
  }

  .form-select,
  .segment-button {
    min-height: 44px;
  }

  .theme-segment-group {
    flex-wrap: wrap;
  }

  .theme-segment-group .segment-button {
    flex: 1 1 4rem;
  }

  .theme-reset-button {
    color: var(--fasti-text-primary);
  }

  .offcanvas-footer {
    flex-wrap: wrap;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    background: var(--fasti-surface-archive);
  }

  .offcanvas-footer .btn {
    min-height: 44px;
  }
</style>
