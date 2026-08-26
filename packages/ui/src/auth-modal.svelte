<script lang="ts">
  import { IconInfoCircle, IconShieldCheck } from "@tabler/icons-svelte";

  interface Props {
    open: boolean;
    currentUser?: {
      username: string;
      displayName: string;
      authMethod: "passkey" | "oidc" | "pin" | "pat" | "password" | "anonymous";
      role: "admin" | "member";
    } | null;
    onClose: () => void;
    onSignIn: (method: string, data: unknown) => void;
    onSignOut: () => void;
  }

  let {
    open,
    currentUser = null,
    onClose,
    onSignIn: _onSignIn,
    onSignOut,
  }: Props = $props();

  function handleKeyDown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

{#if open}
  <div
    class="modal modal-blur fade show d-block"
    tabindex="-1"
    role="dialog"
    aria-modal="true"
    aria-labelledby="auth-modal-title"
    style="background: rgba(0, 0, 0, 0.65); z-index: 1060;"
  >
    <div class="modal-dialog modal-dialog-centered" role="document">
      <div class="modal-content border shadow-lg">
        <div class="modal-header">
          <div class="d-flex align-items-center gap-2">
            <div class="p-2 rounded bg-primary-lt" aria-hidden="true">
              <IconShieldCheck size={24} class="text-primary" />
            </div>
            <div>
              <h2 class="modal-title h3 mb-0" id="auth-modal-title">
                {currentUser ? "Account & Identity" : "Authentication"}
              </h2>
              <p class="text-muted small mb-0">
                Fasti only shows authentication methods that the host can verify.
              </p>
            </div>
          </div>
          <button
            type="button"
            class="btn-close"
            onclick={onClose}
            aria-label="Close authentication dialog"
          ></button>
        </div>

        <div class="modal-body p-4">
          {#if currentUser}
            <div class="card p-3 border mb-3 bg-body-tertiary">
              <div class="d-flex align-items-center justify-content-between gap-3 flex-wrap">
                <div>
                  <h3 class="h4 mb-1">{currentUser.displayName}</h3>
                  <div class="text-muted font-monospace small">
                    @{currentUser.username} · {currentUser.role}
                  </div>
                  <span class="badge bg-success-lt mt-2">
                    Authenticated via {currentUser.authMethod.toUpperCase()}
                  </span>
                </div>
                <button
                  type="button"
                  class="btn btn-outline-danger"
                  onclick={() => {
                    onSignOut();
                    onClose();
                  }}
                >
                  Sign out
                </button>
              </div>
            </div>
          {:else}
            <div class="alert alert-info d-flex gap-2 align-items-start mb-0" role="status">
              <IconInfoCircle size={20} class="flex-shrink-0 mt-1" aria-hidden="true" />
              <div>
                <strong>Sign-in is not available in this prototype.</strong>
                <div class="mt-1">
                  Password, personal access token, passkey, OIDC, and device-pairing flows are
                  intentionally disabled until a host-side verifier and the matching public
                  capability contract are active. No credential is accepted, stored, or reported
                  as verified by this dialog.
                </div>
              </div>
            </div>
          {/if}
        </div>

        <div class="modal-footer">
          <button type="button" class="btn btn-primary" onclick={onClose}>Close</button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal :global(button:focus-visible) {
    outline: 3px solid var(--fasti-action-primary, currentColor);
    outline-offset: 2px;
  }

  .modal :global(.btn) {
    min-height: 44px;
  }
</style>
