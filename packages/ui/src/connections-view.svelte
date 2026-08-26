<script lang="ts">
  import { IconDeviceTv, IconPlug, IconRadio, IconInfoCircle } from "@tabler/icons-svelte";

  const connectors = [
    {
      id: "nuvio",
      name: "NuvioTV",
      desc: "Planned integration lane for observation submission and later state synchronization.",
      status: "Not available yet",
      icon: IconDeviceTv,
    },
    {
      id: "plex",
      name: "Plex & Tautulli",
      desc: "Planned source-conformance target. No production webhook adapter is active.",
      status: "Not available yet",
      icon: IconPlug,
    },
    {
      id: "jellyfin",
      name: "Jellyfin & Emby",
      desc: "Planned source-conformance target. No production webhook adapter is active.",
      status: "Not available yet",
      icon: IconPlug,
    },
    {
      id: "mpris",
      name: "Desktop MPRIS / D-Bus Observer",
      desc: "Reserved local observation source. It is not active in this workbench build.",
      status: "Not available yet",
      icon: IconRadio,
    },
  ];
</script>

<div class="connections-container">
  <header class="view-header">
    <div>
      <h1 class="view-title">Connections</h1>
      <p class="view-subtitle">
        Fasti will show a connection here only after its capability and host adapter are active.
      </p>
    </div>
  </header>

  <section class="availability-card" aria-labelledby="connections-availability-title">
    <IconInfoCircle size={28} stroke={1.75} aria-hidden="true" />
    <div>
      <h2 id="connections-availability-title">No connection adapters are active</h2>
      <p>
        This prototype does not advertise discovery, pairing, webhook, or Nuvio endpoints that
        the backend cannot serve. Local discovery and source integrations remain later delivery
        work. Existing local Chronicle state is not affected.
      </p>
    </div>
  </section>

  <section class="connectors-section" aria-labelledby="planned-connections-title">
    <h2 class="section-title" id="planned-connections-title">Planned integrations</h2>

    <div class="connectors-grid">
      {#each connectors as conn (conn.id)}
        <article class="connector-card">
          <div class="card-head">
            <conn.icon size={24} stroke={1.75} class="conn-icon" aria-hidden="true" />
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
    max-width: 960px;
    margin: 0 auto;
    padding: 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .view-header {
    border-bottom: 2px solid color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
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

  .availability-card {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    padding: 20px;
    border: 1px solid color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 8px;
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
    border: 1px solid color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
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
    border-radius: 3px;
  }

  .conn-desc {
    font-size: 0.88rem;
  }
</style>
