<script lang="ts">
  import {
    FastiClient,
    FastiProtocolError,
    type HealthResponse,
  } from "@fasti/sdk";
  import {
    StatusPanel,
    type StatusProblem,
    type StatusViewState,
  } from "@fasti/ui";
  import { onMount, tick } from "svelte";
  import markDark from "../../../brand/logos/fasti-mark-dark.svg?url";
  import markLight from "../../../brand/logos/fasti-mark-light.svg?url";
  import { applyTheme, resolveTheme, type Theme } from "./theme.js";

  const client = new FastiClient({
    baseUrl: window.location.origin,
    timeoutMs: 3_000,
    retryPolicy: { maxAttempts: 1 },
  });

  let viewState: StatusViewState = $state("loading");
  let health: HealthResponse | undefined = $state();
  let problem: StatusProblem | undefined = $state();
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
      detail: "Fasti did not answer on the configured loopback service.",
      recovery: "Start the local service, then try again.",
    };
  }

  async function inspect(restoreRetryFocus = false): Promise<void> {
    request?.abort();
    const currentRequest = new AbortController();
    request = currentRequest;
    viewState = "loading";
    health = undefined;
    problem = undefined;
    try {
      const response = await client.health({ signal: currentRequest.signal });
      if (request !== currentRequest) return;
      health = response;
      viewState = "healthy";
    } catch (error) {
      if (currentRequest.signal.aborted || request !== currentRequest) return;
      problem = problemFor(error);
      viewState = "blocked";
      if (restoreRetryFocus) {
        await tick();
        if (request === currentRequest && viewState === "blocked") {
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
  state={viewState}
  {health}
  {problem}
  {theme}
  mark={theme === "dark" ? markDark : markLight}
  onRetry={retry}
  onToggleTheme={toggleTheme}
/>
