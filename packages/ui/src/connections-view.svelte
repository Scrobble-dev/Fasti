<script lang="ts">
  import IconAlertTriangle from "@tabler/icons-svelte/icons/alert-triangle";
  import IconCheck from "@tabler/icons-svelte/icons/check";
  import IconDeviceTv from "@tabler/icons-svelte/icons/device-tv";
  import IconInfoCircle from "@tabler/icons-svelte/icons/info-circle";
  import IconPlug from "@tabler/icons-svelte/icons/plug";
  import IconRefresh from "@tabler/icons-svelte/icons/refresh";
  import IconRadio from "@tabler/icons-svelte/icons/radio";
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
        Adapter readiness reported by the running Fasti node, not a health check
        of configured connections.
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
    class="connectors-section"
    aria-labelledby="integration-status-title"
  >
    <div class="section-heading">
      <h2 class="section-title" id="integration-status-title">
        Adapter readiness
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
    <p class="readiness-note" id="readiness-note">
      Endpoint readiness, platform support, and active states come from the
      runtime. This page does not infer them.
    </p>

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
      <!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users must be able to scroll the responsive table.) -->
      <div
        class="table-responsive readiness-scroll"
        role="region"
        aria-label="Adapter readiness table"
        tabindex="0"
      >
        <table
          class="table readiness-table"
          aria-labelledby="integration-status-title"
          aria-describedby="readiness-note"
        >
          <thead>
            <tr>
              <th scope="col">Adapter</th>
              <th scope="col">Readiness</th>
              <th scope="col">Endpoint</th>
              <th scope="col">Platform</th>
              <th scope="col">Setup action</th>
            </tr>
          </thead>
          <tbody>
            {#each integrations as integration (integration.id)}
              {@const IntegrationIcon = iconFor(integration.id)}
              {@const StateIcon = stateIcon(integration.state)}
              <tr data-integration={integration.id}>
                <th scope="row">
                  <span class="conn-name">
                    <IntegrationIcon
                      size={20}
                      stroke={1.75}
                      aria-hidden="true"
                    />
                    {integration.label}
                  </span>
                  <p class="conn-desc">{integration.detail}</p>
                </th>
                <td>
                  <span
                    class="badge conn-status-pill"
                    data-state={integration.state}
                  >
                    <StateIcon size={14} stroke={2} aria-hidden="true" />
                    {stateLabel(integration.state)}
                  </span>
                </td>
                <td>{integration.endpoint_ready ? "Ready" : "Not exposed"}</td>
                <td>{integration.available ? "Supported" : "Unavailable"}</td>
                <td>{integration.setup_action}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </section>

  <ApiClientsPanel {host} />
</div>

<style>
  .connections-container {
    min-width: 0;
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
    --tblr-btn-color: var(--fasti-text-muted);
    --tblr-btn-hover-color: var(--fasti-text-primary);
    --tblr-btn-hover-bg: var(--fasti-surface-paper);
    --tblr-btn-active-color: var(--fasti-text-primary);
    --tblr-btn-active-bg: var(--fasti-surface-paper);
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }
  .refresh-button:focus-visible,
  .readiness-scroll:focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }

  .section-heading {
    margin-bottom: 8px;
    align-items: baseline;
  }
  .section-title {
    font-size: 1.4rem;
    margin: 0;
  }
  .status-summary,
  .readiness-note {
    margin: 0;
    color: var(--fasti-text-muted);
    font-size: 0.875rem;
  }
  .readiness-note {
    margin-bottom: 16px;
  }

  .alert {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }
  .readiness-scroll,
  .empty-card {
    background: var(--fasti-surface-paper);
  }
  .readiness-scroll {
    max-width: 100%;
    overflow-x: auto;
  }
  .readiness-table {
    width: 100%;
    min-width: 720px;
    margin: 0;
    color: var(--fasti-text-primary);
  }
  .readiness-table th,
  .readiness-table td {
    padding: 12px;
    vertical-align: top;
    white-space: normal;
    overflow-wrap: anywhere;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }
  .readiness-table th:first-child {
    width: 32%;
  }
  .readiness-table th:last-child {
    width: 28%;
  }
  .readiness-table thead th {
    color: var(--fasti-text-muted);
    background: transparent;
  }
  .readiness-table thead th,
  .readiness-table td:nth-child(3),
  .readiness-table td:nth-child(4) {
    white-space: nowrap;
    overflow-wrap: normal;
  }
  .conn-name {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
    font-weight: 600;
  }
  .conn-name :global(svg),
  .conn-status-pill :global(svg) {
    flex-shrink: 0;
  }
  .conn-status-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
    overflow-wrap: normal;
    text-align: left;
    color: var(--fasti-text-primary);
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
    color: var(--fasti-text-muted);
    font-weight: 400;
    line-height: 1.5;
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
