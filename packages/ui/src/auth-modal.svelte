<script lang="ts">
  import { IconExternalLink, IconX } from "@tabler/icons-svelte";
  import { dialogFocus } from "./dialog-focus.js";
  import type { AccessProjectionResponse } from "./types.js";

  interface Props {
    show: boolean;
    onClose: () => void;
    onOpenAccountSecurity: () => void;
    projection?: AccessProjectionResponse;
    problem?: string;
  }

  let { show, onClose, onOpenAccountSecurity, projection, problem }: Props =
    $props();
  let dialog = $state<HTMLDialogElement>();

  $effect(() => {
    if (!dialog) return;
    if (show && !dialog.open) {
      dialog.showModal();
    } else if (!show && dialog.open) {
      dialog.close();
    }
  });
</script>

<dialog
  bind:this={dialog}
  use:dialogFocus
  class="auth-dialog"
  aria-labelledby="auth-modal-title"
  oncancel={(event) => {
    event.preventDefault();
    onClose();
  }}
  onclick={(event) => {
    if (event.target === event.currentTarget) onClose();
  }}
>
  {#if show}
    <section class="card modal-card">
      <header class="card-header modal-header">
        <div>
          <h2 id="auth-modal-title" class="card-title">Account access</h2>
          <p class="card-subtitle text-secondary mb-0">
            Check access status or open the permanent account task map.
          </p>
        </div>
        <button
          type="button"
          class="btn btn-icon btn-ghost-secondary"
          onclick={onClose}
          aria-label="Close account dialog"
        >
          <IconX size={18} aria-hidden="true" />
        </button>
      </header>

      <div class="card-body modal-body">
        {#if problem}
          <div class="alert alert-warning" role="status">{problem}</div>
        {/if}
        <div
          class="alert mb-3"
          class:alert-success={projection}
          class:alert-warning={problem && !projection}
          class:alert-info={!projection && !problem}
          role="status"
        >
          <div>
            <div class="d-flex flex-wrap align-items-center gap-2 mb-2">
              <strong
                >{projection
                  ? "Signed in"
                  : problem
                    ? "Access state unavailable"
                    : "Sign-in required"}</strong
              >
              <span
                class="badge"
                class:bg-success-lt={projection}
                class:bg-secondary-lt={!projection}
              >
                {projection
                  ? projection.authentication.method.replaceAll("_", " ")
                  : problem
                    ? "Not confirmed"
                    : "No active Fasti session"}
              </span>
            </div>
            <p class="mb-0">
              {projection
                ? `${projection.sessions.length} active browser ${projection.sessions.length === 1 ? "session" : "sessions"}.`
                : problem
                  ? "Open Account and security to retry the governed access check."
                  : "Open Account and security to sign in with TrailBase and review the exact access state."}
            </p>
          </div>
        </div>

        <button
          type="button"
          class="btn btn-primary"
          onclick={onOpenAccountSecurity}
        >
          Open Account and security
          <IconExternalLink size={16} aria-hidden="true" />
        </button>
      </div>
    </section>
  {/if}
</dialog>

<style>
  .auth-dialog {
    width: min(42rem, calc(100vw - 2rem));
    max-height: calc(100vh - 2rem);
    margin: auto;
    padding: 0;
    border: 0;
    border-radius: calc(
      var(--tblr-border-radius-lg, 0.5rem) * var(--tblr-border-radius-scale, 1)
    );
    background: transparent;
    color: inherit;
    overflow: auto;
  }

  .auth-dialog::backdrop {
    background: rgb(15 23 42 / 58%);
  }

  .auth-dialog:not([open]) {
    display: none;
  }

  .modal-card {
    margin: 0;
  }

  .modal-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  .modal-body {
    min-width: 0;
  }

  .auth-dialog :global(.text-secondary) {
    color: var(--fasti-text-muted) !important;
  }

  .auth-dialog :global(.bg-secondary-lt) {
    background: var(--fasti-surface-archive) !important;
    color: var(--fasti-text-primary) !important;
  }

  .auth-dialog :global(.bg-success-lt) {
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 14%,
      var(--fasti-surface-paper)
    ) !important;
    color: var(--fasti-state-verified) !important;
  }

  :global(.auth-dialog .btn) {
    min-height: 44px;
  }

  @media (max-width: 36rem) {
    .auth-dialog {
      width: calc(100vw - 1rem);
      max-height: calc(100vh - 1rem);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .auth-dialog {
      scroll-behavior: auto;
    }
  }
</style>
