<script lang="ts">
  import { onMount } from "svelte";
  import { IconExternalLink, IconKey, IconRefresh, IconSettings } from "@tabler/icons-svelte";
  import NetworkSettings from "./network-settings.svelte";
  import { hostProblemText } from "./host-problem.js";
  import type {
    NetworkConfiguration,
    ProviderCredentialStatus,
    SaveNetworkConfigurationRequest,
    WorkbenchHost,
  } from "./types.js";

  interface Props {
    host: WorkbenchHost;
  }

  let { host }: Props = $props();

  let active: "network" | "providers" | "system" = $state("network");
  let network = $state<NetworkConfiguration>();
  let networkLoading = $state(false);
  let networkProblem = $state<string>();
  let providers = $state<ProviderCredentialStatus[]>([]);
  let providerLoading = $state(false);
  let providerProblem = $state<string>();
  let providerNotice = $state<string>();
  let editing = $state<Record<string, string>>({});
  let busyProvider = $state<string>();

  async function loadNetwork(): Promise<void> {
    if (networkLoading) return;
    networkLoading = true;
    networkProblem = undefined;
    try {
      network = await host.loadNetworkConfiguration();
    } catch (error) {
      networkProblem = hostProblemText(error, "Fasti could not load network configuration.");
    } finally {
      networkLoading = false;
    }
  }

  async function saveNetwork(
    input: SaveNetworkConfigurationRequest,
  ): Promise<NetworkConfiguration> {
    const saved = await host.saveNetworkConfiguration(input);
    network = saved;
    return saved;
  }

  async function loadProviders(): Promise<void> {
    if (providerLoading) return;
    providerLoading = true;
    providerProblem = undefined;
    try {
      providers = await host.providerCredentialStatus();
    } catch (error) {
      providerProblem = hostProblemText(error, "Fasti could not load provider status.");
    } finally {
      providerLoading = false;
    }
  }

  async function saveProvider(provider: string): Promise<void> {
    const credential = editing[provider]?.trim();
    if (!credential || busyProvider) return;
    busyProvider = provider;
    providerProblem = undefined;
    providerNotice = undefined;
    try {
      providers = await host.saveProviderCredential(provider, credential);
      editing = { ...editing, [provider]: "" };
      providerNotice = "Credential saved in the platform credential store.";
    } catch (error) {
      providerProblem = hostProblemText(error, "Fasti rejected the provider credential.");
    } finally {
      busyProvider = undefined;
    }
  }

  async function deleteProvider(provider: string): Promise<void> {
    if (busyProvider) return;
    busyProvider = provider;
    providerProblem = undefined;
    providerNotice = undefined;
    try {
      providers = await host.deleteProviderCredential(provider);
      providerNotice = "Credential removed from the platform credential store.";
    } catch (error) {
      providerProblem = hostProblemText(error, "Fasti could not remove the provider credential.");
    } finally {
      busyProvider = undefined;
    }
  }

  onMount(() => {
    void Promise.all([loadNetwork(), loadProviders()]);
  });
</script>

<div class="settings-container">
  <header>
    <h1>Settings</h1>
    <p>Only settings with an active host capability are editable here.</p>
  </header>

  <div class="settings-layout">
    <nav aria-label="Settings sections">
      <button type="button" class:active={active === "network"} onclick={() => (active = "network")}>Network</button>
      <button type="button" class:active={active === "providers"} onclick={() => (active = "providers")}>Metadata credentials</button>
      <button type="button" class:active={active === "system"} onclick={() => (active = "system")}>Capability status</button>
    </nav>

    <main>
      {#if active === "network"}
        <NetworkSettings
          configuration={network}
          loading={networkLoading}
          loadProblem={networkProblem}
          onSave={saveNetwork}
          onTest={(endpoint) => host.testEndpointConnection(endpoint)}
          onRetry={() => void loadNetwork()}
        />
      {:else if active === "providers"}
        <section aria-labelledby="provider-settings-title">
          <div class="section-heading">
            <div>
              <h2 id="provider-settings-title">Metadata credentials</h2>
              <p>
                Fasti never reads a stored secret back into this interface. Credential entry is
                available only when the host can write to a protected credential store.
              </p>
            </div>
            <button type="button" class="secondary" onclick={() => void loadProviders()} disabled={providerLoading}>
              <IconRefresh size={18} aria-hidden="true" /> {providerLoading ? "Loading…" : "Refresh"}
            </button>
          </div>

          <div class="provider-list">
            {#each providers as provider (provider.provider)}
              <article class="provider-card">
                <div class="provider-heading">
                  <div>
                    <h3>{provider.label}</h3>
                    <p>
                      {provider.configured
                        ? `Configured from ${provider.source.replace("_", " ")}.`
                        : "No credential is configured."}
                    </p>
                  </div>
                  <a href={provider.docs_url} target="_blank" rel="noopener noreferrer">
                    Documentation <IconExternalLink size={14} aria-hidden="true" />
                  </a>
                </div>

                {#if provider.writable}
                  <form
                    class="credential-form"
                    onsubmit={(event) => {
                      event.preventDefault();
                      void saveProvider(provider.provider);
                    }}
                  >
                    <label for={`provider-${provider.provider}`}>New credential</label>
                    <div>
                      <input
                        id={`provider-${provider.provider}`}
                        type="password"
                        autocomplete="off"
                        value={editing[provider.provider] ?? ""}
                        oninput={(event) =>
                          (editing = {
                            ...editing,
                            [provider.provider]: event.currentTarget.value,
                          })}
                        disabled={busyProvider === provider.provider}
                      />
                      <button
                        type="submit"
                        class="primary"
                        disabled={!editing[provider.provider]?.trim() || Boolean(busyProvider)}
                      >
                        <IconKey size={18} aria-hidden="true" /> Save
                      </button>
                      {#if provider.configured}
                        <button
                          type="button"
                          class="danger"
                          onclick={() => void deleteProvider(provider.provider)}
                          disabled={Boolean(busyProvider)}
                        >
                          Remove
                        </button>
                      {/if}
                    </div>
                  </form>
                {:else}
                  <p class="managed-note">
                    This distribution does not accept a secret for this provider. Use the native
                    or server host when the provider requires protected credentials.
                  </p>
                {/if}
              </article>
            {/each}
          </div>

          {#if providerNotice}<p class="notice" role="status">{providerNotice}</p>{/if}
          {#if providerProblem}<p class="problem" role="alert">{providerProblem}</p>{/if}
        </section>
      {:else}
        <section aria-labelledby="capability-settings-title">
          <h2 id="capability-settings-title">Configuration capability status</h2>
          <p>
            Fasti does not render configuration forms before their host-side validation and storage
            capability exists.
          </p>
          <dl class="status-list">
            <div><dt>Network policy and endpoint</dt><dd>Active</dd></div>
            <div><dt>Protected metadata credentials</dt><dd>Host-dependent</dd></div>
            <div><dt>Scoped external API clients</dt><dd>Managed in Connections on the trusted packaged host</dd></div>
            <div><dt>OIDC administration</dt><dd>Not active</dd></div>
            <div><dt>Apprise notification administration</dt><dd>Not active</dd></div>
            <div><dt>Source-specific importers</dt><dd>Not active</dd></div>
            <div><dt>Native Nuvio pairing</dt><dd>Not active</dd></div>
          </dl>
        </section>
      {/if}
    </main>
  </div>
</div>

<style>
  .settings-container {
    max-width: 1080px;
    margin: 0 auto;
    padding: 32px 24px 64px;
  }

  header {
    margin-bottom: 24px;
    padding-bottom: 16px;
    border-bottom: 2px solid color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
  }

  h1,
  h2,
  h3,
  p {
    margin-top: 0;
  }

  h1,
  h2 {
    font-family: var(--fasti-font-display);
  }

  h1 {
    margin-bottom: 4px;
    font-size: 2.4rem;
  }

  header p,
  section > p,
  .provider-card p {
    color: var(--fasti-text-muted);
  }

  .settings-layout {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: 24px;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  nav button {
    min-height: 44px;
    border: 0;
    border-radius: 5px;
    padding: 9px 12px;
    text-align: left;
    background: transparent;
    color: var(--fasti-text-muted);
    cursor: pointer;
  }

  nav button:hover,
  nav button.active {
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  nav button.active {
    font-weight: 700;
    box-shadow: inset 3px 0 0 var(--fasti-action-primary);
  }

  button:focus-visible,
  a:focus-visible,
  input:focus-visible {
    outline: 3px solid var(--fasti-action-primary);
    outline-offset: 2px;
  }

  .section-heading,
  .provider-heading,
  .credential-form > div {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  .provider-list {
    display: grid;
    gap: 12px;
    margin-top: 20px;
  }

  .provider-card {
    border: 1px solid var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    border-radius: 7px;
    padding: 16px;
    background: var(--fasti-surface-paper);
  }

  .provider-card h3,
  .provider-card p {
    margin-bottom: 4px;
  }

  .provider-heading a {
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  .credential-form {
    margin-top: 14px;
  }

  .credential-form label {
    display: block;
    margin-bottom: 5px;
    font-weight: 650;
  }

  .credential-form > div {
    align-items: center;
  }

  input {
    flex: 1;
    min-width: 0;
    min-height: 44px;
    border: 1px solid var(--fasti-border, currentColor);
    border-radius: 5px;
    padding: 8px 10px;
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  .primary,
  .secondary,
  .danger {
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border-radius: 5px;
    padding: 8px 13px;
    font-weight: 650;
    cursor: pointer;
  }

  .primary {
    border: 1px solid var(--fasti-action-primary);
    background: var(--fasti-action-primary);
    color: var(--fasti-action-contrast, white);
  }

  .secondary {
    border: 1px solid var(--fasti-border, currentColor);
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  .danger {
    border: 1px solid var(--fasti-state-error, #b42318);
    background: transparent;
    color: var(--fasti-state-error, #b42318);
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.62;
  }

  .managed-note {
    margin: 12px 0 0;
  }

  .notice {
    color: var(--fasti-state-verified, #207a42);
    font-weight: 600;
  }

  .problem {
    color: var(--fasti-state-error, #b42318);
    font-weight: 600;
  }

  .status-list {
    display: grid;
    gap: 1px;
    margin: 20px 0 0;
    border: 1px solid var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    border-radius: 7px;
    overflow: hidden;
  }

  .status-list div {
    display: grid;
    grid-template-columns: minmax(180px, 1fr) minmax(220px, 1fr);
    gap: 16px;
    padding: 12px 14px;
    background: var(--fasti-surface-paper);
  }

  .status-list dt {
    font-weight: 650;
  }

  .status-list dd {
    margin: 0;
    color: var(--fasti-text-muted);
  }

  @media (max-width: 47.99rem) {
    .settings-container {
      padding: 24px 16px 48px;
    }

    .settings-layout {
      grid-template-columns: minmax(0, 1fr);
    }

    nav {
      display: grid;
      grid-template-columns: 1fr;
    }

    .section-heading,
    .provider-heading,
    .credential-form > div,
    .status-list div {
      grid-template-columns: 1fr;
      flex-direction: column;
    }

    .credential-form > div,
    input,
    .primary,
    .secondary,
    .danger {
      width: 100%;
    }
  }
</style>
