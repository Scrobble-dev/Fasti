<script lang="ts">
  import {
    IconPlug,
    IconDeviceTv,
    IconBrandApple,
    IconWorld,
    IconQrcode,
    IconCheck,
    IconRadio,
    IconCopy,
  } from "@tabler/icons-svelte";

  const connectors = [
    {
      id: "nuvio",
      name: "NuvioTV 2-Way Sync Engine (B7)",
      desc: "Monotonic cursor sync with automatic loop suppression and self-draining outbox.",
      endpoint: "http://127.0.0.1:8420/api/v1/nuvio/observations",
      status: "Active & Connected",
      icon: IconDeviceTv,
    },
    {
      id: "plex",
      name: "Plex & Tautulli Webhook Adapter (B6)",
      desc: "Ingests media.scrobble, media.play, media.stop events with TMDB/IMDb GUID deduplication.",
      endpoint: "http://127.0.0.1:8420/api/v1/webhooks/plex",
      status: "Ready for Webhooks",
      icon: IconPlug,
    },
    {
      id: "jellyfin",
      name: "Jellyfin & Emby Webhook Adapter (B6)",
      desc: "Captures PlaybackStop and UserDataSaved events with progress heartbeats.",
      endpoint: "http://127.0.0.1:8420/api/v1/webhooks/jellyfin",
      status: "Ready for Webhooks",
      icon: IconPlug,
    },
    {
      id: "mpris",
      name: "Desktop MPRIS / D-Bus Observer",
      desc: "Captures local media completions from Spotify, VLC, MPV, Tidal on Linux/macOS/Windows.",
      endpoint: "ipc://dev.scrobble.fasti.mpris",
      status: "Listening Locally",
      icon: IconRadio,
    },
  ];

  let copiedEndpoint: string | null = $state(null);

  function copyToClipboard(text: string): void {
    navigator.clipboard.writeText(text);
    copiedEndpoint = text;
    setTimeout(() => (copiedEndpoint = null), 2000);
  }
</script>

<div class="connections-container">
  <header class="view-header">
    <div>
      <h1 class="view-title">Connections & Ingest</h1>
      <p class="view-subtitle">
        Connect your players, media servers, and browser scrobblers into Fasti.
      </p>
    </div>
  </header>

  <!-- Local Network Pairing Card -->
  <section class="pairing-card" aria-label="Local Device Pairing">
    <div class="pairing-icon-box">
      <IconQrcode size={48} stroke={1.5} />
    </div>
    <div class="pairing-content">
      <h2 class="pairing-title">Local Discovery & Secure Pairing</h2>
      <p class="pairing-desc">
        Fasti advertises securely on your local network via DNS-SD / mDNS.
        Native apps like NuvioTV and Fasti Desktop discover the node
        automatically without typing IP addresses.
      </p>
      <div class="pairing-meta">
        <code>mDNS: fasti-local.local:8420</code>
        <span class="bullet">·</span>
        <span class="safe-badge">Rootless & Isolated</span>
      </div>
    </div>
  </section>

  <!-- Connectors Grid -->
  <section class="connectors-section">
    <h2 class="section-title">Active Ingestion Adapters</h2>

    <div class="connectors-grid">
      {#each connectors as conn (conn.id)}
        <div class="connector-card">
          <div class="card-head">
            <conn.icon size={24} stroke={1.75} class="conn-icon" />
            <div>
              <h3 class="conn-name">{conn.name}</h3>
              <span class="conn-status-pill">{conn.status}</span>
            </div>
          </div>

          <p class="conn-desc">{conn.desc}</p>

          <div class="endpoint-box">
            <code class="endpoint-text">{conn.endpoint}</code>
            <button
              type="button"
              class="copy-btn"
              onclick={() => copyToClipboard(conn.endpoint)}
              aria-label="Copy endpoint URL"
            >
              {#if copiedEndpoint === conn.endpoint}
                <IconCheck size={14} stroke={3} class="copied" />
              {:else}
                <IconCopy size={14} stroke={2} />
              {/if}
            </button>
          </div>
        </div>
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

  .view-subtitle {
    margin: 0;
    color: var(--fasti-text-muted);
    font-size: 0.95rem;
  }

  .pairing-card {
    display: flex;
    gap: 20px;
    align-items: center;
    background: var(--fasti-surface-paper);
    border: 1px solid var(--fasti-brand-gold);
    border-radius: 8px;
    padding: 24px;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.03);
  }

  .pairing-icon-box {
    color: var(--fasti-brand-gold);
  }

  .pairing-title {
    font-family: var(--fasti-font-display);
    font-size: 1.3rem;
    font-weight: 600;
    margin: 0 0 4px;
  }

  .pairing-desc {
    margin: 0 0 10px;
    font-size: 0.88rem;
    color: var(--fasti-text-muted);
    max-width: 65ch;
  }

  .pairing-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: var(--fasti-font-mono);
    font-size: 0.78rem;
  }

  .safe-badge {
    color: var(--fasti-state-verified);
    font-weight: 700;
  }

  .section-title {
    font-family: var(--fasti-font-display);
    font-size: 1.4rem;
    margin: 0 0 16px;
    color: var(--fasti-text-primary);
  }

  .connectors-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(400px, 1fr));
    gap: 20px;
  }

  .connector-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
    padding: 20px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 12px;
  }

  .card-head {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }

  :global(.conn-icon) {
    color: var(--fasti-action-primary);
    flex-shrink: 0;
    margin-top: 2px;
  }

  .conn-name {
    font-family: var(--fasti-font-display);
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0 0 2px;
  }

  .conn-status-pill {
    display: inline-block;
    font-family: var(--fasti-font-mono);
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--fasti-state-verified);
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 12%,
      transparent
    );
    padding: 2px 6px;
    border-radius: 3px;
  }

  .conn-desc {
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
    margin: 0;
    line-height: 1.4;
  }

  .endpoint-box {
    display: flex;
    align-items: center;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    border-radius: 4px;
    padding: 6px 10px;
    overflow: hidden;
  }

  .endpoint-text {
    flex: 1;
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .copy-btn {
    background: transparent;
    border: none;
    padding: 4px;
    cursor: pointer;
    color: var(--fasti-text-muted);
  }

  :global(.copied) {
    color: var(--fasti-state-verified);
  }
</style>
