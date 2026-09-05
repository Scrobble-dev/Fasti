<script lang="ts">
  import IconAlertTriangle from "@tabler/icons-svelte/icons/alert-triangle";
  import IconCheck from "@tabler/icons-svelte/icons/check";
  import IconDeviceTv from "@tabler/icons-svelte/icons/device-tv";
  import IconInfoCircle from "@tabler/icons-svelte/icons/info-circle";
  import IconPlug from "@tabler/icons-svelte/icons/plug";
  import IconRefresh from "@tabler/icons-svelte/icons/refresh";
  import IconRadio from "@tabler/icons-svelte/icons/radio";
  import IconSettings from "@tabler/icons-svelte/icons/settings";
  import { onMount } from "svelte";
  import ApiClientsPanel from "./api-clients-panel.svelte";
  import {
    hasIntegrationStatusHost,
    type IntegrationRuntimeState,
    type IntegrationRuntimeStatus,
  } from "./integration-status.js";
  import type { WorkbenchHost } from "./types.js";

  interface Props {
    host?: WorkbenchHost;
  }

  let { host }: Props = $props();
  let integrations = $state<IntegrationRuntimeStatus[]>([]);
  let loading = $state(true);
  let problem = $state<string>();

  const iconFor = (id: string) => {
    if (id === "nuvio") return IconDeviceTv;
    if (id === "mpris") return IconRadio;
    return IconPlug;
  };

  const stateLabel = (state: IntegrationRuntimeState): string => {
    switch (state) {
      case "available":
        return "Available";
      case "setup_required":
        return "Setup required";
      case "active":
        return "Active";
      case "degraded":
        return "Needs attention";
      case "disabled":
        return "Disabled";
      case "unsupported":
        return "Not supported here";
      case "error":
        return "Error";
    }
  };

  const stateIcon = (state: IntegrationRuntimeState) => {
    if (state === "active" || state === "available") return IconCheck;
    if (state === "degraded" || state === "error") return IconAlertTriangle;
    return IconInfoCircle;
  };

  async function refresh(): Promise<void> {
    loading = true;
    problem = undefined;
    if (!hasIntegrationStatusHost(host)) {
      integrations = [];
      problem = "Runtime integration status is not available from this host.";
      loading = false;
      return;
    }
    try {
      integrations = await host.listIntegrations();
    } catch (error) {
      integrations = [];
      problem =
        error instanceof Error
          ? error.message
          : "Fasti could not load integration status.";
    } finally {
      loading = false;
    }
  }

  onMount(() => void refresh());
</script>

<div class="connections-container">
  <header class="view-header">
    <div>
      <h1 class="view-title">Connections</h1>
      <p class="view-subtitle">
        Configure external observers and see the state reported by the running
        Fasti node.
      </p>
    </div>
    <button
      type="button"
      class="btn btn-outline-secondary refresh-button"
      disabled={loading}
      onclick={() => void refresh()}
    >
      <IconRefresh size={18} stroke={1.75} aria-hidden="true" />
      Refresh status
    </button>
  </header>

  <section
    class="card availability-card"
    aria-labelledby="connections-availability-title"
  >
    <div class="card-body availability-body">
      <IconInfoCircle size={28} stroke={1.75} aria-hidden="true" />
      <div>
        <h2 id="connections-availability-title" class="card-title">
          Connection status comes from the running node
        </h2>
        <p class="text-secondary mb-0">
          Endpoint readiness and platform support are not inferred from this
          page. A connection becomes active only when its runtime reports that
          state.
        </p>
      </div>
    </div>
  </section>

  <ApiClientsPanel {host} />

  <section
    class="connectors-section"
    aria-labelledby="integration-status-title"
  >
    <div class="section-heading">
      <h2 class="section-title" id="integration-status-title">
        Integration status
      </h2>
      <p class="status-summary" aria-live="polite">
        {#if loading}
          Loading integration status…
        {:else if problem}
          Integration status needs attention.
        {:else}
          {integrations.length} integration{integrations.length === 1
            ? ""
            : "s"} reported.
        {/if}
      </p>
    </div>

    {#if problem}
      <div class="alert alert-warning" role="status">
        <IconAlertTriangle size={22} aria-hidden="true" />
        <div>
          <strong>Could not read runtime status.</strong>
          <div>{problem}</div>
        </div>
      </div>
    {:else if !loading && integrations.length === 0}
      <div class="card empty-card">
        <div class="card-body">
          <h3 class="card-title">No integration capability was reported</h3>
          <p class="text-secondary mb-0">
            Check that this interface points to the intended Fasti node, then
            refresh.
          </p>
        </div>
      </div>
    {/if}

    {#if integrations.length > 0}
      <div class="connectors-grid">
        {#each integrations as integration (integration.id)}
          {@const IntegrationIcon = iconFor(integration.id)}
          {@const StateIcon = stateIcon(integration.state)}
          <article
            class="card connector-card"
            data-integration={integration.id}
          >
            <div class="card-body">
              <div class="card-head">
                <IntegrationIcon
                  size={26}
                  stroke={1.75}
                  class="conn-icon"
                  aria-hidden="true"
                />
                <div class="card-heading-copy">
                  <h3 class="card-title conn-name">{integration.label}</h3>
                  <span
                    class="badge conn-status-pill"
                    data-state={integration.state}
                  >
                    <StateIcon size={14} stroke={2} aria-hidden="true" />
                    {stateLabel(integration.state)}
                  </span>
                </div>
              </div>
              <p class="text-secondary conn-desc">{integration.detail}</p>
              <div class="setup-action">
                <IconSettings size={18} stroke={1.75} aria-hidden="true" />
                <span>{integration.setup_action}</span>
              </div>
              <dl class="runtime-facts">
                <div>
                  <dt>Endpoint</dt>
                  <dd>
                    {integration.endpoint_ready ? "Ready" : "Not exposed"}
                  </dd>
                </div>
                <div>
                  <dt>Platform</dt>
                  <dd>{integration.available ? "Supported" : "Unavailable"}</dd>
                </div>
              </dl>
            </div>
          </article>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .connections-container {
    max-width: 1040px;
    margin: 0 auto;
    padding: 32px 24px 64px;
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .view-header,
  .section-heading {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
  }

  .view-header {
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
    padding-bottom: 16px;
  }

  .view-title,
  .section-title {
    font-family: var(--fasti-font-display);
    color: var(--fasti-text-primary);
  }

  .view-title {
    font-size: 2.4rem;
    font-weight: 600;
    margin: 0 0 4px;
  }
  .view-subtitle {
    color: var(--fasti-text-muted);
    margin: 0;
    max-width: 70ch;
    line-height: 1.5;
  }
  .refresh-button {
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }
  .refresh-button:focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }

  .availability-card {
    background: var(--fasti-surface-paper);
  }
  .availability-body {
    display: flex;
    gap: 16px;
    align-items: flex-start;
  }
  .availability-body :global(svg) {
    flex-shrink: 0;
    color: var(--fasti-text-muted);
  }

  .section-heading {
    margin-bottom: 16px;
    align-items: baseline;
  }
  .section-title {
    font-size: 1.4rem;
    margin: 0;
  }
  .status-summary {
    margin: 0;
    color: var(--fasti-text-muted);
    font-size: 0.875rem;
  }

  .alert {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .connectors-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 300px), 1fr));
    gap: 20px;
  }
  .connector-card,
  .empty-card {
    background: var(--fasti-surface-paper);
  }
  .connector-card .card-body {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .card-head {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .card-heading-copy {
    min-width: 0;
  }
  :global(.conn-icon) {
    color: var(--fasti-text-muted);
    flex-shrink: 0;
    margin-top: 2px;
  }
  .conn-name {
    margin: 0 0 6px;
    font-size: 1.1rem;
  }
  .conn-status-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    white-space: normal;
    text-align: left;
  }
  .conn-status-pill[data-state="active"] {
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 18%,
      transparent
    );
    color: var(--fasti-text-primary);
  }
  .conn-status-pill[data-state="degraded"],
  .conn-status-pill[data-state="error"] {
    background: color-mix(
      in srgb,
      var(--fasti-state-attention) 20%,
      transparent
    );
    color: var(--fasti-text-primary);
  }
  .conn-desc {
    margin: 0;
    line-height: 1.5;
  }
  .setup-action {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: 0.875rem;
    line-height: 1.45;
  }
  .setup-action :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
  }
  .runtime-facts {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
    margin: 0;
  }
  .runtime-facts div {
    padding-top: 10px;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }
  .runtime-facts dt {
    color: var(--fasti-text-muted);
    font-size: 0.75rem;
  }
  .runtime-facts dd {
    margin: 2px 0 0;
    font-weight: 600;
  }

  @media (max-width: 47.99rem) {
    .connections-container {
      padding: 24px 16px 48px;
    }
    .view-header,
    .section-heading {
      flex-direction: column;
      align-items: stretch;
    }
    .view-title {
      font-size: 2rem;
    }
    .refresh-button {
      width: 100%;
      justify-content: center;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .refresh-button {
      transition: none;
    }
  }
</style>
