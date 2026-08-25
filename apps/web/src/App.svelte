<script lang="ts">
  import { FastiClient, FastiProtocolError } from "@fasti/sdk";
  import {
    StatusPanel,
    type StatusPanelState,
    type StatusProblem,
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
      detail: "Fasti did not answer on the configured loopback service.",
      recovery: "Start the local service, then try again.",
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
  onRetry={retry}
  onToggleTheme={toggleTheme}
/>
