<script lang="ts">
  import type {
    EndpointConnectionStatus,
    NetworkClass,
    NetworkConfiguration,
    OutboundAccessPolicy,
    SaveNetworkConfigurationRequest,
  } from "./types.js";
  import { hostProblemText } from "./host-problem.js";

  interface Props {
    scope: "client" | "node";
    configuration?: NetworkConfiguration;
    loading?: boolean;
    loadProblem?: string;
    onSave: (
      input: SaveNetworkConfigurationRequest,
    ) => Promise<NetworkConfiguration>;
    onTest: (endpoint: string) => Promise<EndpointConnectionStatus>;
    onRetry?: () => void;
  }

  const NETWORK_CLASSES: readonly NetworkClass[] = [
    "public",
    "loopback",
    "private",
    "link_local",
    "multicast",
    "unspecified",
    "documentation",
    "reserved",
  ];

  let {
    scope,
    configuration,
    loading = false,
    loadProblem,
    onSave,
    onTest,
    onRetry,
  }: Props = $props();
  let draft: SaveNetworkConfigurationRequest | undefined = $state();
  let loadedConfiguration: NetworkConfiguration | undefined;
  let busy: "save" | "test" | undefined = $state();
  let notice = $state("");
  let problem = $state("");
  const clientOnly = $derived(scope === "client");

  $effect(() => {
    if (!configuration || configuration === loadedConfiguration) return;
    loadedConfiguration = configuration;
    draft = {
      service_url: configuration.connection.service_url.value,
      public_url: configuration.connection.public_url.value,
      outbound_policy: copyPolicy(configuration.outbound_policy),
    };
  });

  function copyPolicy(policy: OutboundAccessPolicy): OutboundAccessPolicy {
    return {
      allow_providers: [...policy.allow_providers],
      deny_providers: [...policy.deny_providers],
      allow_capabilities: [...policy.allow_capabilities],
      deny_capabilities: [...policy.deny_capabilities],
      allow_hosts: [...policy.allow_hosts],
      deny_hosts: [...policy.deny_hosts],
      allow_networks: [...policy.allow_networks],
      deny_networks: [...policy.deny_networks],
    };
  }

  function listText(values: readonly string[]): string {
    return values.join("\n");
  }

  function parseList(value: string): string[] {
    return value
      .split(/[\n,]/)
      .map((item) => item.trim())
      .filter(Boolean);
  }

  function updateList(
    key: keyof Pick<
      OutboundAccessPolicy,
      | "allow_providers"
      | "deny_providers"
      | "allow_capabilities"
      | "deny_capabilities"
      | "allow_hosts"
      | "deny_hosts"
    >,
    value: string,
  ): void {
    if (!draft) return;
    draft = {
      ...draft,
      outbound_policy: {
        ...draft.outbound_policy,
        [key]: parseList(value),
      },
    };
  }

  function toggleNetwork(
    key: "allow_networks" | "deny_networks",
    network: NetworkClass,
    checked: boolean,
  ): void {
    if (!draft) return;
    const current = draft.outbound_policy[key];
    draft = {
      ...draft,
      outbound_policy: {
        ...draft.outbound_policy,
        [key]: checked
          ? [...new Set([...current, network])]
          : current.filter((item) => item !== network),
      },
    };
  }

  async function save(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (!draft || busy) return;
    busy = "save";
    notice = "";
    problem = "";
    try {
      await onSave({
        ...draft,
        public_url: draft.public_url?.trim() || null,
      });
      notice = "Settings saved.";
    } catch (error) {
      problem = hostProblemText(
        error,
        "The trusted desktop host rejected this request.",
      );
    } finally {
      busy = undefined;
    }
  }

  async function testConnection(): Promise<void> {
    if (!draft || busy) return;
    busy = "test";
    notice = "";
    problem = "";
    try {
      const result = await onTest(draft.service_url);
      notice = `Connected to ${result.endpoint} with ${result.scheme.toUpperCase()}. Service ${result.version} is ${result.status}.`;
    } catch (error) {
      problem = hostProblemText(
        error,
        "The trusted desktop host rejected this request.",
      );
    } finally {
      busy = undefined;
    }
  }
</script>

{#snippet outboundPolicyFields(policy: OutboundAccessPolicy)}
  <fieldset
    class="settings-fieldset policy-fieldset"
    disabled={!!busy || clientOnly}
    aria-describedby={clientOnly ? "outbound-policy-client-note" : undefined}
  >
    <legend>Provider outbound access</legend>
    <p class="help">
      One exact value per line. A provider manifest is the maximum grant. Allow
      lists can only narrow access. Deny lists always win. Blank allow lists
      keep the manifest limit.
    </p>
    <div class="policy-grid">
      {#each [["allow_providers", "Allowed providers"], ["deny_providers", "Denied providers"], ["allow_capabilities", "Allowed capabilities"], ["deny_capabilities", "Denied capabilities"], ["allow_hosts", "Allowed hosts"], ["deny_hosts", "Denied hosts"]] as item}
        <label class="form-label">
          {item[1]}
          <textarea
            class="form-control"
            rows="3"
            value={listText(
              policy[item[0] as keyof OutboundAccessPolicy] as string[],
            )}
            oninput={(event) =>
              updateList(
                item[0] as Parameters<typeof updateList>[0],
                event.currentTarget.value,
              )}
            spellcheck="false"></textarea>
        </label>
      {/each}
    </div>

    <div class="network-grid">
      <fieldset>
        <legend>Allowed network classes</legend>
        <p class="help">Leave all clear to keep the provider manifest limit.</p>
        {#each NETWORK_CLASSES as network}
          <label class="check-label">
            <input
              type="checkbox"
              class="form-check-input"
              checked={policy.allow_networks.includes(network)}
              onchange={(event) =>
                toggleNetwork(
                  "allow_networks",
                  network,
                  event.currentTarget.checked,
                )}
            />
            {network.replaceAll("_", " ")}
          </label>
        {/each}
      </fieldset>
      <fieldset>
        <legend>Denied network classes</legend>
        {#each NETWORK_CLASSES as network}
          <label class="check-label">
            <input
              type="checkbox"
              class="form-check-input"
              checked={policy.deny_networks.includes(network)}
              onchange={(event) =>
                toggleNetwork(
                  "deny_networks",
                  network,
                  event.currentTarget.checked,
                )}
            />
            {network.replaceAll("_", " ")}
          </label>
        {/each}
      </fieldset>
    </div>
  </fieldset>
{/snippet}

<section class="section-pane" aria-labelledby="advanced-settings-title">
  <h2 id="advanced-settings-title" class="pane-title" tabindex="-1">
    Advanced network access
  </h2>
  <p class="pane-desc">
    {clientOnly
      ? "Choose the service endpoint used by this browser. Node public URL and outbound policy remain visible but require a trusted host."
      : "Set the service address, public URL, and outbound access policy. Managed environment and build values are read-only."}
  </p>

  {#if loading}
    <p role="status">Loading trusted settings…</p>
  {:else if loadProblem}
    <p class="problem" role="alert">{loadProblem}</p>
    {#if onRetry}
      <button
        id="network-retry"
        type="button"
        class="btn btn-outline-secondary"
        onclick={onRetry}
      >
        Retry host connection
      </button>
    {/if}
  {:else if !configuration || !draft}
    <p class="problem" role="alert">
      The trusted desktop host did not return network settings.
    </p>
  {:else}
    <form onsubmit={save}>
      <fieldset class="settings-fieldset" disabled={!!busy}>
        <legend>Connection</legend>
        <div class="fields two-columns">
          <label class="form-label">
            Service URL
            <input
              type="url"
              class="form-control"
              required
              bind:value={draft.service_url}
              disabled={configuration.connection.service_url.managed}
              aria-describedby="service-url-help"
            />
            <span class="source"
              >{configuration.connection.service_url.source}</span
            >
          </label>
          <label class="form-label">
            Public URL (optional)
            <input
              type="url"
              class="form-control"
              bind:value={draft.public_url}
              disabled={clientOnly ||
                configuration.connection.public_url.managed}
              placeholder="https://fasti.example.internal"
              aria-describedby="public-url-help"
            />
            <span class="source"
              >{clientOnly
                ? "node host required"
                : configuration.connection.public_url.source}</span
            >
          </label>
        </div>
        <p id="service-url-help" class="help">
          HTTP is accepted only for loopback. HTTPS uses the platform trust
          store. For a .internal name, configure DNS and install the CA in the
          operating system.
        </p>
        <p id="public-url-help" class="help">
          Use an HTTPS URL without a port when a separate reverse proxy owns
          port 443. This value does not expose the local API.
        </p>

        <p class="help">
          Include a custom port in the service URL. For IPv4 loopback, localhost
          and 127.0.0.1 are interchangeable client aliases. IPv6 [::1] is
          separate. The daemon or container owns its listener and safe
          port-collision recovery.
        </p>

        <div class="actions">
          <button type="submit" class="btn btn-primary" disabled={!!busy}>
            {busy === "save"
              ? "Saving…"
              : clientOnly
                ? "Save service URL"
                : "Save network settings"}
          </button>
          <button
            type="button"
            class="btn btn-outline-secondary"
            disabled={!!busy}
            onclick={testConnection}
          >
            {busy === "test" ? "Testing…" : "Test service URL"}
          </button>
        </div>
      </fieldset>

      {#if notice}<p class="notice" role="status">{notice}</p>{/if}
      {#if problem}<p class="problem" role="alert">{problem}</p>{/if}

      {#if clientOnly}
        <details class="managed-policy card">
          <summary class="card-header">
            <span>Provider outbound access</span>
            <span class="badge managed-policy-badge">Node host required</span>
          </summary>
          <div class="card-body">
            <p id="outbound-policy-client-note" class="managed-note">
              This browser cannot read or change the node's provider outbound
              policy. Use the trusted desktop or server host to edit these
              fields.
            </p>
            {@render outboundPolicyFields(draft.outbound_policy)}
          </div>
        </details>
      {:else}
        {@render outboundPolicyFields(draft.outbound_policy)}
      {/if}
    </form>
  {/if}
</section>

<style>
  form,
  fieldset,
  .fields,
  .policy-grid,
  .network-grid {
    display: grid;
    gap: 16px;
  }

  .settings-fieldset,
  .network-grid > fieldset {
    min-width: 0;
    margin: 0;
    padding: 20px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 28%, transparent);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
  }

  legend {
    padding: 0 6px;
    font-weight: 700;
  }

  label {
    display: grid;
    gap: 6px;
    min-width: 0;
    font-weight: 600;
  }

  input,
  textarea,
  button {
    min-height: 44px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 35%, transparent);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  input,
  textarea {
    padding: 10px 12px;
  }

  textarea {
    resize: vertical;
    font-family: var(--fasti-font-mono);
    font-weight: 400;
  }

  button {
    padding: 10px 16px;
    font-weight: 700;
    cursor: pointer;
  }

  button:disabled,
  input:disabled,
  textarea:disabled {
    cursor: not-allowed;
    opacity: 0.68;
  }

  .btn-primary:disabled {
    opacity: 1;
  }

  :is(input, textarea, button):focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }

  .two-columns,
  .policy-grid,
  .network-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .source {
    width: fit-content;
    font-family: var(--fasti-font-mono);
    font-size: 0.74rem;
    font-weight: 400;
    color: var(--fasti-text-muted);
    text-transform: uppercase;
  }

  .help,
  .managed-note,
  .notice,
  .problem {
    margin: 0;
    color: var(--fasti-text-muted);
    font-size: 0.86rem;
  }

  .pane-desc,
  .help,
  .managed-note {
    max-width: 75ch;
  }

  .check-label {
    display: flex;
    align-items: center;
    min-height: 44px;
    font-weight: 400;
    text-transform: capitalize;
  }

  .check-label input {
    min-height: auto;
    width: 20px;
    height: 20px;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }

  .notice {
    color: var(--fasti-state-verified);
  }

  .problem {
    color: var(--fasti-state-error, #b42318);
  }

  .managed-policy summary {
    min-height: 44px;
    display: list-item;
    padding-block: 11px;
    cursor: pointer;
    font-weight: 700;
  }

  .managed-policy summary .badge {
    float: inline-end;
    margin-inline-start: 12px;
  }

  .managed-policy-badge {
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  .managed-policy summary:focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }

  .managed-policy .card-body {
    display: grid;
    gap: 16px;
  }

  .managed-policy .policy-fieldset {
    padding: 0;
    border: 0;
  }

  @media (max-width: 48rem) {
    .two-columns,
    .policy-grid,
    .network-grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
