<script lang="ts">
  import {
    IconX,
    IconFingerprint,
    IconShieldLock,
    IconDevices2,
    IconLockAccess,
    IconIdBadge,
    IconCopy,
    IconCheck,
    IconRefresh,
    IconBrandGithub,
    IconBrandGoogle,
    IconBrandApple,
  } from "@tabler/icons-svelte";

  interface Props {
    show: boolean;
    onClose: () => void;
    onSignIn?: (method: string, credentials: unknown) => void;
  }

  let { show, onClose, onSignIn }: Props = $props();

  type AuthMethod = "passkey" | "oidc" | "pin" | "password" | "pat";

  let dialog: HTMLDialogElement | undefined;
  let activeMethod = $state<AuthMethod>("passkey");

  $effect(() => {
    if (!dialog) return;
    if (show && !dialog.open) dialog.showModal();
    else if (!show && dialog.open) dialog.close();
  });

  const METHOD_TABS: Array<{ id: AuthMethod; label: string; icon: any }> = [
    { id: "passkey", label: "Passkey", icon: IconFingerprint },
    { id: "oidc", label: "OIDC / SSO", icon: IconShieldLock },
    { id: "pin", label: "NuvioTV Device", icon: IconDevices2 },
    { id: "password", label: "Master Password", icon: IconLockAccess },
    { id: "pat", label: "Access Token", icon: IconIdBadge },
  ];

  // --- Passkey / WebAuthn ---
  const passkeySupported =
    typeof navigator !== "undefined" && "credentials" in navigator;
  let passkeyBusy = $state(false);
  let passkeyError = $state("");

  async function handlePasskeySignIn(): Promise<void> {
    if (!passkeySupported || passkeyBusy) return;
    passkeyBusy = true;
    passkeyError = "";
    try {
      // ponytail: challenge is a client-only placeholder — a real WebAuthn
      // relying party must issue this server-side once that backend exists.
      const credential = await navigator.credentials.get({
        publicKey: {
          challenge: crypto.getRandomValues(new Uint8Array(32)),
          userVerification: "preferred",
          timeout: 60000,
        },
      });
      onSignIn?.("passkey", credential);
    } catch (error) {
      passkeyError =
        error instanceof Error
          ? error.message
          : "Passkey sign-in was cancelled or failed.";
    } finally {
      passkeyBusy = false;
    }
  }

  // --- OIDC / SSO ---
  const OIDC_PROVIDERS = [
    { id: "authentik", name: "Authentik", icon: IconShieldLock },
    { id: "authelia", name: "Authelia", icon: IconShieldLock },
    { id: "keycloak", name: "Keycloak", icon: IconShieldLock },
    { id: "pocket_id", name: "Pocket ID", icon: IconShieldLock },
    { id: "github", name: "GitHub", icon: IconBrandGithub },
    { id: "google", name: "Google", icon: IconBrandGoogle },
    { id: "apple", name: "Apple", icon: IconBrandApple },
  ];
  let selectedOidcProvider = $state<string | null>(null);
  let oidcIssuerUrl = $state("");

  function handleSelectOidcProvider(id: string): void {
    selectedOidcProvider = id;
  }

  function handleOidcSubmit(e: Event): void {
    e.preventDefault();
    if (oidcIssuerUrl.trim().length === 0) return;
    onSignIn?.("oidc", {
      provider: selectedOidcProvider,
      issuerUrl: oidcIssuerUrl.trim(),
    });
  }

  // --- NuvioTV Device PIN ---
  const PIN_TTL_MS = 10 * 60 * 1000;

  function generatePairingCode(): string {
    // ponytail: still client-only and never registered with a host, so no
    // device can actually validate this code -- same placeholder status as
    // the WebAuthn challenge above. Real issuance needs a host command with
    // a server-tracked expiry. This only fixes the randomness: a predictable
    // Math.random() PIN would make guessing collisions dramatically easier
    // once real validation exists.
    const buffer = new Uint32Array(1);
    crypto.getRandomValues(buffer);
    return String(100000 + (buffer[0] % 900000));
  }

  let pairingCode = $state(generatePairingCode());
  let pairingExpiresAt = $state(Date.now() + PIN_TTL_MS);
  let nowTick = $state(Date.now());
  let pinCopied = $state(false);

  $effect(() => {
    const interval = setInterval(() => (nowTick = Date.now()), 1000);
    return () => clearInterval(interval);
  });

  const pinSecondsRemaining = $derived(
    Math.max(0, Math.round((pairingExpiresAt - nowTick) / 1000)),
  );
  const pinExpired = $derived(pinSecondsRemaining <= 0);
  const pinCountdownLabel = $derived(
    `${Math.floor(pinSecondsRemaining / 60)}:${String(pinSecondsRemaining % 60).padStart(2, "0")}`,
  );

  function handleRegeneratePin(): void {
    pairingCode = generatePairingCode();
    pairingExpiresAt = Date.now() + PIN_TTL_MS;
    pinCopied = false;
  }

  async function handleCopyPin(): Promise<void> {
    try {
      await navigator.clipboard.writeText(pairingCode);
      pinCopied = true;
      setTimeout(() => (pinCopied = false), 2000);
    } catch {
      pinCopied = false;
    }
  }

  // --- Local Master Password ---
  let passwordUsername = $state("");
  let passwordValue = $state("");
  let sessionTimeoutMinutes = $state(60);

  function handlePasswordSubmit(e: Event): void {
    e.preventDefault();
    if (!passwordUsername.trim() || !passwordValue) return;
    onSignIn?.("password", {
      username: passwordUsername.trim(),
      password: passwordValue,
      sessionTimeoutMinutes,
    });
    // Clear the secret out of this component's own state immediately after
    // handing it off -- it should not keep sitting in memory once onSignIn
    // has it, and a caller that re-opens this modal must not find it prefilled.
    passwordValue = "";
  }

  // --- Personal Access Token ---
  let patValue = $state("");
  const patValid = $derived(patValue.trim().startsWith("fst_pat_"));

  function handlePatSubmit(e: Event): void {
    e.preventDefault();
    if (!patValid) return;
    onSignIn?.("pat", { token: patValue.trim() });
    // Same reasoning as handlePasswordSubmit: don't keep the token sitting
    // in this component's state once it's been handed off.
    patValue = "";
  }
</script>

<dialog
  bind:this={dialog}
  class="modal-backdrop"
  aria-labelledby="auth-modal-title"
  oncancel={onClose}
  onclick={(event) => {
    if (event.target === event.currentTarget) onClose();
  }}
>
  <div class="modal-card">
    <div class="modal-header">
      <h2 id="auth-modal-title" class="modal-title">Sign in to Fasti</h2>
      <button
        type="button"
        class="close-btn"
        onclick={onClose}
        aria-label="Close dialog"
      >
        <IconX size={18} />
      </button>
    </div>

    <nav class="method-tabs" aria-label="Sign-in method">
      {#each METHOD_TABS as tab}
        <button
          type="button"
          class="method-tab-btn"
          class:active={activeMethod === tab.id}
          onclick={() => (activeMethod = tab.id)}
        >
          <tab.icon size={16} />
          {tab.label}
        </button>
      {/each}
    </nav>

    <div class="modal-body">
      {#if activeMethod === "passkey"}
        <div class="method-pane">
          <p class="body-desc">
            Use a passkey stored on this device or a nearby security key.
          </p>
          {#if passkeySupported}
            <button
              type="button"
              class="btn-primary-full"
              onclick={handlePasskeySignIn}
              disabled={passkeyBusy}
            >
              <IconFingerprint size={18} />
              {passkeyBusy ? "Waiting for passkey…" : "Sign in with Passkey"}
            </button>
            {#if passkeyError}
              <p class="inline-problem" role="alert">{passkeyError}</p>
            {/if}
          {:else}
            <p class="inline-problem" role="alert">
              This browser does not support WebAuthn passkeys. Try Chrome,
              Safari, Edge, or Firefox on a device with platform authentication
              enabled.
            </p>
          {/if}
        </div>
      {:else if activeMethod === "oidc"}
        <form class="method-pane" onsubmit={handleOidcSubmit}>
          <p class="body-desc">
            Sign in with your identity provider. Pick a common provider or enter
            a custom issuer URL.
          </p>
          <div class="provider-grid">
            {#each OIDC_PROVIDERS as prov}
              <button
                type="button"
                class="provider-btn"
                class:selected={selectedOidcProvider === prov.id}
                onclick={() => handleSelectOidcProvider(prov.id)}
              >
                <prov.icon size={18} />
                <span>{prov.name}</span>
              </button>
            {/each}
          </div>
          <div class="form-field">
            <label for="oidc-issuer-url">Issuer URL</label>
            <input
              id="oidc-issuer-url"
              type="url"
              class="form-input"
              placeholder="https://auth.example.com/application/o/fasti/"
              bind:value={oidcIssuerUrl}
              required
            />
          </div>
          <button type="submit" class="btn-primary-full">
            Continue with SSO
          </button>
        </form>
      {:else if activeMethod === "pin"}
        <div class="method-pane">
          <p class="body-desc">
            Enter this pairing code on your NuvioTV device to link it to your
            Fasti account.
          </p>
          <div class="pin-display" class:expired={pinExpired}>
            {pairingCode.slice(0, 3)}
            {pairingCode.slice(3)}
          </div>
          <p class="pin-countdown" class:expired={pinExpired}>
            {#if pinExpired}
              Code expired — regenerate to continue.
            {:else}
              Expires in {pinCountdownLabel}
            {/if}
          </p>
          <div class="pin-actions">
            <button type="button" class="btn-secondary" onclick={handleCopyPin}>
              {#if pinCopied}
                <IconCheck size={16} /> Copied
              {:else}
                <IconCopy size={16} /> Copy code
              {/if}
            </button>
            <button
              type="button"
              class="btn-secondary"
              onclick={handleRegeneratePin}
            >
              <IconRefresh size={16} /> Regenerate
            </button>
          </div>
        </div>
      {:else if activeMethod === "password"}
        <form class="method-pane" onsubmit={handlePasswordSubmit}>
          <div class="form-field">
            <label for="auth-username">Username</label>
            <input
              id="auth-username"
              type="text"
              class="form-input"
              autocomplete="username"
              bind:value={passwordUsername}
              required
            />
          </div>
          <div class="form-field">
            <label for="auth-password">Master Password</label>
            <input
              id="auth-password"
              type="password"
              class="form-input"
              autocomplete="current-password"
              bind:value={passwordValue}
              required
            />
          </div>
          <div class="form-field">
            <label for="auth-session-timeout">Session Timeout</label>
            <select
              id="auth-session-timeout"
              class="form-input"
              bind:value={sessionTimeoutMinutes}
            >
              <option value={15}>15 minutes</option>
              <option value={60}>1 hour</option>
              <option value={480}>8 hours</option>
              <option value={1440}>24 hours</option>
              <option value={0}>Never</option>
            </select>
          </div>
          <button type="submit" class="btn-primary-full">Sign In</button>
        </form>
      {:else if activeMethod === "pat"}
        <form class="method-pane" onsubmit={handlePatSubmit}>
          <p class="body-desc">
            Sign in with a scoped Personal Access Token generated in Settings.
          </p>
          <div class="form-field">
            <label for="auth-pat">Access Token</label>
            <input
              id="auth-pat"
              type="password"
              class="form-input mono"
              placeholder="fst_pat_..."
              autocomplete="off"
              spellcheck="false"
              bind:value={patValue}
            />
          </div>
          {#if patValue.length > 0 && !patValid}
            <p class="inline-problem" role="alert">
              Tokens must start with "fst_pat_".
            </p>
          {/if}
          <button type="submit" class="btn-primary-full" disabled={!patValid}>
            Sign In
          </button>
        </form>
      {/if}
    </div>
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
    background: rgba(0, 0, 0, 0.5);
  }

  .modal-backdrop:not([open]) {
    display: none;
  }

  .modal-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 8px;
    width: 100%;
    max-width: 480px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.2);
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .modal-title {
    font-family: var(--fasti-font-display);
    font-size: 1.2rem;
    margin: 0;
  }
  .close-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 44px;
    min-height: 44px;
    background: transparent;
    border: none;
    cursor: pointer;
    color: var(--fasti-text-muted);
  }

  .modal-card :is(button, input, select):focus-visible {
    outline: 3px solid var(--fasti-action-primary);
    outline-offset: 2px;
  }

  .method-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    padding: 8px 12px 0;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    background: var(--fasti-surface-archive);
  }
  .method-tab-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 10px 12px;
    min-height: 44px;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--fasti-text-muted);
    cursor: pointer;
    white-space: nowrap;
  }
  .method-tab-btn.active {
    color: var(--fasti-action-primary);
    border-bottom-color: var(--fasti-action-primary);
  }

  .modal-body {
    padding: 20px;
  }

  .method-pane {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .body-desc {
    font-size: 0.88rem;
    color: var(--fasti-text-muted);
    margin: 0;
  }

  .form-field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .form-field label {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--fasti-text-muted);
  }
  .form-input {
    height: 44px;
    min-height: 44px;
    padding: 8px 12px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-size: 0.9rem;
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }
  .form-input.mono {
    font-family: var(--fasti-font-mono);
  }

  .btn-primary-full {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 10px 16px;
    background: var(--fasti-action-primary);
    color: white;
    border: none;
    border-radius: 4px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-primary-full:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    min-height: 44px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-weight: 600;
    font-size: 0.85rem;
    cursor: pointer;
  }

  .inline-problem {
    margin: 0;
    font-size: 0.86rem;
    color: var(--fasti-state-error, #b42318);
  }

  .provider-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 8px;
  }
  .provider-btn {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 12px 8px;
    background: var(--fasti-surface-archive);
    border: 2px solid transparent;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.78rem;
    font-weight: 600;
  }
  .provider-btn.selected {
    border-color: var(--fasti-action-primary);
    background: color-mix(in srgb, var(--fasti-action-primary) 8%, transparent);
  }

  .pin-display {
    text-align: center;
    font-family: var(--fasti-font-mono);
    font-size: 2.2rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    padding: 18px;
    background: var(--fasti-surface-archive);
    border-radius: 6px;
    color: var(--fasti-text-primary);
  }
  .pin-display.expired {
    opacity: 0.5;
  }
  .pin-countdown {
    text-align: center;
    margin: 0;
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
  }
  .pin-countdown.expired {
    color: var(--fasti-state-error, #b42318);
    font-weight: 600;
  }
  .pin-actions {
    display: flex;
    gap: 10px;
    justify-content: center;
  }
</style>
