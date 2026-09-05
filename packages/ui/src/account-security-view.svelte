<script lang="ts">
  import {
    FastiAbortError,
    FastiProblemError,
    type ProblemDetails,
  } from "@fasti/sdk";
  import {
    IconAlertTriangle,
    IconClock,
    IconDevices,
    IconKey,
    IconLoader2,
    IconLogout,
    IconRefresh,
    IconShieldCheck,
    IconUserCheck,
    IconWorld,
    IconX,
  } from "@tabler/icons-svelte";
  import { onMount, tick } from "svelte";
  import { hostProblemText } from "./host-problem.js";
  import type {
    AccessProjectionResponse,
    ReadTrailBaseContinuationResponse,
    WorkbenchHost,
  } from "./types.js";

  type Mode = "task_map" | "first_run";
  type FirstRunStep = AccessProjectionResponse["first_run_steps"][number];
  type ViewState =
    | { readonly kind: "loading" }
    | { readonly kind: "signed_out" }
    | {
        readonly kind: "continuation";
        readonly continuation: ReadTrailBaseContinuationResponse;
      }
    | { readonly kind: "ready"; readonly projection: AccessProjectionResponse }
    | {
        readonly kind: "problem";
        readonly detail: string;
        readonly problem?: ProblemDetails;
        readonly dismissible: boolean;
      };

  let {
    host,
    mode,
    projection,
    readAccessProjection,
    onProjection,
    onOpenAccountSecurity,
    onStartFirstRun,
    onOpenConnections,
    onLeaveFirstRun,
    initialNotice,
    onInitialNoticeConsumed,
    callbackMarker,
    onCallbackConsumed,
  }: {
    host: WorkbenchHost;
    mode: Mode;
    projection?: AccessProjectionResponse;
    readAccessProjection: () => Promise<AccessProjectionResponse>;
    onProjection?: (projection?: AccessProjectionResponse) => void;
    onOpenAccountSecurity?: () => void;
    onStartFirstRun?: () => void;
    onOpenConnections?: () => void;
    onLeaveFirstRun?: (completed?: boolean) => void;
    initialNotice?: string;
    onInitialNoticeConsumed?: () => void;
    callbackMarker?: "continue" | "failed";
    onCallbackConsumed?: () => void;
  } = $props();

  let viewState: ViewState = $state({ kind: "loading" });
  let busy = $state("");
  let remembered = $state(false);
  let selectedChoice: number | undefined = $state();
  let notice = $state("");
  let generation = 0;
  let readController: AbortController | undefined;

  $effect(() => {
    if (viewState.kind !== "problem") return;
    requestAnimationFrame(() =>
      document.getElementById("access-problem")?.focus(),
    );
  });

  const signedOutCodes = new Set([
    "authentication_failed",
    "browser_session_expired",
    "browser_session_revoked",
    "session_policy_changed",
  ]);
  const missingContinuationCodes = new Set(["auth_browser_binding_invalid"]);

  function problemFor(error: unknown): ProblemDetails | undefined {
    return error instanceof FastiProblemError ? error.problem : undefined;
  }

  function problemState(
    error: unknown,
    fallback: string,
    continuationCanBeDismissed = false,
  ): ViewState {
    const problem = problemFor(error);
    return {
      kind: "problem",
      detail: problem?.detail ?? hostProblemText(error, fallback),
      problem,
      dismissible: Boolean(problem && continuationCanBeDismissed),
    };
  }

  function priorStateWasRetained(error: unknown): boolean {
    const problem = problemFor(error);
    if (!problem || signedOutCodes.has(problem.code)) return false;
    const safeState = problem.safe_state;
    return safeState === "no_mutation" || safeState === "prior_state_retained";
  }

  function signInMethodsState(
    projection: AccessProjectionResponse,
  ): FirstRunStep["state"] {
    const states = projection.first_run_steps
      .filter(({ key }) => key === "strong_sign_in" || key === "recovery")
      .map(({ state }) => state);
    if (states.length > 0 && states.every((state) => state === "verified"))
      return "verified";
    if (states.includes("failed_safely")) return "failed_safely";
    if (states.includes("needs_attention")) return "needs_attention";
    if (states.includes("loading")) return "loading";
    if (states.includes("empty")) return "empty";
    return "unavailable";
  }

  function nextRead(): { generation: number; signal: AbortSignal } {
    readController?.abort();
    readController = new AbortController();
    return { generation: ++generation, signal: readController.signal };
  }

  function wasAborted(error: unknown): boolean {
    return (
      error instanceof FastiAbortError ||
      (error instanceof DOMException && error.name === "AbortError")
    );
  }

  async function focusHeading(): Promise<void> {
    await tick();
    document
      .getElementById(
        mode === "first_run" ? "first-run-title" : "account-security-title",
      )
      ?.focus();
  }

  async function focusNotice(): Promise<void> {
    await tick();
    document.getElementById("access-notice")?.focus();
  }

  async function loadProjection(
    currentGeneration: number,
    moveFocus = false,
  ): Promise<
    | { readonly kind: "ready"; readonly projection: AccessProjectionResponse }
    | { readonly kind: "signed_out" | "problem" | "stale" }
  > {
    try {
      const projection = await readAccessProjection();
      if (currentGeneration !== generation) return { kind: "stale" };
      viewState = { kind: "ready", projection };
      if (moveFocus) await focusHeading();
      return { kind: "ready", projection };
    } catch (error) {
      if (wasAborted(error) || currentGeneration !== generation)
        return { kind: "stale" };
      const problem = problemFor(error);
      if (problem && signedOutCodes.has(problem.code)) {
        onProjection?.(undefined);
        viewState = { kind: "signed_out" };
        return { kind: "signed_out" };
      } else {
        viewState = problemState(
          error,
          "Fasti could not load account and security state.",
        );
        return { kind: "problem" };
      }
    }
  }

  async function loadContinuation(
    currentGeneration: number,
    signal: AbortSignal,
    failedCallback = false,
  ): Promise<"continuation" | "missing" | "problem" | "stale"> {
    if (!host.readTrailBaseContinuation) return "missing";
    try {
      const continuation = await host.readTrailBaseContinuation(signal);
      if (currentGeneration !== generation) return "stale";
      selectedChoice = undefined;
      remembered = continuation.remembered;
      viewState = { kind: "continuation", continuation };
      await focusHeading();
      return "continuation";
    } catch (error) {
      if (wasAborted(error) || currentGeneration !== generation) return "stale";
      const problem = problemFor(error);
      if (problem && missingContinuationCodes.has(problem.code)) {
        if (failedCallback) {
          notice =
            "Sign-in did not complete. Review the current state before you start again.";
        }
        return "missing";
      }
      viewState = problemState(
        error,
        "Fasti could not resume this sign-in.",
        true,
      );
      return "problem";
    }
  }

  async function load(): Promise<void> {
    const currentRead = nextRead();
    const resumeFromCallback = callbackMarker !== undefined;
    const failedCallback = callbackMarker === "failed";
    if (callbackMarker !== undefined) onCallbackConsumed?.();
    viewState = { kind: "loading" };
    notice = initialNotice ?? "";
    if (resumeFromCallback) {
      const continuationOutcome = await loadContinuation(
        currentRead.generation,
        currentRead.signal,
        failedCallback,
      );
      if (continuationOutcome !== "missing") return;
    }
    if (projection) {
      viewState = { kind: "ready", projection };
      await focusHeading();
      return;
    }
    const projectionOutcome = await loadProjection(
      currentRead.generation,
      true,
    );
    if (projectionOutcome.kind !== "signed_out") return;
    const continuationOutcome = await loadContinuation(
      currentRead.generation,
      currentRead.signal,
      failedCallback,
    );
    if (continuationOutcome === "missing") await focusHeading();
  }

  function trailBaseAuthorizationUrl(raw: string): string {
    const target = new URL(raw);
    if (
      target.origin !== "http://127.0.0.1:4000" ||
      target.pathname !== "/_/auth/login"
    ) {
      throw new TypeError(
        "TrailBase returned an unexpected authorization URL.",
      );
    }
    return target.href;
  }

  async function startSignIn(firstAdministrator: boolean): Promise<void> {
    const start = firstAdministrator
      ? host.startFirstAdministratorBootstrap
      : host.startTrailBaseSignIn;
    if (!start) {
      viewState = {
        kind: "problem",
        detail: firstAdministrator
          ? "First-administrator account confirmation is available only in the trusted packaged host."
          : "This host does not expose TrailBase browser sign-in.",
        dismissible: false,
      };
      return;
    }
    busy = firstAdministrator ? "bootstrap" : "sign_in";
    notice = "";
    try {
      const started = firstAdministrator
        ? await host.startFirstAdministratorBootstrap!()
        : await host.startTrailBaseSignIn!({ remembered });
      window.location.assign(
        trailBaseAuthorizationUrl(started.authorization_url),
      );
    } catch (error) {
      viewState = problemState(
        error,
        firstAdministrator
          ? "Fasti could not start first-administrator account confirmation."
          : "Fasti could not start sign-in.",
      );
    } finally {
      busy = "";
    }
  }

  async function completeContinuation(): Promise<void> {
    if (
      viewState.kind !== "continuation" ||
      selectedChoice === undefined ||
      !host.completeTrailBaseContinuation
    )
      return;
    busy = "continue";
    try {
      await host.completeTrailBaseContinuation({
        candidate_revision: viewState.continuation.candidate_revision,
        choice_ordinal: selectedChoice,
      });
      history.replaceState(
        {},
        "",
        mode === "first_run" ? "/first-run" : "/settings/account",
      );
      notice = "Account access confirmed. Review the remaining security tasks.";
      const currentRead = nextRead();
      const outcome = await loadProjection(currentRead.generation, true);
      if (
        mode === "first_run" &&
        outcome.kind === "ready" &&
        outcome.projection.first_run_steps.every(
          (step) => step.state === "verified",
        )
      ) {
        onLeaveFirstRun?.(true);
      }
    } catch (error) {
      const problem = problemFor(error);
      if (problem?.code === "auth_selection_changed") {
        const currentRead = nextRead();
        viewState = { kind: "loading" };
        const outcome = await loadContinuation(
          currentRead.generation,
          currentRead.signal,
        );
        if (outcome === "continuation") {
          notice = "Your available access changed. Review the current choices.";
        } else if (outcome === "missing") {
          viewState = problemState(
            error,
            "Fasti could not refresh the changed access choices.",
            true,
          );
        }
      } else {
        viewState = problemState(
          error,
          "Fasti could not confirm this access choice.",
          true,
        );
      }
    } finally {
      busy = "";
    }
  }

  async function dismissContinuation(): Promise<void> {
    if (!host.cancelTrailBaseContinuation) return;
    if (
      !window.confirm(
        "Dismiss this saved sign-in evidence? You will need to start sign-in again.",
      )
    )
      return;
    busy = "dismiss";
    try {
      await host.cancelTrailBaseContinuation();
      viewState = { kind: "signed_out" };
      notice = "The saved sign-in evidence was dismissed.";
      await focusNotice();
    } catch (error) {
      viewState = problemState(
        error,
        "Fasti could not dismiss the saved sign-in evidence.",
      );
    } finally {
      busy = "";
    }
  }

  async function endSession(
    sessionId: string,
    current: boolean,
  ): Promise<void> {
    if (
      !window.confirm(
        current
          ? "Sign out this browser now?"
          : "Revoke this browser session? That browser will need to sign in again.",
      )
    )
      return;
    const previousProjection =
      viewState.kind === "ready" ? viewState.projection : projection;
    busy = sessionId;
    onProjection?.(undefined);
    try {
      if (current) {
        if (!host.endBrowserSession)
          throw new Error("Session ending is unavailable.");
        await host.endBrowserSession();
        viewState = { kind: "signed_out" };
        notice = "This browser session ended.";
        await focusNotice();
      } else {
        if (!host.revokeBrowserSession)
          throw new Error("Session revocation is unavailable.");
        await host.revokeBrowserSession(sessionId);
        notice = "The selected browser session was revoked.";
        const currentRead = nextRead();
        const outcome = await loadProjection(currentRead.generation);
        if (outcome.kind === "ready") await focusNotice();
      }
    } catch (error) {
      if (previousProjection && priorStateWasRetained(error)) {
        onProjection?.(previousProjection);
      }
      viewState = problemState(
        error,
        "Fasti could not change the selected browser session.",
      );
    } finally {
      busy = "";
    }
  }

  async function revokeOtherSessions(): Promise<void> {
    if (!host.revokeOtherBrowserSessions) return;
    if (
      !window.confirm(
        "Revoke every other browser session? Those browsers will need to sign in again.",
      )
    )
      return;
    const previousProjection =
      viewState.kind === "ready" ? viewState.projection : projection;
    busy = "revoke_others";
    onProjection?.(undefined);
    try {
      const result = await host.revokeOtherBrowserSessions();
      notice = `${result.revoked_count} other browser ${result.revoked_count === 1 ? "session was" : "sessions were"} revoked.`;
      const currentRead = nextRead();
      const outcome = await loadProjection(currentRead.generation);
      if (outcome.kind === "ready") await focusNotice();
    } catch (error) {
      if (previousProjection && priorStateWasRetained(error)) {
        onProjection?.(previousProjection);
      }
      viewState = problemState(
        error,
        "Fasti could not revoke the other browser sessions.",
      );
    } finally {
      busy = "";
    }
  }

  async function rotateCurrentSession(): Promise<void> {
    if (!host.rotateBrowserSession) return;
    if (
      !window.confirm(
        "Rotate this browser session now? Other requests using the old session will stop working.",
      )
    )
      return;
    const previousProjection =
      viewState.kind === "ready" ? viewState.projection : projection;
    busy = "rotate";
    onProjection?.(undefined);
    try {
      await host.rotateBrowserSession();
      notice = "This browser session was rotated.";
      const currentRead = nextRead();
      const outcome = await loadProjection(currentRead.generation);
      if (outcome.kind === "ready") await focusNotice();
    } catch (error) {
      if (previousProjection && priorStateWasRetained(error)) {
        onProjection?.(previousProjection);
      }
      viewState = problemState(
        error,
        "Fasti could not rotate this browser session.",
      );
    } finally {
      busy = "";
    }
  }

  function stateLabel(value: string): string {
    return value.replaceAll("_", " ");
  }

  function stepLabel(key: string): string {
    switch (key) {
      case "account_confirmed":
        return "Account confirmed";
      case "strong_sign_in":
        return "Strong sign-in";
      case "recovery":
        return "Recovery";
      case "devices_and_clients":
        return "Devices and clients";
      case "external_identity":
        return "External identity";
      default:
        return stateLabel(key);
    }
  }

  function overflowTabStop(node: HTMLElement): { destroy: () => void } {
    const update = () => {
      if (node.scrollWidth > node.clientWidth)
        node.setAttribute("tabindex", "0");
      else node.removeAttribute("tabindex");
    };
    const observer = new ResizeObserver(update);
    observer.observe(node);
    update();
    return { destroy: () => observer.disconnect() };
  }

  function dateTime(value: string): string {
    const parsed = new Date(value);
    return Number.isNaN(parsed.valueOf())
      ? "Unknown"
      : new Intl.DateTimeFormat(undefined, {
          dateStyle: "medium",
          timeStyle: "short",
        }).format(parsed);
  }

  function stepDetail(key: string): string {
    switch (key) {
      case "account_confirmed":
        return "TrailBase identity is linked to one Fasti access subject.";
      case "strong_sign_in":
        return "Passkeys and stronger sign-in controls activate in a later reviewed Access package.";
      case "recovery":
        return "Recovery-code management activates after its complete lifecycle is implemented and reviewed.";
      case "devices_and_clients":
        return "Device grants, registered clients, and personal access tokens activate in later Access packages.";
      case "external_identity":
        return "Generic OpenID Connect and managed Authentik support activate in later Access packages.";
      default:
        return "This security task has no published action in the current package.";
    }
  }

  function firstIncompleteStep(
    currentProjection: AccessProjectionResponse,
  ): FirstRunStep | undefined {
    return currentProjection.first_run_steps.find(
      (step) => step.state !== "verified",
    );
  }

  function firstIncompleteStepIndex(
    currentProjection: AccessProjectionResponse,
  ): number {
    return currentProjection.first_run_steps.findIndex(
      (step) => step.state !== "verified",
    );
  }

  onMount(() => {
    void load().then(async () => {
      if (!initialNotice) return;
      await new Promise<void>((resolve) =>
        requestAnimationFrame(() => resolve()),
      );
      await focusNotice();
      onInitialNoticeConsumed?.();
    });
    return () => readController?.abort();
  });
</script>

<section
  class="access-surface"
  class:first-run={mode === "first_run"}
  aria-labelledby={mode === "first_run"
    ? "first-run-title"
    : "account-security-title"}
  data-testid={mode === "first_run"
    ? "first-run-guided-setup"
    : "account-security-task-map"}
>
  <header class="access-heading">
    <div>
      <svelte:element
        this={mode === "first_run" ? "h1" : "h2"}
        id={mode === "first_run" ? "first-run-title" : "account-security-title"}
        class="h2 mb-1"
        tabindex="-1"
      >
        {mode === "first_run"
          ? "Secure your Fasti account"
          : "Account and security"}
      </svelte:element>
      <p class="text-secondary mb-0">
        {mode === "first_run"
          ? "Complete each confirmed task in order. You can save and finish later."
          : "Protect your account, review access, and connect trusted clients."}
      </p>
    </div>
    {#if mode === "first_run"}
      <button
        type="button"
        class="btn btn-outline-secondary"
        disabled={Boolean(busy)}
        onclick={onOpenAccountSecurity}
      >
        Manage existing access
      </button>
    {/if}
  </header>

  {#if notice}
    <p
      id="access-notice"
      class="alert alert-success"
      role="status"
      tabindex="-1"
    >
      {notice}
    </p>
  {/if}

  {#if viewState.kind === "ready" && viewState.projection.profile_grants_truncated}
    <p class="alert alert-info mb-0" role="status">
      This bounded view does not include every available media profile grant.
    </p>
  {/if}

  {#if viewState.kind === "loading"}
    <div class="card access-state" aria-live="polite">
      <div class="card-body d-flex align-items-center gap-2">
        <span class="access-spinner" aria-hidden="true">
          <IconLoader2 size={20} />
        </span>
        <span>Loading account and security state…</span>
      </div>
    </div>
  {:else if viewState.kind === "signed_out"}
    <div class="card access-state">
      <div class="card-body">
        <svelte:element this={mode === "first_run" ? "h2" : "h3"} class="h3"
          >Confirm account access</svelte:element
        >
        <p>
          Sign in with TrailBase. Fasti issues its own private browser session
          only after local access is confirmed.
          {#if mode === "first_run" && host.startFirstAdministratorBootstrap}
            Use your own verified TrailBase account. Do not use the TrailBase
            installation administrator as a shared person account.
          {/if}
        </p>
        {#if mode === "first_run" && !host.startFirstAdministratorBootstrap}
          <p
            id="first-administrator-unavailable"
            class="alert alert-warning"
            role="status"
          >
            <strong
              >First-administrator confirmation is unavailable here.</strong
            >
            The packaged WebView cannot yet retain the required Secure callback cookie.
            On Unix, run
            <code
              >fasti access bootstrap-administrator --data-root &lt;Fasti data
              root&gt; --trailbase-root &lt;TrailBase root&gt;</code
            >
            from the installation owner's terminal. Then sign in to this browser.
          </p>
        {/if}
        <label
          class="form-check remember-browser-check d-flex align-items-center mb-3"
        >
          <input
            class="form-check-input"
            type="checkbox"
            bind:checked={remembered}
          />
          <span class="form-check-label">
            Remember this browser within the Fasti session policy{mode ===
              "first_run" && host.startFirstAdministratorBootstrap
              ? " when signing in to an existing account"
              : ""}
          </span>
        </label>
        <div class="d-flex flex-wrap gap-2">
          {#if mode === "first_run"}
            <button
              type="button"
              class="btn btn-primary"
              disabled={!host.startFirstAdministratorBootstrap || Boolean(busy)}
              aria-describedby={host.startFirstAdministratorBootstrap
                ? undefined
                : "first-administrator-unavailable"}
              onclick={() => void startSignIn(true)}
            >
              <IconUserCheck size={18} aria-hidden="true" />
              {busy === "bootstrap"
                ? "Starting…"
                : "Confirm first Fasti administrator"}
            </button>
          {/if}
          <button
            type="button"
            class={mode === "first_run" && host.startFirstAdministratorBootstrap
              ? "btn btn-outline-primary"
              : "btn btn-primary"}
            disabled={Boolean(busy)}
            onclick={() => void startSignIn(false)}
          >
            <IconKey size={18} aria-hidden="true" />
            {busy === "sign_in"
              ? "Starting…"
              : "Sign in to an existing account"}
          </button>
        </div>
      </div>
    </div>
  {:else if viewState.kind === "problem"}
    <div
      id="access-problem"
      class="alert alert-warning access-problem"
      role="alert"
      tabindex="-1"
    >
      <IconAlertTriangle size={22} aria-hidden="true" />
      <div>
        <svelte:element
          this={mode === "first_run" ? "h2" : "h3"}
          class="h3 mb-1"
        >
          {viewState.problem?.title ?? "Account access needs attention"}
        </svelte:element>
        <p class="mb-2">{viewState.detail}</p>
        {#if viewState.problem?.next_actions[0]}
          <p class="mb-3">
            <strong>Next:</strong>
            {viewState.problem.next_actions[0].label}
          </p>
        {/if}
        {#if viewState.problem}
          <dl class="problem-evidence mb-3">
            <div>
              <dt>Safe state</dt>
              <dd>{stateLabel(viewState.problem.safe_state)}</dd>
            </div>
            <div>
              <dt>Retry</dt>
              <dd>{stateLabel(viewState.problem.retryability)}</dd>
            </div>
          </dl>
        {/if}
        <div class="d-flex flex-wrap gap-2">
          {#if !viewState.problem || viewState.problem.retryability === "retry_safe"}
            <button
              type="button"
              class="btn btn-primary"
              disabled={Boolean(busy)}
              onclick={() => void load()}
            >
              <IconRefresh size={18} aria-hidden="true" /> Retry
            </button>
          {/if}
          {#if viewState.dismissible && host.cancelTrailBaseContinuation}
            <button
              type="button"
              class="btn btn-outline-secondary"
              disabled={Boolean(busy)}
              onclick={() => void dismissContinuation()}
            >
              <IconX size={18} aria-hidden="true" /> Dismiss saved evidence
            </button>
          {/if}
        </div>
      </div>
    </div>
  {:else if viewState.kind === "continuation"}
    <div class="card access-state">
      <div class="card-header">
        <div>
          <svelte:element
            this={mode === "first_run" ? "h2" : "h3"}
            class="card-title h3 mb-1">Choose where to continue</svelte:element
          >
          <p class="card-subtitle text-secondary mb-0">
            TrailBase confirmed your identity. Fasti still needs one explicit
            local access choice.
          </p>
        </div>
      </div>
      <div
        class="list-group list-group-flush"
        role="radiogroup"
        aria-label="Available Fasti access"
      >
        {#each viewState.continuation.choices as choice}
          <label class="list-group-item continuation-choice">
            <input
              class="form-check-input"
              type="radio"
              name="access-choice"
              value={choice.choice_ordinal}
              checked={selectedChoice === choice.choice_ordinal}
              onchange={() => (selectedChoice = choice.choice_ordinal)}
            />
            <span>
              <strong
                >Workspace {choice.workspace_ordinal}, profile {choice.profile_ordinal}</strong
              >
              <span class="text-secondary d-block">
                {stateLabel(choice.role)} · {stateLabel(
                  choice.membership_state,
                )}
              </span>
              <span class="text-secondary d-block">
                Workspace created {dateTime(choice.workspace_created_at)} · profile
                created {dateTime(choice.profile_created_at)}
              </span>
            </span>
          </label>
        {/each}
      </div>
      <div class="card-footer d-flex flex-wrap gap-2">
        <button
          type="button"
          class="btn btn-primary"
          disabled={selectedChoice === undefined || Boolean(busy)}
          onclick={() => void completeContinuation()}
        >
          <IconShieldCheck size={18} aria-hidden="true" />
          {busy === "continue" ? "Confirming…" : "Confirm access"}
        </button>
        {#if mode === "first_run" && onLeaveFirstRun}
          <button
            type="button"
            class="btn btn-outline-secondary"
            disabled={Boolean(busy)}
            onclick={() => onLeaveFirstRun?.()}
          >
            Save and leave
          </button>
        {/if}
        <button
          type="button"
          class="btn btn-outline-danger"
          disabled={Boolean(busy)}
          onclick={() => void dismissContinuation()}
        >
          Cancel sign-in
        </button>
      </div>
    </div>
  {:else if mode === "first_run"}
    <div class="first-run-layout">
      <ol
        class="list-group first-run-steps"
        aria-label="Account security setup steps"
      >
        {#each viewState.projection.first_run_steps as step, index}
          <li
            class="list-group-item d-flex gap-3 align-items-start"
            aria-current={index ===
            firstIncompleteStepIndex(viewState.projection)
              ? "step"
              : undefined}
          >
            <span class="step-number" aria-hidden="true">{index + 1}</span>
            <div class="flex-fill">
              <div class="d-flex flex-wrap justify-content-between gap-2">
                <strong>{stepLabel(step.key)}</strong>
                <span
                  class="badge"
                  class:bg-success-lt={step.state === "verified"}
                  class:bg-warning-lt={step.state === "needs_attention"}
                  class:bg-secondary-lt={![
                    "verified",
                    "needs_attention",
                  ].includes(step.state)}
                >
                  {stateLabel(step.state)}
                </span>
              </div>
              <p class="text-secondary mb-0 mt-1">{stepDetail(step.key)}</p>
              {#if index === firstIncompleteStepIndex(viewState.projection)}
                <button
                  type="button"
                  class="btn btn-outline-primary mt-3"
                  disabled={!onOpenAccountSecurity}
                  onclick={onOpenAccountSecurity}
                  >Review {stepLabel(step.key)} status</button
                >
              {/if}
            </div>
          </li>
        {/each}
      </ol>
      <div class="d-flex flex-wrap gap-2">
        <button
          type="button"
          class="btn btn-primary"
          onclick={() => onLeaveFirstRun?.()}>Save and finish later</button
        >
        <button
          type="button"
          class="btn btn-outline-secondary"
          onclick={() => onLeaveFirstRun?.()}>Back</button
        >
      </div>
    </div>
  {:else}
    {#if firstIncompleteStep(viewState.projection)}
      <div class="alert alert-warning d-flex flex-wrap gap-3" role="status">
        <div>
          <strong>One security task needs attention</strong>
          <p class="mb-0">
            {stepLabel(firstIncompleteStep(viewState.projection)!.key)}:
            {stepDetail(firstIncompleteStep(viewState.projection)!.key)}
          </p>
        </div>
        {#if onStartFirstRun}
          <button
            type="button"
            class="btn btn-primary ms-auto"
            onclick={onStartFirstRun}>Resume setup</button
          >
        {/if}
      </div>
    {/if}

    <div
      class="list-group access-task-map"
      aria-label="Account and security tasks"
    >
      <div class="list-group-item task-row">
        <IconKey size={22} aria-hidden="true" />
        <div class="task-copy">
          <h3 class="h4 mb-1">Sign-in methods</h3>
          <p class="text-secondary mb-0">
            Current method: {stateLabel(
              viewState.projection.authentication.method,
            )}. Passkeys, TOTP management, and recovery controls activate in
            later Access packages.
          </p>
        </div>
        <span
          class="badge"
          class:bg-success-lt={signInMethodsState(viewState.projection) ===
            "verified"}
          class:bg-warning-lt={["needs_attention", "failed_safely"].includes(
            signInMethodsState(viewState.projection),
          )}
          class:bg-secondary-lt={signInMethodsState(viewState.projection) ===
            "unavailable"}
          >{stateLabel(signInMethodsState(viewState.projection))}</span
        >
        <button
          type="button"
          class="btn btn-outline-secondary"
          disabled
          aria-describedby="sign-in-methods-reason">Manage</button
        >
        <span id="sign-in-methods-reason" class="visually-hidden"
          >Management is unavailable until the later sign-in-method package is
          active.</span
        >
      </div>

      <details class="list-group-item access-detail">
        <summary class="task-row">
          <IconClock size={22} aria-hidden="true" />
          <span class="task-copy">
            <span class="h4 d-block mb-1">Browser sessions</span>
            <span class="text-secondary"
              >{viewState.projection.sessions.length} active browser {viewState
                .projection.sessions.length === 1
                ? "session"
                : "sessions"}.</span
            >
          </span>
          <span class="badge bg-success-lt">Active</span>
          <span class="btn btn-outline-primary" aria-hidden="true">Review</span>
        </summary>
        <div
          class="table-responsive access-table"
          use:overflowTabStop
          aria-label="Browser session inventory"
        >
          <table class="table table-vcenter mb-0">
            <caption class="visually-hidden"
              >Active Fasti browser sessions</caption
            >
            <thead
              ><tr
                ><th scope="col">Browser</th><th scope="col">Last used</th><th
                  scope="col">Expires</th
                ><th scope="col">Action</th></tr
              ></thead
            >
            <tbody>
              {#if viewState.projection.sessions.length}
                {#each viewState.projection.sessions as session}
                  <tr>
                    <th scope="row"
                      >{session.is_current
                        ? "This browser"
                        : "Other browser"}</th
                    >
                    <td>{dateTime(session.last_seen_at)}</td>
                    <td>{dateTime(session.idle_expires_at)}</td>
                    <td>
                      <button
                        type="button"
                        class={session.is_current
                          ? "btn btn-outline-danger"
                          : "btn btn-outline-secondary"}
                        aria-label={session.is_current
                          ? "Sign out this browser"
                          : `Revoke session last used ${dateTime(session.last_seen_at)}`}
                        disabled={Boolean(busy)}
                        onclick={() =>
                          void endSession(
                            session.browser_session_id,
                            session.is_current,
                          )}
                      >
                        <IconLogout size={16} aria-hidden="true" />
                        {busy === session.browser_session_id
                          ? "Working…"
                          : session.is_current
                            ? "Sign out"
                            : "Revoke"}
                      </button>
                    </td>
                  </tr>
                {/each}
              {:else}
                <tr
                  ><td colspan="4">No active browser sessions were returned.</td
                  ></tr
                >
              {/if}
            </tbody>
          </table>
        </div>
        {#if viewState.projection.sessions_truncated}
          <p class="alert alert-info m-3" role="status">
            This bounded view does not include every active browser session.
          </p>
        {/if}
        <div class="card-footer d-flex flex-wrap gap-2">
          {#if viewState.projection.sessions.some((session) => !session.is_current)}
            <button
              type="button"
              class="btn btn-outline-danger"
              disabled={Boolean(busy)}
              onclick={() => void revokeOtherSessions()}
            >
              {busy === "revoke_others" ? "Revoking…" : "Revoke other sessions"}
            </button>
          {/if}
          <button
            type="button"
            class="btn btn-outline-secondary"
            disabled={Boolean(busy) || !host.rotateBrowserSession}
            onclick={() => void rotateCurrentSession()}
          >
            {busy === "rotate" ? "Rotating…" : "Rotate this session"}
          </button>
        </div>
      </details>

      <div class="list-group-item task-row">
        <IconDevices size={22} aria-hidden="true" />
        <div class="task-copy">
          <h3 class="h4 mb-1">Devices and clients</h3>
          <p class="text-secondary mb-0">
            Existing API client credentials remain under Connections. Device
            grants, OAuth clients, and personal access tokens activate in later
            Access packages.
          </p>
        </div>
        <span class="badge bg-warning-lt">Partial</span>
        <button
          type="button"
          class="btn btn-outline-secondary"
          disabled={!onOpenConnections}
          onclick={onOpenConnections}>Open connections</button
        >
      </div>

      <div class="list-group-item task-row">
        <IconWorld size={22} aria-hidden="true" />
        <div class="task-copy">
          <h3 class="h4 mb-1">External identity providers</h3>
          <p class="text-secondary mb-0">
            Generic OpenID Connect and managed Authentik support activate in
            later Access packages.
          </p>
        </div>
        <span class="badge bg-secondary-lt">Unavailable</span>
        <button type="button" class="btn btn-outline-secondary" disabled
          >Open</button
        >
      </div>

      <details class="list-group-item access-detail">
        <summary class="task-row">
          <IconShieldCheck size={22} aria-hidden="true" />
          <span class="task-copy"
            ><span class="h4 d-block mb-1">Security evidence</span><span
              class="text-secondary"
              >Confirmed sign-in, policy, and recent local evidence.</span
            ></span
          >
          <span class="badge bg-success-lt">Verified</span>
          <span class="btn btn-outline-primary" aria-hidden="true">Open</span>
        </summary>
        <dl class="evidence-grid">
          <div>
            <dt>Sign-in verified</dt>
            <dd>{dateTime(viewState.projection.authentication.verified_at)}</dd>
          </div>
          <div>
            <dt>Recent authentication</dt>
            <dd>
              {stateLabel(
                viewState.projection.authentication.recent_authentication.state,
              )}
            </dd>
          </div>
          <div>
            <dt>Idle timeout</dt>
            <dd>
              {Math.round(
                viewState.projection.session_policy.idle_timeout_seconds / 60,
              )} minutes
            </dd>
          </div>
          <div>
            <dt>TrailBase trust</dt>
            <dd>{stateLabel(viewState.projection.trailbase.state)}</dd>
          </div>
          <div>
            <dt>Workspace role</dt>
            <dd>{stateLabel(viewState.projection.membership.role)}</dd>
          </div>
          <div>
            <dt>Membership</dt>
            <dd>{stateLabel(viewState.projection.membership.lifecycle)}</dd>
          </div>
        </dl>
        {#if viewState.projection.evidence.length}
          <div
            class="table-responsive access-table"
            use:overflowTabStop
            aria-label="Recent account security evidence"
          >
            <table class="table table-vcenter mb-0">
              <caption class="visually-hidden"
                >Recent account security evidence</caption
              >
              <thead
                ><tr
                  ><th scope="col">Event</th><th scope="col">State</th><th
                    scope="col">Evidence</th
                  ><th scope="col">Time</th></tr
                ></thead
              >
              <tbody>
                {#each viewState.projection.evidence as evidence}
                  <tr>
                    <th scope="row">{stateLabel(evidence.kind)}</th>
                    <td>{stateLabel(evidence.state)}</td>
                    <td
                      >{evidence.failure
                        ? stateLabel(evidence.failure)
                        : stateLabel(
                            evidence.ceremony_state ?? "confirmed",
                          )}</td
                    >
                    <td>{dateTime(evidence.occurred_at)}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
        {#if viewState.projection.evidence_truncated}
          <p class="alert alert-info m-3" role="status">
            This bounded view does not include every security evidence item.
          </p>
        {/if}
      </details>
    </div>
  {/if}
</section>

<style>
  .access-surface {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 1rem;
    min-width: 0;
  }

  .access-surface :global(.text-secondary) {
    color: var(--fasti-text-muted) !important;
  }

  .access-surface :global(.bg-success-lt) {
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 14%,
      var(--fasti-surface-paper)
    ) !important;
    color: color-mix(
      in srgb,
      var(--fasti-state-verified) 92%,
      var(--fasti-text-primary)
    ) !important;
  }

  .access-surface :global(.bg-warning-lt) {
    background: color-mix(
      in srgb,
      var(--fasti-state-attention) 14%,
      var(--fasti-surface-paper)
    ) !important;
    color: var(--fasti-state-attention) !important;
  }

  .access-surface :global(.bg-secondary-lt) {
    background: var(--fasti-surface-archive) !important;
    color: var(--fasti-text-primary) !important;
  }

  .access-surface :global(.btn-outline-secondary:not(:disabled)) {
    border-color: var(--fasti-text-muted);
    color: var(--fasti-text-primary);
  }

  .access-surface :global(.btn-outline-primary) {
    border-color: color-mix(
      in srgb,
      var(--fasti-action-primary) 82%,
      var(--fasti-text-primary)
    );
    color: color-mix(
      in srgb,
      var(--fasti-action-primary) 82%,
      var(--fasti-text-primary)
    );
  }

  .access-surface :global(.btn-outline-danger:not(:disabled)) {
    border-color: color-mix(
      in srgb,
      var(--tblr-danger) 82%,
      var(--fasti-text-primary)
    );
    color: color-mix(
      in srgb,
      var(--tblr-danger) 82%,
      var(--fasti-text-primary)
    );
  }

  .access-surface :global(.btn) {
    min-height: var(--fasti-touch-target-min);
    overflow-wrap: anywhere;
    white-space: normal;
  }

  .access-heading,
  .task-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  .access-heading > div,
  .task-copy {
    min-width: 0;
    flex: 1 1 18rem;
  }

  .access-state,
  .access-problem {
    max-width: 52rem;
  }

  .task-row {
    align-items: center;
    min-height: 4.5rem;
  }

  .task-row > :global(svg) {
    flex: 0 0 auto;
    margin-top: 0.2rem;
  }

  .task-row .badge,
  .task-row .btn {
    flex: 0 0 auto;
  }

  .access-surface :global(.badge) {
    white-space: normal;
    overflow-wrap: anywhere;
  }

  .access-detail {
    min-width: 0;
    padding: 0;
  }

  .access-detail > summary {
    padding: var(--tblr-list-group-item-padding-y, 1rem)
      var(--tblr-list-group-item-padding-x, 1rem);
    cursor: pointer;
    list-style: none;
  }

  .access-detail > summary::-webkit-details-marker {
    display: none;
  }

  .access-table {
    border-top: var(--tblr-border-width) solid var(--tblr-border-color);
  }

  .continuation-choice {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    min-height: 4.5rem;
    cursor: pointer;
  }

  .remember-browser-check {
    min-height: 2.75rem;
  }

  .first-run-layout,
  .first-run-steps {
    display: grid;
    min-width: 0;
    gap: 1rem;
  }

  .first-run-steps :global(.flex-fill) {
    min-width: 0;
  }

  .step-number {
    display: inline-grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    border: 1px solid var(--tblr-border-color);
    border-radius: 50%;
    font-variant-numeric: tabular-nums;
  }

  .evidence-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 1rem;
    margin: 0;
    padding: 1rem;
    border-top: var(--tblr-border-width) solid var(--tblr-border-color);
  }

  .evidence-grid div {
    min-width: 0;
  }

  .evidence-grid dt {
    color: var(--tblr-secondary-color);
    font-weight: 600;
  }

  .evidence-grid dd {
    margin: 0.25rem 0 0;
    overflow-wrap: anywhere;
  }

  .problem-evidence {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem 1.5rem;
  }

  .problem-evidence div {
    display: flex;
    gap: 0.4rem;
  }

  .problem-evidence dd {
    margin: 0;
  }

  .access-spinner {
    animation: access-spin 0.9s linear infinite;
  }

  @keyframes access-spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (max-width: 47.99rem) {
    .access-heading,
    .task-row,
    .first-run-steps :global(.list-group-item) {
      align-items: stretch;
      flex-direction: column;
    }

    .access-heading > div,
    .task-copy {
      flex-basis: auto;
    }

    .task-row .btn,
    .access-heading .btn {
      width: 100%;
    }

    .evidence-grid {
      grid-template-columns: 1fr;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .access-spinner {
      animation: none;
    }
  }

  @media (forced-colors: active) {
    .step-number,
    .access-table,
    .evidence-grid {
      border-color: CanvasText;
    }
  }
</style>
