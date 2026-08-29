<script lang="ts">
  import {
    IconDeviceDesktop,
    IconKey,
    IconLock,
    IconLogin,
    IconLogout,
    IconPencil,
    IconRefresh,
    IconShieldCheck,
    IconTrash,
    IconUserShield,
    IconUsers,
    IconX,
  } from "@tabler/icons-svelte";
  import type {
    BrowserSession,
    BrowserSessionItem,
    BrowserUser,
    WorkbenchHost,
  } from "./types.js";

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

  const usernamePattern = "[a-z0-9][a-z0-9._\\-]{2,63}";
  const selectedUser = $derived(
    users.find((user) => user.user_id === selectedUserId) ?? null,
  );

  $effect(() => {
    if (!dialog) return;
    if (show && !dialog.open) {
      dialog.showModal();
    } else if (!show && dialog.open) {
      dialog.close();
      problem = "";
      notice = "";
    }
  });

  $effect(() => {
    if (!show || !session) return;
    void refreshAccountData();
  });

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
    if (details?.code === "record_not_found") {
      return "That browser session is no longer active. Refresh the session list.";
    }
    if (
      details?.violations?.some(
        (violation) => violation.code === "last_active_administrator_required",
      )
    ) {
      return "This is the only active administrator. Keep the account active.";
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

  async function refreshAccountData(): Promise<void> {
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
    } catch {
      sessions = [];
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
          <h2 id="auth-modal-title" class="card-title">
            Account access & security
          </h2>
          <p class="text-secondary mb-0">
            Manage browser sessions and account access. Media profiles and other
            authentication methods stay separate.
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
              <label class="form-label" for="auth-session-timeout"
                >Session duration</label
              >
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
              <IconLogin size={18} aria-hidden="true" />
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
              <IconLogout size={17} aria-hidden="true" /> Sign out
            </button>
          </section>

          <section
            class="section-block"
            aria-labelledby="future-security-title"
          >
            <div class="section-heading">
              <div>
                <h3 id="future-security-title" class="h3 mb-1">
                  Additional account options
                </h3>
                <p class="text-secondary mb-0">
                  These options stay visible so the intended account model is
                  clear. They do not accept or store security data until the
                  server capability exists.
                </p>
              </div>
              <IconShieldCheck size={22} aria-hidden="true" />
            </div>

            <div class="future-method-list">
              <div class="card card-sm">
                <div class="card-body future-method">
                  <IconUsers size={20} aria-hidden="true" />
                  <div>
                    <strong>Media profiles</strong>
                    <p class="text-secondary mb-0">
                      Not available in this build. Profile creation, selection,
                      permissions, and PIN verification need server-owned
                      capabilities.
                    </p>
                  </div>
                  <span class="badge bg-secondary text-white"
                    >Not available</span
                  >
                </div>
              </div>
              <div class="card card-sm">
                <div class="card-body future-method">
                  <IconKey size={20} aria-hidden="true" />
                  <div>
                    <strong>Passkey</strong>
                    <p class="text-secondary mb-0">
                      Not available in this build. WebAuthn enrollment must be
                      verified and stored by the server.
                    </p>
                  </div>
                  <span class="badge bg-secondary text-white"
                    >Not available</span
                  >
                </div>
              </div>
              <div class="card card-sm">
                <div class="card-body future-method">
                  <IconLock size={20} aria-hidden="true" />
                  <div>
                    <strong>Authenticator app</strong>
                    <p class="text-secondary mb-0">
                      Not available in this build. Fasti does not generate or
                      verify TOTP secrets in the browser.
                    </p>
                  </div>
                  <span class="badge bg-secondary text-white"
                    >Not available</span
                  >
                </div>
              </div>
              <div class="card card-sm">
                <div class="card-body future-method">
                  <IconShieldCheck size={20} aria-hidden="true" />
                  <div>
                    <strong>OIDC / SSO</strong>
                    <p class="text-secondary mb-0">
                      Not available in this build. Provider discovery, client
                      secrets, and token exchange require a server-owned
                      configuration path.
                    </p>
                  </div>
                  <span class="badge bg-secondary text-white"
                    >Not available</span
                  >
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
                    Review when each browser session was used. End a session
                    that you do not recognize.
                  </p>
                </div>
                <button
                  type="button"
                  class="btn btn-outline-secondary btn-icon"
                  onclick={loadSessions}
                  disabled={busy}
                  aria-label="Refresh active sessions"
                >
                  <IconRefresh size={17} aria-hidden="true" />
                </button>
              </div>

              {#if sessions.length === 0}
                <div class="empty-state">
                  No active session inventory is available.
                </div>
              {:else}
                <ul class="session-list">
                  {#each sessions as item (item.sessionId)}
                    <li class="card card-sm session-card">
                      <div class="card-body session-card-body">
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
                              <span class="badge bg-green-lt text-dark fw-bold"
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
                          </dl>
                        </div>
                        {#if !item.isCurrent && host.endSpecificSession}
                          <div class="session-actions">
                            {#if pendingSessionId === item.sessionId}
                              <p class="text-secondary mb-2">
                                End this session?
                              </p>
                              <div class="button-row">
                                <button
                                  type="button"
                                  class="btn btn-outline-danger action-button"
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
                              </div>
                            {:else}
                              <button
                                type="button"
                                class="btn btn-outline-danger action-button"
                                onclick={() => endSession(item.sessionId)}
                                disabled={busy}
                              >
                                <IconTrash size={16} aria-hidden="true" /> End session
                              </button>
                            {/if}
                          </div>
                        {/if}
                      </div>
                    </li>
                  {/each}
                </ul>

                {#if sessions.some((item) => !item.isCurrent) && host.endOtherSessions}
                  <div class="danger-zone">
                    {#if confirmEndOthers}
                      <p class="mb-2">
                        End every other browser session for this account?
                      </p>
                      <div class="button-row">
                        <button
                          type="button"
                          class="btn btn-outline-danger action-button"
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
                        <IconTrash size={16} aria-hidden="true" /> End all other sessions
                      </button>
                    {/if}
                  </div>
                {/if}
              {/if}
            </section>
          {/if}

          {#if session.user.is_admin && host.listBrowserUsers}
            <section
              class="section-block"
              aria-labelledby="browser-users-title"
            >
              <div class="section-heading">
                <div>
                  <h3 id="browser-users-title" class="h3 mb-1">
                    Browser accounts
                  </h3>
                  <p class="text-secondary mb-0">
                    Browser accounts are not media profiles.
                  </p>
                </div>
                <button
                  type="button"
                  class="btn btn-outline-secondary btn-icon"
                  onclick={loadUsers}
                  disabled={busy}
                  aria-label="Refresh browser accounts"
                >
                  <IconRefresh size={17} aria-hidden="true" />
                </button>
              </div>

              {#if users.length === 0}
                <div class="empty-state">
                  No browser accounts are available.
                </div>
              {:else}
                <ul class="account-list">
                  {#each users as user (user.user_id)}
                    <li class="card card-sm">
                      <div class="card-body account-card-body">
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
                              <span class="badge bg-green-lt text-dark fw-bold"
                                >Signed in</span
                              >
                            {/if}
                          </div>
                          <div class="user-meta-row">
                            <span class="role-pill"
                              >{user.is_admin ? "Administrator" : "User"}</span
                            >
                            <span class="status-meta"
                              >{user.active
                                ? "Enabled"
                                : "Disabled"}{user.is_test_account
                                ? " · test"
                                : ""}</span
                            >
                          </div>
                        </div>
                        <button
                          type="button"
                          class="btn btn-outline-secondary action-button"
                          onclick={() => beginEdit(user)}
                          disabled={busy}
                        >
                          <IconPencil size={16} aria-hidden="true" /> Edit
                        </button>
                      </div>
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
                <label class="form-label" for="edit-password"
                  >New password</label
                >
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
                <label class="form-label" for="current-password"
                  >Your current password</label
                >
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
              <button
                type="submit"
                class="btn btn-primary action-button"
                disabled={busy || !host.updateBrowserUser}
              >
                Save changes
              </button>
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
                  class="btn btn-danger text-white action-button"
                  onclick={deleteUser}
                  disabled={busy ||
                    !confirmDeleteUser ||
                    !currentPassword ||
                    !host.deleteBrowserUser}
                >
                  <IconTrash size={17} aria-hidden="true" /> Delete user
                </button>
              </div>
            </form>
          {/if}
        {/if}
      </div>
    </section>
  {/if}
</dialog>

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
  .form-stack,
  .section-block {
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
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid var(--tblr-border-color);
  }

  .future-method-list,
  .session-list,
  .account-list {
    display: grid;
    gap: 0.75rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .future-method {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: start;
    gap: 0.75rem;
  }

  .session-card-body,
  .account-card-body {
    display: grid;
    align-items: start;
    gap: 0.75rem;
  }

  .session-card-body {
    grid-template-columns: auto minmax(0, 1fr) auto;
  }

  .account-card-body {
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
    grid-template-columns: repeat(3, minmax(0, 1fr));
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

  .danger-zone,
  .empty-state {
    padding: 1rem;
    border-radius: var(--tblr-border-radius);
  }

  .danger-zone {
    border: 1px solid
      color-mix(in srgb, var(--fasti-state-error) 35%, transparent);
  }

  .empty-state {
    border: 1px dashed var(--tblr-border-color);
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
    .session-card-body,
    .account-card-body,
    .future-method {
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

  :global(.btn-danger) {
    background-color: var(--fasti-brand-mark, #8b2e2a) !important;
    border-color: var(--fasti-brand-mark, #8b2e2a) !important;
    color: #ffffff !important;
  }
  :global(.btn-danger:hover:not(:disabled)) {
    background-color: #722421 !important;
    border-color: #722421 !important;
    color: #ffffff !important;
  }
</style>
