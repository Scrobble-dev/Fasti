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
    onTrackRecord?: (candidate: ProviderSearchCandidate) => Promise<void>;
    embedded?: boolean;
    actionLabel?: string;
    completedLabel?: string;
    actionProblemFallback?: string;
    selectedProviderId?: string;
    selectionExplicit?: boolean;
  }

  let {
    providerCredentials,
    loading = false,
    hostProblem,
    onSearch,
    onOpenSettings,
    onRetry,
    onTrackRecord,
    embedded = false,
    actionLabel = "Track Now",
    completedLabel = "Added to library",
    actionProblemFallback = "Fasti could not add this title to your library.",
    selectedProviderId = $bindable(""),
    selectionExplicit = $bindable(false),
  }: Props = $props();
  let query = $state("");
  let results: ProviderSearchCandidate[] = $state([]);
  let searching = $state(false);
  let problem = $state("");
  let searched = $state(false);
  let completedQuery = $state("");
  let trackingId = $state("");
  let trackedIds = $state<Set<string>>(new Set());
  let trackProblem = $state("");
  let searchRevision = 0;
  let searchProviderId = "";

  async function trackRecord(
    candidate: ProviderSearchCandidate,
  ): Promise<void> {
    if (!onTrackRecord || trackingId) return;
    trackingId = candidate.provider_id;
    trackProblem = "";
    try {
      await onTrackRecord(candidate);
      trackedIds = new Set([...trackedIds, candidate.provider_id]);
    } catch (error) {
      trackProblem = hostProblemText(error, actionProblemFallback);
    } finally {
      trackingId = "";
    }
  }
  const supportedProviders = $derived(
    (providerCredentials ?? []).filter((provider) =>
      ["google-books", "tmdb"].includes(provider.provider),
    ),
  );
  const selectedProvider = $derived(
    supportedProviders.find(
      (provider) => provider.provider === selectedProviderId,
    ),
  );

  $effect(() => {
    if (supportedProviders.length === 0) return;
    if (
      selectionExplicit &&
      supportedProviders.some(
        (provider) => provider.provider === selectedProviderId,
      )
    ) {
      return;
    }
    selectionExplicit = false;
    selectedProviderId =
      supportedProviders.find((provider) => provider.configured)?.provider ??
      supportedProviders[0].provider;
  });

  $effect(() => {
    const providerId = selectedProviderId;
    if (providerId === searchProviderId) return;
    searchProviderId = providerId;
    searchRevision += 1;
    searching = false;
    results = [];
    problem = "";
    searched = false;
    completedQuery = "";
  });

  function selectProvider(provider: string): void {
    selectionExplicit = true;
    selectedProviderId = provider;
  }
  async function search(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const value = query.trim();
    if (!value || !selectedProvider?.configured || searching) return;
    if (
      /[\u0000-\u001f\u007f]/u.test(value) ||
      new TextEncoder().encode(value).byteLength > 256
    ) {
      problem = "Use 1 to 256 UTF-8 bytes and no control characters.";
      results = [];
      searched = false;
      return;
    }
    const provider = selectedProvider;
    const revision = ++searchRevision;
    searching = true;
    problem = "";
    searched = false;
    try {
      const nextResults = await onSearch(provider.provider, value);
      if (revision !== searchRevision) return;
      results = nextResults;
      completedQuery = value;
      searched = true;
    } catch (error) {
      if (revision !== searchRevision) return;
      results = [];
      problem = hostProblemText(
        error,
        `${provider.label} search failed. Check the provider credential and network policy.`,
      );
    } finally {
      if (revision === searchRevision) searching = false;
    }
  }
</script>

<div class="discover-container" class:embedded>
  {#if !embedded}
    <header class="discover-header">
      <div class="heading-row">
        <IconCompass size={28} class="discover-icon" />
        <h1 id="discover-title" class="view-title" tabindex="-1">Discover</h1>
      </div>
      <p class="view-subtitle">
        Search configured metadata providers through the trusted Fasti host.
      </p>
    </header>
  {/if}

  {#if loading}
    <p role="status">Loading provider status…</p>
  {:else if hostProblem && providerCredentials === undefined}
    <div class="unavailable">
      <p class="problem" role="alert">{hostProblem}</p>
      <button
        id="provider-retry"
        type="button"
        class="btn btn-outline-secondary"
        onclick={onRetry}
      >
        Retry host connection
      </button>
    </div>
  {:else if supportedProviders.length === 0 || !selectedProvider}
    <section class="unavailable" aria-labelledby="discover-setup-title">
      <svelte:element this={embedded ? "h4" : "h2"} id="discover-setup-title">
        No search provider is available
      </svelte:element>
      <p>The active host did not return a supported provider status.</p>
      <button type="button" class="btn btn-outline-secondary" onclick={onRetry}
        >Refresh provider status</button
      >
    </section>
  {:else}
    <div class="provider-choice">
      <label for="provider-choice">Metadata provider</label>
      <select
        id="provider-choice"
        class="form-select"
        value={selectedProviderId}
        onchange={(event) => selectProvider(event.currentTarget.value)}
      >
        {#each supportedProviders as provider (provider.provider)}
          <option value={provider.provider}>
            {provider.label}{provider.configured ? "" : " — setup required"}
          </option>
        {/each}
      </select>
    </div>

    {#if !selectedProvider.configured}
      <section class="unavailable" aria-labelledby="discover-setup-title">
        <svelte:element this={embedded ? "h4" : "h2"} id="discover-setup-title">
          {selectedProvider.label} needs a credential
        </svelte:element>
        <p>
          Add one in Settings. Fasti stores it in the platform credential store.
        </p>
        <button type="button" class="btn btn-primary" onclick={onOpenSettings}
          >Open provider settings</button
        >
      </section>
    {:else}
      <form class="search-form" onsubmit={search} role="search">
        <label for="provider-search">Search {selectedProvider.label}</label>
        <div class="search-row">
          <input
            id="provider-search"
            type="search"
            class="form-control"
            required
            maxlength="256"
            bind:value={query}
            disabled={searching}
            placeholder={selectedProvider.provider === "google-books"
              ? "Title, author, or ISBN"
              : "Movie or series title"}
            autocomplete="off"
          />
          <button
            type="submit"
            class="btn btn-primary"
            disabled={searching || !query.trim()}
          >
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
        <svelte:element this={embedded ? "h4" : "h2"} id="search-results-title">
          Search results
        </svelte:element>
        {#if searching}
          <p role="status">Searching {selectedProvider.label}…</p>
        {:else if problem}
          <p class="problem" role="alert">{problem}</p>
        {:else if searched && results.length === 0}
          <p role="status">No compatible titles found for {completedQuery}.</p>
        {:else if results.length > 0}
          <p role="status">
            {results.length}
            {results.length === 1 ? "result" : "results"} for
            {completedQuery}.
          </p>
          {#if !onTrackRecord}
            <p id="tracking-unavailable" class="result-action-note">
              Search does not change your library. Adding a result is
              unavailable until the host can save the record and identifier
              together.
            </p>
          {/if}
          <ol>
            {#each results as result (result.provider_id)}
              <li>
                <svelte:element
                  this={embedded ? "h5" : "h3"}
                  class="result-title"
                >
                  {result.title}
                </svelte:element>
                {#if result.original_title}
                  <p>Original title: {result.original_title}</p>
                {/if}
                {#if result.authors.length > 0}
                  <p>By {result.authors.join(", ")}</p>
                {/if}
                {#if result.overview}
                  <p class="result-overview">{result.overview}</p>
                {/if}
                <dl>
                  <div>
                    <dt>Provider</dt>
                    <dd>{result.provider}</dd>
                  </div>
                  <div>
                    <dt>Type</dt>
                    <dd>{result.kind}</dd>
                  </div>
                  {#if result.release_year}
                    <div>
                      <dt>Year</dt>
                      <dd>{result.release_year}</dd>
                    </div>
                  {/if}
                  <div>
                    <dt>Provider ID</dt>
                    <dd><code>{result.provider_id}</code></dd>
                  </div>
                </dl>
                {#if onTrackRecord}
                  <button
                    type="button"
                    class="track-btn"
                    disabled={Boolean(trackingId) ||
                      trackedIds.has(result.provider_id)}
                    onclick={() => trackRecord(result)}
                  >
                    {#if trackedIds.has(result.provider_id)}
                      {completedLabel}
                    {:else if trackingId === result.provider_id}
                      Adding…
                    {:else}
                      {actionLabel}
                    {/if}
                  </button>
                {:else}
                  <button
                    type="button"
                    class="track-btn"
                    aria-describedby="tracking-unavailable"
                    disabled>Tracking unavailable</button
                  >
                {/if}
              </li>
            {/each}
          </ol>
          {#if trackProblem}
            <p class="problem" role="alert">{trackProblem}</p>
          {/if}
        {:else}
          <p>Enter a title or provider identifier.</p>
        {/if}
      </section>
    {/if}
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
  .discover-container.embedded {
    max-width: none;
    margin: 0;
    padding: 0;
    gap: 16px;
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
  .result-action-note,
  li p {
    margin: 0;
    color: var(--fasti-text-muted);
  }

  .unavailable,
  .provider-choice,
  .search-form,
  .results {
    padding: 24px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-surface-paper);
  }

  .unavailable :is(h2, h4),
  .results :is(h2, h4) {
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

  input,
  select {
    flex: 1;
    min-width: 0;
    padding: 10px 12px;
  }

  button,
  input,
  select {
    min-height: 44px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 35%, transparent);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
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
    color: var(--fasti-action-contrast);
    font-weight: 700;
    cursor: pointer;
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.68;
  }

  :is(button, input, select):focus-visible {
    outline: 3px solid var(--fasti-focus);
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

  .result-title {
    margin: 0 0 6px;
  }

  .result-overview {
    margin-top: 8px;
    max-width: 72ch;
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

  .track-btn {
    margin-top: 12px;
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
