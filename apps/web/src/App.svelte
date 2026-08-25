<script lang="ts">
  import {
    FastiClient,
    FastiProtocolError,
    connectionEndpoint,
  } from "@fasti/sdk";
  import SetupPanel, {
    type DesktopProblem,
    type SetupViewState,
  } from "@fasti/ui/setup";
  import StatusPanel, {
    type StatusPanelState,
    type StatusProblem,
  } from "@fasti/ui/status";
  import { onMount, tick, type Component } from "svelte";
  import markDark from "../../../brand/logos/fasti-mark-dark.svg?url";
  import markLight from "../../../brand/logos/fasti-mark-light.svg?url";
  import { applyTheme, resolveTheme, type Theme } from "./theme.js";

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

  const isTauri = "__TAURI_INTERNALS__" in window;
  const client = new FastiClient({
    baseUrl: buildApiUrl || window.location.origin,
    timeoutMs: 3_000,
    retryPolicy: { maxAttempts: 1 },
  });

  let status: StatusPanelState = $state({ view: "loading" });
  let theme: Theme = $state(resolveTheme());
  let request: AbortController | undefined;
  let setupState: SetupViewState = $state("loading");
  let setupProblem: DesktopProblem | undefined = $state();
  let cleanupPending = $state(false);
  let DesktopWorkbench: Component | undefined = $state();

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
      const [{ default: workbench }, { invoke }] = await Promise.all([
        import("@fasti/ui/workbench"),
        import("@tauri-apps/api/core"),
        import("@tabler/core/dist/css/tabler.min.css"),
      ]);
      DesktopWorkbench = workbench;
      applySetupStatus(await invoke<SetupStatus>("setup_status"));
    } catch (error) {
      applySetupProblem(error);
    }
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
  {#if setupState === "ready" && DesktopWorkbench}
    <DesktopWorkbench />
  {:else}
    <SetupPanel
      state={setupState}
      problem={setupProblem}
      {cleanupPending}
      onSetup={setup}
    />
  {/if}
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
  />
{/if}
