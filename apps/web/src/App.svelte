<script lang="ts">
  import {
    FastiClient,
    FastiProtocolError,
    connectionEndpoint,
  } from "@fasti/sdk";
  import type { WorkbenchHost } from "@fasti/ui";
  import SetupPanel, {
    type DesktopProblem,
    type SetupViewState,
  } from "@fasti/ui/setup";
  import StatusPanel, {
    type StatusPanelState,
    type StatusProblem,
  } from "@fasti/ui/status";
  import FastiWorkbench from "@fasti/ui/workbench";
  import "@tabler/core/dist/css/tabler.min.css";
  import { onMount, tick } from "svelte";
  import markDark from "../../../brand/logos/fasti-mark-dark.svg?url";
  import markLight from "../../../brand/logos/fasti-mark-light.svg?url";
  import { applyTheme, resolveTheme, type Theme } from "./theme.js";
  import { createWebHost } from "./web-host.js";

  interface SetupStatus {
    readonly phase: "needs_setup" | "ready";
    readonly proof_cleanup_pending: boolean;
  }

  type BuildEnvironment = Readonly<Record<string, string | undefined>>;

  const buildEnvironment = (
    import.meta as ImportMeta & { readonly env: BuildEnvironment }
  ).env;
  const buildApiUrl = buildEnvironment.VITE_FASTI_API_URL?.trim();
  const buildPublicUrl = buildEnvironment.VITE_FASTI_PUBLIC_URL?.trim();
  const configuredFallback =
    buildEnvironment.VITE_FASTI_PORT_FALLBACK ?? "fail";
  if (configuredFallback !== "auto" && configuredFallback !== "fail") {
    throw new TypeError("VITE_FASTI_PORT_FALLBACK must be auto or fail");
  }

  const endpoint = connectionEndpoint(
    buildApiUrl || "http://127.0.0.1:8420",
    buildApiUrl ? "build" : "default",
  );
  const publicEndpoint = buildPublicUrl
    ? connectionEndpoint(buildPublicUrl, "build")
    : undefined;

  const isTauri =
    typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  const client = new FastiClient({
    baseUrl:
      buildApiUrl ||
      (typeof window !== "undefined"
        ? window.location.origin
        : "http://127.0.0.1:8420"),
    timeoutMs: 3_000,
    retryPolicy: { maxAttempts: 1 },
  });

  const WORKBENCH_PATHS = new Set([
    "/connections",
    "/settings",
    "/discover",
    "/reconciliation",
    "/reviews",
    "/library",
    "/calendar",
  ]);

  function computeInitialSurface(): "status" | "workbench" {
    if (typeof window === "undefined") return "status";
    const path = window.location.pathname;
    if (path === "/status") return "status";
    if (WORKBENCH_PATHS.has(path) || path.startsWith("/records")) {
      return "workbench";
    }
    const urlParams = new URLSearchParams(window.location.search);
    if (urlParams.get("surface") === "workbench") return "workbench";
    if (urlParams.get("surface") === "status") return "status";
    try {
      const saved = localStorage.getItem("fasti-surface");
      if (saved === "workbench") return "workbench";
      if (saved === "status") return "status";
    } catch {}
    return "status";
  }

  let status: StatusPanelState = $state({ view: "loading" });
  let theme: Theme = $state(resolveTheme());
  let request: AbortController | undefined;
  let setupState: SetupViewState = $state("loading");
  let setupProblem: DesktopProblem | undefined = $state();
  let cleanupPending = $state(false);
  let host: WorkbenchHost = $state(
    createWebHost(
      buildApiUrl ||
        (typeof window !== "undefined"
          ? window.location.origin
          : "http://127.0.0.1:8420"),
    ),
  );
  let activeSurface = $state<"status" | "workbench">(computeInitialSurface());

  function statusProblemFor(error: unknown): StatusProblem {
    if (error instanceof FastiProtocolError) {
      return {
        title: "The local service returned an invalid response",
        detail:
          "Fasti rejected the response because it did not match the generated health contract.",
        recovery: "Stop the local service, rebuild it, and start it again.",
      };
    }
    return {
      title: "The local service is unavailable",
      detail: "Fasti did not answer on the configured service URL.",
      recovery:
        "Check the network settings, start the service, then try again.",
    };
  }

  async function inspectHealth(restoreRetryFocus = false): Promise<void> {
    request?.abort();
    const currentRequest = new AbortController();
    request = currentRequest;
    status = { view: "loading" };
    try {
      const response = await client.health({ signal: currentRequest.signal });
      if (request !== currentRequest) return;
      status = { view: "healthy", health: response };
    } catch (error) {
      if (currentRequest.signal.aborted || request !== currentRequest) return;
      status = { view: "blocked", problem: statusProblemFor(error) };
      if (restoreRetryFocus) {
        await tick();
        if (request === currentRequest && status.view === "blocked") {
          document.getElementById("retry-health")?.focus();
        }
      }
    }
  }

  function applySetupStatus(next: SetupStatus): void {
    setupState = next.phase;
    cleanupPending = next.proof_cleanup_pending;
    setupProblem = undefined;
  }

  function applySetupProblem(error: unknown): void {
    const candidate =
      error !== null && typeof error === "object"
        ? (error as Partial<DesktopProblem>)
        : {};
    setupProblem = {
      code: candidate.code ?? "desktop_host_unavailable",
      title: candidate.title ?? "Fasti desktop host is unavailable",
      detail:
        candidate.detail ??
        "This interface must run inside the trusted Fasti desktop host.",
      next_action:
        candidate.next_action ??
        "Open the Fasti desktop application and try again.",
    };
    setupState = "blocked";
  }

  async function inspectDesktop(): Promise<void> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      host = {
        loadNetworkConfiguration: () => invoke("load_network_configuration"),
        saveNetworkConfiguration: (input) =>
          invoke("save_network_configuration", { input }),
        testEndpointConnection: (endpoint) =>
          invoke("test_endpoint_connection", { input: { endpoint } }),
        providerCredentialStatus: () => invoke("provider_credential_status"),
        saveProviderCredential: (provider, credential) =>
          invoke("save_provider_credential", {
            input: { provider, credential },
          }),
        deleteProviderCredential: (provider) =>
          invoke("delete_provider_credential", { input: { provider } }),
        searchProvider: (provider, query) =>
          invoke("search_provider", { input: { provider, query } }),
        listApiClients: () => invoke("list_api_clients"),
        createApiClient: (scopes) =>
          invoke("create_api_client", { input: { scopes } }),
        revokeApiClient: (credentialId) =>
          invoke("revoke_api_client", {
            input: { credential_id: credentialId },
          }),
        listReviews: () => invoke("list_reviews"),
        resolveReview: (input) => invoke("resolve_review", { input }),
        listRecords: () => invoke("list_records"),
        createRecord: (grain) => invoke("create_record", { grain }),
        attachIdentifier: (input) => invoke("attach_identifier", { input }),
        registerNamespace: (input) => invoke("register_namespace", { input }),
        listTrackingDispositions: () => invoke("list_tracking_dispositions"),
        setTrackingDisposition: (recordId, disposition) =>
          invoke("set_tracking_disposition", {
            input: { record_id: recordId, disposition },
          }),
      };
      applySetupStatus(await invoke<SetupStatus>("setup_status"));
    } catch (error) {
      applySetupProblem(error);
    }
  }

  function openWorkbench(): void {
    activeSurface = "workbench";
    if (typeof window !== "undefined" && window.location.hash) {
      window.history.replaceState(null, "", window.location.pathname);
    }
    try {
      localStorage.setItem("fasti-surface", "workbench");
    } catch {}
  }

  async function setup(): Promise<void> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      applySetupStatus(await invoke<SetupStatus>("complete_setup"));
    } catch (error) {
      applySetupProblem(error);
    }
  }

  function retryHealth(): void {
    void inspectHealth(true);
  }

  function toggleTheme(): void {
    theme = theme === "dark" ? "light" : "dark";
    applyTheme(theme);
  }

  onMount(() => {
    if (isTauri) {
      void inspectDesktop();
      return;
    }
    void inspectHealth();
    return () => request?.abort();
  });
</script>

<a class="skip-link" href="#main-content">Skip to main content</a>

{#if isTauri}
  {#if setupState === "ready" && host}
    <FastiWorkbench {host} />
  {:else}
    <SetupPanel
      state={setupState}
      problem={setupProblem}
      {cleanupPending}
      onSetup={setup}
    />
  {/if}
{:else}
  {#if activeSurface === "workbench" && host}
    <FastiWorkbench {host} />
  {:else}
    <StatusPanel
      {status}
      {theme}
      mark={theme === "dark" ? markDark : markLight}
      {endpoint}
      {publicEndpoint}
      portFallback={configuredFallback}
      onRetry={retryHealth}
      onToggleTheme={toggleTheme}
      onOpenWorkbench={openWorkbench}
    />
  {/if}
{/if}
