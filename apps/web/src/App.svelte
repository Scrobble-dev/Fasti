<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import {
    SetupPanel,
    type DesktopProblem,
    type SetupViewState,
  } from "@fasti/ui";
  import { onMount } from "svelte";

  interface SetupStatus {
    readonly phase: "needs_setup" | "ready";
    readonly proof_cleanup_pending: boolean;
  }

  let viewState: SetupViewState = $state("loading");
  let problem: DesktopProblem | undefined = $state();
  let cleanupPending = $state(false);

  function applyStatus(status: SetupStatus): void {
    viewState = status.phase;
    cleanupPending = status.proof_cleanup_pending;
    problem = undefined;
  }

  function applyProblem(error: unknown): void {
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
    viewState = "loading";
    try {
      applyStatus(await invoke<SetupStatus>("setup_status"));
    } catch (error) {
      applyProblem(error);
    }
  }

  async function setup(): Promise<void> {
    viewState = "loading";
    try {
      applyStatus(await invoke<SetupStatus>("complete_setup"));
    } catch (error) {
      applyProblem(error);
    }
  }

  onMount(inspect);
</script>

<a class="skip-link" href="#main-content">Skip to main content</a>
<SetupPanel state={viewState} {problem} {cleanupPending} onSetup={setup} />
