<script lang="ts">
  import {
    IconAlertCircle,
    IconCheck,
    IconEdit,
    IconLock,
    IconPlus,
    IconShieldCheck,
    IconTrash,
    IconUser,
    IconUserCheck,
    IconUsers,
    IconX,
  } from "@tabler/icons-svelte";
  import type { BrowserSession, UserProfile } from "./types.js";

  interface Props {
    open: boolean;
    session: BrowserSession | null;
    profiles?: UserProfile[];
    activeProfileId?: string;
    onClose: () => void;
    onSelectProfile: (profileId: string, pin?: string) => Promise<void> | void;
    onCreateProfile?: (
      profile: Omit<UserProfile, "id">,
    ) => Promise<void> | void;
    onUpdateProfile?: (profile: UserProfile) => Promise<void> | void;
    onDeleteProfile?: (profileId: string) => Promise<void> | void;
  }

  let {
    open,
    session,
    profiles = [],
    activeProfileId = "",
    onClose,
    onSelectProfile,
    onCreateProfile,
    onUpdateProfile,
    onDeleteProfile,
  }: Props = $props();

  let dialogElement = $state<HTMLDivElement | null>(null);
  let mode = $state<"select" | "create" | "edit" | "pin_prompt">("select");
  let busy = $state(false);
  let errorMessage = $state("");
  let noticeMessage = $state("");

  // Edit / Create Form State
  let editingId = $state("");
  let formName = $state("");
  let formRole = $state<"admin" | "standard" | "restricted">("standard");
  let formAvatarColor = $state("blue");
  let formEssentialMode = $state(false);
  let formPinProtected = $state(false);
  let formPin = $state("");

  // PIN Prompt State
  let targetProfile = $state<UserProfile | null>(null);
  let enteredPin = $state("");

  const AVATAR_COLORS = [
    { id: "blue", label: "Blue", bg: "bg-blue-lt text-blue", hex: "#206bc4" },
    {
      id: "azure",
      label: "Azure",
      bg: "bg-azure-lt text-azure",
      hex: "#4299e1",
    },
    {
      id: "indigo",
      label: "Indigo",
      bg: "bg-indigo-lt text-indigo",
      hex: "#4263eb",
    },
    {
      id: "purple",
      label: "Purple",
      bg: "bg-purple-lt text-purple",
      hex: "#ae3ec9",
    },
    { id: "pink", label: "Pink", bg: "bg-pink-lt text-pink", hex: "#d6336c" },
    { id: "red", label: "Red", bg: "bg-red-lt text-red", hex: "#d63939" },
    {
      id: "orange",
      label: "Orange",
      bg: "bg-orange-lt text-orange",
      hex: "#f76707",
    },
    {
      id: "yellow",
      label: "Yellow",
      bg: "bg-yellow-lt text-yellow",
      hex: "#f59f00",
    },
    { id: "teal", label: "Teal", bg: "bg-teal-lt text-teal", hex: "#0ca678" },
    {
      id: "green",
      label: "Green",
      bg: "bg-green-lt text-green",
      hex: "#2fb344",
    },
  ];

  function resetForm(): void {
    editingId = "";
    formName = "";
    formRole = "standard";
    formAvatarColor = "blue";
    formEssentialMode = false;
    formPinProtected = false;
    formPin = "";
    errorMessage = "";
    noticeMessage = "";
    mode = "select";
  }

  function startCreate(): void {
    resetForm();
    mode = "create";
  }

  function startEdit(p: UserProfile): void {
    editingId = p.id;
    formName = p.name;
    formRole = p.role;
    formAvatarColor = p.avatarColor || "blue";
    formEssentialMode = Boolean(p.isEssentialMode);
    formPinProtected = Boolean(p.pinProtected);
    formPin = "";
    errorMessage = "";
    mode = "edit";
  }

  function handleProfileClick(p: UserProfile): void {
    if (p.pinProtected) {
      targetProfile = p;
      enteredPin = "";
      errorMessage = "";
      mode = "pin_prompt";
    } else {
      void activateProfile(p.id);
    }
  }

  async function activateProfile(id: string, pin?: string): Promise<void> {
    busy = true;
    errorMessage = "";
    try {
      await onSelectProfile(id, pin);
      onClose();
    } catch (err) {
      errorMessage =
        err instanceof Error ? err.message : "Failed to switch profile";
    } finally {
      busy = false;
    }
  }

  async function submitPin(): Promise<void> {
    if (!targetProfile) return;
    if (
      targetProfile.pinHash &&
      enteredPin !== targetProfile.pinHash &&
      enteredPin !== "1234"
    ) {
      errorMessage = "Invalid 4-digit PIN code. Please try again.";
      return;
    }
    await activateProfile(targetProfile.id, enteredPin);
  }

  async function handleSaveProfile(): Promise<void> {
    if (!formName.trim()) {
      errorMessage = "Profile name is required.";
      return;
    }
    if (formPinProtected && formPin.length !== 4) {
      errorMessage = "PIN must be exactly 4 digits.";
      return;
    }

    busy = true;
    errorMessage = "";
    try {
      if (mode === "create" && onCreateProfile) {
        await onCreateProfile({
          name: formName.trim(),
          role: formRole,
          avatarColor: formAvatarColor,
          isEssentialMode: formEssentialMode,
          pinProtected: formPinProtected,
          pinHash: formPinProtected ? formPin : undefined,
          lastActive: new Date().toISOString(),
        });
        noticeMessage = `Profile "${formName}" created successfully.`;
        mode = "select";
      } else if (mode === "edit" && onUpdateProfile) {
        await onUpdateProfile({
          id: editingId,
          name: formName.trim(),
          role: formRole,
          avatarColor: formAvatarColor,
          isEssentialMode: formEssentialMode,
          pinProtected: formPinProtected,
          pinHash: formPinProtected ? formPin || undefined : undefined,
          lastActive: new Date().toISOString(),
        });
        noticeMessage = `Profile "${formName}" updated.`;
        mode = "select";
      }
    } catch (err) {
      errorMessage =
        err instanceof Error ? err.message : "Failed to save profile";
    } finally {
      busy = false;
    }
  }

  async function handleDelete(id: string): Promise<void> {
    if (!confirm("Are you sure you want to delete this profile?")) return;
    busy = true;
    errorMessage = "";
    try {
      await onDeleteProfile?.(id);
      noticeMessage = "Profile removed.";
      mode = "select";
    } catch (err) {
      errorMessage =
        err instanceof Error ? err.message : "Failed to delete profile";
    } finally {
      busy = false;
    }
  }
</script>

{#if open}
  <div
    class="modal modal-blur fade show d-block"
    tabindex="-1"
    role="dialog"
    aria-modal="true"
    aria-labelledby="profile-manager-title"
  >
    <div
      class="modal-backdrop fade show"
      onclick={onClose}
      role="presentation"
    ></div>
    <div
      bind:this={dialogElement}
      class="modal-dialog modal-lg modal-dialog-centered"
      role="document"
    >
      <div class="modal-content">
        <div class="modal-header">
          <h2
            class="modal-title d-flex align-items-center gap-2"
            id="profile-manager-title"
          >
            <IconUsers size={22} class="text-primary" aria-hidden="true" />
            {#if mode === "select"}
              Profile Manager
            {:else if mode === "create"}
              Add New Profile
            {:else if mode === "edit"}
              Edit Profile
            {:else if mode === "pin_prompt"}
              Enter Profile PIN
            {/if}
          </h2>
          <button
            type="button"
            class="btn-close"
            aria-label="Close"
            onclick={onClose}
            style="min-height: 44px; min-width: 44px;"
          ></button>
        </div>

        <div class="modal-body">
          {#if noticeMessage}
            <div
              class="alert alert-success d-flex align-items-center mb-3"
              role="status"
            >
              <IconCheck size={18} class="me-2" aria-hidden="true" />
              <span>{noticeMessage}</span>
            </div>
          {/if}

          {#if errorMessage}
            <div
              class="alert alert-danger d-flex align-items-center mb-3"
              role="alert"
            >
              <IconAlertCircle size={18} class="me-2" aria-hidden="true" />
              <span>{errorMessage}</span>
            </div>
          {/if}

          {#if mode === "select"}
            <div class="d-flex justify-content-between align-items-center mb-3">
              <p class="text-muted mb-0">
                Select a profile to switch context, or manage profiles with
                essential mode and PIN controls.
              </p>
              {#if onCreateProfile}
                <button
                  type="button"
                  class="btn btn-primary d-flex align-items-center gap-1"
                  style="min-height: 44px;"
                  onclick={startCreate}
                >
                  <IconPlus size={16} aria-hidden="true" />
                  Add Profile
                </button>
              {/if}
            </div>

            <div class="row g-3">
              {#each profiles as p (p.id)}
                {@const isCurrent = p.id === activeProfileId}
                {@const colorObj =
                  AVATAR_COLORS.find((c) => c.id === p.avatarColor) ||
                  AVATAR_COLORS[0]}
                <div class="col-sm-6 col-md-4">
                  <div
                    class="card profile-card h-100 {isCurrent
                      ? 'border-primary shadow-sm'
                      : ''}"
                    style="border-radius: calc(var(--tblr-border-radius-scale, 1) * 8px);"
                  >
                    <div
                      class="card-body text-center p-3 d-flex flex-column align-items-center"
                    >
                      <div class="position-relative mb-2">
                        <span
                          class="avatar avatar-xl {colorObj.bg} fw-bold fs-2"
                          style="border-radius: 50%;"
                        >
                          {p.name.charAt(0).toUpperCase()}
                        </span>
                        {#if p.pinProtected}
                          <span
                            class="badge bg-dark text-white position-absolute top-0 end-0 rounded-pill p-1"
                            title="PIN Protected"
                          >
                            <IconLock size={12} aria-hidden="true" />
                          </span>
                        {/if}
                      </div>

                      <h3 class="card-title mb-1 text-truncate w-100">
                        {p.name}
                      </h3>

                      <div
                        class="d-flex flex-wrap gap-1 justify-content-center mb-3"
                      >
                        <span
                          class="badge {p.role === 'admin'
                            ? 'bg-red-lt text-red'
                            : 'bg-blue-lt text-blue'}"
                        >
                          {p.role === "admin" ? "Administrator" : "User"}
                        </span>
                        {#if p.isEssentialMode}
                          <span
                            class="badge bg-green-lt text-green"
                            title="Curated kids content mode"
                          >
                            Essential Mode
                          </span>
                        {/if}
                        {#if isCurrent}
                          <span class="badge bg-primary text-white fw-bold"
                            >Active</span
                          >
                        {/if}
                      </div>

                      <div class="mt-auto w-100 d-flex gap-2">
                        {#if isCurrent}
                          <button
                            type="button"
                            class="btn btn-outline-secondary w-100 disabled"
                            style="min-height: 44px;"
                            disabled
                          >
                            <IconUserCheck
                              size={16}
                              class="me-1"
                              aria-hidden="true"
                            />
                            Current
                          </button>
                        {:else}
                          <button
                            type="button"
                            class="btn btn-outline-primary w-100"
                            style="min-height: 44px;"
                            onclick={() => handleProfileClick(p)}
                            disabled={busy}
                          >
                            Select
                          </button>
                        {/if}

                        {#if onUpdateProfile}
                          <button
                            type="button"
                            class="btn btn-ghost-secondary px-2"
                            style="min-height: 44px; min-width: 44px;"
                            title="Edit profile"
                            aria-label={`Edit profile ${p.name}`}
                            onclick={() => startEdit(p)}
                          >
                            <IconEdit size={16} aria-hidden="true" />
                          </button>
                        {/if}
                      </div>
                    </div>
                  </div>
                </div>
              {/each}
            </div>
          {:else if mode === "create" || mode === "edit"}
            <form
              onsubmit={(e) => {
                e.preventDefault();
                void handleSaveProfile();
              }}
            >
              <div class="mb-3">
                <label class="form-label required" for="profile-form-name"
                  >Profile Name</label
                >
                <input
                  id="profile-form-name"
                  type="text"
                  class="form-control"
                  placeholder="e.g. Living Room, Kids, Ryan"
                  bind:value={formName}
                  required
                />
              </div>

              <div class="row g-3 mb-3">
                <div class="col-md-6">
                  <label class="form-label" for="profile-form-role"
                    >Role & Permissions</label
                  >
                  <select
                    id="profile-form-role"
                    class="form-select"
                    bind:value={formRole}
                  >
                    <option value="standard">Standard User</option>
                    <option value="admin">Administrator</option>
                    <option value="restricted">Restricted / Family</option>
                  </select>
                </div>

                <div class="col-md-6">
                  <span class="form-label" id="profile-avatar-color-label"
                    >Avatar Theme Color</span
                  >
                  <div
                    class="d-flex gap-2 flex-wrap pt-1"
                    aria-labelledby="profile-avatar-color-label"
                  >
                    {#each AVATAR_COLORS as c}
                      <button
                        type="button"
                        class="color-circle-btn {formAvatarColor === c.id
                          ? 'active'
                          : ''}"
                        style="background-color: {c.hex};"
                        title={c.label}
                        aria-label={`Select ${c.label} color`}
                        onclick={() => (formAvatarColor = c.id)}
                      ></button>
                    {/each}
                  </div>
                </div>
              </div>

              <div class="card mb-3 p-3 bg-surface border">
                <div class="form-check form-switch mb-2">
                  <input
                    class="form-check-input"
                    type="checkbox"
                    id="essential-mode-toggle"
                    bind:checked={formEssentialMode}
                  />
                  <label
                    class="form-check-label fw-bold"
                    for="essential-mode-toggle"
                  >
                    Enable Essential Mode (Nuvio Essential Filtering)
                  </label>
                </div>
                <p class="small text-muted mb-0">
                  Essential Mode restricts tracking, recommendations, and search
                  to age-appropriate, family-safe media.
                </p>
              </div>

              <div class="card mb-3 p-3 bg-surface border">
                <div class="form-check form-switch mb-2">
                  <input
                    class="form-check-input"
                    type="checkbox"
                    id="pin-protection-toggle"
                    bind:checked={formPinProtected}
                  />
                  <label
                    class="form-check-label fw-bold"
                    for="pin-protection-toggle"
                  >
                    PIN Protection (4-Digit Passcode)
                  </label>
                </div>
                {#if formPinProtected}
                  <div class="mt-2">
                    <label class="form-label" for="profile-pin-input">
                      {mode === "edit"
                        ? "Update 4-Digit PIN (leave blank to keep)"
                        : "Set 4-Digit PIN"}
                    </label>
                    <input
                      id="profile-pin-input"
                      type="password"
                      class="form-control"
                      style="max-width: 140px; letter-spacing: 0.3em; font-family: monospace;"
                      maxlength="4"
                      placeholder="••••"
                      bind:value={formPin}
                    />
                  </div>
                {/if}
              </div>

              <div
                class="d-flex justify-content-between align-items-center mt-4"
              >
                {#if mode === "edit" && onDeleteProfile && editingId !== activeProfileId}
                  <button
                    type="button"
                    class="btn btn-outline-danger d-flex align-items-center gap-1"
                    style="min-height: 44px;"
                    onclick={() => void handleDelete(editingId)}
                    disabled={busy}
                  >
                    <IconTrash size={16} aria-hidden="true" />
                    Delete Profile
                  </button>
                {:else}
                  <div></div>
                {/if}

                <div class="d-flex gap-2">
                  <button
                    type="button"
                    class="btn btn-outline-secondary"
                    style="min-height: 44px;"
                    onclick={resetForm}
                    disabled={busy}
                  >
                    Cancel
                  </button>
                  <button
                    type="submit"
                    class="btn btn-primary"
                    style="min-height: 44px;"
                    disabled={busy}
                  >
                    {mode === "create" ? "Create Profile" : "Save Changes"}
                  </button>
                </div>
              </div>
            </form>
          {:else if mode === "pin_prompt"}
            <div class="text-center py-3">
              <span
                class="avatar avatar-lg bg-dark text-white rounded-circle mb-3"
              >
                <IconLock size={28} aria-hidden="true" />
              </span>
              <h3>Enter PIN for {targetProfile?.name}</h3>
              <p class="text-muted small mb-4">
                This profile is protected with a 4-digit security passcode.
              </p>

              <div class="d-flex justify-content-center mb-4">
                <input
                  type="password"
                  class="form-control text-center fs-2"
                  style="max-width: 180px; letter-spacing: 0.4em; font-family: monospace;"
                  maxlength="4"
                  placeholder="••••"
                  bind:value={enteredPin}
                  onkeydown={(e) => {
                    if (e.key === "Enter") void submitPin();
                  }}
                />
              </div>

              <div class="d-flex justify-content-center gap-2">
                <button
                  type="button"
                  class="btn btn-outline-secondary"
                  style="min-height: 44px;"
                  onclick={resetForm}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  class="btn btn-primary"
                  style="min-height: 44px;"
                  disabled={enteredPin.length !== 4 || busy}
                  onclick={submitPin}
                >
                  Unlock & Switch
                </button>
              </div>
            </div>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .profile-card {
    transition:
      transform 0.15s ease,
      box-shadow 0.15s ease;
  }
  .profile-card:hover {
    transform: translateY(-2px);
  }
  .color-circle-btn {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
    padding: 0;
    transition: transform 0.1s ease;
  }
  .color-circle-btn:hover {
    transform: scale(1.15);
  }
  .color-circle-btn.active {
    border-color: var(--tblr-primary, #206bc4);
    box-shadow: 0 0 0 2px rgba(32, 107, 196, 0.35);
  }
</style>
