<script lang="ts">
  import type { CustomFieldDefinition, ScopedApiToken } from "./types.js";
  import {
    IconSettings,
    IconDatabaseImport,
    IconKey,
    IconAdjustments,
    IconPlus,
    IconTrash,
    IconCheck,
    IconShieldCheck,
  } from "@tabler/icons-svelte";

  interface Props {
    customFields: CustomFieldDefinition[];
    tokens: ScopedApiToken[];
  }

  let { customFields, tokens }: Props = $props();

  let activeTab: "schema" | "import" | "tokens" | "a11y" = $state("schema");

  let newFieldKey = $state("");
  let newFieldLabel = $state("");
  let newFieldTarget = $state("game");

  let newTokenName = $state("");
  let generatedToken: string | null = $state(null);
</script>

<div class="settings-container">
  <header class="view-header">
    <div>
      <h1 class="view-title">Settings & Studio</h1>
      <p class="view-subtitle">
        Manage custom schemas, import archives, API tokens, and accessibility.
      </p>
    </div>
  </header>

  <!-- Settings Navigation Tabs -->
  <nav class="settings-tabs" aria-label="Settings categories">
    <button
      type="button"
      class="tab-btn"
      class:active={activeTab === "schema"}
      onclick={() => (activeTab = "schema")}
    >
      <IconAdjustments size={16} /> Custom Types & Fields
    </button>
    <button
      type="button"
      class="tab-btn"
      class:active={activeTab === "import"}
      onclick={() => (activeTab = "import")}
    >
      <IconDatabaseImport size={16} /> Importers & Migration
    </button>
    <button
      type="button"
      class="tab-btn"
      class:active={activeTab === "tokens"}
      onclick={() => (activeTab = "tokens")}
    >
      <IconKey size={16} /> Scoped API Tokens
    </button>
    <button
      type="button"
      class="tab-btn"
      class:active={activeTab === "a11y"}
      onclick={() => (activeTab = "a11y")}
    >
      <IconShieldCheck size={16} /> Accessibility & Display
    </button>
  </nav>

  <main class="settings-body">
    {#if activeTab === "schema"}
      <section class="section-pane">
        <h2 class="pane-heading">Custom Field Definitions</h2>
        <p class="pane-desc">
          Add dynamic, typed fields with dotted hierarchy. Identity promotion
          requires explicit namespace registration to avoid collisions.
        </p>

        <table class="settings-table">
          <thead>
            <tr>
              <th scope="col">Field Key</th>
              <th scope="col">Display Label</th>
              <th scope="col">Target Grain</th>
              <th scope="col">Value Type</th>
              <th scope="col">Namespace</th>
            </tr>
          </thead>
          <tbody>
            {#each customFields as field}
              <tr>
                <td class="mono"><code>{field.key}</code></td>
                <td><strong>{field.label}</strong></td>
                <td><span class="type-tag">{field.targetType}</span></td>
                <td class="mono">{field.valueType}</td>
                <td class="mono">{field.registeredNamespace ?? "—"}</td>
              </tr>
            {/each}
          </tbody>
        </table>

        <div class="add-box">
          <h3 class="add-title">Add Custom Field Definition</h3>
          <div class="input-row">
            <input
              type="text"
              placeholder="e.g. games.gog_product_id"
              bind:value={newFieldKey}
              class="text-input mono"
              aria-label="Field Key"
            />
            <input
              type="text"
              placeholder="e.g. GOG Product ID"
              bind:value={newFieldLabel}
              class="text-input"
              aria-label="Display Label"
            />
            <button type="button" class="btn-primary">
              <IconPlus size={16} /> Add Field
            </button>
          </div>
        </div>
      </section>
    {:else if activeTab === "import"}
      <section class="section-pane">
        <h2 class="pane-heading">Historical Tracker Importers</h2>
        <p class="pane-desc">
          Zero-data-loss archive migration. Fasti preserves every row, rewatch
          timestamp, and partial identifier without forced auto-merging.
        </p>

        <div class="importer-grid">
          <div class="importer-card">
            <h3 class="importer-name">Floppy Database / JSON</h3>
            <p class="importer-meta">
              Direct lossless import of Floppy history, ratings, lists, notes,
              and custom types.
            </p>
            <button type="button" class="btn-secondary"
              >Import Floppy Archive</button
            >
          </div>

          <div class="importer-card">
            <h3 class="importer-name">Yamtrack CSV / Export</h3>
            <p class="importer-meta">
              Ingests legacy Yamtrack media logs and custom tags with
              deterministic checkpointing.
            </p>
            <button type="button" class="btn-secondary"
              >Import Yamtrack CSV</button
            >
          </div>

          <div class="importer-card">
            <h3 class="importer-name">SIMKL JSON Export</h3>
            <p class="importer-meta">
              Ingests anime/movie/TV history preserving multi-provider crosswalk
              IDs (IMDb/Kitsu/MAL).
            </p>
            <button type="button" class="btn-secondary"
              >Import SIMKL Export</button
            >
          </div>

          <div class="importer-card">
            <h3 class="importer-name">Trakt.tv History ZIP</h3>
            <p class="importer-meta">
              Imports Trakt watched history, ratings, watchlist, and custom
              collections.
            </p>
            <button type="button" class="btn-secondary"
              >Import Trakt Archive</button
            >
          </div>
        </div>
      </section>
    {:else if activeTab === "tokens"}
      <section class="section-pane">
        <h2 class="pane-heading">Scoped Personal Access Tokens (PAT)</h2>
        <p class="pane-desc">
          Generate fine-grained bearer credentials with restricted capability
          scopes for media players and webhooks.
        </p>

        <div class="tokens-list">
          {#each tokens as tok}
            <div class="token-row">
              <div>
                <h4 class="token-name">{tok.name}</h4>
                <code class="token-code">{tok.tokenPrefix}</code>
              </div>
              <div class="token-scopes">
                {#each tok.scopes as sc}
                  <span class="scope-chip">{sc}</span>
                {/each}
              </div>
            </div>
          {/each}
        </div>
      </section>
    {:else if activeTab === "a11y"}
      <section class="section-pane">
        <h2 class="pane-heading">
          Accessibility & Neurodiversity (ADHD/AuDHD)
        </h2>
        <p class="pane-desc">
          Certified WCAG 2.2 AAA standards, persistent non-toast status bars,
          and reduced-distraction ergonomic defaults.
        </p>

        <div class="a11y-toggles">
          <div class="toggle-card">
            <div>
              <h4 class="toggle-title">High-Contrast Focus Outlines</h4>
              <p class="toggle-desc">
                Enforces 3px solid Horological Gold focus rings on all
                interactive elements.
              </p>
            </div>
            <span class="active-badge">Active (AAA)</span>
          </div>

          <div class="toggle-card">
            <div>
              <h4 class="toggle-title">Persistent Status Bars</h4>
              <p class="toggle-desc">
                Disables ephemeral disappearing toasts; all system errors remain
                visible in status regions.
              </p>
            </div>
            <span class="active-badge">Enforced</span>
          </div>

          <div class="toggle-card">
            <div>
              <h4 class="toggle-title">Reduced Motion Respect</h4>
              <p class="toggle-desc">
                Automatically disables animations when prefers-reduced-motion is
                signaled.
              </p>
            </div>
            <span class="active-badge">Enforced</span>
          </div>
        </div>
      </section>
    {/if}
  </main>
</div>

<style>
  .settings-container {
    max-width: 960px;
    margin: 0 auto;
    padding: 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 24px;
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

  .settings-tabs {
    display: flex;
    gap: 4px;
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .tab-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    font-size: 0.92rem;
    font-weight: 500;
    color: var(--fasti-text-muted);
    cursor: pointer;
    margin-bottom: -2px;
  }

  .tab-btn.active {
    color: var(--fasti-action-primary);
    border-bottom-color: var(--fasti-action-primary);
    font-weight: 600;
  }

  .settings-body {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
    padding: 24px;
  }

  .pane-heading {
    font-family: var(--fasti-font-display);
    font-size: 1.4rem;
    font-weight: 600;
    margin: 0 0 4px;
    color: var(--fasti-text-primary);
  }

  .pane-desc {
    color: var(--fasti-text-muted);
    font-size: 0.88rem;
    margin: 0 0 20px;
    max-width: 65ch;
  }

  .settings-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.88rem;
    text-align: left;
    margin-bottom: 24px;
  }

  .settings-table th,
  .settings-table td {
    padding: 10px 14px;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }

  .settings-table th {
    background: var(--fasti-surface-archive);
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    text-transform: uppercase;
    color: var(--fasti-text-muted);
  }

  .mono {
    font-family: var(--fasti-font-mono);
  }

  .type-tag {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    text-transform: uppercase;
    padding: 2px 6px;
    background: var(--fasti-surface-archive);
    border-radius: 3px;
  }

  .add-box {
    padding: 16px;
    background: var(--fasti-surface-archive);
    border-radius: 6px;
  }

  .add-title {
    font-size: 0.95rem;
    font-weight: 600;
    margin: 0 0 10px;
  }

  .input-row {
    display: flex;
    gap: 10px;
  }

  .text-input {
    flex: 1;
    height: 38px;
    padding: 8px 12px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    background: var(--fasti-surface-paper);
  }

  .btn-primary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    background: var(--fasti-action-primary);
    color: white;
    font-weight: 600;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }

  .importer-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(380px, 1fr));
    gap: 16px;
  }

  .importer-card {
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    border-radius: 6px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 12px;
  }

  .importer-name {
    font-family: var(--fasti-font-display);
    font-size: 1.15rem;
    margin: 0;
  }

  .importer-meta {
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
    margin: 0;
  }

  .btn-secondary {
    align-self: flex-start;
    padding: 8px 14px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
  }

  .tokens-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .token-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    background: var(--fasti-surface-archive);
    border-radius: 4px;
  }

  .token-name {
    margin: 0 0 2px;
    font-size: 0.95rem;
  }

  .token-code {
    font-family: var(--fasti-font-mono);
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
  }

  .token-scopes {
    display: flex;
    gap: 6px;
  }

  .scope-chip {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    background: color-mix(
      in srgb,
      var(--fasti-action-primary) 15%,
      transparent
    );
    color: var(--fasti-action-primary);
    padding: 2px 6px;
    border-radius: 3px;
  }

  .a11y-toggles {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .toggle-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 14px 18px;
    background: var(--fasti-surface-archive);
    border-radius: 4px;
  }

  .toggle-title {
    margin: 0 0 2px;
    font-size: 0.95rem;
  }

  .toggle-desc {
    margin: 0;
    font-size: 0.82rem;
    color: var(--fasti-text-muted);
  }

  .active-badge {
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    font-weight: 700;
    color: var(--fasti-state-verified);
  }
</style>
