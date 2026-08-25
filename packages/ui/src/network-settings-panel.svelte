<script lang="ts">
  import type { ConnectionEndpoint } from "@fasti/sdk";

  let {
    endpoint,
    publicEndpoint,
    portFallback,
  }: {
    endpoint: ConnectionEndpoint;
    publicEndpoint?: ConnectionEndpoint;
    portFallback: "auto" | "fail";
  } = $props();

  const sourceLabels = {
    default: "Default",
    saved: "Saved in the app",
    environment: "Environment managed",
    build: "Build managed",
  } as const;
</script>

<section
  id="network-settings"
  class="settings-panel"
  aria-labelledby="network-settings-title"
>
  <h2 id="network-settings-title">Network settings</h2>
  <p class="settings-intro">
    The service, public address, and listener remain separate. This diagnostic
    build shows the active connection values. It does not change daemon
    settings.
  </p>

  <dl>
    <div>
      <dt>Service URL</dt>
      <dd>{endpoint.url}</dd>
    </div>
    <div>
      <dt>Service port</dt>
      <dd>{endpoint.port}</dd>
    </div>
    <div>
      <dt>Value source</dt>
      <dd>{sourceLabels[endpoint.source]}</dd>
    </div>
    <div>
      <dt>Transport trust</dt>
      <dd>
        {endpoint.scheme === "https"
          ? "HTTPS with the system CA store"
          : "HTTP on loopback only"}
      </dd>
    </div>
    <div>
      <dt>Public URL</dt>
      <dd>{publicEndpoint?.url ?? "Not configured"}</dd>
    </div>
    <div>
      <dt>Port recovery</dt>
      <dd>
        {portFallback === "auto"
          ? "Automatic on the same loopback address"
          : "Fail if the preferred port is occupied"}
      </dd>
    </div>
  </dl>

  {#if endpoint.loopbackAliases.length > 0}
    <div class="aliases" aria-labelledby="loopback-aliases-title">
      <h3 id="loopback-aliases-title">Loopback alternatives</h3>
      <p>These addresses refer to the same local machine and port.</p>
      <ul>
        {#each endpoint.loopbackAliases as alias}
          <li><code>{alias}</code></li>
        {/each}
      </ul>
    </div>
  {/if}

  <p class="trust-note">
    A <code>.internal</code> address needs local DNS and a certificate trusted by
    the operating system. Fasti does not install a CA or bypass certificate validation.
  </p>
</section>

<style>
  .settings-panel {
    width: 100%;
    max-width: 44rem;
    min-width: 0;
    margin-block-start: clamp(40px, 7vw, 72px);
    border-block: 1px solid var(--fasti-border);
    padding-block: clamp(28px, 5vw, 48px);
  }

  h2,
  h3,
  p {
    overflow-wrap: anywhere;
  }

  h2,
  h3 {
    margin-block-start: 0;
    color: var(--fasti-foreground);
    font-family: var(--fasti-font-display);
    font-weight: 600;
  }

  h2 {
    margin-block-end: 12px;
    font-size: clamp(1.75rem, 5vw, 2.75rem);
    letter-spacing: -0.025em;
  }

  h3 {
    margin-block-end: 8px;
    font-size: 1.25rem;
  }

  p {
    max-width: 42rem;
    margin-block: 0;
    line-height: 1.65;
  }

  .settings-intro,
  .aliases > p,
  .trust-note {
    color: var(--fasti-muted);
  }

  dl {
    margin: 28px 0 0;
    display: grid;
    gap: 1px;
    background: var(--fasti-border);
  }

  dl > div {
    min-width: 0;
    padding: 12px 16px;
    display: grid;
    grid-template-columns: minmax(8rem, 0.45fr) minmax(0, 1fr);
    gap: 16px;
    background: var(--fasti-panel);
  }

  dt {
    color: var(--fasti-muted);
    font-weight: 600;
  }

  dd {
    min-width: 0;
    margin: 0;
    font-family: var(--fasti-font-mono);
    overflow-wrap: anywhere;
  }

  .aliases,
  .trust-note {
    margin-block-start: 28px;
  }

  ul {
    margin: 16px 0 0;
    padding-inline-start: 1.5rem;
  }

  li + li {
    margin-block-start: 8px;
  }

  code {
    color: var(--fasti-foreground);
    font-family: var(--fasti-font-mono);
    overflow-wrap: anywhere;
  }

  @media (max-width: 420px) {
    dl > div {
      grid-template-columns: 1fr;
      gap: 4px;
    }
  }
</style>
