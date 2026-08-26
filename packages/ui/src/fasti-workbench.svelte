<script lang="ts">
  import { onMount } from "svelte";
  import {
    IconBook2,
    IconChevronRight,
    IconDatabase,
    IconPlugConnected,
    IconSettings,
    IconShieldCheck,
  } from "@tabler/icons-svelte";
  import ConnectionsView from "./connections-view.svelte";
  import RuntimeSettingsView from "./runtime-settings-view.svelte";
  import type { WorkbenchHost } from "./types.js";

  interface Props {
    host: WorkbenchHost;
  }

  type Section = "overview" | "connections" | "settings";

  let { host }: Props = $props();
  let activeSection = $state<Section>("overview");

  const credentialAdministration = $derived(
    Boolean(
      host.listApiClients && host.createApiClient && host.revokeApiClient,
    ),
  );

  function sectionFromPath(): Section {
    if (typeof window === "undefined") return "overview";
    if (window.location.pathname === "/connections") return "connections";
    if (window.location.pathname === "/settings") return "settings";
    return "overview";
  }

  function select(section: Section): void {
    activeSection = section;
    if (typeof window === "undefined") return;
    const path =
      section === "connections"
        ? "/connections"
        : section === "settings"
          ? "/settings"
          : "/";
    if (window.location.pathname !== path) {
      window.history.pushState({}, "", path);
    }
    window.requestAnimationFrame(() =>
      document.getElementById("main-content")?.focus(),
    );
  }

  onMount(() => {
    activeSection = sectionFromPath();
    const sync = () => (activeSection = sectionFromPath());
    window.addEventListener("popstate", sync);
    return () => window.removeEventListener("popstate", sync);
  });
</script>

<div class="workbench-shell">
  <aside class="rail" aria-label="Fasti workbench">
    <button
      type="button"
      class="brand"
      onclick={() => select("overview")}
      aria-label="Fasti overview"
    >
      <span class="brand-mark" aria-hidden="true">F</span>
      <span>Fasti</span>
    </button>

    <nav aria-label="Workbench sections">
      <button
        type="button"
        class:active={activeSection === "overview"}
        aria-current={activeSection === "overview" ? "page" : undefined}
        onclick={() => select("overview")}
      >
        <IconBook2 size={20} aria-hidden="true" />
        <span>Overview</span>
      </button>
      <button
        type="button"
        class:active={activeSection === "connections"}
        aria-current={activeSection === "connections" ? "page" : undefined}
        onclick={() => select("connections")}
      >
        <IconPlugConnected size={20} aria-hidden="true" />
        <span>Connections</span>
      </button>
      <button
        type="button"
        class:active={activeSection === "settings"}
        aria-current={activeSection === "settings" ? "page" : undefined}
        onclick={() => select("settings")}
      >
        <IconSettings size={20} aria-hidden="true" />
        <span>Settings</span>
      </button>
    </nav>
  </aside>

  <main id="main-content" class="main-content" tabindex="-1">
    {#if activeSection === "connections"}
      <ConnectionsView {host} />
    {:else if activeSection === "settings"}
      <RuntimeSettingsView {host} />
    {:else}
      <div class="overview">
        <header class="overview-header">
          <p class="eyebrow">Local workbench</p>
          <h1>Current Fasti capability</h1>
          <p>
            This surface reports only behavior that the active host and local
            API can perform. It does not load sample media or substitute browser
            storage for the Chronicle.
          </p>
        </header>

        <section class="truth-grid" aria-label="Current capability status">
          <article>
            <IconShieldCheck size={28} aria-hidden="true" />
            <div>
              <h2>Durable occurrence ingress</h2>
              <p>
                <strong>Active on the local API.</strong> Scoped bearer clients
                can submit complete consumption occurrences to
                <code>POST /api/v1/observations</code>. Fasti stores evidence,
                applies idempotency, and returns a durable receipt.
              </p>
            </div>
          </article>

          <article>
            <IconDatabase size={28} aria-hidden="true" />
            <div>
              <h2>Media library presentation</h2>
              <p>
                <strong>Not active yet.</strong> Record listing, Chronicle listing,
                metadata editing, collections, ratings, imports, and progress need
                their application and public contracts before this workbench can present
                them as working product state.
              </p>
            </div>
          </article>

          <article>
            <IconPlugConnected size={28} aria-hidden="true" />
            <div>
              <h2>Nuvio pathway</h2>
              <p>
                <strong
                  >Fasti-side occurrence ingress is ready for an authenticated
                  observer.</strong
                >
                Current upstream Nuvio exposes Trakt and SIMKL tracking providers,
                not Fasti. Native Nuvio pairing, progress synchronization, and two-way
                state are therefore not claimed here.
              </p>
              <button
                type="button"
                class="inline-action"
                onclick={() => select("connections")}
              >
                Open Connections <IconChevronRight
                  size={17}
                  aria-hidden="true"
                />
              </button>
            </div>
          </article>

          <article>
            <IconSettings size={28} aria-hidden="true" />
            <div>
              <h2>External client credentials</h2>
              <p>
                {credentialAdministration
                  ? "The trusted packaged host can create, list, and revoke independently scoped API client credentials. Plaintext is returned once and is not stored by the workbench."
                  : "This host does not expose credential administration. Browser distributions fail closed and do not create or persist API bearer secrets."}
              </p>
              <button
                type="button"
                class="inline-action"
                onclick={() => select("connections")}
              >
                Manage API clients <IconChevronRight
                  size={17}
                  aria-hidden="true"
                />
              </button>
            </div>
          </article>
        </section>

        <section class="next-step" aria-labelledby="next-step-title">
          <h2 id="next-step-title">Next implementation gate</h2>
          <p>
            Activate record and Chronicle query/mutation contracts, then bind
            the media UI to those services. Until that gate passes, Fasti keeps
            the richer prototype out of the runtime path instead of presenting
            fake success.
          </p>
        </section>
      </div>
    {/if}
  </main>
</div>

<style>
  .workbench-shell {
    min-height: 100dvh;
    display: grid;
    grid-template-columns: 216px minmax(0, 1fr);
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  .rail {
    position: sticky;
    top: 0;
    height: 100dvh;
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 18px 12px;
    background: var(--fasti-surface-paper);
    border-right: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
  }

  .brand {
    min-height: 48px;
    display: flex;
    align-items: center;
    gap: 10px;
    border: 0;
    background: transparent;
    color: var(--fasti-text-primary);
    padding: 6px 8px;
    font-family: var(--fasti-font-display);
    font-size: 1.35rem;
    font-weight: 750;
    cursor: pointer;
  }

  .brand-mark {
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    border-radius: 7px;
    background: var(--fasti-brand-mark);
    color: white;
    font-family: var(--fasti-font-mono);
    font-size: 1rem;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  nav button {
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: 10px;
    border: 0;
    border-radius: 6px;
    padding: 9px 11px;
    background: transparent;
    color: var(--fasti-text-muted);
    text-align: left;
    cursor: pointer;
  }

  nav button:hover,
  nav button.active {
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  nav button.active {
    box-shadow: inset 3px 0 0 var(--fasti-action-primary);
    font-weight: 700;
  }

  button:focus-visible,
  .main-content:focus-visible {
    outline: 3px solid var(--fasti-action-primary);
    outline-offset: 2px;
  }

  .main-content {
    min-width: 0;
  }

  .overview {
    max-width: 1080px;
    margin: 0 auto;
    padding: 48px 32px 72px;
  }

  .overview-header {
    max-width: 72ch;
    margin-bottom: 32px;
  }

  .eyebrow {
    margin: 0 0 6px;
    color: var(--fasti-brand-mark);
    font-family: var(--fasti-font-mono);
    font-size: 0.78rem;
    font-weight: 750;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  h1,
  h2,
  p {
    margin-top: 0;
  }

  h1,
  h2 {
    font-family: var(--fasti-font-display);
  }

  h1 {
    margin-bottom: 8px;
    font-size: clamp(2rem, 5vw, 3rem);
  }

  .overview-header > p:last-child,
  article p,
  .next-step p {
    color: var(--fasti-text-muted);
    line-height: 1.6;
  }

  .truth-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 18px;
  }

  article {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding: 20px;
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    border-radius: 8px;
    background: var(--fasti-surface-paper);
  }

  article > :global(svg) {
    flex: 0 0 auto;
    color: var(--fasti-action-primary);
  }

  article h2 {
    margin-bottom: 6px;
    font-size: 1.2rem;
  }

  article p {
    margin-bottom: 0;
  }

  article code,
  .next-step code {
    overflow-wrap: anywhere;
  }

  .inline-action {
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-top: 12px;
    border: 0;
    background: transparent;
    color: var(--fasti-action-primary);
    padding: 6px 0;
    font-weight: 700;
    cursor: pointer;
  }

  .next-step {
    margin-top: 24px;
    padding: 20px;
    border: 1px dashed
      var(--fasti-border, color-mix(in srgb, currentColor 25%, transparent));
    border-radius: 8px;
  }

  .next-step h2 {
    margin-bottom: 5px;
    font-size: 1.2rem;
  }

  .next-step p {
    margin-bottom: 0;
  }

  @media (max-width: 56rem) {
    .workbench-shell {
      grid-template-columns: 72px minmax(0, 1fr);
    }

    .rail {
      padding-inline: 8px;
    }

    .brand {
      justify-content: center;
    }

    .brand > span:last-child,
    nav button span {
      position: absolute;
      width: 1px;
      height: 1px;
      padding: 0;
      margin: -1px;
      overflow: hidden;
      clip: rect(0, 0, 0, 0);
      white-space: nowrap;
      border: 0;
    }

    nav button {
      justify-content: center;
    }

    .overview {
      padding: 32px 20px 56px;
    }

    .truth-grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  @media (max-width: 35rem) {
    .workbench-shell {
      display: block;
      padding-bottom: 72px;
    }

    .rail {
      position: fixed;
      inset: auto 0 0 0;
      z-index: 100;
      width: auto;
      height: 64px;
      flex-direction: row;
      align-items: center;
      padding: 6px 8px;
      border-right: 0;
      border-top: 1px solid
        var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    }

    .brand {
      display: none;
    }

    nav {
      width: 100%;
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 4px;
    }

    nav button {
      min-height: 52px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    * {
      scroll-behavior: auto !important;
    }
  }
</style>
