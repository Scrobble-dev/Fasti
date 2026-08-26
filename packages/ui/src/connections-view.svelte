<script lang="ts">
  import { IconPlugOff } from "@tabler/icons-svelte";
</script>

<div class="connections-container">
  <header class="view-header">
    <h1 class="view-title">Connections & Ingest</h1>
    <p class="view-subtitle">Connector setup is not available in this build.</p>
  </header>

  <section class="unavailable-card" aria-labelledby="connectors-unavailable">
    <IconPlugOff size={36} aria-hidden="true" />
    <div>
      <h2 id="connectors-unavailable">No connector capabilities are active</h2>
      <p>
        Fasti will show pairing details and endpoint URLs only after the
        matching ingest capability is implemented and reported by the node.
      </p>
    </div>
  </section>
</div>

<style>
  .connections-container {
    max-width: 960px;
    margin: 0 auto;
    padding: 32px 24px;
  }

  .view-header {
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
    padding-bottom: 16px;
  }

  .view-title,
  .unavailable-card h2 {
    font-family: var(--fasti-font-display);
    color: var(--fasti-text-primary);
  }

  .view-title {
    font-size: 2.4rem;
    margin: 0;
  }

  .view-subtitle,
  .unavailable-card p {
    color: var(--fasti-text-muted);
  }

  .view-subtitle,
  .unavailable-card h2,
  .unavailable-card p {
    margin: 4px 0 0;
  }

  .unavailable-card {
    display: flex;
    gap: 16px;
    margin-top: 32px;
    padding: 24px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
    background: var(--fasti-surface-paper);
  }
</style>
