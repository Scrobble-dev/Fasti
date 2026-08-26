<script lang="ts">
  import { onMount } from "svelte";
  import type { ThemeSettings } from "./types.js";
  import {
    IconSun,
    IconMoon,
    IconDeviceDesktop,
    IconX,
    IconDeviceFloppy,
    IconRotate2,
    IconCheck,
  } from "@tabler/icons-svelte";

  interface Props {
    open: boolean;
    themeSettings: ThemeSettings;
    onClose: () => void;
    onUpdateTheme: (updates: Partial<ThemeSettings>) => void;
  }

  let { open = false, themeSettings, onClose, onUpdateTheme }: Props = $props();

  function handleKeyDown(e: KeyboardEvent): void {
    if (e.key === "Escape" && open) {
      onClose();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  });

  const SCHEMES = [
    { id: "#8B2E2A", name: "Fasti Oxblood", hex: "#8B2E2A" },
    { id: "#1E4FA3", name: "Chronicle Blue", hex: "#1E4FA3" },
    { id: "#2E6F63", name: "Verdigris", hex: "#2E6F63" },
    { id: "#D4AF37", name: "Horological Gold", hex: "#D4AF37" },
    { id: "#066fd1", name: "Tabler Blue", hex: "#066fd1" },
    { id: "#4263eb", name: "Indigo", hex: "#4263eb" },
    { id: "#ae3ec9", name: "Purple", hex: "#ae3ec9" },
    { id: "#d6336c", name: "Pink", hex: "#d6336c" },
    { id: "#d63939", name: "Crimson", hex: "#d63939" },
    { id: "#f76707", name: "Orange", hex: "#f76707" },
    { id: "#2fb344", name: "Emerald", hex: "#2fb344" },
    { id: "#0ca678", name: "Teal", hex: "#0ca678" },
  ];

  const BASES = [
    { id: "slate", name: "Slate" },
    { id: "gray", name: "Gray" },
    { id: "zinc", name: "Zinc" },
    { id: "neutral", name: "Neutral" },
    { id: "stone", name: "Stone" },
  ];

  const RADII = [
    { id: 0, label: "Sharp (0px)" },
    { id: 0.5, label: "Subdued (2px)" },
    { id: 1, label: "Standard (4px)" },
    { id: 1.5, label: "Soft (6px)" },
    { id: 2, label: "Round (8px)" },
  ];

  function handleReset(): void {
    onUpdateTheme({
      mode: "light",
      accentColor: "#8B2E2A",
      fontFamily: "sans-serif",
      themeBase: "slate",
      cornerRadius: 1,
      density: "normal",
    });
  }
</script>

{#if open}
  <div class="drawer-overlay" onclick={onClose} aria-hidden="true"></div>
  <aside
    class="theme-drawer-panel card shadow-lg border-start"
    aria-label="Theme settings drawer"
  >
    <header
      class="drawer-header card-header d-flex align-items-center justify-content-between p-3"
    >
      <div>
        <h2 class="card-title m-0 fs-3 fw-bold">Appearance Studio</h2>
        <span class="text-muted small"
          >Live layout, color, and typographic settings</span
        >
      </div>
      <button
        type="button"
        class="btn btn-sm btn-icon btn-ghost-secondary"
        onclick={onClose}
        aria-label="Close theme settings"
      >
        <IconX size={20} />
      </button>
    </header>

    <div
      class="drawer-body card-body p-3 overflow-y-auto d-flex flex-column gap-4"
    >
      <!-- 1. Color Mode -->
      <section class="drawer-section">
        <span
          class="form-label d-block fw-bold text-uppercase font-monospace fs-6 mb-2"
          >Color Mode</span
        >
        <div class="btn-group w-100" role="group" aria-label="Theme Mode">
          <button
            type="button"
            class="btn btn-outline-secondary d-flex flex-column align-items-center gap-1 py-2"
            class:active={themeSettings.mode === "light"}
            onclick={() => onUpdateTheme({ mode: "light" })}
          >
            <IconSun size={18} />
            <span class="small font-medium">Light Paper</span>
          </button>

          <button
            type="button"
            class="btn btn-outline-secondary d-flex flex-column align-items-center gap-1 py-2"
            class:active={themeSettings.mode === "dark"}
            onclick={() => onUpdateTheme({ mode: "dark" })}
          >
            <IconMoon size={18} />
            <span class="small font-medium">Dark Slate</span>
          </button>

          <button
            type="button"
            class="btn btn-outline-secondary d-flex flex-column align-items-center gap-1 py-2"
            class:active={themeSettings.mode === "night"}
            onclick={() => onUpdateTheme({ mode: "night" })}
          >
            <IconDeviceDesktop size={18} />
            <span class="small font-medium">Night OLED</span>
          </button>
        </div>
      </section>

      <!-- 2. Color Scheme Palette -->
      <section class="drawer-section">
        <span
          class="form-label d-block fw-bold text-uppercase font-monospace fs-6 mb-2"
          >Primary Accent</span
        >
        <div class="swatches-grid">
          {#each SCHEMES as s}
            <button
              type="button"
              class="swatch-btn d-flex align-items-center justify-content-center"
              class:selected={themeSettings.accentColor === s.hex}
              style="background-color: {s.hex};"
              title={s.name}
              onclick={() => onUpdateTheme({ accentColor: s.hex })}
              aria-label={s.name}
            >
              {#if themeSettings.accentColor === s.hex}
                <IconCheck size={14} stroke={3} class="text-white" />
              {/if}
            </button>
          {/each}
        </div>
      </section>

      <!-- 3. Font Family -->
      <section class="drawer-section">
        <label
          class="form-label fw-bold text-uppercase font-monospace fs-6 mb-2"
          for="theme-font-select">Typographic System</label
        >
        <select
          id="theme-font-select"
          class="form-select"
          value={themeSettings.fontFamily ?? "sans-serif"}
          onchange={(e) =>
            onUpdateTheme({ fontFamily: e.currentTarget.value as any })}
        >
          <option value="sans-serif"
            >Atkinson Hyperlegible (Sans-serif UI)</option
          >
          <option value="serif">Newsreader (Editorial Annal Serif)</option>
          <option value="monospace">IBM Plex Mono (Evidence Monospace)</option>
        </select>
      </section>

      <!-- 4. Layout Density -->
      <section class="drawer-section">
        <span
          class="form-label d-block fw-bold text-uppercase font-monospace fs-6 mb-2"
          >Layout Density</span
        >
        <div class="btn-group w-100" role="group" aria-label="Layout Density">
          {#each [{ id: "compact", label: "Compact" }, { id: "normal", label: "Normal" }, { id: "spacious", label: "Spacious" }] as d}
            <button
              type="button"
              class="btn btn-outline-secondary py-2 small"
              class:active={(themeSettings.density ?? "normal") === d.id}
              onclick={() => onUpdateTheme({ density: d.id as any })}
            >
              {d.label}
            </button>
          {/each}
        </div>
      </section>

      <!-- 5. Corner Radius -->
      <section class="drawer-section">
        <span
          class="form-label d-block fw-bold text-uppercase font-monospace fs-6 mb-2"
          >Corner Radius</span
        >
        <div class="btn-group w-100" role="group" aria-label="Corner Radius">
          {#each RADII as r}
            <button
              type="button"
              class="btn btn-outline-secondary py-1 small"
              class:active={(themeSettings.cornerRadius ?? 1) === r.id}
              onclick={() => onUpdateTheme({ cornerRadius: r.id })}
              title={r.label}
            >
              {r.id}
            </button>
          {/each}
        </div>
      </section>
    </div>

    <footer
      class="drawer-footer card-footer d-flex align-items-center justify-content-between p-3 border-top"
    >
      <button
        type="button"
        class="btn btn-ghost-secondary d-flex align-items-center gap-1"
        onclick={handleReset}
      >
        <IconRotate2 size={16} /> Reset
      </button>
      <button
        type="button"
        class="btn btn-primary d-flex align-items-center gap-1"
        onclick={onClose}
      >
        <IconDeviceFloppy size={16} /> Done
      </button>
    </footer>
  </aside>
{/if}

<style>
  .drawer-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    z-index: 999;
    backdrop-filter: blur(2px);
  }

  .theme-drawer-panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: 360px;
    max-width: 92vw;
    background: var(--fasti-surface-paper);
    z-index: 1000;
    display: flex;
    flex-direction: column;
    box-shadow: -10px 0 30px rgba(0, 0, 0, 0.18);
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

  .swatches-grid {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 8px;
  }

  .swatch-btn {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 6px;
    border: 2px solid transparent;
    cursor: pointer;
    transition:
      transform 120ms ease,
      box-shadow 120ms ease;
    padding: 0;
  }

  .swatch-btn:hover {
    transform: scale(1.08);
  }

  .swatch-btn:focus-visible {
    outline: 3px solid var(--fasti-action-primary, #1e4fa3);
    outline-offset: 2px;
  }

  .swatch-btn.selected {
    border-color: #ffffff;
    box-shadow: 0 0 0 2px var(--fasti-text-primary);
  }
</style>
