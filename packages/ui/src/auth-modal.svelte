<script lang="ts">
  import {
    IconLogin,
    IconLogout,
    IconPencil,
    IconTrash,
    IconUserShield,
    IconX,
  } from "@tabler/icons-svelte";
  import type { BrowserSession, BrowserUser, WorkbenchHost } from "./types.js";

  interface Props {
    show: boolean;
    host: WorkbenchHost;
    session: BrowserSession | null;
    onClose: () => void;
    onSessionChange: (session: BrowserSession | null) => void;
  }

  // Matches apps/fastid/src/main.rs's DEVELOPMENT_UNBOUNDED_SESSION_MINUTES.
  // Only accepted by the server when FASTI_DEVELOPMENT_AUTO_LOGIN is set on a
  // loopback durable listener; rejected (422) otherwise. The UI has no
  // capability signal for that server-side env var, so it approximates the
  // same "loopback" half of the gate from the page's own origin -- hides the
  // option on every non-loopback (i.e. non-dev) deployment, at the cost of
  // still showing it on a loopback host that hasn't actually set the flag.
  const DEVELOPMENT_UNBOUNDED_SESSION_MINUTES = 100 * 365 * 24 * 60;
  const isLoopbackHost =
    typeof window !== "undefined" &&
    // WHATWG URL host serialization brackets IPv6 literals, so
    // `location.hostname` for the IPv6 loopback address is "[::1]", not
    // "::1".
    ["localhost", "127.0.0.1", "::1", "[::1]"].includes(
      window.location.hostname,
    );

  let { show, host, session, onClose, onSessionChange }: Props = $props();
  let dialog: HTMLDialogElement | undefined;
  let username = $state("");
  let password = $state("");
  let sessionTimeoutMinutes = $state(60);
  let users = $state<BrowserUser[]>([]);
  let selectedUserId = $state<string | null>(null);
  let editUsername = $state("");
  let editPassword = $state("");
  let editActive = $state(true);
  let currentPassword = $state("");
  let confirmDelete = $state(false);
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
    if (show && !dialog.open) dialog.showModal();
    else if (!show && dialog.open) dialog.close();
  });

  $effect(() => {
    if (show && session?.user.is_admin) void loadUsers();
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

  function formatExpiry(value: string): string {
    const date = new Date(value);
    return Number.isNaN(date.getTime())
      ? value
      : new Intl.DateTimeFormat(undefined, {
          dateStyle: "medium",
          timeStyle: "short",
        }).format(date);
  }

  async function loadUsers(): Promise<void> {
    if (!host.listBrowserUsers) return;
    try {
      users = await host.listBrowserUsers();
    } catch (error) {
      problem = messageFor(error);
    }
  }

  async function signIn(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (!host.createBrowserSession || busy) return;
    busy = true;
    problem = "";
    notice = "";
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
      if (result.user.is_admin) await loadUsers();
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
    problem = "";
    try {
      await host.endBrowserSession();
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

  function beginEdit(user: BrowserUser): void {
    selectedUserId = user.user_id;
    editUsername = user.username;
    editPassword = "";
    editActive = user.active;
    currentPassword = "";
    confirmDelete = false;
    problem = "";
    notice = "";
  }

  async function saveUser(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (!selectedUser || !host.updateBrowserUser || busy) return;
    const usernameChanged = editUsername.trim() !== selectedUser.username;
    const activeChanged = editActive !== selectedUser.active;
    if (!usernameChanged && !editPassword && !activeChanged) {
      problem = "Change the username, password, or active state before saving.";
      return;
    }
    busy = true;
    problem = "";
    notice = "";
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
      currentPassword = "";
      const currentUserUpdated = updated.user_id === session?.user.user_id;
      const sessionInvalidated =
        currentUserUpdated &&
        (usernameChanged || Boolean(editPassword) || !updated.active);
      editPassword = "";
      if (sessionInvalidated) {
        username = updated.username;
        users = [];
        onSessionChange(null);
        notice = "Account updated. Sign in again with the new details.";
      } else {
        if (currentUserUpdated && session) {
          onSessionChange({ ...session, user: updated });
        }
        notice = `Saved ${updated.username}.`;
      }
      selectedUserId = null;
    } catch (error) {
      problem = messageFor(error);
    } finally {
      busy = false;
    }
  }

  async function deleteUser(): Promise<void> {
    if (!selectedUser || !host.deleteBrowserUser || !confirmDelete || busy)
      return;
    const user = selectedUser;
    busy = true;
    problem = "";
    notice = "";
    try {
      await host.deleteBrowserUser(user.user_id, currentPassword);
      users = users.filter((candidate) => candidate.user_id !== user.user_id);
      const deletedCurrentUser = user.user_id === session?.user.user_id;
      selectedUserId = null;
      currentPassword = "";
      confirmDelete = false;
      if (deletedCurrentUser) onSessionChange(null);
      notice = deletedCurrentUser
        ? "Account deleted. The development seed will not recreate it."
        : "Account deleted.";
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
  <section class="modal-card">
    <header class="modal-header">
      <div>
        <h2 id="auth-modal-title">Account access</h2>
        <p>Browser sessions stay separate from integration tokens.</p>
      </div>
      <button
        type="button"
        class="icon-button"
        onclick={onClose}
        aria-label="Close account dialog"
      >
        <IconX size={18} />
      </button>
    </header>

    <div class="modal-body">
      {#if notice}
        <p class="notice" role="status">{notice}</p>
      {/if}
      {#if problem}
        <p id="auth-problem" class="problem" role="alert">{problem}</p>
      {/if}

      {#if !session}
        <form class="form-stack" onsubmit={signIn}>
          <div class="form-field">
            <label for="auth-username">Username</label>
            <input
              id="auth-username"
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
          <div class="form-field">
            <label for="auth-password">Password</label>
            <input
              id="auth-password"
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
          <div class="form-field">
            <label for="auth-session-timeout">Session duration</label>
            <select
              id="auth-session-timeout"
              bind:value={sessionTimeoutMinutes}
            >
              <option value={15}>15 minutes</option>
              <option value={60}>1 hour</option>
              <option value={480}>8 hours</option>
              <option value={1440}>24 hours</option>
              {#if isLoopbackHost}
                <option value={DEVELOPMENT_UNBOUNDED_SESSION_MINUTES}
                  >Don't expire (dev only)</option
                >
              {/if}
            </select>
          </div>
          <button
            type="submit"
            class="primary-button"
            disabled={busy || !host.createBrowserSession}
          >
            <IconLogin size={18} />
            {busy ? "Signing in…" : "Sign in"}
          </button>
          {#if !host.createBrowserSession}
            <p class="hint">
              This host does not provide browser account sessions.
            </p>
          {/if}
        </form>
      {:else}
        <div class="session-summary">
          <IconUserShield size={22} aria-hidden="true" />
          <div>
            <strong>{session.user.username}</strong>
            <span>
              {session.user.is_admin ? "Administrator" : "User"}
              {session.user.is_test_account ? " · test account" : ""}
            </span>
            <span>Session expires {formatExpiry(session.expires_at)}</span>
          </div>
          <button
            type="button"
            class="secondary-button"
            onclick={signOut}
            disabled={busy}
          >
            <IconLogout size={17} /> Sign out
          </button>
        </div>

        {#if session.user.is_admin && host.listBrowserUsers}
          <section class="users" aria-labelledby="browser-users-title">
            <div class="section-heading">
              <h3 id="browser-users-title">Profiles & Accounts</h3>
              <button
                type="button"
                class="text-button"
                onclick={loadUsers}
                disabled={busy}>Refresh</button
              >
            </div>
            {#if users.length === 0}
              <p class="hint">No browser users are available.</p>
            {:else}
              <ul class="profile-cards-list">
                {#each users as user (user.user_id)}
                  {@const isCurrent = user.user_id === session?.user.user_id}
                  {@const initial = user.username
                    ? user.username.charAt(0).toUpperCase()
                    : "U"}
                  <li class="profile-card" class:active-profile={isCurrent}>
                    <div class="profile-avatar-row">
                      <div
                        class="profile-avatar"
                        class:admin-avatar={user.is_admin}
                      >
                        <span>{initial}</span>
                      </div>
                      <div class="user-name">
                        <div class="user-title-row">
                          <strong>{user.username}</strong>
                          {#if isCurrent}
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
                    </div>
                    <div class="profile-card-actions">
                      <button
                        type="button"
                        class="secondary-button"
                        onclick={() => beginEdit(user)}
                      >
                        <IconPencil size={16} /> Edit
                      </button>
                    </div>
                  </li>
                {/each}
              </ul>
            {/if}
          </section>
        {/if}

        {#if selectedUser}
          <form class="edit-panel form-stack" onsubmit={saveUser}>
            <div class="section-heading">
              <h3>Edit {selectedUser.username}</h3>
              <button
                type="button"
                class="text-button"
                onclick={() => (selectedUserId = null)}>Cancel</button
              >
            </div>
            <div class="form-field">
              <label for="edit-username">Username</label>
              <input
                id="edit-username"
                type="text"
                autocomplete="username"
                minlength="3"
                maxlength="64"
                pattern={usernamePattern}
                bind:value={editUsername}
                required
              />
            </div>
            <div class="form-field">
              <label for="edit-password">
                New password <span>(leave blank to keep it)</span>
              </label>
              <input
                id="edit-password"
                type="password"
                autocomplete="new-password"
                minlength="8"
                maxlength="128"
                bind:value={editPassword}
              />
            </div>
            <label class="check-row">
              <input type="checkbox" bind:checked={editActive} />
              Account is active
            </label>
            <div class="form-field">
              <label for="current-password">Your current password</label>
              <input
                id="current-password"
                type="password"
                autocomplete="current-password"
                minlength="8"
                maxlength="128"
                bind:value={currentPassword}
                required
              />
              <span>Required to save changes or delete this user.</span>
            </div>
            <div class="form-actions">
              <button
                type="submit"
                class="primary-button"
                disabled={busy || !host.updateBrowserUser}>Save changes</button
              >
            </div>
            <div class="delete-zone">
              <label class="check-row">
                <input type="checkbox" bind:checked={confirmDelete} />
                I understand that deleting {selectedUser.username} cannot be undone.
              </label>
              <button
                type="button"
                class="danger-button"
                onclick={deleteUser}
                disabled={busy ||
                  !confirmDelete ||
                  !currentPassword ||
                  !host.deleteBrowserUser}
              >
                <IconTrash size={17} /> Delete user
              </button>
            </div>
          </form>
        {/if}
      {/if}
    </div>
  </section>
</dialog>

<style>
  .auth-dialog {
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
  .auth-dialog::backdrop {
    background: rgba(0, 0, 0, 0.55);
  }
  .auth-dialog:not([open]) {
    display: none;
  }
  .modal-card {
    width: min(100%, 680px);
    max-height: min(88dvh, 760px);
    overflow: auto;
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
    border-radius: calc(12px * var(--tblr-border-radius-scale, 1));
    box-shadow: 0 14px 36px rgba(0, 0, 0, 0.28);
  }
  .modal-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 20px 24px 16px;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 22%, transparent);
  }
  h2,
  h3,
  p {
    margin: 0;
  }
  h2 {
    font-family: var(--fasti-font-display);
    font-size: 1.25rem;
  }
  h3 {
    font-size: 1rem;
  }
  .modal-header p,
  .hint,
  .session-summary span,
  .user-name span,
  .form-field span {
    color: var(--fasti-text-muted);
    font-size: 0.875rem;
  }
  .modal-header p {
    margin-top: 4px;
  }
  .modal-body {
    padding: 24px;
  }
  .form-stack {
    display: grid;
    gap: 16px;
  }
  .form-field {
    display: grid;
    gap: 6px;
  }
  .form-field label {
    font-weight: 650;
    font-size: 0.875rem;
  }
  input,
  select {
    width: 100%;
    min-height: 44px;
    padding: 9px 11px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 38%, transparent);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
    font: inherit;
    font-size: max(1rem, 16px);
  }
  button {
    font: inherit;
  }
  :is(button, input, select):focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }
  .icon-button,
  .primary-button,
  .secondary-button,
  .danger-button,
  .text-button {
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    cursor: pointer;
  }
  .icon-button {
    min-width: 44px;
    border: 0;
    background: transparent;
    color: var(--fasti-text-muted);
  }
  .primary-button,
  .secondary-button,
  .danger-button {
    padding: 9px 14px;
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    font-weight: 650;
  }
  .primary-button {
    border: 0;
    background: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
  }
  .secondary-button {
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 35%, transparent);
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }
  .danger-button {
    border: 1px solid var(--fasti-state-error, #b42318);
    background: transparent;
    color: var(--fasti-state-error, #b42318);
  }
  .text-button {
    border: 0;
    background: transparent;
    color: var(--fasti-action-primary);
    font-weight: 650;
  }
  button:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .notice,
  .problem {
    padding: 10px 12px;
    margin-bottom: 16px;
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    overflow-wrap: anywhere;
  }
  .notice {
    background: color-mix(
      in srgb,
      var(--fasti-state-success, #087a55) 12%,
      transparent
    );
  }
  .problem {
    background: color-mix(
      in srgb,
      var(--fasti-state-error, #b42318) 11%,
      transparent
    );
    color: var(--fasti-state-error, #b42318);
  }
  .session-summary {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
  }
  .session-summary div,
  .user-name {
    min-width: 0;
    display: grid;
    gap: 2px;
    overflow-wrap: anywhere;
  }
  .users {
    margin-top: 28px;
  }
  .section-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .profile-cards-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 12px;
    margin-bottom: 0;
    padding: 0;
    list-style: none;
  }
  .profile-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 14px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    border-radius: calc(8px * var(--tblr-border-radius-scale, 1));
    transition: border-color 100ms ease;
  }
  .profile-card.active-profile {
    border-color: var(--fasti-action-primary);
    background: color-mix(
      in srgb,
      var(--fasti-action-primary) 6%,
      var(--fasti-surface-archive)
    );
  }
  .profile-avatar-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .profile-avatar {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    background: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
    display: grid;
    place-items: center;
    font-weight: 700;
    font-size: 0.95rem;
    flex-shrink: 0;
  }
  .profile-avatar.admin-avatar {
    background: var(--fasti-brand-mark, #8b2e2a);
    color: var(--fasti-brand-contrast, #fff);
  }
  .user-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .user-meta-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
  }
  .role-pill {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
  }
  .edit-panel {
    margin-top: 28px;
    padding-top: 20px;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 28%, transparent);
  }
  .check-row {
    display: flex;
    min-height: 44px;
    align-items: center;
    gap: 10px;
    line-height: 1.4;
  }
  .check-row input {
    width: 20px;
    min-height: 20px;
    margin-top: 1px;
    accent-color: var(--fasti-action-primary);
  }
  .form-actions {
    display: flex;
    justify-content: flex-end;
  }
  .delete-zone {
    display: grid;
    gap: 12px;
    margin-top: 8px;
    padding-top: 16px;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-state-error, #b42318) 35%, transparent);
  }
  @media (max-width: 36rem) {
    .modal-card {
      max-height: calc(100dvh - 16px);
    }
    .modal-header,
    .modal-body {
      padding-inline: 16px;
    }
    .session-summary {
      grid-template-columns: auto minmax(0, 1fr);
    }
    .session-summary .secondary-button {
      grid-column: 1 / -1;
      width: 100%;
    }
  }
</style>
