<script lang="ts">
  import IconAlertTriangle from "@tabler/icons-svelte/icons/alert-triangle";
  import IconCheck from "@tabler/icons-svelte/icons/check";
  import IconMoon from "@tabler/icons-svelte/icons/moon";
  import IconRefresh from "@tabler/icons-svelte/icons/refresh";
  import IconSettings from "@tabler/icons-svelte/icons/settings";
  import IconSun from "@tabler/icons-svelte/icons/sun";
  import type { ConnectionEndpoint } from "@fasti/sdk";
  import NetworkSettingsPanel from "./network-settings-panel.svelte";
  import type { StatusPanelState } from "./status-types.js";

  let {
    status,
    theme,
    mark,
    endpoint,
    publicEndpoint,
    portFallback,
    onRetry,
    onToggleTheme,
    onOpenWorkbench,
  }: {
    status: StatusPanelState;
    theme: "light" | "dark";
    mark: string;
    endpoint: ConnectionEndpoint;
    publicEndpoint?: ConnectionEndpoint;
    portFallback: "auto" | "fail";
    onRetry: () => void;
    onToggleTheme: () => void;
    onOpenWorkbench?: () => void;
  } = $props();
</script>

<div class="status-shell">
  <header class="site-header">
    <div class="header-inner">
      <div class="brand">
        <img class="brand-mark" src={mark} alt="" width="36" height="36" />
        <span>Fasti</span>
      </div>
      <div class="header-actions">
        {#if onOpenWorkbench}
          <button
            type="button"
            class="button workbench-launch"
            onclick={onOpenWorkbench}
          >
            Open Media Workbench
          </button>
        {/if}
        <a class="button settings-link" href="#network-settings">
          <IconSettings size={20} stroke={1.8} aria-hidden="true" />
          Network settings
        </a>
        <button
          type="button"
          class="button theme-toggle"
          onclick={onToggleTheme}
        >
          {#if theme === "dark"}
            <IconSun size={20} stroke={1.8} aria-hidden="true" />
            Use light theme
          {:else}
            <IconMoon size={20} stroke={1.8} aria-hidden="true" />
            Use dark theme
          {/if}
        </button>
      </div>
    </div>
  </header>

  <main id="main-content" class="status-main" tabindex="-1">
    <section class="status-panel" aria-labelledby="status-title">
      <h1 id="status-title">Local service status</h1>
      <p class="intro">
        This page checks the local service health contract. A healthy response
        proves service availability. It does not authenticate Records or
        occurrence ingress.
      </p>

      <div class="state-region">
        {#if status.view === "loading"}
          <div class="state-heading" role="status" aria-atomic="true">
            <span class="state-icon spinner" aria-hidden="true">
              <IconRefresh size={28} stroke={1.8} />
            </span>
            <div>
              <h2>Checking the local service</h2>
              <p>Waiting for the health response.</p>
            </div>
          </div>
        {:else if status.view === "healthy"}
          <div class="state-heading available" role="status" aria-atomic="true">
            <span class="state-icon" aria-hidden="true">
              <IconCheck size={28} stroke={2} />
            </span>
            <div>
              <h2>Local service available</h2>
              <p>The generated health contract accepted the response.</p>
            </div>
          </div>
          <dl>
            <div>
              <dt>Status</dt>
              <dd>{status.health.status}</dd>
            </div>
            <div>
              <dt>Version</dt>
              <dd>{status.health.version}</dd>
            </div>
          </dl>
        {:else}
          <div class="state-heading problem" role="alert">
            <span class="state-icon" aria-hidden="true">
              <IconAlertTriangle size={28} stroke={1.8} />
            </span>
            <div>
              <h2>{status.problem.title}</h2>
              <p>{status.problem.detail}</p>
              <p><strong>Recovery:</strong> {status.problem.recovery}</p>
            </div>
          </div>
          <p class="command-help">
            From the repository root, run
            <code>./scripts/dev.sh</code>.
          </p>
          <div class="recovery-actions">
            <button
              id="retry-health"
              type="button"
              class="button primary"
              onclick={onRetry}
            >
              <IconRefresh size={20} stroke={1.8} aria-hidden="true" />
              Try again
            </button>
            {#if onOpenWorkbench}
              <button
                type="button"
                class="button workbench-launch"
                onclick={onOpenWorkbench}
              >
                Open Media Workbench
              </button>
            {/if}
          </div>
        {/if}
      </div>

      <p class="scope-note">
        Records and durable occurrence ingress have separate authenticated
        contracts. Provider, review, activity, and account features report their
        own availability in the Workbench.
      </p>
    </section>

    <NetworkSettingsPanel {endpoint} {publicEndpoint} {portFallback} />
  </main>
</div>

<style>
  .status-shell {
    min-height: 100svh;
    background: var(--fasti-background);
    color: var(--fasti-foreground);
  }

  .site-header {
    border-block-end: 1px solid var(--fasti-border);
    background: var(--fasti-panel);
  }

  .header-inner {
    width: min(100%, 72rem);
    min-width: 0;
    margin-inline: auto;
    padding: 12px clamp(16px, 4vw, 32px);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .brand {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    color: var(--fasti-foreground);
    font-family: var(--fasti-font-display);
    font-size: 1.5rem;
    font-weight: 600;
    letter-spacing: -0.02em;
  }

  .brand-mark {
    width: 36px;
    height: 36px;
    display: block;
  }

  .button {
    min-width: var(--fasti-touch-target-min);
    min-height: var(--fasti-touch-target-min);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    border: 1px solid transparent;
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    padding: 9px 14px;
    font-weight: 700;
    line-height: 1.2;
    cursor: pointer;
    text-decoration: none;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .recovery-actions {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    margin-top: 16px;
  }

  .settings-link,
  .theme-toggle {
    border-color: var(--fasti-border);
    color: var(--fasti-foreground);
    background: transparent;
  }

  .settings-link:hover,
  .theme-toggle:hover {
    border-color: var(--fasti-action);
    color: var(--fasti-action);
    background: transparent;
  }

  .status-main {
    width: min(100%, 72rem);
    min-width: 0;
    margin-inline: auto;
    padding: clamp(48px, 9vw, 96px) clamp(16px, 4vw, 32px);
  }

  .status-panel {
    width: 100%;
    max-width: 44rem;
    min-width: 0;
    border-block: 1px solid var(--fasti-border);
    padding-block: clamp(28px, 5vw, 48px);
  }

  h1,
  h2,
  p {
    overflow-wrap: anywhere;
  }

  h1,
  h2 {
    margin-block-start: 0;
    color: var(--fasti-foreground);
    font-family: var(--fasti-font-display);
    font-weight: 600;
  }

  h1 {
    margin-block-end: 16px;
    font-size: clamp(2.25rem, 7vw, 4.5rem);
    line-height: 1;
    letter-spacing: -0.03em;
    text-wrap: balance;
  }

  h2 {
    margin-block-end: 8px;
    font-size: clamp(1.4rem, 4vw, 2rem);
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  p {
    max-width: 42rem;
    margin-block: 0;
    line-height: 1.65;
  }

  .intro {
    color: var(--fasti-muted);
    font-size: clamp(1rem, 2.5vw, 1.125rem);
  }

  .state-region {
    margin-block: clamp(32px, 6vw, 56px);
    padding-block: clamp(24px, 4vw, 36px);
    border-block: 1px solid var(--fasti-border);
  }

  .state-heading {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr);
    gap: 16px;
    align-items: start;
  }

  .state-icon {
    display: inline-flex;
    margin-block-start: 2px;
    color: var(--fasti-muted);
  }

  .state-heading.available .state-icon {
    color: var(--fasti-verified);
  }

  .state-heading.problem .state-icon {
    color: var(--fasti-attention);
  }

  dl {
    margin: 28px 0 0;
    display: grid;
    gap: 1px;
    background: var(--fasti-border);
  }

  dl > div {
    min-width: 0;
    padding: 12px 16px;
    display: grid;
    grid-template-columns: minmax(6rem, 0.4fr) minmax(0, 1fr);
    gap: 16px;
    background: var(--fasti-panel);
  }

  dt {
    color: var(--fasti-muted);
    font-weight: 600;
  }

  dd {
    min-width: 0;
    margin: 0;
    font-family: var(--fasti-font-mono);
    overflow-wrap: anywhere;
  }

  code {
    color: var(--fasti-foreground);
    font-family: var(--fasti-font-mono);
  }

  .command-help {
    margin-block-start: 24px;
  }

  .primary {
    margin-block-start: 20px;
    border-color: var(--fasti-action);
    background: var(--fasti-action);
    color: var(--fasti-action-contrast);
  }

  .primary:hover {
    border-color: var(--fasti-action);
    background: color-mix(in srgb, var(--fasti-action) 88%, black);
    color: var(--fasti-action-contrast);
  }

  .scope-note {
    color: var(--fasti-muted);
    font-size: 0.925rem;
  }

  .spinner {
    animation: spin 900ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (max-width: 420px) {
    .header-inner {
      align-items: stretch;
      flex-direction: column;
    }

    .theme-toggle {
      width: 100%;
    }

    .header-actions {
      align-items: stretch;
      flex-direction: column;
    }

    .settings-link {
      width: 100%;
    }

    dl > div {
      grid-template-columns: 1fr;
      gap: 4px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }
  }

  @media (forced-colors: active) {
    .state-icon,
    .state-heading.available .state-icon,
    .state-heading.problem .state-icon {
      color: CanvasText;
    }
  }
</style>
