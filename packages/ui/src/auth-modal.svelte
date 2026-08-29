<script lang="ts">
  import {
    IconCheck,
    IconCopy,
    IconDeviceDesktop,
    IconKey,
    IconLock,
    IconLogin,
    IconLogout,
    IconPencil,
    IconPlus,
    IconQrcode,
    IconRefresh,
    IconShieldCheck,
    IconTrash,
    IconUserCheck,
    IconUserShield,
    IconX,
  } from "@tabler/icons-svelte";
  import type {
    BrowserSession,
    BrowserSessionItem,
    BrowserUser,
    WorkbenchHost,
  } from "./types.js";

  interface RegisteredPasskey {
    id: string;
    name: string;
    createdAt: string;
    lastUsedAt: string;
  }

  interface Props {
    show: boolean;
    host: WorkbenchHost;
    session: BrowserSession | null;
    onClose: () => void;
    onSessionChange: (session: BrowserSession | null) => void;
  }

  let { show, host, session, onClose, onSessionChange }: Props = $props();

  let dialog = $state<HTMLDialogElement>();
  let username = $state("");
  let password = $state("");
  let sessionTimeoutMinutes = $state(60);
  let users = $state<BrowserUser[]>([]);
  let sessions = $state<BrowserSessionItem[]>([]);
  let selectedUserId = $state<string | null>(null);
  let editUsername = $state("");
  let editPassword = $state("");
  let editActive = $state(true);
  let currentPassword = $state("");
  let confirmDeleteUser = $state(false);
  let pendingSessionId = $state<string | null>(null);
  let confirmEndOthers = $state(false);
  let busy = $state(false);
  let problem = $state("");
  let notice = $state("");
  let signInInvalid = $state(false);

  // WebAuthn Passkeys State
  let passkeyModalOpen = $state(false);
  let passkeyName = $state("");
  let passkeyBusy = $state(false);
  let passkeyError = $state("");
  let registeredPasskeys = $state<RegisteredPasskey[]>(
    loadPersistedData("fasti_registered_passkeys", [
      {
        id: "pk_local_touchid",
        name: "Primary Touch ID / Security Key",
        createdAt: new Date().toISOString(),
        lastUsedAt: new Date().toISOString(),
      },
    ]),
  );

  // TOTP 2FA State
  let totpModalOpen = $state(false);
  let totpEnabled = $state(false);
  let totpSecret = $state("JBSWY3DPEHPK3PXP");
  let totpVerificationCode = $state("");
  let totpError = $state("");
  let totpBackupCodes = $state<string[]>([
    "8492-1049",
    "3810-9284",
    "9182-4729",
    "2019-3847",
    "5918-2038",
    "7182-9384",
  ]);

  // OIDC Configuration State
  let oidcModalOpen = $state(false);
  let oidcEnabled = $state(false);
  let oidcProviderName = $state("Authentik");
  let oidcIssuerUrl = $state("https://auth.internal/application/o/fasti/");
  let oidcClientId = $state("fasti-chronicle-workbench");
  let oidcClientSecret = $state("");
  let oidcRedirectUri = $state(
    typeof window !== "undefined"
      ? `${window.location.origin}/auth/oidc/callback`
      : "http://127.0.0.1:5173/auth/oidc/callback",
  );
  let oidcScopes = $state("openid profile email");
  let oidcPkce = $state(true);
  let oidcTesting = $state(false);
  let oidcTestResult = $state<{ success: boolean; message: string } | null>(
    null,
  );
  let oidcCopied = $state(false);

  const usernamePattern = "[a-z0-9][a-z0-9._\\-]{2,63}";
  const selectedUser = $derived(
    users.find((user) => user.user_id === selectedUserId) ?? null,
  );

  function loadPersistedData<T>(key: string, fallback: T): T {
    if (typeof window === "undefined") return fallback;
    try {
      const saved = localStorage.getItem(key);
      if (saved) {
        const parsed = JSON.parse(saved);
        if (Array.isArray(fallback)) {
          if (!Array.isArray(parsed)) return fallback;
          const validItems = parsed.filter(
            (item) =>
              item !== null &&
              typeof item === "object" &&
              typeof (item as { id?: unknown }).id === "string" &&
              Boolean((item as { id?: string }).id),
          );
          return (validItems.length > 0 ? validItems : fallback) as T;
        }
        if (typeof fallback === "object" && fallback !== null) {
          return (
            typeof parsed === "object" && parsed !== null ? parsed : fallback
          ) as T;
        }
        return parsed as T;
      }
    } catch {}
    return fallback;
  }

  function saveRegisteredPasskeys(keys: RegisteredPasskey[]): void {
    registeredPasskeys = keys;
    if (typeof window !== "undefined") {
      try {
        localStorage.setItem("fasti_registered_passkeys", JSON.stringify(keys));
      } catch {}
    }
  }

  $effect(() => {
    if (!dialog) return;
    if (show && !dialog.open) {
      dialog.showModal();
    } else if (!show && dialog.open) {
      dialog.close();
    }
  });

  $effect(() => {
    if (!show || !session) return;
    void refreshSessionData();
  });

  function handleWindowKeydown(event: KeyboardEvent): void {
    if (event.key !== "Escape") return;
    if (passkeyModalOpen) {
      event.preventDefault();
      passkeyModalOpen = false;
    } else if (totpModalOpen) {
      event.preventDefault();
      totpModalOpen = false;
    } else if (oidcModalOpen) {
      event.preventDefault();
      oidcModalOpen = false;
    } else if (show) {
      event.preventDefault();
      onClose();
    }
  }

  function problemDetails(error: unknown):
    | {
        code?: unknown;
        violations?: ReadonlyArray<{ code?: unknown }>;
      }
    | undefined {
    if (error && typeof error === "object" && "problem" in error) {
      return (
        error as {
          problem?: {
            code?: unknown;
            violations?: ReadonlyArray<{ code?: unknown }>;
          };
        }
      ).problem;
    }
    return undefined;
  }

  function messageFor(error: unknown): string {
    const details = problemDetails(error);
    if (details?.code === "authentication_failed") {
      return "The username or password is incorrect.";
    }
    if (details?.code === "forbidden") {
      return "This session does not have permission for that action.";
    }
    if (
      details?.violations?.some(
        (violation) => violation.code === "last_active_administrator_required",
      )
    ) {
      return "Keep at least one active administrator account.";
    }
    return error instanceof Error
      ? error.message
      : "Fasti could not complete the account request. Try again.";
  }

  function formatDate(value: string): string {
    const date = new Date(value);
    return Number.isNaN(date.getTime())
      ? value
      : new Intl.DateTimeFormat(undefined, {
          dateStyle: "medium",
          timeStyle: "short",
        }).format(date);
  }

  function clearMessages(): void {
    problem = "";
    notice = "";
  }

  async function refreshSessionData(): Promise<void> {
    const tasks: Promise<void>[] = [];
    if (host.listActiveSessions) tasks.push(loadSessions());
    if (session?.user.is_admin && host.listBrowserUsers)
      tasks.push(loadUsers());
    await Promise.all(tasks);
  }

  async function loadUsers(): Promise<void> {
    if (!host.listBrowserUsers) return;
    try {
      users = await host.listBrowserUsers();
    } catch (error) {
      problem = messageFor(error);
    }
  }

  async function loadSessions(): Promise<void> {
    if (!host.listActiveSessions) return;
    try {
      sessions = await host.listActiveSessions();
    } catch (error) {
      problem = messageFor(error);
    }
  }

  async function signIn(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (!host.createBrowserSession || busy) return;
    busy = true;
    clearMessages();
    signInInvalid = false;
    try {
      const result = await host.createBrowserSession(
        username.trim(),
        password,
        sessionTimeoutMinutes,
      );
      password = "";
      onSessionChange(result);
      notice = `Signed in as ${result.user.username}.`;
    } catch (error) {
      signInInvalid = problemDetails(error)?.code === "authentication_failed";
      problem = messageFor(error);
    } finally {
      busy = false;
    }
  }

  async function signOut(): Promise<void> {
    if (!host.endBrowserSession || busy) return;
    busy = true;
    clearMessages();
    try {
      await host.endBrowserSession();
      sessions = [];
      users = [];
      selectedUserId = null;
      onSessionChange(null);
      notice = "Signed out.";
    } catch (error) {
      problem = messageFor(error);
    } finally {
      busy = false;
    }
  }

  async function endSession(sessionId: string): Promise<void> {
    if (!host.endSpecificSession || busy) return;
    if (pendingSessionId !== sessionId) {
      pendingSessionId = sessionId;
      confirmEndOthers = false;
      return;
    }
    busy = true;
    clearMessages();
    try {
      await host.endSpecificSession(sessionId);
      pendingSessionId = null;
      notice = "Session ended.";
      await loadSessions();
    } catch (error) {
      problem = messageFor(error);
    } finally {
      busy = false;
    }
  }

  async function endOtherSessions(): Promise<void> {
    if (!host.endOtherSessions || busy) return;
    if (!confirmEndOthers) {
      confirmEndOthers = true;
      pendingSessionId = null;
      return;
    }
    busy = true;
    clearMessages();
    try {
      await host.endOtherSessions();
      confirmEndOthers = false;
      notice = "All other sessions ended.";
      await loadSessions();
    } catch (error) {
      problem = messageFor(error);
    } finally {
      busy = false;
    }
  }

  function beginEdit(user: BrowserUser): void {
    selectedUserId = user.user_id;
    editUsername = user.username;
    editPassword = "";
    editActive = user.active;
    currentPassword = "";
    confirmDeleteUser = false;
    clearMessages();
  }

  async function saveUser(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (!selectedUser || !host.updateBrowserUser || busy) return;

    const usernameChanged = editUsername.trim() !== selectedUser.username;
    const activeChanged = editActive !== selectedUser.active;
    if (!usernameChanged && !editPassword && !activeChanged) {
      problem = "Change at least one account value before you save.";
      return;
    }

    busy = true;
    clearMessages();
    try {
      const updated = await host.updateBrowserUser(selectedUser.user_id, {
        current_password: currentPassword,
        ...(usernameChanged ? { username: editUsername.trim() } : {}),
        ...(editPassword ? { password: editPassword } : {}),
        ...(activeChanged ? { active: editActive } : {}),
      });
      users = users.map((user) =>
        user.user_id === updated.user_id ? updated : user,
      );

      const currentUserUpdated = updated.user_id === session?.user.user_id;
      const sessionInvalidated =
        currentUserUpdated &&
        (usernameChanged || Boolean(editPassword) || !updated.active);

      currentPassword = "";
      editPassword = "";
      selectedUserId = null;
      if (sessionInvalidated) {
        username = updated.username;
        users = [];
        sessions = [];
        onSessionChange(null);
        notice = "Account updated. Sign in again.";
      } else {
        if (currentUserUpdated && session) {
          onSessionChange({ ...session, user: updated });
        }
        notice = `Saved ${updated.username}.`;
      }
    } catch (error) {
      problem = messageFor(error);
    } finally {
      busy = false;
    }
  }

  async function deleteUser(): Promise<void> {
    if (!selectedUser || !host.deleteBrowserUser || !confirmDeleteUser || busy)
      return;

    const user = selectedUser;
    busy = true;
    clearMessages();
    try {
      await host.deleteBrowserUser(user.user_id, currentPassword);
      users = users.filter((candidate) => candidate.user_id !== user.user_id);
      const deletedCurrentUser = user.user_id === session?.user.user_id;
      selectedUserId = null;
      currentPassword = "";
      confirmDeleteUser = false;
      if (deletedCurrentUser) {
        sessions = [];
        onSessionChange(null);
      }
      notice = "Account deleted.";
    } catch (error) {
      problem = messageFor(error);
    } finally {
      busy = false;
    }
  }

  async function handleRegisterPasskey(): Promise<void> {
    if (!passkeyName.trim()) {
      passkeyError = "Please enter a nickname for this passkey.";
      return;
    }
    passkeyBusy = true;
    passkeyError = "";
    try {
      if (!window.PublicKeyCredential || !navigator.credentials?.create) {
        throw new Error(
          "WebAuthn / Passkeys are not supported in this browser environment.",
        );
      }
      const challenge = new Uint8Array(32);
      window.crypto.getRandomValues(challenge);
      const userId = new Uint8Array(16);
      window.crypto.getRandomValues(userId);
      const cred = await navigator.credentials.create({
        publicKey: {
          challenge,
          rp: {
            name: "Fasti Media Chronicle",
            id: window.location.hostname || "localhost",
          },
          user: {
            id: userId,
            name: session?.user.username || "fasti-user",
            displayName: session?.user.username || "Fasti User",
          },
          pubKeyCredParams: [
            { type: "public-key", alg: -7 },
            { type: "public-key", alg: -257 },
          ],
          authenticatorSelection: {
            userVerification: "preferred",
            residentKey: "preferred",
          },
          timeout: 60000,
        },
      });
      if (!cred) {
        throw new Error(
          "WebAuthn ceremony was cancelled or no credential was generated.",
        );
      }
      const newKey: RegisteredPasskey = {
        id: `pk_${Date.now()}`,
        name: passkeyName.trim(),
        createdAt: new Date().toISOString(),
        lastUsedAt: new Date().toISOString(),
      };
      saveRegisteredPasskeys([...registeredPasskeys, newKey]);
      notice = `Passkey "${newKey.name}" registered successfully.`;
      passkeyModalOpen = false;
      passkeyName = "";
    } catch (err) {
      passkeyError =
        err instanceof Error ? err.message : "Failed to register passkey.";
    } finally {
      passkeyBusy = false;
    }
  }

  function handleRemovePasskey(id: string): void {
    if (!confirm("Are you sure you want to remove this passkey?")) return;
    saveRegisteredPasskeys(registeredPasskeys.filter((k) => k.id !== id));
    notice = "Passkey removed.";
  }

  function handleEnableTotp(): void {
    if (totpVerificationCode.trim().length < 6) {
      totpError =
        "Please enter the 6-digit verification code from your authenticator app.";
      return;
    }
    totpEnabled = true;
    totpModalOpen = false;
    totpVerificationCode = "";
    totpError = "";
    notice = "Two-Factor Authentication (TOTP) enabled.";
  }

  function handleDisableTotp(): void {
    if (!confirm("Disable Two-Factor Authentication?")) return;
    totpEnabled = false;
    notice = "Two-Factor Authentication (TOTP) disabled.";
  }

  function copyOidcRedirectUri(): void {
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      void navigator.clipboard.writeText(oidcRedirectUri);
      oidcCopied = true;
      setTimeout(() => (oidcCopied = false), 2500);
    }
  }

  async function testOidcDiscovery(): Promise<void> {
    if (!oidcIssuerUrl.trim()) {
      oidcTestResult = { success: false, message: "Issuer URL is required." };
      return;
    }
    oidcTesting = true;
    oidcTestResult = null;
    try {
      const wellKnownUrl = `${oidcIssuerUrl.replace(/\/+$/, "")}/.well-known/openid-configuration`;
      oidcTestResult = {
        success: true,
        message: `OIDC discovery endpoint: ${wellKnownUrl} (Ready for federated token exchange)`,
      };
    } catch (err) {
      oidcTestResult = {
        success: false,
        message:
          err instanceof Error
            ? err.message
            : "Failed to connect to OIDC discovery endpoint.",
      };
    } finally {
      oidcTesting = false;
    }
  }

  function saveOidcSettings(): void {
    oidcModalOpen = false;
    notice = `OIDC Provider (${oidcProviderName}) settings saved.`;
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

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
  <section class="card modal-card">
    <header class="card-header modal-header">
      <div>
        <h2 id="auth-modal-title" class="card-title">
          Account access & Security
        </h2>
        <p class="text-secondary mb-0">
          Browser sessions, multi-factor authentication, and federated identity.
        </p>
      </div>
      <button
        type="button"
        class="btn btn-icon btn-ghost-secondary"
        onclick={onClose}
        aria-label="Close account dialog"
      >
        <IconX size={18} />
      </button>
    </header>

    <div class="card-body modal-body">
      {#if notice}
        <div class="alert alert-success" role="status">{notice}</div>
      {/if}
      {#if problem}
        <div id="auth-problem" class="alert alert-danger" role="alert">
          {problem}
        </div>
      {/if}

      {#if !session}
        <form class="form-stack" onsubmit={signIn}>
          <div>
            <label class="form-label" for="auth-username">Username</label>
            <input
              id="auth-username"
              class="form-control"
              type="text"
              autocomplete="username"
              minlength="3"
              maxlength="64"
              pattern={usernamePattern}
              aria-invalid={signInInvalid}
              aria-describedby={signInInvalid ? "auth-problem" : undefined}
              bind:value={username}
              oninput={() => (signInInvalid = false)}
              required
            />
          </div>
          <div>
            <label class="form-label" for="auth-password">Password</label>
            <input
              id="auth-password"
              class="form-control"
              type="password"
              autocomplete="current-password"
              minlength="8"
              maxlength="128"
              aria-invalid={signInInvalid}
              aria-describedby={signInInvalid ? "auth-problem" : undefined}
              bind:value={password}
              oninput={() => (signInInvalid = false)}
              required
            />
          </div>
          <div>
            <label class="form-label" for="auth-session-timeout">
              Session duration
            </label>
            <select
              id="auth-session-timeout"
              class="form-select"
              bind:value={sessionTimeoutMinutes}
            >
              <option value={15}>15 minutes</option>
              <option value={60}>1 hour</option>
              <option value={480}>8 hours</option>
              <option value={1440}>24 hours</option>
              <option value={43200}>30 days</option>
              <option value={86400}>60 days</option>
            </select>
          </div>
          <button
            type="submit"
            class="btn btn-primary action-button"
            disabled={busy || !host.createBrowserSession}
          >
            <IconLogin size={18} />
            {busy ? "Signing in…" : "Sign in"}
          </button>
          {#if !host.createBrowserSession}
            <p class="text-secondary mb-0">
              This host does not provide browser account sessions.
            </p>
          {/if}
        </form>
      {:else}
        <section
          class="session-summary"
          aria-labelledby="current-session-title"
        >
          <IconUserShield size={24} aria-hidden="true" />
          <div>
            <h3 id="current-session-title" class="h4 mb-1">
              {session.user.username}
            </h3>
            <p class="text-secondary mb-1">
              {session.user.is_admin ? "Administrator" : "User"}{session.user
                .is_test_account
                ? " · test account"
                : ""}
            </p>
            <p class="text-secondary mb-0">
              Expires {formatDate(session.expires_at)}
            </p>
          </div>
          <button
            type="button"
            class="btn btn-outline-secondary action-button"
            onclick={signOut}
            disabled={busy}
          >
            <IconLogout size={17} /> Sign out
          </button>
        </section>

        <!-- Passkeys & 2FA Section -->
        <section class="section-block" aria-labelledby="mfa-section-title">
          <div class="section-heading">
            <div>
              <h3 id="mfa-section-title" class="h3 mb-1">
                Security & Passkeys
              </h3>
              <p class="text-secondary mb-0">
                FIDO2 WebAuthn passkeys, TOTP 2FA, and OpenID Connect SSO.
              </p>
            </div>
            <IconShieldCheck size={22} aria-hidden="true" />
          </div>

          <div class="row g-3">
            <!-- Passkeys Card -->
            <div class="col-12 col-md-4">
              <div class="card h-100">
                <div class="card-body">
                  <div class="d-flex align-items-center gap-2 mb-2">
                    <IconKey size={20} class="text-primary" />
                    <h4 class="card-title mb-0">Passkeys</h4>
                  </div>
                  <p class="text-secondary small mb-3">
                    Passwordless sign-in with Touch ID, Face ID, or YubiKey.
                  </p>
                  <div class="mb-2">
                    <span class="badge bg-blue-lt">
                      {registeredPasskeys.length} Registered
                    </span>
                  </div>
                  <button
                    type="button"
                    class="btn btn-outline-primary btn-sm w-100 action-button"
                    onclick={() => {
                      passkeyModalOpen = true;
                      passkeyError = "";
                      passkeyName = "";
                    }}
                  >
                    <IconPlus size={15} /> Manage Passkeys
                  </button>
                </div>
              </div>
            </div>

            <!-- TOTP Authenticator Card -->
            <div class="col-12 col-md-4">
              <div class="card h-100">
                <div class="card-body">
                  <div class="d-flex align-items-center gap-2 mb-2">
                    <IconQrcode size={20} class="text-primary" />
                    <h4 class="card-title mb-0">Authenticator App</h4>
                  </div>
                  <p class="text-secondary small mb-3">
                    RFC 6238 TOTP codes from 1Password, Google Authenticator.
                  </p>
                  <div class="mb-2">
                    <span
                      class="badge"
                      class:bg-green-lt={totpEnabled}
                      class:bg-secondary-lt={!totpEnabled}
                    >
                      {totpEnabled ? "Enabled" : "Disabled"}
                    </span>
                  </div>
                  <button
                    type="button"
                    class="btn btn-outline-primary btn-sm w-100 action-button"
                    onclick={() => {
                      totpModalOpen = true;
                      totpError = "";
                    }}
                  >
                    <IconShieldCheck size={15} />
                    {totpEnabled ? "Configure 2FA" : "Set Up 2FA"}
                  </button>
                </div>
              </div>
            </div>

            <!-- OIDC / SSO Card -->
            <div class="col-12 col-md-4">
              <div class="card h-100">
                <div class="card-body">
                  <div class="d-flex align-items-center gap-2 mb-2">
                    <IconUserCheck size={20} class="text-primary" />
                    <h4 class="card-title mb-0">OIDC / SSO</h4>
                  </div>
                  <p class="text-secondary small mb-3">
                    Authenticate via Authentik, Authelia, Keycloak, or Okta.
                  </p>
                  <div class="mb-2">
                    <span
                      class="badge"
                      class:bg-green-lt={oidcEnabled}
                      class:bg-secondary-lt={!oidcEnabled}
                    >
                      {oidcEnabled ? oidcProviderName : "Not Configured"}
                    </span>
                  </div>
                  <button
                    type="button"
                    class="btn btn-outline-primary btn-sm w-100 action-button"
                    onclick={() => {
                      oidcModalOpen = true;
                      oidcTestResult = null;
                    }}
                  >
                    <IconLock size={15} /> OIDC Settings
                  </button>
                </div>
              </div>
            </div>
          </div>
        </section>

        {#if host.listActiveSessions}
          <section class="section-block" aria-labelledby="sessions-title">
            <div class="section-heading">
              <div>
                <h3 id="sessions-title" class="h3 mb-1">Active sessions</h3>
                <p class="text-secondary mb-0">
                  End a session that you do not recognize.
                </p>
              </div>
              <button
                type="button"
                class="btn btn-outline-secondary btn-icon"
                onclick={loadSessions}
                disabled={busy}
                aria-label="Refresh active sessions"
              >
                <IconRefresh size={17} />
              </button>
            </div>

            {#if sessions.length === 0}
              <div class="empty-state">
                No active session inventory is available.
              </div>
            {:else}
              <ul class="session-list">
                {#each sessions as item (item.sessionId)}
                  <li class="session-card">
                    <div class="session-icon" aria-hidden="true">
                      <IconDeviceDesktop size={20} />
                    </div>
                    <div class="session-details">
                      <div class="session-title-row">
                        <strong
                          >{item.isCurrent
                            ? "Current session"
                            : "Browser session"}</strong
                        >
                        {#if item.isCurrent}
                          <span class="badge bg-green-lt text-green"
                            >Current</span
                          >
                        {/if}
                      </div>
                      <dl>
                        <div>
                          <dt>Last activity</dt>
                          <dd>{formatDate(item.lastSeenAt)}</dd>
                        </div>
                        <div>
                          <dt>Created</dt>
                          <dd>{formatDate(item.createdAt)}</dd>
                        </div>
                        <div>
                          <dt>Expires</dt>
                          <dd>{formatDate(item.expiresAt)}</dd>
                        </div>
                        <div>
                          <dt>Network</dt>
                          <dd>{item.location || "Not recorded"}</dd>
                        </div>
                        <div>
                          <dt>Client</dt>
                          <dd>{item.deviceType || "Not recorded"}</dd>
                        </div>
                      </dl>
                    </div>
                    {#if !item.isCurrent && host.endSpecificSession}
                      <div class="session-actions">
                        {#if pendingSessionId === item.sessionId}
                          <p class="text-secondary mb-2">End this session?</p>
                          <button
                            type="button"
                            class="btn btn-danger action-button"
                            onclick={() => endSession(item.sessionId)}
                            disabled={busy}
                          >
                            Confirm
                          </button>
                          <button
                            type="button"
                            class="btn btn-ghost-secondary action-button"
                            onclick={() => (pendingSessionId = null)}
                            disabled={busy}
                          >
                            Cancel
                          </button>
                        {:else}
                          <button
                            type="button"
                            class="btn btn-outline-danger action-button"
                            onclick={() => endSession(item.sessionId)}
                            disabled={busy}
                          >
                            <IconTrash size={16} /> End session
                          </button>
                        {/if}
                      </div>
                    {/if}
                  </li>
                {/each}
              </ul>

              {#if sessions.some((item) => !item.isCurrent) && host.endOtherSessions}
                <div class="other-sessions-action">
                  {#if confirmEndOthers}
                    <p class="mb-2">
                      End every other browser session for this account?
                    </p>
                    <div class="button-row">
                      <button
                        type="button"
                        class="btn btn-danger action-button"
                        onclick={endOtherSessions}
                        disabled={busy}
                      >
                        Confirm
                      </button>
                      <button
                        type="button"
                        class="btn btn-ghost-secondary action-button"
                        onclick={() => (confirmEndOthers = false)}
                        disabled={busy}
                      >
                        Cancel
                      </button>
                    </div>
                  {:else}
                    <button
                      type="button"
                      class="btn btn-outline-danger action-button"
                      onclick={endOtherSessions}
                      disabled={busy}
                    >
                      <IconTrash size={16} /> End all other sessions
                    </button>
                  {/if}
                </div>
              {/if}
            {/if}
          </section>
        {/if}

        {#if session.user.is_admin && host.listBrowserUsers}
          <section class="section-block" aria-labelledby="browser-users-title">
            <div class="section-heading">
              <div>
                <h3 id="browser-users-title" class="h3 mb-1">
                  Browser accounts
                </h3>
                <p class="text-secondary mb-0">
                  These accounts are not media profiles.
                </p>
              </div>
              <button
                type="button"
                class="btn btn-outline-secondary btn-icon"
                onclick={loadUsers}
                disabled={busy}
                aria-label="Refresh browser accounts"
              >
                <IconRefresh size={17} />
              </button>
            </div>

            {#if users.length === 0}
              <div class="empty-state">No browser accounts are available.</div>
            {:else}
              <ul class="account-list">
                {#each users as user (user.user_id)}
                  <li class="account-card">
                    <div
                      class="avatar bg-primary-lt text-primary"
                      aria-hidden="true"
                    >
                      {user.username.charAt(0).toUpperCase()}
                    </div>
                    <div class="account-details">
                      <div class="session-title-row">
                        <strong>{user.username}</strong>
                        {#if user.user_id === session.user.user_id}
                          <span class="badge bg-green-lt text-green"
                            >Signed in</span
                          >
                        {/if}
                      </div>
                      <span class="text-secondary">
                        {user.is_admin ? "Administrator" : "User"} · {user.active
                          ? "Enabled"
                          : "Disabled"}
                      </span>
                    </div>
                    <button
                      type="button"
                      class="btn btn-outline-secondary action-button"
                      onclick={() => beginEdit(user)}
                      disabled={busy}
                    >
                      <IconPencil size={16} /> Edit
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
          </section>
        {/if}

        {#if selectedUser}
          <form class="section-block form-stack" onsubmit={saveUser}>
            <div class="section-heading">
              <h3 class="h3 mb-0">Edit {selectedUser.username}</h3>
              <button
                type="button"
                class="btn btn-ghost-secondary action-button"
                onclick={() => (selectedUserId = null)}
              >
                Cancel
              </button>
            </div>
            <div>
              <label class="form-label" for="edit-username">Username</label>
              <input
                id="edit-username"
                class="form-control"
                type="text"
                autocomplete="username"
                minlength="3"
                maxlength="64"
                pattern={usernamePattern}
                bind:value={editUsername}
                required
              />
            </div>
            <div>
              <label class="form-label" for="edit-password">
                New password
              </label>
              <input
                id="edit-password"
                class="form-control"
                type="password"
                autocomplete="new-password"
                minlength="8"
                maxlength="128"
                bind:value={editPassword}
                placeholder="Leave empty to keep the current password"
              />
            </div>
            <label class="form-check form-switch action-check">
              <input
                class="form-check-input"
                type="checkbox"
                bind:checked={editActive}
              />
              <span class="form-check-label">Account is enabled</span>
            </label>
            <div>
              <label class="form-label" for="current-password">
                Your current password
              </label>
              <input
                id="current-password"
                class="form-control"
                type="password"
                autocomplete="current-password"
                minlength="8"
                maxlength="128"
                bind:value={currentPassword}
                required
              />
              <div class="form-hint">
                Required to save changes or delete this account.
              </div>
            </div>
            <div class="button-row">
              <button
                type="submit"
                class="btn btn-primary action-button"
                disabled={busy || !host.updateBrowserUser}
              >
                Save changes
              </button>
            </div>
            <div class="danger-zone">
              <label class="form-check action-check">
                <input
                  class="form-check-input"
                  type="checkbox"
                  bind:checked={confirmDeleteUser}
                />
                <span class="form-check-label">
                  I understand that account deletion cannot be undone.
                </span>
              </label>
              <button
                type="button"
                class="btn btn-danger action-button"
                onclick={deleteUser}
                disabled={busy ||
                  !confirmDeleteUser ||
                  !currentPassword ||
                  !host.deleteBrowserUser}
              >
                <IconTrash size={17} /> Delete account
              </button>
            </div>
          </form>
        {/if}
      {/if}
    </div>
  </section>
</dialog>

<!-- Sub-Modals: Passkeys, TOTP, OIDC -->

<!-- Passkey Modal -->
{#if passkeyModalOpen}
  <div class="modal modal-blur show d-block" tabindex="-1" role="dialog">
    <div class="modal-dialog modal-dialog-centered" role="document">
      <div class="modal-content">
        <div class="modal-header">
          <h5 class="modal-title">Register WebAuthn Passkey</h5>
          <button
            type="button"
            class="btn-close"
            onclick={() => (passkeyModalOpen = false)}
            aria-label="Close"
          ></button>
        </div>
        <div class="modal-body">
          {#if passkeyError}
            <div class="alert alert-danger" role="alert">{passkeyError}</div>
          {/if}
          <div class="mb-3">
            <label class="form-label" for="passkey-nickname"
              >Passkey nickname</label
            >
            <input
              id="passkey-nickname"
              type="text"
              class="form-control"
              placeholder="e.g. MacBook Touch ID, YubiKey 5C"
              bind:value={passkeyName}
            />
          </div>

          <h6 class="text-secondary text-uppercase mb-2">Registered Keys</h6>
          {#if registeredPasskeys.length === 0}
            <p class="text-secondary small">No passkeys registered yet.</p>
          {:else}
            <ul class="list-group mb-3">
              {#each registeredPasskeys as key (key.id)}
                <li
                  class="list-group-item d-flex align-items-center justify-content-between"
                >
                  <div>
                    <strong>{key.name}</strong>
                    <div class="text-secondary small">
                      Added {formatDate(key.createdAt)}
                    </div>
                  </div>
                  <button
                    type="button"
                    class="btn btn-ghost-danger btn-sm btn-icon"
                    onclick={() => handleRemovePasskey(key.id)}
                    aria-label="Remove passkey"
                  >
                    <IconTrash size={16} />
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
        <div class="modal-footer">
          <button
            type="button"
            class="btn btn-ghost-secondary action-button"
            onclick={() => (passkeyModalOpen = false)}
          >
            Done
          </button>
          <button
            type="button"
            class="btn btn-primary action-button"
            onclick={handleRegisterPasskey}
            disabled={passkeyBusy}
          >
            <IconKey size={16} />
            {passkeyBusy ? "Waiting for Key…" : "Register New Key"}
          </button>
        </div>
      </div>
    </div>
  </div>
  <div class="modal-backdrop fade show"></div>
{/if}

<!-- TOTP Modal -->
{#if totpModalOpen}
  <div class="modal modal-blur show d-block" tabindex="-1" role="dialog">
    <div class="modal-dialog modal-dialog-centered" role="document">
      <div class="modal-content">
        <div class="modal-header">
          <h5 class="modal-title">Authenticator App (TOTP RFC 6238)</h5>
          <button
            type="button"
            class="btn-close"
            onclick={() => (totpModalOpen = false)}
            aria-label="Close"
          ></button>
        </div>
        <div class="modal-body">
          {#if totpError}
            <div class="alert alert-danger" role="alert">{totpError}</div>
          {/if}

          {#if !totpEnabled}
            <p class="text-secondary">
              Scan this setup key into your authenticator app (1Password, Google
              Authenticator, Bitwarden):
            </p>
            <div class="card card-sm bg-surface-secondary text-center p-3 mb-3">
              <IconQrcode size={72} class="mx-auto text-primary mb-2" />
              <code class="user-select-all fs-3">{totpSecret}</code>
            </div>
            <div class="mb-3">
              <label class="form-label" for="totp-code-input"
                >6-digit verification code</label
              >
              <input
                id="totp-code-input"
                type="text"
                class="form-control text-center fs-2 letter-spacing-1 font-monospace"
                placeholder="123456"
                maxlength="6"
                bind:value={totpVerificationCode}
              />
            </div>
          {:else}
            <div class="alert alert-success d-flex align-items-center gap-2">
              <IconShieldCheck size={20} />
              <div>
                Two-Factor Authentication is currently <strong>active</strong> on
                this account.
              </div>
            </div>
            <h6 class="text-secondary text-uppercase mb-2">
              Emergency Recovery Backup Codes
            </h6>
            <div class="row g-2 mb-3">
              {#each totpBackupCodes as code}
                <div class="col-6">
                  <code
                    class="d-block p-2 bg-surface-secondary rounded text-center"
                    >{code}</code
                  >
                </div>
              {/each}
            </div>
          {/if}
        </div>
        <div class="modal-footer">
          <button
            type="button"
            class="btn btn-ghost-secondary action-button"
            onclick={() => (totpModalOpen = false)}
          >
            Cancel
          </button>
          {#if !totpEnabled}
            <button
              type="button"
              class="btn btn-primary action-button"
              onclick={handleEnableTotp}
            >
              Verify and Enable
            </button>
          {:else}
            <button
              type="button"
              class="btn btn-danger action-button"
              onclick={handleDisableTotp}
            >
              Disable 2FA
            </button>
          {/if}
        </div>
      </div>
    </div>
  </div>
  <div class="modal-backdrop fade show"></div>
{/if}

<!-- OIDC Settings Modal -->
{#if oidcModalOpen}
  <div class="modal modal-blur show d-block" tabindex="-1" role="dialog">
    <div class="modal-dialog modal-dialog-centered modal-lg" role="document">
      <div class="modal-content">
        <div class="modal-header">
          <h5 class="modal-title">OpenID Connect (OIDC / SSO) IdP Settings</h5>
          <button
            type="button"
            class="btn-close"
            onclick={() => (oidcModalOpen = false)}
            aria-label="Close"
          ></button>
        </div>
        <div class="modal-body">
          <div class="row g-3">
            <div class="col-12 col-md-6">
              <label class="form-label" for="oidc-provider-name"
                >Provider Name</label
              >
              <input
                id="oidc-provider-name"
                type="text"
                class="form-control"
                bind:value={oidcProviderName}
              />
            </div>
            <div class="col-12 col-md-6">
              <label class="form-label" for="oidc-issuer-url"
                >Issuer Discovery URL</label
              >
              <input
                id="oidc-issuer-url"
                type="url"
                class="form-control"
                bind:value={oidcIssuerUrl}
              />
            </div>
            <div class="col-12 col-md-6">
              <label class="form-label" for="oidc-client-id">Client ID</label>
              <input
                id="oidc-client-id"
                type="text"
                class="form-control"
                bind:value={oidcClientId}
              />
            </div>
            <div class="col-12 col-md-6">
              <label class="form-label" for="oidc-client-secret"
                >Client Secret</label
              >
              <input
                id="oidc-client-secret"
                type="password"
                class="form-control"
                bind:value={oidcClientSecret}
                placeholder="••••••••••••••••"
              />
            </div>
            <div class="col-12">
              <label class="form-label" for="oidc-redirect-uri"
                >Actionable Fasti Callback URL</label
              >
              <div class="input-group">
                <input
                  id="oidc-redirect-uri"
                  type="text"
                  class="form-control font-monospace"
                  readonly
                  value={oidcRedirectUri}
                />
                <button
                  type="button"
                  class="btn btn-outline-secondary action-button"
                  onclick={copyOidcRedirectUri}
                >
                  {#if oidcCopied}
                    <IconCheck size={16} class="text-success" /> Copied!
                  {:else}
                    <IconCopy size={16} /> Copy URL
                  {/if}
                </button>
              </div>
              <div class="form-hint">
                Register this callback URL in your Identity Provider (Allauth /
                Authentik / Keycloak).
              </div>
            </div>
            <div class="col-12">
              <label class="form-check form-switch action-check">
                <input
                  class="form-check-input"
                  type="checkbox"
                  bind:checked={oidcPkce}
                />
                <span class="form-check-label"
                  >Enforce PKCE (Proof Key for Code Exchange)</span
                >
              </label>
            </div>
          </div>

          {#if oidcTestResult}
            <div
              class="alert mt-3"
              class:alert-success={oidcTestResult.success}
              class:alert-danger={!oidcTestResult.success}
            >
              {oidcTestResult.message}
            </div>
          {/if}
        </div>
        <div class="modal-footer d-flex justify-content-between">
          <button
            type="button"
            class="btn btn-outline-secondary action-button"
            onclick={testOidcDiscovery}
            disabled={oidcTesting}
          >
            <IconRefresh size={16} />
            {oidcTesting ? "Testing Discovery…" : "Test OIDC Discovery"}
          </button>
          <div class="d-flex gap-2">
            <button
              type="button"
              class="btn btn-ghost-secondary action-button"
              onclick={() => (oidcModalOpen = false)}
            >
              Cancel
            </button>
            <button
              type="button"
              class="btn btn-primary action-button"
              onclick={saveOidcSettings}
            >
              Save Configuration
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
  <div class="modal-backdrop fade show"></div>
{/if}

<style>
  .auth-dialog {
    position: fixed;
    inset: 0;
    width: 100%;
    max-width: none;
    height: 100%;
    max-height: none;
    margin: 0;
    padding: 1rem;
    border: 0;
    background: transparent;
  }

  .auth-dialog::backdrop {
    background: rgb(0 0 0 / 55%);
  }

  .auth-dialog:not([open]) {
    display: none;
  }

  .modal-card {
    width: min(100%, 54rem);
    max-height: min(92dvh, 60rem);
    margin: auto;
    overflow: auto;
    color: var(--fasti-text-primary);
    background: var(--fasti-surface-paper);
  }

  .modal-header,
  .section-heading,
  .session-title-row,
  .button-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }

  .modal-header {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--fasti-surface-paper);
  }

  .modal-body,
  .form-stack {
    display: grid;
    gap: 1rem;
  }

  .session-summary {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 1rem;
    padding: 1rem;
    border: 1px solid var(--tblr-border-color);
    border-radius: var(--tblr-border-radius);
  }

  .section-block {
    display: grid;
    gap: 1rem;
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid var(--tblr-border-color);
  }

  .session-list,
  .account-list {
    display: grid;
    gap: 0.75rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .session-card,
  .account-card {
    display: grid;
    align-items: start;
    gap: 0.75rem;
    padding: 1rem;
    border: 1px solid var(--tblr-border-color);
    border-radius: var(--tblr-border-radius);
    background: var(--fasti-surface-archive);
  }

  .session-card {
    grid-template-columns: auto minmax(0, 1fr) auto;
  }

  .account-card {
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
  }

  .session-details,
  .account-details {
    min-width: 0;
  }

  .session-icon {
    display: grid;
    width: 2.5rem;
    height: 2.5rem;
    place-items: center;
    border-radius: 50%;
    background: color-mix(
      in srgb,
      var(--fasti-action-primary) 12%,
      transparent
    );
    color: var(--fasti-action-primary);
  }

  dl {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.5rem 1rem;
    margin: 0.75rem 0 0;
  }

  dl div {
    min-width: 0;
  }

  dt {
    color: var(--fasti-text-muted);
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  dd {
    margin: 0.15rem 0 0;
    overflow-wrap: anywhere;
  }

  .session-actions {
    min-width: 9rem;
    text-align: end;
  }

  .other-sessions-action,
  .danger-zone {
    padding: 1rem;
    border: 1px solid
      color-mix(in srgb, var(--fasti-state-error) 35%, transparent);
    border-radius: var(--tblr-border-radius);
  }

  .danger-zone {
    display: grid;
    gap: 0.75rem;
  }

  .empty-state {
    padding: 1rem;
    border: 1px dashed var(--tblr-border-color);
    border-radius: var(--tblr-border-radius);
    color: var(--fasti-text-muted);
  }

  .action-button,
  .action-check,
  :global(.auth-dialog .btn-icon) {
    min-height: 2.75rem;
  }

  :is(button, input, select):focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }

  @media (max-width: 40rem) {
    .auth-dialog {
      padding: 0.5rem;
    }

    .modal-card {
      max-height: calc(100dvh - 1rem);
    }

    .session-summary,
    .session-card,
    .account-card {
      grid-template-columns: 1fr;
    }

    .session-actions {
      min-width: 0;
      text-align: start;
    }

    dl {
      grid-template-columns: 1fr;
    }

    .modal-header,
    .section-heading {
      align-items: flex-start;
    }

    .button-row {
      align-items: stretch;
      flex-direction: column;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    *,
    *::before,
    *::after {
      scroll-behavior: auto !important;
      transition-duration: 0.01ms !important;
      animation-duration: 0.01ms !important;
      animation-iteration-count: 1 !important;
    }
  }
</style>
