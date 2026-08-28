<script lang="ts">
  import {
    IconDeviceTv,
    IconInfoCircle,
    IconPlug,
    IconRadio,
  } from "@tabler/icons-svelte";
  import ApiClientsPanel from "./api-clients-panel.svelte";
  import type { WorkbenchHost } from "./types.js";

  interface Props {
    host?: WorkbenchHost;
  }

  let { host }: Props = $props();

  const connectors = [
    {
      id: "nuvio",
      name: "NuvioTV",
      desc: "Fasti now has authenticated durable occurrence ingress and scoped client credentials. Current upstream Nuvio still needs a Fasti tracking provider before it can submit those observations directly.",
      status: "Client integration required",
      icon: IconDeviceTv,
    },
    {
      id: "plex",
      name: "Plex & Tautulli",
      desc: "No production webhook adapter is mounted. Fasti does not publish a Plex webhook URL in this build.",
      status: "Not available",
      icon: IconPlug,
    },
    {
      id: "jellyfin",
      name: "Jellyfin & Emby",
      desc: "No production webhook adapter is mounted. Fasti does not publish a Jellyfin or Emby webhook URL in this build.",
      status: "Not available",
      icon: IconPlug,
    },
    {
      id: "mpris",
      name: "Desktop MPRIS observer",
      desc: "The shared observation contract can support local observers, but no production MPRIS adapter is active in this build.",
      status: "Not available",
      icon: IconRadio,
    },
  ];
</script>

<div class="connections-container">
  <header class="view-header">
    <h1 class="view-title">Connections</h1>
    <p class="view-subtitle">
      See what Fasti can accept now, create scoped external-client credentials,
      and distinguish active capability from later integration work.
    </p>
  </header>

  <section
    class="availability-card"
    aria-labelledby="connections-availability-title"
  >
    <IconInfoCircle size={28} stroke={1.75} aria-hidden="true" />
    <div>
      <h2 id="connections-availability-title">
        Durable occurrence ingress is active locally
      </h2>
      <p>
        Fasti now accepts authenticated consumption occurrences through the
        local API and returns durable idempotency receipts. This is not the same
        as native Nuvio support. Partial progress, pairing, discovery, two-way
        state synchronization, and source-specific webhooks remain unavailable
        until their own contracts and adapters are active.
      </p>
    </div>
  </section>

  <ApiClientsPanel {host} />

  <section
    class="connectors-section"
    aria-labelledby="integration-status-title"
  >
    <h2 class="section-title" id="integration-status-title">
      Integration status
    </h2>
    <div class="connectors-grid">
      {#each connectors as conn (conn.id)}
        <article class="connector-card">
          <div class="card-head">
            <conn.icon
              size={24}
              stroke={1.75}
              class="conn-icon"
              aria-hidden="true"
            />
            <div>
              <h3 class="conn-name">{conn.name}</h3>
              <span class="conn-status-pill">{conn.status}</span>
            </div>
          </div>
          <p class="conn-desc">{conn.desc}</p>
        </article>
      {/each}
    </div>
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

  .view-header {
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
    padding-bottom: 16px;
  }

  .view-title {
    font-family: var(--fasti-font-display);
    font-size: 2.4rem;
    font-weight: 600;
    margin: 0 0 4px;
    color: var(--fasti-text-primary);
  }

  .view-subtitle,
  .availability-card p,
  .conn-desc {
    color: var(--fasti-text-muted);
  }

  .view-subtitle,
  .availability-card p,
  .conn-desc,
  .section-title,
  .conn-name,
  .availability-card h2 {
    margin-top: 0;
  }

  .view-subtitle {
    margin-bottom: 0;
    max-width: 70ch;
    line-height: 1.5;
  }

  .availability-card {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    padding: 20px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: calc(8px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-surface-paper);
  }

  .availability-card h2 {
    margin-bottom: 4px;
    font-size: 1.15rem;
  }

  .availability-card p,
  .conn-desc {
    margin-bottom: 0;
    line-height: 1.5;
  }

  .section-title {
    font-family: var(--fasti-font-display);
    font-size: 1.4rem;
    margin-bottom: 16px;
    color: var(--fasti-text-primary);
  }

  .connectors-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 320px), 1fr));
    gap: 20px;
  }

  .connector-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .card-head {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }

  :global(.conn-icon) {
    color: var(--fasti-text-muted);
    flex-shrink: 0;
    margin-top: 2px;
  }

  .conn-name {
    font-family: var(--fasti-font-display);
    font-size: 1.1rem;
    font-weight: 600;
    margin-bottom: 4px;
  }

  .conn-status-pill {
    display: inline-block;
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--fasti-text-muted);
    background: var(--fasti-surface-archive);
    padding: 3px 7px;
    border-radius: calc(3px * var(--tblr-border-radius-scale, 1));
  }

  .conn-desc {
    font-size: 0.88rem;
  }

  @media (max-width: 47.99rem) {
    .connections-container {
      padding: 24px 16px 48px;
    }

    .view-title {
      font-size: 2rem;
    }
  }
</style>
