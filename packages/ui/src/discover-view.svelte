<script lang="ts">
  import type {
    ProviderCredentialStatus,
    ProviderSearchCandidate,
  } from "./types.js";
  import { hostProblemText } from "./host-problem.js";
  import { IconCompass, IconSearch } from "@tabler/icons-svelte";

  interface Props {
    providerCredentials?: ProviderCredentialStatus[];
    loading?: boolean;
    hostProblem?: string;
    onSearch: (
      provider: string,
      query: string,
    ) => Promise<ProviderSearchCandidate[]>;
    onOpenSettings: () => void;
    onRetry: () => void;
  }

  let {
    providerCredentials,
    loading = false,
    hostProblem,
    onSearch,
    onOpenSettings,
    onRetry,
  }: Props = $props();
  let query = $state("");
  let results: ProviderSearchCandidate[] = $state([]);
  let searching = $state(false);
  let problem = $state("");
  let searched = $state(false);
  let completedQuery = $state("");
  const googleBooks = $derived(
    providerCredentials?.find((item) => item.provider === "google-books"),
  );

  async function search(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const value = query.trim();
    if (!value || !googleBooks?.configured || searching) return;
    searching = true;
    problem = "";
    searched = false;
    try {
      results = await onSearch("google-books", value);
      completedQuery = value;
      searched = true;
    } catch (error) {
      results = [];
      problem = hostProblemText(
        error,
        "Google Books search failed. Check the provider key and network policy.",
      );
    } finally {
      searching = false;
    }
  }
</script>

<div class="discover-container">
  <header class="discover-header">
    <div class="heading-row">
      <IconCompass size={28} class="discover-icon" />
      <h1 id="discover-title" class="view-title" tabindex="-1">Discover</h1>
    </div>
    <p class="view-subtitle">
      Search Google Books through the trusted Fasti desktop host.
    </p>
  </header>

  {#if loading}
    <p role="status">Loading provider status…</p>
  {:else if hostProblem && providerCredentials === undefined}
    <div class="unavailable">
      <p class="problem" role="alert">{hostProblem}</p>
      <button id="provider-retry" type="button" onclick={onRetry}>
        Retry host connection
      </button>
    </div>
  {:else if !googleBooks?.configured}
    <section class="unavailable" aria-labelledby="discover-setup-title">
      <h2 id="discover-setup-title">Google Books needs an API key</h2>
      <p>
        Add a key in Settings. Fasti stores it in the platform credential store.
      </p>
      <button type="button" onclick={onOpenSettings}
        >Open provider settings</button
      >
    </section>
  {:else}
    <form class="search-form" onsubmit={search} role="search">
      <label for="provider-search">Search books</label>
      <div class="search-row">
        <input
          id="provider-search"
          type="search"
          required
          maxlength="256"
          bind:value={query}
          disabled={searching}
          placeholder="Title, author, or ISBN"
          autocomplete="off"
        />
        <button type="submit" disabled={searching || !query.trim()}>
          <IconSearch size={18} aria-hidden="true" />
          {searching ? "Searching…" : "Search"}
        </button>
      </div>
    </form>

    <section
      class="results"
      aria-labelledby="search-results-title"
      aria-busy={searching}
    >
      <h2 id="search-results-title">Search results</h2>
      {#if searching}
        <p role="status">Searching Google Books…</p>
      {:else if problem}
        <p class="problem" role="alert">{problem}</p>
      {:else if searched && results.length === 0}
        <p role="status">No matching books found for {completedQuery}.</p>
      {:else if results.length > 0}
        <p role="status">
          {results.length}
          {results.length === 1 ? "result" : "results"} for
          {completedQuery}.
        </p>
        <ol>
          {#each results as result (result.provider_id)}
            <li>
              <h3>{result.title}</h3>
              {#if result.description}<p>{result.description}</p>{/if}
              <dl>
                <div>
                  <dt>Provider</dt>
                  <dd>{result.provider}</dd>
                </div>
                <div>
                  <dt>Type</dt>
                  <dd>{result.kind}</dd>
                </div>
                <div>
                  <dt>Provider ID</dt>
                  <dd><code>{result.provider_id}</code></dd>
                </div>
              </dl>
            </li>
          {/each}
        </ol>
      {:else}
        <p>Enter a title, author, or ISBN.</p>
      {/if}
    </section>
  {/if}
</div>

<style>
  .discover-container {
    max-width: 1000px;
    margin: 0 auto;
    padding: 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  .discover-header {
    padding-bottom: 16px;
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
  }

  .heading-row,
  .search-row,
  dl div {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  :global(.discover-icon) {
    color: var(--fasti-brand-mark);
  }

  .view-title {
    margin: 0;
    font-family: var(--fasti-font-display);
    font-size: 2.4rem;
    font-weight: 600;
  }

  .view-subtitle,
  .unavailable p,
  .results > p,
  li p {
    margin: 0;
    color: var(--fasti-text-muted);
  }

  .unavailable,
  .search-form,
  .results {
    padding: 24px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
    background: var(--fasti-surface-paper);
  }

  .unavailable h2,
  .results h2 {
    margin: 0 0 8px;
    font-family: var(--fasti-font-display);
  }

  .unavailable button {
    margin-top: 16px;
  }

  .search-form {
    display: grid;
    gap: 8px;
  }

  .search-form label {
    font-weight: 700;
  }

  input {
    flex: 1;
    min-width: 0;
    padding: 10px 12px;
  }

  button,
  input {
    min-height: 44px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 35%, transparent);
    border-radius: 4px;
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 10px 16px;
    background: var(--fasti-action-primary);
    border-color: var(--fasti-action-primary);
    color: white;
    font-weight: 700;
    cursor: pointer;
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.68;
  }

  :is(button, input):focus-visible {
    outline: 3px solid var(--fasti-action-primary);
    outline-offset: 2px;
  }

  ol {
    list-style: none;
    margin: 16px 0 0;
    padding: 0;
    display: grid;
    gap: 12px;
  }

  li {
    padding: 16px 0;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 22%, transparent);
  }

  li h3 {
    margin: 0 0 6px;
  }

  dl {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 16px;
    margin: 12px 0 0;
    font-size: 0.82rem;
  }

  dt {
    font-weight: 700;
  }

  dd {
    margin: 0;
  }

  .problem {
    color: var(--fasti-state-error, #b42318);
  }

  @media (max-width: 47.99rem) {
    .discover-container {
      padding: 24px 16px;
    }

    .search-row {
      align-items: stretch;
      flex-direction: column;
    }
  }
</style>
