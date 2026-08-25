<script lang="ts">
  import { normalizeBaseUrl, type ConnectionEndpoint } from "@fasti/sdk";
  import {
    IconCheck,
    IconExternalLink,
    IconLock,
    IconNetwork,
    IconServer,
  } from "@tabler/icons-svelte";
  import type { DesktopProblem } from "./setup-types.js";
  import type { ConnectionTestStatus } from "./types.js";

  interface Props {
    endpoint: ConnectionEndpoint;
    onSave: (value: string) => Promise<ConnectionEndpoint>;
    onTest: (value: string) => Promise<ConnectionTestStatus>;
  }

  let { endpoint, onSave, onTest }: Props = $props();
  let draft = $state("");
  let syncedUrl = $state("");
  let busy: "save" | "test" | undefined = $state();
  let status: ConnectionTestStatus | undefined = $state();
  let problem: DesktopProblem | undefined = $state();
  const caddyExample = `fasti.internal {
  reverse_proxy 127.0.0.1:8420
  tls internal
}`;

  $effect(() => {
    if (endpoint.url !== syncedUrl) {
      syncedUrl = endpoint.url;
      draft = endpoint.url;
      status = undefined;
      problem = undefined;
    }
  });

  function asProblem(error: unknown): DesktopProblem {
    if (error !== null && typeof error === "object") {
      const candidate = error as Partial<DesktopProblem>;
      if (
        typeof candidate.code === "string" &&
        typeof candidate.title === "string" &&
        typeof candidate.detail === "string" &&
        typeof candidate.next_action === "string"
      ) {
        return candidate as DesktopProblem;
      }
    }
    return {
      code: "invalid_endpoint",
      title: "The endpoint is not valid",
      detail:
        error instanceof Error ? error.message : "Enter a complete node URL.",
      next_action:
        "Use an HTTP or HTTPS origin, such as http://127.0.0.1:8420 or https://fasti.internal.",
    };
  }

  function normalizedDraft(): string {
    return normalizeBaseUrl(draft.trim()).origin;
  }

  async function save(): Promise<void> {
    if (endpoint.managed || busy !== undefined) return;
    busy = "save";
    status = undefined;
    problem = undefined;
    try {
      const saved = await onSave(normalizedDraft());
      draft = saved.url;
      syncedUrl = saved.url;
    } catch (error) {
      problem = asProblem(error);
    } finally {
      busy = undefined;
    }
  }

  async function testConnection(): Promise<void> {
    if (busy !== undefined) return;
    busy = "test";
    status = undefined;
    problem = undefined;
    try {
      status = await onTest(normalizedDraft());
    } catch (error) {
      problem = asProblem(error);
    } finally {
      busy = undefined;
    }
  }

  function useAlias(alias: string): void {
    if (!endpoint.managed) draft = alias;
  }
</script>

<section class="connection-pane" aria-labelledby="connection-heading">
  <div class="heading-row">
    <div>
      <h2 id="connection-heading">Connection</h2>
      <p>
        Set the node URL that this client uses. The daemon bind address is
        separate.
      </p>
    </div>
    <span class="source" class:managed={endpoint.managed}>
      {endpoint.managed ? "Managed" : endpoint.source}
    </span>
  </div>

  <form
    onsubmit={(event) => {
      event.preventDefault();
      void save();
    }}
  >
    <label for="fasti-endpoint">Node URL</label>
    <div class="input-row">
      <input
        id="fasti-endpoint"
        type="url"
        inputmode="url"
        autocomplete="url"
        bind:value={draft}
        disabled={endpoint.managed}
        aria-describedby="endpoint-help"
      />
      <button type="submit" disabled={endpoint.managed || busy !== undefined}>
        {busy === "save" ? "Saving" : "Save"}
      </button>
      <button
        type="button"
        class="secondary"
        disabled={busy !== undefined}
        onclick={() => void testConnection()}
      >
        {busy === "test" ? "Testing" : "Test connection"}
      </button>
    </div>
    <p id="endpoint-help" class="help">
      Use only the origin. Do not include credentials, a path, a query, or a
      fragment.
    </p>
  </form>

  {#if endpoint.managed}
    <p class="managed-note">
      <IconLock size={18} /> This value comes from the {endpoint.source} and is read-only.
    </p>
  {/if}

  <dl class="facts">
    <div>
      <dt>Effective URL</dt>
      <dd>{endpoint.url}</dd>
    </div>
    <div>
      <dt>Port</dt>
      <dd>
        {new URL(endpoint.url).port ||
          (endpoint.trust === "https" ? "443" : "80")}
      </dd>
    </div>
    <div>
      <dt>Trust</dt>
      <dd>
        {endpoint.trust === "https"
          ? "HTTPS with system certificate trust"
          : "HTTP without certificate validation"}
      </dd>
    </div>
  </dl>

  {#if endpoint.loopbackAliases.length > 0}
    <div class="aliases">
      <h3>Loopback alternatives</h3>
      <p>These names reach the same device in this network namespace.</p>
      <div class="alias-list">
        {#each endpoint.loopbackAliases as alias}
          <button
            type="button"
            class="alias"
            disabled={endpoint.managed}
            onclick={() => useAlias(alias)}>{alias}</button
          >
        {/each}
      </div>
      <p class="help">
        In a container or Android app, localhost refers to that container or
        device.
      </p>
    </div>
  {/if}

  {#if status}
    <div class="result success" role="status">
      <IconCheck size={20} />
      <div>
        <strong>Connected to Fasti {status.version}</strong><span
          >{status.endpoint}</span
        >
      </div>
    </div>
  {/if}

  {#if problem}
    <div class="result error" role="alert">
      <IconNetwork size={20} />
      <div>
        <strong>{problem.title}</strong><span>{problem.detail}</span><span
          >{problem.next_action}</span
        >
      </div>
    </div>
  {/if}

  <details>
    <summary
      ><IconServer size={18} /> Use a private <code>.internal</code> name</summary
    >
    <p>
      Keep <code>fastid</code> on HTTP behind a reverse proxy. Configure the
      proxy for <code>fasti.internal</code>, then trust its CA through the
      operating system.
    </p>
    <pre><code>{caddyExample}</code></pre>
    <p>
      Fasti never imports a CA private key and never disables certificate
      validation.
    </p>
    <a
      href="https://caddyserver.com/docs/quick-starts/reverse-proxy"
      target="_blank"
      rel="noopener">Caddy reverse proxy guide <IconExternalLink size={14} /></a
    >
  </details>
</section>

<style>
  .connection-pane {
    max-width: 76ch;
  }
  .heading-row {
    display: flex;
    align-items: start;
    justify-content: space-between;
    gap: 24px;
    margin-bottom: 28px;
  }
  h2 {
    margin: 0 0 6px;
    font-family: var(--fasti-font-display);
    font-size: 1.75rem;
    color: var(--fasti-text-primary);
  }
  h3 {
    margin: 0 0 4px;
    font-size: 1rem;
    color: var(--fasti-text-primary);
  }
  p {
    margin: 0;
    color: var(--fasti-text-muted);
    line-height: 1.55;
  }
  .source {
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 45%, transparent);
    border-radius: 999px;
    padding: 6px 10px;
    text-transform: capitalize;
    font-size: 0.8rem;
    font-weight: 700;
  }
  .source.managed {
    color: var(--fasti-state-attention);
    border-color: currentColor;
  }
  form {
    margin-bottom: 18px;
  }
  label {
    display: block;
    margin-bottom: 8px;
    color: var(--fasti-text-primary);
    font-weight: 700;
  }
  .input-row {
    display: grid;
    grid-template-columns: minmax(14rem, 1fr) auto auto;
    gap: 8px;
  }
  input,
  button,
  summary {
    min-height: var(--fasti-touch-target-min, 44px);
  }
  input {
    min-width: 0;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 50%, transparent);
    border-radius: 6px;
    padding: 10px 12px;
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }
  input:disabled {
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-muted);
  }
  button {
    border: 1px solid var(--fasti-action-primary);
    border-radius: 6px;
    padding: 9px 14px;
    background: var(--fasti-action-primary);
    color: white;
    font-weight: 700;
    cursor: pointer;
  }
  button.secondary,
  button.alias {
    background: transparent;
    color: var(--fasti-action-primary);
  }
  button:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }
  .help {
    margin-top: 8px;
    font-size: 0.86rem;
  }
  .managed-note {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 20px;
    color: var(--fasti-state-attention);
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    margin: 24px 0;
    border-block: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
  }
  .facts div {
    min-width: 0;
    padding: 14px 12px;
  }
  dt {
    margin-bottom: 5px;
    color: var(--fasti-text-muted);
    font-size: 0.78rem;
    font-weight: 700;
    text-transform: uppercase;
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
    color: var(--fasti-text-primary);
    font-variant-numeric: tabular-nums;
  }
  .aliases {
    margin: 24px 0;
  }
  .alias-list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 12px 0 0;
  }
  button.alias {
    font-family: var(--fasti-font-mono);
    font-size: 0.8rem;
  }
  .result {
    display: flex;
    gap: 10px;
    align-items: start;
    margin: 20px 0;
    padding: 14px;
    border: 1px solid currentColor;
    border-radius: 6px;
  }
  .result div {
    display: grid;
    gap: 3px;
  }
  .result span {
    color: var(--fasti-text-muted);
  }
  .success {
    color: var(--fasti-state-verified);
  }
  .error {
    color: var(--fasti-brand-mark);
  }
  details {
    margin-top: 28px;
    border-block: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    padding: 12px 0;
  }
  summary {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    color: var(--fasti-text-primary);
    font-weight: 700;
  }
  details p {
    margin: 12px 0;
  }
  pre {
    overflow-x: auto;
    padding: 12px;
    background: var(--fasti-surface-archive);
    border-radius: 6px;
  }
  a {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--fasti-action-primary);
    text-underline-offset: 3px;
  }
  @media (max-width: 760px) {
    .heading-row {
      display: grid;
      gap: 12px;
    }
    .source {
      justify-self: start;
    }
    .input-row,
    .facts {
      grid-template-columns: 1fr;
    }
    .facts div + div {
      border-top: 1px solid
        color-mix(in srgb, var(--fasti-text-muted) 24%, transparent);
    }
  }
</style>
