<script lang="ts">
  import { IconCompass } from "@tabler/icons-svelte";
</script>

<div class="discover-container">
  <header class="discover-header">
    <div class="heading-row">
      <IconCompass size={28} class="discover-icon" />
      <h1 class="view-title">Discover</h1>
    </div>
    <p class="view-subtitle">
      Search providers when the local node exposes an active discovery
      capability.
    </p>
  </header>

  <section class="unavailable" aria-labelledby="discover-unavailable-title">
    <h2 id="discover-unavailable-title">No discovery capability</h2>
    <p>The local node does not report an active provider search capability.</p>
  </section>
</div>

<style>
  .discover-container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .discover-header {
    padding-bottom: 16px;
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
  }

  .heading-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  :global(.discover-icon) {
    color: var(--fasti-brand-mark);
  }

  .view-title {
    margin: 0;
    font-family: var(--fasti-font-display);
    font-size: 2.4rem;
    font-weight: 600;
  }

  .view-subtitle,
  .unavailable p {
    margin: 0;
    color: var(--fasti-text-muted);
  }

  .unavailable {
    padding: 32px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    border-radius: 6px;
    background: var(--fasti-surface-paper);
  }

  .unavailable h2 {
    margin: 0 0 8px;
    font-family: var(--fasti-font-display);
  }
</style>
