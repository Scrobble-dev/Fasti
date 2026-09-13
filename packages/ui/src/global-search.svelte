<script lang="ts">
  import { onMount } from "svelte";
  import IconArrowRight from "@tabler/icons-svelte/icons/arrow-right";
  import IconCommand from "@tabler/icons-svelte/icons/command";
  import IconSearch from "@tabler/icons-svelte/icons/search";
  import type { MediaRecord, NavItemConfig } from "./types.js";

  interface Props {
    records: MediaRecord[];
    navItems: NavItemConfig[];
    onSelectRecord: (recordId: string) => void;
    onSelectSection: (section: string) => void;
  }

  type SearchResult =
    | {
        id: string;
        kind: "record";
        label: string;
        meta: string;
        record: MediaRecord;
      }
    | {
        id: string;
        kind: "command";
        label: string;
        meta: string;
        section: string;
      };

  let { records, navItems, onSelectRecord, onSelectSection }: Props = $props();
  let root: HTMLDivElement | undefined;
  let input: HTMLInputElement | undefined;
  let query = $state("");
  let open = $state(false);
  let activeIndex = $state(0);
  const searchableSections = new Set([
    "home",
    "discover",
    "library",
    "calendar",
    "reconciliation",
    "connections",
    "settings",
  ]);

  const results = $derived.by<SearchResult[]>(() => {
    const needle = query.trim().toLocaleLowerCase();
    const recordMatches = records
      .filter((record) => {
        if (!needle) return false;
        return [record.title, record.originalTitle, ...record.tags]
          .filter(Boolean)
          .some((value) => value!.toLocaleLowerCase().includes(needle));
      })
      .slice(0, 6)
      .map((record) => ({
        id: `record-${record.id}`,
        kind: "record" as const,
        label: record.title,
        meta: [record.mediaKind, record.releaseYear]
          .filter(Boolean)
          .join(" · "),
        record,
      }));
    const commandMatches = [...navItems]
      .filter((item) => item.visible && searchableSections.has(item.id))
      .sort((left, right) => left.order - right.order)
      .filter(
        (item) => !needle || item.label.toLocaleLowerCase().includes(needle),
      )
      .slice(0, needle ? 4 : 6)
      .map((item) => ({
        id: `command-${item.id}`,
        kind: "command" as const,
        label: `Open ${item.label}`,
        meta: "Navigation command",
        section: item.id,
      }));
    return [...recordMatches, ...commandMatches];
  });

  $effect(() => {
    if (activeIndex >= results.length)
      activeIndex = Math.max(0, results.length - 1);
  });

  function choose(result: SearchResult): void {
    if (result.kind === "record") onSelectRecord(result.record.id);
    else onSelectSection(result.section);
    query = "";
    open = false;
  }

  function handleInputKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape" && open) {
      event.preventDefault();
      query = "";
      open = false;
      input?.focus();
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) open = true;
      const direction = event.key === "ArrowDown" ? 1 : -1;
      activeIndex =
        results.length === 0
          ? 0
          : (activeIndex + direction + results.length) % results.length;
      queueMicrotask(() =>
        document
          .getElementById(results[activeIndex]?.id ?? "")
          ?.scrollIntoView({ block: "nearest" }),
      );
      return;
    }
    if (event.key === "Enter" && open && results[activeIndex]) {
      event.preventDefault();
      choose(results[activeIndex]);
    }
  }

  function handleResultKeydown(event: KeyboardEvent): void {
    if (event.key !== "Escape") return;
    event.preventDefault();
    query = "";
    open = false;
    input?.focus();
  }

  function handleFocusOut(): void {
    queueMicrotask(() => {
      if (!root?.contains(document.activeElement)) open = false;
    });
  }

  onMount(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      if (
        document.querySelector(
          'dialog[open], [role="dialog"][aria-modal="true"]',
        )
      )
        return;
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        input?.focus();
        open = true;
      }
    };
    const handlePointerDown = (event: PointerEvent) => {
      if (root && !root.contains(event.target as Node)) open = false;
    };
    window.addEventListener("keydown", handleShortcut);
    window.addEventListener("pointerdown", handlePointerDown);
    return () => {
      window.removeEventListener("keydown", handleShortcut);
      window.removeEventListener("pointerdown", handlePointerDown);
    };
  });
</script>

<div class="global-search" bind:this={root} onfocusout={handleFocusOut}>
  <IconSearch size={17} aria-hidden="true" />
  <input
    bind:this={input}
    bind:value={query}
    type="search"
    placeholder="Search records or commands"
    aria-label="Search records or commands"
    role="combobox"
    aria-autocomplete="list"
    aria-expanded={open}
    aria-controls="global-search-results"
    aria-activedescendant={open && results[activeIndex]
      ? results[activeIndex].id
      : undefined}
    onfocus={() => (open = true)}
    oninput={() => {
      activeIndex = 0;
      open = true;
    }}
    onkeydown={handleInputKeydown}
  />
  <kbd><span>Ctrl</span> K</kbd>

  {#if open}
    <div id="global-search-results" class="search-results" role="listbox">
      {#if results.length === 0}
        <p role="status">No records or commands match.</p>
      {:else}
        {#each results as result, index (result.id)}
          <button
            id={result.id}
            type="button"
            role="option"
            tabindex="-1"
            aria-selected={index === activeIndex}
            class:active={index === activeIndex}
            onpointermove={() => (activeIndex = index)}
            onclick={() => choose(result)}
            onkeydown={handleResultKeydown}
          >
            {#if result.kind === "record"}
              <span class="result-art" aria-hidden="true">
                {#if result.record.posterUrl}
                  <img src={result.record.posterUrl} alt="" />
                {:else}
                  {result.record.mediaKind.slice(0, 1).toUpperCase()}
                {/if}
              </span>
            {:else}
              <span class="result-art command" aria-hidden="true"
                ><IconCommand size={17} /></span
              >
            {/if}
            <span class="result-copy">
              <strong>{result.label}</strong>
              <small>{result.meta || "Fasti record"}</small>
            </span>
            {#if result.kind === "record" && result.record.userRating}
              <span class="rating">★ {result.record.userRating}</span>
            {/if}
            <IconArrowRight size={16} aria-hidden="true" />
          </button>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .global-search {
    position: relative;
    width: min(42vw, 520px);
    min-width: 240px;
    display: flex;
    align-items: center;
    color: var(--fasti-text-muted);
  }

  .global-search > :global(svg) {
    position: absolute;
    left: 12px;
    pointer-events: none;
  }

  input {
    width: 100%;
    min-height: 44px;
    padding: 8px 72px 8px 38px;
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  input:focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }

  kbd {
    position: absolute;
    right: 8px;
    display: inline-flex;
    gap: 3px;
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 20%, transparent));
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    padding: 2px 5px;
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-muted);
    font-size: 0.68rem;
  }

  .search-results {
    position: absolute;
    top: calc(100% + 7px);
    left: 0;
    right: 0;
    z-index: 1100;
    max-height: min(65dvh, 480px);
    overflow: auto;
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 20%, transparent));
    border-radius: calc(7px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-surface-paper);
    box-shadow: 0 14px 32px rgba(0, 0, 0, 0.18);
    padding: 6px;
  }

  .search-results > p {
    margin: 0;
    padding: 14px;
    color: var(--fasti-text-muted);
  }

  .search-results button {
    width: 100%;
    min-height: 52px;
    display: flex;
    align-items: center;
    gap: 10px;
    border: 0;
    border-radius: calc(5px * var(--tblr-border-radius-scale, 1));
    padding: 6px 9px;
    background: transparent;
    color: var(--fasti-text-primary);
    text-align: left;
    cursor: pointer;
  }

  .search-results button.active,
  .search-results button:hover {
    background: var(--fasti-surface-archive);
  }

  .result-art {
    width: 34px;
    height: 44px;
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    overflow: hidden;
    border-radius: calc(3px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-muted);
    font-family: var(--fasti-font-mono);
    font-weight: 700;
  }

  .result-art.command {
    height: 34px;
  }

  .result-art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .result-copy {
    min-width: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .result-copy strong,
  .result-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .result-copy small {
    color: var(--fasti-text-muted);
    text-transform: capitalize;
  }

  .rating {
    flex: 0 0 auto;
    color: var(--fasti-brand-gold);
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    font-weight: 700;
  }

  @media (max-width: 47.99rem) {
    .global-search {
      min-width: 0;
      width: 100%;
    }

    kbd {
      display: none;
    }

    input {
      padding-right: 10px;
    }
  }
</style>
