<script lang="ts">
  import type {
    CreateRecordResult,
    LocalSearchCursorDto,
    LocalSearchResponseDto,
    ProviderCredentialStatus,
    ProviderSearchCandidate,
  } from "./types.js";
  import { hostProblemText } from "./host-problem.js";
  import IconCompass from "@tabler/icons-svelte/icons/compass";
  import IconSearch from "@tabler/icons-svelte/icons/search";

  interface Props {
    providerCredentials?: ProviderCredentialStatus[];
    loading?: boolean;
    hostProblem?: string;
    onSearch: (
      provider: string,
      query: string,
    ) => Promise<ProviderSearchCandidate[]>;
    onSearchLocal?: (
      query: string,
      after?: LocalSearchCursorDto,
    ) => Promise<LocalSearchResponseDto>;
    onOpenSettings: () => void;
    onRetry: () => void;
    onOpenRecord?: (recordId: string) => void;
    onCandidateAction?: (
      candidate: ProviderSearchCandidate,
    ) => Promise<CreateRecordResult | void>;
    embedded?: boolean;
    actionLabel?: string;
    pendingLabel?: string;
    completedLabel?: string;
    actionProblemFallback?: string;
    actionUnavailableLabel?: string;
    actionUnavailableText?: string;
    selectedProviderId?: string;
    selectionExplicit?: boolean;
  }

  let {
    providerCredentials,
    loading = false,
    hostProblem,
    onSearch,
    onSearchLocal,
    onOpenSettings,
    onRetry,
    onOpenRecord,
    onCandidateAction,
    embedded = false,
    actionLabel = "Create Record",
    pendingLabel = "Creating…",
    completedLabel = "Record ready",
    actionProblemFallback = "Fasti could not create this Record.",
    actionUnavailableLabel = "Record creation unavailable",
    actionUnavailableText = "Search does not create a Record. Record creation is unavailable until the host can save the Record, identifier, and metadata together.",
    selectedProviderId = $bindable(""),
    selectionExplicit = $bindable(false),
  }: Props = $props();
  let query = $state("");
  let results: ProviderSearchCandidate[] = $state([]);
  let localResults: LocalSearchResponseDto["records"] = $state([]);
  let localNext: LocalSearchCursorDto | undefined = $state();
  let localProblem = $state("");
  let searching = $state(false);
  let problem = $state("");
  let searched = $state(false);
  let completedQuery = $state("");
  let actionKey = $state("");
  let completedKeys = $state<Set<string>>(new Set());
  let createdRecordIds = $state<Record<string, string>>({});
  let actionProblem = $state("");
  let actionProblemKey = $state("");
  let searchRevision = 0;
  let searchProviderId = "";

  function candidateKey(candidate: ProviderSearchCandidate): string {
    return `${candidate.provider}:${candidate.kind}:${candidate.provider_id}`;
  }

  async function runCandidateAction(
    candidate: ProviderSearchCandidate,
  ): Promise<void> {
    const key = candidateKey(candidate);
    if (!onCandidateAction || actionKey || completedKeys.has(key)) return;
    actionKey = key;
    actionProblem = "";
    actionProblemKey = "";
    try {
      const result = await onCandidateAction(candidate);
      completedKeys = new Set([...completedKeys, key]);
      if (result) {
        createdRecordIds = { ...createdRecordIds, [key]: result.record_id };
      }
    } catch (error) {
      actionProblem = hostProblemText(error, actionProblemFallback);
      actionProblemKey = key;
    } finally {
      actionKey = "";
    }
  }
  const supportedProviders = $derived(
    (providerCredentials ?? []).filter(
      (provider) => provider.capability_id === "metadata.search",
    ),
  );
  function providerAvailable(provider: ProviderCredentialStatus): boolean {
    return (
      ["available", "degraded"].includes(provider.state) &&
      ["not_required", "optional", "stored_unverified", "valid"].includes(
        provider.credential_state,
      )
    );
  }
  const selectedProvider = $derived(
    supportedProviders.find(
      (provider) => provider.provider === selectedProviderId,
    ),
  );
  const searchAvailable = $derived(
    Boolean(onSearchLocal) ||
      Boolean(selectedProvider && providerAvailable(selectedProvider)),
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
      supportedProviders.find(providerAvailable)?.provider ??
      supportedProviders[0].provider;
  });

  $effect(() => {
    const providerId = selectedProviderId;
    if (providerId === searchProviderId) return;
    searchProviderId = providerId;
    searchRevision += 1;
    searching = false;
    results = [];
    localResults = [];
    localNext = undefined;
    localProblem = "";
    problem = "";
    actionProblem = "";
    actionProblemKey = "";
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
    if (!value || searching) return;
    if (
      /[\u0000-\u001f\u007f]/u.test(value) ||
      new TextEncoder().encode(value).byteLength > 256
    ) {
      problem = "Use 1 to 256 UTF-8 bytes and no control characters.";
      results = [];
      localResults = [];
      localNext = undefined;
      searched = false;
      return;
    }
    const provider =
      selectedProvider && providerAvailable(selectedProvider)
        ? selectedProvider
        : undefined;
    const revision = ++searchRevision;
    searching = true;
    problem = "";
    localProblem = "";
    actionProblem = "";
    actionProblemKey = "";
    searched = false;
    try {
      const [localOutcome, providerOutcome] = await Promise.allSettled([
        onSearchLocal?.(value),
        provider ? onSearch(provider.provider, value) : undefined,
      ]);
      if (revision !== searchRevision) return;
      if (localOutcome.status === "fulfilled" && localOutcome.value) {
        localResults = [...localOutcome.value.records];
        localNext = localOutcome.value.next ?? undefined;
      } else {
        localResults = [];
        localNext = undefined;
        if (localOutcome.status === "rejected") {
          localProblem = hostProblemText(
            localOutcome.reason,
            "Fasti could not search the local Library.",
          );
        }
      }
      if (providerOutcome.status === "fulfilled") {
        results = providerOutcome.value ?? [];
      } else {
        results = [];
        problem = hostProblemText(
          providerOutcome.reason,
          `${provider?.label ?? "Provider"} search failed. Local results are still available.`,
        );
      }
      completedQuery = value;
      searched = true;
    } finally {
      if (revision === searchRevision) searching = false;
    }
  }

  async function loadMoreLocal(): Promise<void> {
    if (!onSearchLocal || !localNext || searching) return;
    const revision = ++searchRevision;
    searching = true;
    localProblem = "";
    try {
      const page = await onSearchLocal(completedQuery, localNext);
      if (revision !== searchRevision) return;
      localResults = [...localResults, ...page.records];
      localNext = page.next ?? undefined;
    } catch (error) {
      if (revision !== searchRevision) return;
      localProblem = hostProblemText(
        error,
        "Fasti could not load the next local Search page.",
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
        Search local Records first, then a configured metadata provider.
      </p>
    </header>
  {/if}

  <form class="search-form" onsubmit={search} role="search">
    <label for="provider-search">Search your Library and providers</label>
    <div class="search-row">
      <input
        id="provider-search"
        type="search"
        class="form-control"
        required
        maxlength="256"
        bind:value={query}
        disabled={searching || !searchAvailable}
        placeholder="Title or provider identifier"
        autocomplete="off"
      />
      <button
        type="submit"
        class="btn btn-primary"
        disabled={searching || !searchAvailable || !query.trim()}
      >
        <IconSearch size={18} aria-hidden="true" />
        {searching ? "Searching…" : "Search"}
      </button>
    </div>
  </form>

  <section
    class="results"
    aria-labelledby="local-search-results-title"
    aria-busy={searching}
  >
    <svelte:element
      this={embedded ? "h4" : "h2"}
      id="local-search-results-title"
    >
      Your Library
    </svelte:element>
    {#if localProblem}
      <p class="problem" role="alert">{localProblem}</p>
    {/if}
    {#if searched && localResults.length === 0 && !localProblem}
      <p role="status">No local Records found for {completedQuery}.</p>
    {:else if localResults.length > 0}
      <p role="status">
        {localResults.length}
        {localResults.length === 1 ? "local Record" : "local Records"} for
        {completedQuery}.
      </p>
      <ol>
        {#each localResults as record (record.record_id)}
          <li>
            <svelte:element this={embedded ? "h5" : "h3"} class="result-title">
              {record.title.value ?? "Untitled Record"}
            </svelte:element>
            <dl>
              <div>
                <dt>Source</dt>
                <dd>Local Library</dd>
              </div>
              <div>
                <dt>Type</dt>
                <dd>{record.grain}</dd>
              </div>
              {#if record.release_year?.value}
                <div>
                  <dt>Year</dt>
                  <dd>{record.release_year.value}</dd>
                </div>
              {/if}
            </dl>
            {#if onOpenRecord}
              <button
                type="button"
                class="track-btn"
                onclick={() => onOpenRecord(record.record_id)}
                >Open Record</button
              >
            {/if}
          </li>
        {/each}
      </ol>
      {#if localNext}
        <button
          type="button"
          class="btn btn-outline-secondary"
          disabled={searching}
          onclick={loadMoreLocal}>Load more local Records</button
        >
      {/if}
    {:else if !searched}
      <p>Local Records remain searchable without a network connection.</p>
    {/if}
  </section>

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
            {provider.label}{providerAvailable(provider)
              ? ""
              : " — setup required"}
          </option>
        {/each}
      </select>
    </div>

    {#if !providerAvailable(selectedProvider)}
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
          {#if !onCandidateAction}
            <p id="candidate-action-unavailable" class="result-action-note">
              {actionUnavailableText}
            </p>
          {/if}
          <ol>
            {#each results as result (candidateKey(result))}
              {@const resultKey = candidateKey(result)}
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
                {#if onCandidateAction}
                  <button
                    type="button"
                    class="track-btn"
                    aria-disabled={Boolean(actionKey) ||
                      completedKeys.has(resultKey)}
                    onclick={() => runCandidateAction(result)}
                  >
                    {#if completedKeys.has(resultKey)}
                      {completedLabel}
                    {:else if actionKey === resultKey}
                      {pendingLabel}
                    {:else}
                      {actionLabel}
                    {/if}
                  </button>
                  {#if createdRecordIds[resultKey]}
                    <p class="result-action-status" role="status">
                      Record ID: <code>{createdRecordIds[resultKey]}</code>
                    </p>
                  {/if}
                  {#if actionProblemKey === resultKey}
                    <p class="problem" role="alert">{actionProblem}</p>
                  {/if}
                {:else}
                  <button
                    type="button"
                    class="track-btn"
                    aria-describedby="candidate-action-unavailable"
                    disabled>{actionUnavailableLabel}</button
                  >
                {/if}
              </li>
            {/each}
          </ol>
        {:else}
          <p>
            Provider results appear here when a configured source is available.
          </p>
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

  button[aria-disabled="true"] {
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

  .result-action-status {
    margin-top: 8px;
  }

  .result-action-status code {
    overflow-wrap: anywhere;
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
