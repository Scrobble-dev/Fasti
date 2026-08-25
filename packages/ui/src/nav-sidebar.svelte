<script lang="ts">
  import type { ActiveNavSection } from "./types.js";
  import {
    IconClock,
    IconCompass,
    IconLayoutGrid,
    IconPlayerPlay,
    IconCalendar,
    IconGitPullRequest,
    IconPlug,
    IconSettings,
    IconCircleCheck,
    IconDeviceDesktop,
  } from "@tabler/icons-svelte";

  interface Props {
    activeSection: ActiveNavSection;
    openReviewCount?: number;
    onSelectSection: (section: ActiveNavSection) => void;
  }

  let { activeSection, openReviewCount = 1, onSelectSection }: Props = $props();

  const navItems = $derived([
    {
      id: "chronicle" as ActiveNavSection,
      label: "Chronicle",
      icon: IconClock,
    },
    {
      id: "discover" as ActiveNavSection,
      label: "Discover",
      icon: IconCompass,
    },
    {
      id: "library" as ActiveNavSection,
      label: "Library",
      icon: IconLayoutGrid,
    },
    {
      id: "up_next" as ActiveNavSection,
      label: "Up Next",
      icon: IconPlayerPlay,
    },
    {
      id: "calendar" as ActiveNavSection,
      label: "Calendar",
      icon: IconCalendar,
    },
    {
      id: "reconciliation" as ActiveNavSection,
      label: "Review Inbox",
      icon: IconGitPullRequest,
      badge: openReviewCount > 0 ? openReviewCount : undefined,
    },
    {
      id: "connections" as ActiveNavSection,
      label: "Connections",
      icon: IconPlug,
    },
    {
      id: "settings" as ActiveNavSection,
      label: "Settings",
      icon: IconSettings,
    },
  ]);
</script>

<aside class="sidebar-shell" aria-label="Main Navigation">
  <div class="brand-header">
    <div class="mark-row">
      <svg
        class="brand-mark"
        viewBox="0 0 32 32"
        width="24"
        height="24"
        aria-hidden="true"
      >
        <rect
          x="4"
          y="4"
          width="4"
          height="24"
          fill="currentColor"
          opacity="0.4"
        />
        <rect
          x="12"
          y="4"
          width="4"
          height="24"
          fill="currentColor"
          opacity="0.7"
        />
        <rect x="20" y="4" width="4" height="24" fill="currentColor" />
        <rect
          x="2"
          y="4"
          width="24"
          height="4"
          fill="var(--fasti-brand-mark)"
        />
        <circle cx="22" cy="16" r="3" fill="var(--fasti-brand-gold)" />
      </svg>
      <span class="brand-name">Fasti</span>
    </div>
    <span class="tagline">Living Chronicle</span>
  </div>

  <nav class="nav-list" aria-label="Primary destinations">
    {#each navItems as item (item.id)}
      <button
        type="button"
        class="nav-button"
        class:active={activeSection === item.id ||
          (activeSection === "detail" && item.id === "library")}
        onclick={() => onSelectSection(item.id)}
        aria-current={activeSection === item.id ? "page" : undefined}
      >
        <span class="icon-wrap">
          <item.icon size={20} stroke={1.75} />
        </span>
        <span class="nav-label">{item.label}</span>
        {#if item.badge}
          <span class="badge" aria-label="{item.badge} items need review"
            >{item.badge}</span
          >
        {/if}
      </button>
    {/each}
  </nav>

  <div class="footer-status">
    <div class="node-badge" title="Local node status is healthy">
      <IconCircleCheck size={16} stroke={2} class="status-icon verified" />
      <span class="node-label">Local Node · Loopback</span>
    </div>
    <div class="profile-chip">
      <span class="profile-avatar">RW</span>
      <div class="profile-info">
        <span class="profile-name">Ryan Winkler</span>
        <span class="profile-workspace">Primary Workspace</span>
      </div>
    </div>
  </div>
</aside>

<style>
  .sidebar-shell {
    width: 240px;
    background: var(--fasti-surface-paper);
    border-right: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    display: flex;
    flex-direction: column;
    height: 100vh;
    padding: 20px 14px;
    user-select: none;
    box-sizing: border-box;
  }

  .brand-header {
    margin-bottom: 24px;
    padding: 0 8px;
  }

  .mark-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .brand-mark {
    color: var(--fasti-brand-mark);
  }

  .brand-name {
    font-family: var(--fasti-font-display);
    font-size: 1.5rem;
    font-weight: 600;
    letter-spacing: -0.02em;
    color: var(--fasti-text-primary);
  }

  .tagline {
    display: block;
    margin-top: 2px;
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fasti-text-muted);
  }

  .nav-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
  }

  .nav-button {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    min-height: 40px;
    padding: 8px 12px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--fasti-text-muted);
    font-family: var(--fasti-font-body);
    font-size: 0.92rem;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    transition:
      background 120ms ease,
      color 120ms ease;
  }

  .nav-button:hover {
    background: color-mix(
      in srgb,
      var(--fasti-surface-archive) 60%,
      transparent
    );
    color: var(--fasti-text-primary);
  }

  .nav-button.active {
    background: var(--fasti-surface-archive);
    color: var(--fasti-action-primary);
    font-weight: 600;
    border-left: 3px solid var(--fasti-brand-mark);
  }

  .nav-button:focus-visible {
    outline: 2px solid var(--fasti-brand-gold);
    outline-offset: 2px;
  }

  .icon-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .nav-label {
    flex: 1;
  }

  .badge {
    padding: 2px 7px;
    border-radius: 10px;
    background: var(--fasti-state-attention);
    color: white;
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    font-weight: 700;
  }

  .footer-status {
    padding-top: 14px;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .node-badge {
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    color: var(--fasti-text-muted);
    padding: 4px 6px;
  }

  :global(.status-icon.verified) {
    color: var(--fasti-state-verified);
  }

  .profile-chip {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px;
    border-radius: 4px;
    background: color-mix(
      in srgb,
      var(--fasti-surface-archive) 70%,
      transparent
    );
  }

  .profile-avatar {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--fasti-brand-mark);
    color: white;
    font-family: var(--fasti-font-mono);
    font-size: 0.78rem;
    font-weight: 700;
    display: grid;
    place-items: center;
  }

  .profile-info {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
    overflow: hidden;
  }

  .profile-name {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--fasti-text-primary);
    white-space: nowrap;
    text-overflow: ellipsis;
    overflow: hidden;
  }

  .profile-workspace {
    font-size: 0.72rem;
    color: var(--fasti-text-muted);
  }
</style>
