<script lang="ts">
  import { IconExternalLink, IconX } from "@tabler/icons-svelte";

  interface Props {
    show: boolean;
    onClose: () => void;
    onOpenAccountSecurity: () => void;
  }

  let { show, onClose, onOpenAccountSecurity }: Props = $props();
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
        <div
          class="alert alert-warning mb-3"
          role="status"
          data-testid="account-access-unavailable"
        >
          <div>
            <div class="d-flex flex-wrap align-items-center gap-2 mb-2">
              <strong>Unavailable</strong>
              <span class="badge bg-warning-lt text-dark">PR C1 required</span>
            </div>
            <p class="mb-2">
              Browser sign-in and session management depend on PR C1: TrailBase
              identity bootstrap and production browser sessions.
            </p>
            <p class="mb-0">
              Continue local work that does not need an account. Operators must
              complete and merge PR C1 before enabling account access.
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
