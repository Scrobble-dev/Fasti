<script lang="ts">
  import {
    SetupPanel,
    FastiWorkbench,
    type DesktopProblem,
    type SetupViewState,
  } from "@fasti/ui";
  import { onMount } from "svelte";

  interface SetupStatus {
    readonly phase: "needs_setup" | "ready";
    readonly proof_cleanup_pending: boolean;
  }

  let viewState: SetupViewState = $state("ready"); // Default to ready for web workbench
  let problem: DesktopProblem | undefined = $state();
  let cleanupPending = $state(false);
  let isTauri = $state(false);

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
    const candidate = error as Partial<DesktopProblem>;
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
    } catch (error) {
      console.error("[Fasti UI] Tauri complete_setup failed:", error);
      applyProblem(error);
    }
  }

  onMount(inspect);
</script>

<a class="skip-link" href="#main-content">Skip to main content</a>

{#if viewState === "ready"}
  <FastiWorkbench />
{:else}
  <SetupPanel state={viewState} {problem} {cleanupPending} onSetup={setup} />
{/if}
