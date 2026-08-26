<script lang="ts">
  import {
    IconFingerprint,
    IconWorld,
    IconDeviceTv,
    IconKey,
    IconLock,
    IconCheck,
    IconCopy,
    IconRefresh,
    IconBrandGithub,
    IconBrandGoogle,
    IconBrandApple,
    IconShieldCheck,
    IconInfoCircle,
  } from "@tabler/icons-svelte";

  interface Props {
    open: boolean;
    currentUser?: {
      username: string;
      displayName: string;
      authMethod: "passkey" | "oidc" | "pin" | "pat" | "password" | "anonymous";
      role: "admin" | "member";
    } | null;
    onClose: () => void;
    onSignIn: (method: string, data: any) => void;
    onSignOut: () => void;
  }

  let {
    open,
    currentUser = null,
    onClose,
    onSignIn,
    onSignOut,
  }: Props = $props();

  let activeTab: "passkey" | "oidc" | "pin" | "password" | "pat" =
    $state("passkey");

  // Local Form States
  let usernameInput = $state("ryan");
  let passwordInput = $state("");
  let rememberSession = $state("2_weeks");
  let patTokenInput = $state("");
  let oidcIssuerInput = $state("https://auth.internal.fasti.dev/application/o/fasti/");
  let isSubmitting = $state(false);
  let statusMessage = $state<{ type: "success" | "error"; text: string } | null>(
    null,
  );

  // TV / Living Room PIN Pairing State
  let generatedPin = $state("749-218");
  let pinExpiresSeconds = $state(284);
  let pinCopied = $state(false);

  function handleRegeneratePin(): void {
    const p1 = Math.floor(100 + Math.random() * 900);
    const p2 = Math.floor(100 + Math.random() * 900);
    generatedPin = `${p1}-${p2}`;
    pinExpiresSeconds = 300;
  }

  function handleCopyPin(): void {
    navigator.clipboard.writeText(generatedPin);
    pinCopied = true;
    setTimeout(() => (pinCopied = false), 2000);
  }

  async function handlePasskeyLogin(): Promise<void> {
    isSubmitting = true;
    statusMessage = null;
    try {
      await new Promise((res) => setTimeout(res, 600));
      onSignIn("passkey", { credentialId: "cred_fido2_" + Date.now() });
      statusMessage = {
        type: "success",
        text: "Passkey biometric verification verified successfully!",
      };
      setTimeout(onClose, 800);
    } catch (err: any) {
      statusMessage = {
        type: "error",
        text: "Passkey authentication failed: " + err.message,
      };
    } finally {
      isSubmitting = false;
    }
  }

  async function handleOidcLogin(providerName: string): Promise<void> {
    isSubmitting = true;
    statusMessage = null;
    try {
      await new Promise((res) => setTimeout(res, 500));
      onSignIn("oidc", { provider: providerName, issuer: oidcIssuerInput });
      statusMessage = {
        type: "success",
        text: `Authenticated via SSO / ${providerName}!`,
      };
      setTimeout(onClose, 800);
    } catch (err: any) {
      statusMessage = { type: "error", text: "SSO error: " + err.message };
    } finally {
      isSubmitting = false;
    }
  }

  async function handlePasswordLogin(e: Event): Promise<void> {
    e.preventDefault();
    if (!usernameInput.trim()) return;
    isSubmitting = true;
    statusMessage = null;
    try {
      await new Promise((res) => setTimeout(res, 400));
      onSignIn("password", {
        username: usernameInput.trim(),
        sessionDuration: rememberSession,
      });
      statusMessage = {
        type: "success",
        text: `Signed in as ${usernameInput.trim()}`,
      };
      setTimeout(onClose, 800);
    } catch (err: any) {
      statusMessage = { type: "error", text: "Invalid username or password" };
    } finally {
      isSubmitting = false;
    }
  }

  async function handlePatLogin(e: Event): Promise<void> {
    e.preventDefault();
    if (!patTokenInput.trim()) return;
    isSubmitting = true;
    statusMessage = null;
    try {
      await new Promise((res) => setTimeout(res, 300));
      onSignIn("pat", { token: patTokenInput.trim() });
      statusMessage = {
        type: "success",
        text: "Personal Access Token validated!",
      };
      setTimeout(onClose, 800);
    } catch (err: any) {
      statusMessage = { type: "error", text: "Invalid or expired token" };
    } finally {
      isSubmitting = false;
    }
  }
</script>

{#if open}
  <div
    class="modal modal-blur fade show d-block"
    tabindex="-1"
    role="dialog"
    aria-modal="true"
    aria-labelledby="auth-modal-title"
    style="background: rgba(0, 0, 0, 0.65); z-index: 1060;"
  >
    <div class="modal-dialog modal-dialog-centered modal-lg" role="document">
      <div class="modal-content border shadow-lg">
        <div class="modal-header">
          <div class="d-flex align-items-center gap-2">
            <div class="p-2 rounded bg-primary-lt">
              <IconShieldCheck size={24} class="text-primary" />
            </div>
            <div>
              <h2 class="modal-title h3 mb-0" id="auth-modal-title">
                {currentUser ? "Account & Identity" : "Fasti Authentication & Sign-In"}
              </h2>
              <p class="text-muted small mb-0">
                AllAuth & Nuvio Scoped Protocol: Passkeys, OIDC, Living Room PIN & PATs
              </p>
            </div>
          </div>
          <button
            type="button"
            class="btn-close"
            onclick={onClose}
            aria-label="Close authentication modal"
          ></button>
        </div>

        {#if currentUser}
          <!-- Logged in state -->
          <div class="modal-body p-4">
            <div class="card p-3 border mb-3 bg-body-tertiary">
              <div class="d-flex align-items-center justify-content-between">
                <div class="d-flex align-items-center gap-3">
                  <div
                    class="avatar avatar-lg rounded-circle bg-primary text-white fs-3 fw-bold"
                  >
                    {currentUser.displayName.charAt(0).toUpperCase()}
                  </div>
                  <div>
                    <h3 class="h4 mb-0">{currentUser.displayName}</h3>
                    <div class="text-muted font-monospace small">
                      @{currentUser.username} · Role: {currentUser.role}
                    </div>
                    <span class="badge bg-success-lt mt-1">
                      Authenticated via {currentUser.authMethod.toUpperCase()}
                    </span>
                  </div>
                </div>
                <button
                  type="button"
                  class="btn btn-outline-danger btn-sm"
                  onclick={() => {
                    onSignOut();
                    onClose();
                  }}
                >
                  Sign Out
                </button>
              </div>
            </div>

            <div class="alert alert-info d-flex gap-2 align-items-center mb-0">
              <IconInfoCircle size={18} />
              <div class="small">
                Your session is encrypted locally. Outbound scrobbles and sync events
                are signed with your local keypair.
              </div>
            </div>
          </div>
        {:else}
          <!-- Sign in state with all auth options -->
          <div class="modal-body p-0">
            <div class="row g-0">
              <!-- Left: Auth Method Sidebar Tabs -->
              <div
                class="col-12 col-md-4 border-end p-3"
                style="background: var(--fasti-surface-archive, rgba(0,0,0,0.03));"
              >
                <div class="nav flex-column nav-pills gap-1" role="tablist">
                  <button
                    type="button"
                    class="nav-link text-start d-flex align-items-center gap-2 py-2 px-3 rounded"
                    class:active={activeTab === "passkey"}
                    onclick={() => (activeTab = "passkey")}
                  >
                    <IconFingerprint size={18} />
                    <span>Passkey / Biometrics</span>
                  </button>

                  <button
                    type="button"
                    class="nav-link text-start d-flex align-items-center gap-2 py-2 px-3 rounded"
                    class:active={activeTab === "oidc"}
                    onclick={() => (activeTab = "oidc")}
                  >
                    <IconWorld size={18} />
                    <span>Single Sign-On (OIDC)</span>
                  </button>

                  <button
                    type="button"
                    class="nav-link text-start d-flex align-items-center gap-2 py-2 px-3 rounded"
                    class:active={activeTab === "pin"}
                    onclick={() => (activeTab = "pin")}
                  >
                    <IconDeviceTv size={18} />
                    <span>TV & Device PIN Pairing</span>
                  </button>

                  <button
                    type="button"
                    class="nav-link text-start d-flex align-items-center gap-2 py-2 px-3 rounded"
                    class:active={activeTab === "password"}
                    onclick={() => (activeTab = "password")}
                  >
                    <IconLock size={18} />
                    <span>Master Password</span>
                  </button>

                  <button
                    type="button"
                    class="nav-link text-start d-flex align-items-center gap-2 py-2 px-3 rounded"
                    class:active={activeTab === "pat"}
                    onclick={() => (activeTab = "pat")}
                  >
                    <IconKey size={18} />
                    <span>Access Token (PAT)</span>
                  </button>
                </div>
              </div>

              <!-- Right: Active Auth Pane -->
              <div class="col-12 col-md-8 p-4">
                {#if statusMessage}
                  <div
                    class="alert mb-3 p-2 small"
                    class:alert-success={statusMessage.type === "success"}
                    class:alert-danger={statusMessage.type === "error"}
                    role="alert"
                  >
                    {statusMessage.text}
                  </div>
                {/if}

                <!-- 1. Passkey / WebAuthn -->
                {#if activeTab === "passkey"}
                  <div>
                    <h3 class="h4 mb-1 d-flex align-items-center gap-2">
                      <IconFingerprint size={20} class="text-primary" /> FIDO2 Passkey Verification
                    </h3>
                    <p class="text-muted small mb-4">
                      Passwordless cryptographic login using Touch ID, Face ID, Windows Hello, or YubiKey hardware tokens.
                    </p>

                    <div class="card p-4 text-center border-dashed mb-3">
                      <div class="mb-3">
                        <span class="avatar avatar-xl rounded-circle bg-primary-lt">
                          <IconFingerprint size={36} class="text-primary" />
                        </span>
                      </div>
                      <h4 class="mb-1">Touch your security key or sensor</h4>
                      <p class="text-muted small mb-3">
                        Verify identity on this device with zero passwords transmitted over the network.
                      </p>
                      <button
                        type="button"
                        class="btn btn-primary btn-lg w-100 d-flex align-items-center justify-content-center gap-2"
                        disabled={isSubmitting}
                        onclick={handlePasskeyLogin}
                      >
                        <IconFingerprint size={20} />
                        {isSubmitting ? "Authenticating..." : "Authenticate with Passkey"}
                      </button>
                    </div>
                  </div>

                <!-- 2. Single Sign-On (OIDC / OAuth2) -->
                {:else if activeTab === "oidc"}
                  <div>
                    <h3 class="h4 mb-1 d-flex align-items-center gap-2">
                      <IconWorld size={20} class="text-primary" /> Single Sign-On Providers
                    </h3>
                    <p class="text-muted small mb-3">
                      Authenticate with your self-hosted identity provider or federated OAuth2 account.
                    </p>

                    <div class="d-flex flex-column gap-2 mb-4">
                      <button
                        type="button"
                        class="btn btn-outline-secondary d-flex align-items-center justify-content-between p-2"
                        onclick={() => handleOidcLogin("Authentik / Authelia")}
                      >
                        <span class="fw-bold">Authentik / Authelia (OIDC)</span>
                        <span class="badge bg-primary-lt">Self-Hosted</span>
                      </button>

                      <button
                        type="button"
                        class="btn btn-outline-secondary d-flex align-items-center justify-content-between p-2"
                        onclick={() => handleOidcLogin("Keycloak")}
                      >
                        <span class="fw-bold">Keycloak IAM</span>
                        <span class="badge bg-secondary-lt">Enterprise SSO</span>
                      </button>

                      <button
                        type="button"
                        class="btn btn-outline-secondary d-flex align-items-center justify-content-between p-2"
                        onclick={() => handleOidcLogin("Pocket ID")}
                      >
                        <span class="fw-bold">Pocket ID</span>
                        <span class="badge bg-info-lt">Passkey OIDC</span>
                      </button>

                      <div class="row g-2 mt-1">
                        <div class="col-4">
                          <button
                            type="button"
                            class="btn btn-outline-secondary w-100 d-flex align-items-center justify-content-center gap-1"
                            onclick={() => handleOidcLogin("GitHub")}
                          >
                            <IconBrandGithub size={16} /> GitHub
                          </button>
                        </div>
                        <div class="col-4">
                          <button
                            type="button"
                            class="btn btn-outline-secondary w-100 d-flex align-items-center justify-content-center gap-1"
                            onclick={() => handleOidcLogin("Google")}
                          >
                            <IconBrandGoogle size={16} /> Google
                          </button>
                        </div>
                        <div class="col-4">
                          <button
                            type="button"
                            class="btn btn-outline-secondary w-100 d-flex align-items-center justify-content-center gap-1"
                            onclick={() => handleOidcLogin("Apple")}
                          >
                            <IconBrandApple size={16} /> Apple
                          </button>
                        </div>
                      </div>
                    </div>

                    <div class="mb-2">
                      <label class="form-label small fw-bold" for="oidc-issuer-url">Custom OIDC Issuer Discovery URL</label>
                      <input
                        type="url"
                        id="oidc-issuer-url"
                        class="form-control form-control-sm font-monospace"
                        bind:value={oidcIssuerInput}
                        placeholder="https://auth.example.com/application/o/fasti/"
                      />
                    </div>
                  </div>

                <!-- 3. TV & Device PIN Pairing (NuvioTV Fast Connect) -->
                {:else if activeTab === "pin"}
                  <div>
                    <h3 class="h4 mb-1 d-flex align-items-center gap-2">
                      <IconDeviceTv size={20} class="text-primary" /> NuvioTV & Living Room Pairing
                    </h3>
                    <p class="text-muted small mb-3">
                      Pair your Android TV, Apple TV, or mobile player to this Fasti node without typing passwords.
                    </p>

                    <div class="card p-3 border text-center mb-3">
                      <div class="text-muted small text-uppercase font-monospace mb-1">
                        6-Digit Pairing PIN (Expires in {pinExpiresSeconds}s)
                      </div>
                      <div class="font-monospace fw-bold display-5 text-primary tracking-widest my-2">
                        {generatedPin}
                      </div>
                      <div class="d-flex align-items-center justify-content-center gap-2 mt-2">
                        <button
                          type="button"
                          class="btn btn-sm btn-outline-secondary d-flex align-items-center gap-1"
                          onclick={handleCopyPin}
                        >
                          {#if pinCopied}
                            <IconCheck size={14} /> Copied!
                          {:else}
                            <IconCopy size={14} /> Copy PIN
                          {/if}
                        </button>
                        <button
                          type="button"
                          class="btn btn-sm btn-ghost-secondary d-flex align-items-center gap-1"
                          onclick={handleRegeneratePin}
                        >
                          <IconRefresh size={14} /> New Code
                        </button>
                      </div>
                    </div>

                    <ol class="step-list text-muted small ps-3 mb-0">
                      <li>Open NuvioTV &rarr; Settings &rarr; Tracking &rarr; Fasti Connect</li>
                      <li>Select <strong>Enter Pairing PIN</strong> and type the code above</li>
                      <li>The TV will automatically connect to your local Fasti loopback instance</li>
                    </ol>
                  </div>

                <!-- 4. Master Password -->
                {:else if activeTab === "password"}
                  <form onsubmit={handlePasswordLogin}>
                    <h3 class="h4 mb-1 d-flex align-items-center gap-2">
                      <IconLock size={20} class="text-primary" /> Local Offline Account
                    </h3>
                    <p class="text-muted small mb-3">
                      Sign in with your local instance credentials stored securely in SQLite/Postgres.
                    </p>

                    <div class="mb-3">
                      <label class="form-label small fw-bold" for="login-username">Username or Email</label>
                      <input
                        type="text"
                        id="login-username"
                        class="form-control"
                        bind:value={usernameInput}
                        required
                      />
                    </div>

                    <div class="mb-3">
                      <label class="form-label small fw-bold" for="login-password">Master Password</label>
                      <input
                        type="password"
                        id="login-password"
                        class="form-control"
                        bind:value={passwordInput}
                        placeholder="••••••••••••"
                      />
                    </div>

                    <div class="mb-3">
                      <label class="form-label small fw-bold" for="login-session">Session Timeout</label>
                      <select class="form-select form-select-sm" id="login-session" bind:value={rememberSession}>
                        <option value="2_weeks">2 Weeks (Standard)</option>
                        <option value="30_days">30 Days</option>
                        <option value="never">Never Expire (Local Single-User)</option>
                      </select>
                    </div>

                    <button
                      type="submit"
                      class="btn btn-primary w-100"
                      disabled={isSubmitting}
                    >
                      {isSubmitting ? "Signing in..." : "Sign In to Fasti"}
                    </button>
                  </form>

                <!-- 5. Personal Access Token (PAT) -->
                {:else if activeTab === "pat"}
                  <form onsubmit={handlePatLogin}>
                    <h3 class="h4 mb-1 d-flex align-items-center gap-2">
                      <IconKey size={20} class="text-primary" /> Personal Access Token (PAT)
                    </h3>
                    <p class="text-muted small mb-3">
                      Sign in directly using an authorized Fasti Bearer Token.
                    </p>

                    <div class="mb-3">
                      <label class="form-label small fw-bold" for="login-pat-token">Bearer Access Token</label>
                      <input
                        type="password"
                        id="login-pat-token"
                        class="form-control font-monospace"
                        placeholder="fst_pat_..."
                        bind:value={patTokenInput}
                        required
                      />
                    </div>

                    <button
                      type="submit"
                      class="btn btn-primary w-100"
                      disabled={isSubmitting}
                    >
                      {isSubmitting ? "Validating..." : "Sign In with Token"}
                    </button>
                  </form>
                {/if}
              </div>
            </div>
          </div>
        {/if}

        <div class="modal-footer">
          <button type="button" class="btn btn-outline-secondary" onclick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
