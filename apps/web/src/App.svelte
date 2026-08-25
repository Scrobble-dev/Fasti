<script lang="ts">
  import {
    FastiClient,
    FastiProtocolError,
    connectionEndpoint,
  } from "@fasti/sdk";
  import {
    StatusPanel,
    type StatusPanelState,
    type StatusProblem,
  } from "@fasti/ui";
  import { onMount, tick } from "svelte";
  import markDark from "../../../brand/logos/fasti-mark-dark.svg?url";
  import markLight from "../../../brand/logos/fasti-mark-light.svg?url";
  import { applyTheme, resolveTheme, type Theme } from "./theme.js";

  type BuildEnvironment = Readonly<Record<string, string | undefined>>;

  const buildEnvironment = (
    import.meta as ImportMeta & { readonly env: BuildEnvironment }
  ).env;
  const buildApiUrl = buildEnvironment.VITE_FASTI_API_URL?.trim();
  const buildPublicUrl = buildEnvironment.VITE_FASTI_PUBLIC_URL?.trim();
  const configuredFallback =
    buildEnvironment.VITE_FASTI_PORT_FALLBACK ?? "auto";
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

  const client = new FastiClient({
    baseUrl: buildApiUrl || window.location.origin,
    timeoutMs: 3_000,
    retryPolicy: { maxAttempts: 1 },
  });

  let status: StatusPanelState = $state({ view: "loading" });
  let theme: Theme = $state(resolveTheme());
  let request: AbortController | undefined;

  function problemFor(error: unknown): StatusProblem {
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

  async function inspect(restoreRetryFocus = false): Promise<void> {
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
      status = { view: "blocked", problem: problemFor(error) };
      if (restoreRetryFocus) {
        await tick();
        if (request === currentRequest && status.view === "blocked") {
          document.getElementById("retry-health")?.focus();
        }
      }
    }
  }

  function retry(): void {
    void inspect(true);
  }

  function toggleTheme(): void {
    theme = theme === "dark" ? "light" : "dark";
    applyTheme(theme);
  }

  onMount(() => {
    void inspect();
    return () => request?.abort();
  });
</script>

<a class="skip-link" href="#main-content">Skip to main content</a>
<StatusPanel
  {status}
  {theme}
  mark={theme === "dark" ? markDark : markLight}
  {endpoint}
  {publicEndpoint}
  portFallback={configuredFallback}
  onRetry={retry}
  onToggleTheme={toggleTheme}
/>
