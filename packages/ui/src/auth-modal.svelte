<script lang="ts">
  import {
    IconDevices2,
    IconFingerprint,
    IconIdBadge,
    IconLockAccess,
    IconShieldLock,
    IconX,
  } from "@tabler/icons-svelte";

  interface Props {
    show: boolean;
    onClose: () => void;
    onSubmit: (credential: string) => void | Promise<void>;
  }

  let { show, onClose, onSubmit }: Props = $props();
  let dialog: HTMLDialogElement | undefined;
  let credential = $state("");
  let problem = $state("");
  let submitting = $state(false);
  type AuthMethod = "passkey" | "oidc" | "device" | "password" | "token";
  let activeMethod = $state<AuthMethod>("token");
  const methods = [
    { id: "passkey", label: "Passkey", icon: IconFingerprint },
    { id: "oidc", label: "OIDC / SSO", icon: IconShieldLock },
    { id: "device", label: "NuvioTV Device", icon: IconDevices2 },
    { id: "password", label: "Master Password", icon: IconLockAccess },
    { id: "token", label: "API Credential", icon: IconIdBadge },
  ] as const;
  const credentialValid = $derived(/^[0-9a-f]{64}$/i.test(credential.trim()));

  $effect(() => {
    if (!dialog) return;
    if (show && !dialog.open) dialog.showModal();
    else if (!show && dialog.open) dialog.close();
  });

  function close(): void {
    credential = "";
    problem = "";
    activeMethod = "token";
    onClose();
  }

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (!credentialValid || submitting) return;
    submitting = true;
    problem = "";
    try {
      await onSubmit(credential.trim());
      close();
    } catch (error) {
      problem =
        error instanceof Error
          ? error.message
          : "Fasti rejected this local bearer credential.";
    } finally {
      submitting = false;
    }
  }
</script>

<dialog
  bind:this={dialog}
  class="modal modal-blur show modal-backdrop"
  aria-labelledby="auth-modal-title"
  oncancel={close}
  onclick={(event) => {
    if (event.target === event.currentTarget) close();
  }}
>
  <div class="modal-dialog modal-dialog-centered">
    <form class="modal-content modal-card" onsubmit={submit}>
      <div class="modal-header">
        <h2 id="auth-modal-title">Connect to local Fasti</h2>
        <button
          type="button"
          class="btn btn-icon btn-ghost-secondary close-btn"
          onclick={close}
          aria-label="Close dialog"
        >
          <IconX size={18} />
        </button>
      </div>

      <div
        class="nav nav-tabs method-tabs"
        role="tablist"
        aria-label="Authentication method"
      >
        {#each methods as method}
          <button
            type="button"
            role="tab"
            class="nav-link method-tab"
            class:active={activeMethod === method.id}
            aria-selected={activeMethod === method.id}
            aria-controls="auth-panel"
            onclick={() => (activeMethod = method.id)}
          >
            <method.icon size={16} aria-hidden="true" />
            {method.label}
          </button>
        {/each}
      </div>

      <div id="auth-panel" class="modal-body" role="tabpanel">
        {#if activeMethod === "token"}
          <p id="credential-help">
            Paste an active 64-character scoped API credential. Record access
            needs <code>identity_read</code>. Fasti keeps the secret only in
            this tab's memory and clears it when the page reloads.
          </p>
          <label class="form-label" for="session-credential"
            >API client credential</label
          >
          <input
            id="session-credential"
            type="password"
            class="form-control credential-input"
            autocomplete="off"
            spellcheck="false"
            aria-describedby="credential-help credential-format"
            bind:value={credential}
          />
          <p
            id="credential-format"
            class:problem={credential.length > 0 && !credentialValid}
          >
            Enter exactly 64 hexadecimal characters.
          </p>
          {#if problem}
            <p class="problem" role="alert">{problem}</p>
          {/if}
          <button
            type="submit"
            class="btn btn-primary submit-btn"
            disabled={!credentialValid || submitting}
          >
            {submitting ? "Connecting…" : "Connect"}
          </button>
        {:else if activeMethod === "passkey"}
          <div class="unavailable-state">
            <IconFingerprint size={28} aria-hidden="true" />
            <h3>Passkey sign-in is not active</h3>
            <p>
              Fasti needs a host-owned WebAuthn relying party, server-issued
              challenges, credential registration, and recovery before this
              method can accept input. The Workbench does not create local
              placeholder challenges.
            </p>
          </div>
        {:else if activeMethod === "oidc"}
          <div class="unavailable-state">
            <IconShieldLock size={28} aria-hidden="true" />
            <h3>OIDC and SSO are not active</h3>
            <p>
              This method needs a registered issuer, browser session policy,
              consent, refresh, revocation, and the same canonical Fasti scope
              vocabulary as API credentials. Dynamic client registration stays
              off until that contract is governed.
            </p>
          </div>
        {:else if activeMethod === "device"}
          <div class="unavailable-state">
            <IconDevices2 size={28} aria-hidden="true" />
            <h3>NuvioTV device pairing is not active</h3>
            <p>
              Device pairing needs a server-issued device code, a separate
              browser approval step, bounded expiry, client scopes, polling, and
              revocation. Fasti does not display an unregistered local PIN.
            </p>
          </div>
        {:else}
          <div class="unavailable-state">
            <IconLockAccess size={28} aria-hidden="true" />
            <h3>Master-password sign-in is not active</h3>
            <p>
              Account passwords belong to a host-owned browser session with rate
              limits, CSRF protection, recent-authentication policy, and
              recovery. The Workbench never sends a raw password to the records
              API.
            </p>
          </div>
        {/if}
      </div>
    </form>
  </div>
</dialog>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 9999;
    width: 100%;
    max-width: none;
    height: 100%;
    max-height: none;
    margin: 0;
    border: 0;
    background: transparent;
    display: grid;
    place-items: center;
    padding: 16px;
  }

  .modal-backdrop::backdrop {
    background: rgba(0, 0, 0, 0.55);
  }

  .modal-backdrop:not([open]) {
    display: none;
  }

  .modal-card {
    width: min(100%, 480px);
    overflow: hidden;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 8px;
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  .modal-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 18px 20px;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  h2 {
    margin: 0;
    font-family: var(--fasti-font-display);
    font-size: 1.25rem;
  }

  .close-btn {
    display: inline-grid;
    min-width: 44px;
    min-height: 44px;
    place-items: center;
    border: 0;
    background: transparent;
    color: var(--fasti-text-muted);
    cursor: pointer;
  }

  .method-tabs {
    display: flex;
    overflow-x: auto;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    background: var(--fasti-surface-archive);
  }

  .method-tab {
    display: inline-flex;
    flex: 0 0 auto;
    align-items: center;
    gap: 6px;
    min-height: 44px;
    padding: 8px 12px;
    border: 0;
    border-bottom: 3px solid transparent;
    background: transparent;
    color: var(--fasti-text-muted);
    font-size: 0.8rem;
    font-weight: 700;
    cursor: pointer;
  }

  .method-tab.active {
    border-bottom-color: var(--fasti-action-primary);
    color: var(--fasti-action-primary);
  }

  .modal-body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 20px;
  }

  .modal-body > p {
    margin: 0;
    color: var(--fasti-text-muted);
    line-height: 1.5;
  }

  .unavailable-state {
    display: grid;
    gap: 10px;
    color: var(--fasti-text-muted);
  }

  .unavailable-state > :global(svg) {
    color: var(--fasti-action-primary);
  }

  .unavailable-state h3,
  .unavailable-state p {
    margin: 0;
  }

  .unavailable-state h3 {
    color: var(--fasti-text-primary);
    font-family: var(--fasti-font-display);
  }

  .unavailable-state p {
    line-height: 1.55;
  }

  label {
    font-weight: 700;
  }

  .credential-input {
    min-height: 44px;
    padding: 8px 12px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 35%, transparent);
    border-radius: 4px;
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
    font-family: var(--fasti-font-mono);
  }

  .modal-body > .problem {
    color: var(--fasti-state-error, #b42318);
  }

  .submit-btn {
    min-height: 44px;
    border: 0;
    border-radius: 4px;
    background: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
    font-weight: 750;
    cursor: pointer;
  }

  .submit-btn:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  :is(button, input):focus-visible {
    outline: 3px solid var(--fasti-action-primary);
    outline-offset: 2px;
  }
</style>
