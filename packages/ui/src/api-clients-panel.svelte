<script lang="ts">
  import { onMount, tick } from "svelte";
  import {
    IconCheck,
    IconCopy,
    IconEye,
    IconEyeOff,
    IconKey,
    IconRefresh,
    IconShieldCheck,
    IconTrash,
  } from "@tabler/icons-svelte";
  import type {
    ApiClientCredentialSummary,
    CreatedApiClientCredential,
    WorkbenchHost,
  } from "./types.js";

  interface Props {
    host?: WorkbenchHost;
  }

  let { host }: Props = $props();

  let clients = $state<ApiClientCredentialSummary[]>([]);
  let loading = $state(false);
  let creating = $state(false);
  let revoking = $state<string>();
  let problem = $state<string>();
  let oneTimeCredential = $state<CreatedApiClientCredential>();
  let copied = $state(false);
  let revealCredential = $state(false);
  let pendingRevoke = $state<string>();
  let secretNotice = $state<HTMLDivElement>();

  const canManage = $derived(
    Boolean(
      host?.listApiClients && host?.createApiClient && host?.revokeApiClient,
    ),
  );

  async function load(): Promise<void> {
    if (!host?.listApiClients || loading) return;
    loading = true;
    problem = undefined;
    try {
      clients = await host.listApiClients();
    } catch (error) {
      problem =
        error instanceof Error
          ? error.message
          : "Fasti could not load API clients.";
    } finally {
      loading = false;
    }
  }

  async function createObserver(): Promise<void> {
    if (!host?.createApiClient || creating) return;
    creating = true;
    problem = undefined;
    oneTimeCredential = undefined;
    copied = false;
    revealCredential = false;
    pendingRevoke = undefined;
    try {
      const created = await host.createApiClient(["observation_accept"]);
      oneTimeCredential = created;
      await load();
      await tick();
      secretNotice?.focus();
    } catch (error) {
      problem =
        error instanceof Error
          ? error.message
          : "Fasti could not create an API client.";
    } finally {
      creating = false;
    }
  }

  async function revoke(credentialId: string): Promise<void> {
    if (!host?.revokeApiClient || revoking) return;
    revoking = credentialId;
    problem = undefined;
    try {
      clients = await host.revokeApiClient(credentialId);
      pendingRevoke = undefined;
      if (oneTimeCredential?.credential_id === credentialId) {
        oneTimeCredential = undefined;
        revealCredential = false;
      }
    } catch (error) {
      problem =
        error instanceof Error
          ? error.message
          : "Fasti could not revoke the API client.";
    } finally {
      revoking = undefined;
    }
  }

  async function copyCredential(): Promise<void> {
    if (!oneTimeCredential) return;
    try {
      await navigator.clipboard.writeText(oneTimeCredential.credential);
      copied = true;
      window.setTimeout(() => (copied = false), 1800);
    } catch {
      problem =
        "Clipboard access was denied. Reveal the one-time credential and copy it manually.";
    }
  }

  function closeSecret(): void {
    oneTimeCredential = undefined;
    revealCredential = false;
    copied = false;
  }

  onMount(() => {
    void load();
  });
</script>

<section class="api-client-panel" aria-labelledby="api-client-title">
  <div class="panel-heading">
    <div>
      <h2 id="api-client-title">API clients</h2>
      <p>
        Create a separate, revocable credential for each external observer.
        Fasti stores only a digest and the granted scopes.
      </p>
    </div>
    {#if canManage}
      <button
        type="button"
        class="secondary-action"
        onclick={() => void load()}
        disabled={loading}
      >
        <IconRefresh size={18} aria-hidden="true" />
        {loading ? "Loading…" : "Refresh"}
      </button>
    {/if}
  </div>

  <div class="truth-card" role="status">
    <IconShieldCheck size={24} aria-hidden="true" />
    <div>
      <strong>Current ingress contract</strong>
      <p>
        <code>POST /api/v1/observations</code> accepts authenticated durable consumption
        occurrences on the local Fasti API. Safe retries use the same source event
        identity and return the prior receipt. Partial progress is rejected until
        the separate progress capability is active.
      </p>
      <p>
        Native Nuvio pairing and two-way synchronization are not active. Current
        upstream Nuvio exposes Trakt and SIMKL tracking providers; a Fasti
        provider still needs an upstream client integration before Nuvio can
        send these requests directly.
      </p>
    </div>
  </div>

  {#if !canManage}
    <div class="unavailable" role="status">
      <IconKey size={22} aria-hidden="true" />
      <div>
        <strong>Credential administration is not available in this host.</strong
        >
        <p>
          Use the trusted packaged Fasti host. The browser workbench does not
          create, receive, or persist bearer credentials.
        </p>
      </div>
    </div>
  {:else}
    <div class="create-row">
      <div>
        <strong>Observation client</strong>
        <p>
          Grants only <code>observation_accept</code>. Use one credential per
          device or adapter so it can be revoked independently.
        </p>
      </div>
      <button
        type="button"
        class="primary-action"
        onclick={() => void createObserver()}
        disabled={creating}
      >
        <IconKey size={18} aria-hidden="true" />
        {creating ? "Creating…" : "Create credential"}
      </button>
    </div>

    {#if oneTimeCredential}
      <div
        class="one-time-secret"
        aria-live="polite"
        tabindex="-1"
        bind:this={secretNotice}
      >
        <div>
          <strong>Copy this credential now.</strong>
          <p>
            Fasti will not return this plaintext value again. It is masked by
            default to reduce accidental exposure in screen sharing and
            screenshots. Closing this notice removes it from the workbench
            memory.
          </p>
          <code
            aria-label={revealCredential
              ? "One-time bearer credential"
              : "One-time bearer credential masked"}
          >
            {revealCredential
              ? oneTimeCredential.credential
              : "••••••••••••••••••••••••••••••••"}
          </code>
        </div>
        <div class="secret-actions">
          <button
            type="button"
            class="primary-action"
            onclick={() => void copyCredential()}
          >
            {#if copied}
              <IconCheck size={18} aria-hidden="true" /> Copied
            {:else}
              <IconCopy size={18} aria-hidden="true" /> Copy
            {/if}
          </button>
          <button
            type="button"
            class="secondary-action"
            aria-pressed={revealCredential}
            onclick={() => (revealCredential = !revealCredential)}
          >
            {#if revealCredential}
              <IconEyeOff size={18} aria-hidden="true" /> Hide
            {:else}
              <IconEye size={18} aria-hidden="true" /> Reveal
            {/if}
          </button>
          <button type="button" class="secondary-action" onclick={closeSecret}
            >Done</button
          >
        </div>
      </div>
    {/if}

    {#if clients.length === 0 && !loading}
      <div class="empty-state" role="status">
        <strong>No external API clients</strong>
        <p>
          Your local administrator credential is not shown here. Create a scoped
          client only when an integration needs it.
        </p>
      </div>
    {:else if clients.length > 0}
      <div class="table-wrap">
        <table>
          <caption class="visually-hidden"
            >External Fasti API client credentials</caption
          >
          <thead>
            <tr>
              <th scope="col">Client</th>
              <th scope="col">Scopes</th>
              <th scope="col">Created</th>
              <th scope="col">State</th>
              <th scope="col"><span class="visually-hidden">Actions</span></th>
            </tr>
          </thead>
          <tbody>
            {#each clients as client (client.credential_id)}
              <tr>
                <td>
                  <code>{client.client_id}</code>
                  <span class="credential-id"
                    >Credential {client.credential_id}</span
                  >
                </td>
                <td>
                  <div class="scope-list">
                    {#each client.scopes as scope}
                      <code>{scope}</code>
                    {/each}
                  </div>
                </td>
                <td>{new Date(client.created_at).toLocaleString()}</td>
                <td>{client.active ? "Active" : "Revoked"}</td>
                <td>
                  {#if client.active && pendingRevoke === client.credential_id}
                    <div
                      class="confirm-actions"
                      role="group"
                      aria-label={`Confirm revocation for API client ${client.client_id}`}
                    >
                      <button
                        type="button"
                        class="danger-action"
                        disabled={Boolean(revoking)}
                        onclick={() => void revoke(client.credential_id)}
                      >
                        {revoking === client.credential_id
                          ? "Revoking…"
                          : "Confirm revoke"}
                      </button>
                      <button
                        type="button"
                        class="secondary-action"
                        disabled={Boolean(revoking)}
                        onclick={() => (pendingRevoke = undefined)}
                      >
                        Cancel
                      </button>
                    </div>
                  {:else if client.active}
                    <button
                      type="button"
                      class="danger-action"
                      disabled={Boolean(revoking)}
                      onclick={() => (pendingRevoke = client.credential_id)}
                      aria-label={`Revoke API client ${client.client_id}`}
                    >
                      <IconTrash size={18} aria-hidden="true" /> Revoke
                    </button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/if}

  {#if problem}
    <p class="problem" role="alert">{problem}</p>
  {/if}
</section>

<style>
  .api-client-panel {
    display: grid;
    gap: 20px;
  }

  .panel-heading,
  .create-row,
  .one-time-secret {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 20px;
  }

  h2,
  p {
    margin-top: 0;
  }

  h2 {
    margin-bottom: 4px;
    font-family: var(--fasti-font-display);
  }

  .panel-heading p,
  .create-row p,
  .truth-card p,
  .unavailable p,
  .empty-state p,
  .one-time-secret p {
    color: var(--fasti-text-muted);
    margin-bottom: 0;
    line-height: 1.5;
  }

  .truth-card,
  .unavailable,
  .create-row,
  .one-time-secret,
  .empty-state {
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    border-radius: calc(8px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-surface-paper);
    padding: 18px;
  }

  .truth-card,
  .unavailable {
    display: flex;
    align-items: flex-start;
    gap: 14px;
  }

  .truth-card p + p {
    margin-top: 8px;
  }

  .one-time-secret {
    border-color: var(--fasti-state-attention, #b26a00);
  }

  .one-time-secret:focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }

  .one-time-secret code {
    display: block;
    max-width: 42rem;
    margin-top: 10px;
    padding: 10px;
    overflow-wrap: anywhere;
    background: var(--fasti-surface-archive);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
  }

  .secret-actions,
  .scope-list,
  .confirm-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .primary-action,
  .secondary-action,
  .danger-action {
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    border-radius: calc(5px * var(--tblr-border-radius-scale, 1));
    padding: 8px 14px;
    font-weight: 650;
    cursor: pointer;
  }

  .primary-action {
    border: 1px solid var(--fasti-action-primary);
    background: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
  }

  .secondary-action {
    border: 1px solid var(--fasti-border, currentColor);
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  .danger-action {
    border: 1px solid var(--fasti-state-error, #b42318);
    background: transparent;
    color: var(--fasti-state-error, #b42318);
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.62;
  }

  button:focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }

  .table-wrap {
    overflow-x: auto;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th,
  td {
    text-align: left;
    padding: 12px;
    border-bottom: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 16%, transparent));
    vertical-align: top;
  }

  .credential-id {
    display: block;
    margin-top: 4px;
    color: var(--fasti-text-muted);
    font-size: 0.76rem;
  }

  .scope-list code {
    padding: 2px 5px;
    background: var(--fasti-surface-archive);
    border-radius: calc(3px * var(--tblr-border-radius-scale, 1));
  }

  .problem {
    margin: 0;
    color: var(--fasti-state-error, #b42318);
    font-weight: 600;
  }

  @media (max-width: 47.99rem) {
    .panel-heading,
    .create-row,
    .one-time-secret {
      flex-direction: column;
    }

    .primary-action,
    .secondary-action,
    .danger-action {
      width: 100%;
    }
  }
</style>
