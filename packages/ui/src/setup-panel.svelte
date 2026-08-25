<script lang="ts">
  import { IconArchive, IconLock, IconRefresh } from "@tabler/icons-svelte";
  import type { DesktopProblem, SetupViewState } from "./setup-types.js";

  let {
    state,
    problem = undefined,
    cleanupPending = false,
    onSetup,
  }: {
    state: SetupViewState;
    problem?: DesktopProblem;
    cleanupPending?: boolean;
    onSetup: () => void;
  } = $props();
</script>

<main id="main-content" class="setup-shell">
  <section class="setup-panel" aria-labelledby="setup-title" aria-live="polite">
    <div class="mark" aria-hidden="true">
      <IconArchive size={28} stroke={1.7} />
    </div>
    <p class="eyebrow">Local media record</p>
    <h1 id="setup-title">Fasti</h1>

    {#if state === "loading"}
      <p class="status">
        <span class="spin" aria-hidden="true"><IconRefresh size={20} /></span>
        Checking the local record…
      </p>
    {:else if state === "needs_setup"}
      <h2>Keep this record on this device</h2>
      <p>
        Fasti will create one local Chronicle and save its administrator
        credential in the system credential store.
      </p>
      <p class="assurance">
        <IconLock size={20} /> No credential is sent to a browser or network port.
      </p>
      <button type="button" onclick={onSetup}>Create local record</button>
    {:else if state === "blocked" && problem}
      <div class="problem" role="alert">
        <p class="eyebrow">{problem.code}</p>
        <h2>{problem.title}</h2>
        <p>{problem.detail}</p>
        <p><strong>Next:</strong> {problem.next_action}</p>
      </div>
      <button type="button" class="secondary" onclick={onSetup}>Retry</button>
    {:else}
      <h2>Review inbox</h2>
      <p class="empty-state">No items need review.</p>
      {#if cleanupPending}
        <p class="warning" role="status">
          Fasti is ready, but the system credential store could not remove the
          consumed setup proof.
        </p>
      {/if}
    {/if}
  </section>
</main>

<style>
  .setup-shell {
    min-height: 100%;
    display: grid;
    place-items: center;
    padding: 32px 20px;
  }

  .setup-panel {
    width: min(100%, 620px);
    border-top: 4px solid var(--fasti-brand-mark);
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 38%, transparent);
    padding: 32px 0 40px;
  }

  .mark {
    color: var(--fasti-brand-mark);
    margin-bottom: 24px;
  }

  .eyebrow {
    margin: 0 0 8px;
    color: var(--fasti-text-muted);
    font-family: var(--fasti-font-mono);
    font-size: 0.78rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h1,
  h2,
  p {
    margin-top: 0;
  }

  h1 {
    margin-bottom: 32px;
    font-family: var(--fasti-font-display);
    font-size: clamp(3rem, 10vw, 5rem);
    font-weight: 500;
    line-height: 0.9;
  }

  h2 {
    margin-bottom: 12px;
    font-family: var(--fasti-font-display);
    font-size: clamp(1.55rem, 5vw, 2.25rem);
    font-weight: 500;
  }

  p {
    max-width: 58ch;
    line-height: 1.65;
  }

  .status,
  .assurance {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .assurance {
    color: var(--fasti-state-verified);
  }

  button {
    min-width: 44px;
    min-height: var(--fasti-touch-target-min);
    margin-top: 16px;
    border: 2px solid var(--fasti-action-primary);
    border-radius: 2px;
    padding: 10px 18px;
    background: var(--fasti-action-primary);
    color: white;
    font: 700 1rem var(--fasti-font-body);
    cursor: pointer;
  }

  button.secondary {
    background: transparent;
    color: var(--fasti-action-primary);
  }

  button:focus-visible {
    outline: 3px solid var(--fasti-brand-gold);
    outline-offset: 3px;
  }

  .problem,
  .warning {
    border-left: 4px solid var(--fasti-state-attention);
    padding-left: 16px;
  }

  .empty-state {
    color: var(--fasti-text-muted);
  }

  .spin {
    display: inline-flex;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation: none;
    }
  }
</style>
