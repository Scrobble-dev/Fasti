<script lang="ts">
  import {
    SetupPanel,
    FastiWorkbench,
    type DesktopProblem,
    type OutboundAccessPolicy,
    type ProviderCandidate,
    type ProviderCredentialStatus,
    type SetupViewState,
  } from "@fasti/ui";
  import {
    FastiClient,
    connectionEndpoint as defineConnectionEndpoint,
    type ConnectionEndpoint,
  } from "@fasti/sdk";
  import { onMount } from "svelte";

  interface SetupStatus {
    readonly phase: "needs_setup" | "ready";
    readonly proof_cleanup_pending: boolean;
  }

  let viewState: SetupViewState = $state("loading");
  let problem: DesktopProblem | undefined = $state();
  let cleanupPending = $state(false);
  let isTauri = $state(false);
  let providerCredentials = $state<ProviderCredentialStatus[]>([
    {
      provider: "google-books",
      label: "Google Books",
      configured: false,
      source: "none",
      writable: false,
      docs_url: "https://developers.google.com/books/docs/v1/using",
    },
  ]);
  const managedApiUrl = import.meta.env.VITE_FASTI_API_URL as string;
  const apiUrlManaged = import.meta.env.VITE_FASTI_API_URL_MANAGED as boolean;
  let connection = $state<ConnectionEndpoint>(initialConnection());

  function initialConnection(): ConnectionEndpoint {
    if (apiUrlManaged) {
      return defineConnectionEndpoint(managedApiUrl, "build");
    }
    try {
      const saved = window.localStorage.getItem("fasti.client-endpoint.v1");
      return defineConnectionEndpoint(
        saved ?? managedApiUrl,
        saved ? "saved" : "default",
      );
    } catch {
      return defineConnectionEndpoint(managedApiUrl, "default");
    }
  }

  async function saveConnection(value: string): Promise<ConnectionEndpoint> {
    if (connection.managed) {
      throw {
        code: "managed_endpoint",
        title: "The endpoint is managed",
        detail: "This build supplies the node URL.",
        next_action:
          "Change FASTI_API_URL in the build environment, then rebuild the client.",
      } satisfies DesktopProblem;
    }
    const saved = defineConnectionEndpoint(value, "saved");
    window.localStorage.setItem("fasti.client-endpoint.v1", saved.url);
    connection = saved;
    return saved;
  }

  async function testConnection(value: string) {
    if (isTauri) {
      const { invoke } = await import("@tauri-apps/api/core");
      return invoke<{ endpoint: string; status: "healthy"; version: string }>(
        "test_endpoint_connection",
        { endpoint: value },
      );
    }
    const health = await new FastiClient({
      baseUrl: window.location.origin,
      timeoutMs: 5_000,
      retryPolicy: { maxAttempts: 1 },
    }).health();
    return { endpoint: value, status: health.status, version: health.version };
  }

  async function loadProviderCredentials(): Promise<void> {
    if (!isTauri) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      providerCredentials = await invoke<ProviderCredentialStatus[]>(
        "provider_credential_status",
      );
    } catch (error) {
      console.warn("[Fasti UI] Provider credential status unavailable:", error);
    }
  }

  async function saveProviderKey(
    provider: string,
    key: string | null,
  ): Promise<void> {
    if (!isTauri) {
      throw {
        code: "desktop_provider_required",
        title: "Open Fasti Desktop",
        detail: "The browser cannot read or write provider credentials.",
        next_action: "Open Fasti Desktop, then manage the provider key.",
      } satisfies DesktopProblem;
    }
    const { invoke } = await import("@tauri-apps/api/core");
    providerCredentials = await invoke<ProviderCredentialStatus[]>(
      "save_provider_key",
      { provider, key },
    );
  }

  async function searchProvider(
    provider: string,
    query: string,
    policy: OutboundAccessPolicy,
  ): Promise<ProviderCandidate[]> {
    if (!isTauri) {
      throw {
        code: "desktop_provider_required",
        title: "Open Fasti Desktop",
        detail: "Provider search is not exposed to an unauthenticated browser.",
        next_action: "Open Fasti Desktop, then run the search again.",
      } satisfies DesktopProblem;
    }
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<ProviderCandidate[]>("search_provider", {
      provider,
      query,
      policy,
    });
  }

  function applyStatus(status: SetupStatus): void {
    console.info("[Fasti UI] Host status:", status);
    viewState = status.phase;
    cleanupPending = status.proof_cleanup_pending;
    problem = undefined;
  }

  function applyProblem(error: unknown): void {
    console.warn("[Fasti UI] Host inspection problem:", error);
    // If not in Tauri, default gracefully to web workbench
    if (!("__TAURI_INTERNALS__" in window)) {
      viewState = "ready";
      return;
    }
    const candidate =
      error !== null && typeof error === "object"
        ? (error as Partial<DesktopProblem>)
        : {};
    problem = {
      code: candidate.code ?? "desktop_host_unavailable",
      title: candidate.title ?? "Fasti desktop host is unavailable",
      detail:
        candidate.detail ??
        "This interface must run inside the trusted Fasti desktop host.",
      next_action:
        candidate.next_action ??
        "Open the Fasti desktop application and retry.",
    };
    viewState = "blocked";
  }

  async function inspect(): Promise<void> {
    if ("__TAURI_INTERNALS__" in window) {
      isTauri = true;
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        applyStatus(await invoke<SetupStatus>("setup_status"));
        await loadProviderCredentials();
      } catch (error) {
        console.error("[Fasti UI] Tauri setup_status failed:", error);
        applyProblem(error);
      }
    } else {
      // Standalone web mode - ready
      console.info("[Fasti UI] Running in standalone web mode.");
      viewState = "ready";
    }
  }

  async function setup(): Promise<void> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      applyStatus(await invoke<SetupStatus>("complete_setup"));
      await loadProviderCredentials();
    } catch (error) {
      console.error("[Fasti UI] Tauri complete_setup failed:", error);
      applyProblem(error);
    }
  }

  onMount(inspect);
</script>

<a class="skip-link" href="#main-content">Skip to main content</a>

{#if viewState === "ready"}
  <FastiWorkbench
    connectionEndpoint={connection}
    {providerCredentials}
    onSaveConnection={saveConnection}
    onTestConnection={testConnection}
    onSaveProviderKey={saveProviderKey}
    onSearchProvider={searchProvider}
  />
{:else}
  <SetupPanel state={viewState} {problem} {cleanupPending} onSetup={setup} />
{/if}
