<script lang="ts">
  import type {
    CreateRecordResult,
    LocalSearchCursorDto,
    LocalSearchResponseDto,
    ProviderCredentialStatus,
    ProviderSearchCandidate,
    SearchCandidateDto,
    SearchCandidateDetailsResponse,
    SearchCandidateReceiptDto,
    SearchProviderPageResponse,
  } from "./types.js";
  import { routeSlug, type SearchCandidateRoute } from "./route-slug.js";
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
    onSearchProviderPage?: (
      provider: string,
      query: string,
      page: number,
      offline: boolean,
    ) => Promise<SearchProviderPageResponse>;
    onOpenSettings: () => void;
    onRetry: () => void;
    onOpenRecord?: (recordId: string) => void;
    onCandidateAction?: (
      candidate: ProviderSearchCandidate,
    ) => Promise<CreateRecordResult | void>;
    onCandidateReceiptAction?: (
      receipt: SearchCandidateReceiptDto,
      evidenceMode: "cached" | "refetch",
    ) => Promise<CreateRecordResult | void>;
    onReadCandidate?: (
      receipt: SearchCandidateReceiptDto,
      offline: boolean,
    ) => Promise<SearchCandidateDetailsResponse>;
    embedded?: boolean;
    actionLabel?: string;
    pendingLabel?: string;
    completedLabel?: string;
    actionProblemFallback?: string;
    actionUnavailableLabel?: string;
    actionUnavailableText?: string;
    selectedProviderId?: string;
    selectionExplicit?: boolean;
    candidateRoute?: SearchCandidateRoute;
    candidateRouteProblem?: string;
    onOpenCandidate?: (receipt: SearchCandidateReceiptDto) => void;
    onCloseCandidateRoute?: () => void;
    onReadCandidateRoute?: (
      route: SearchCandidateRoute,
      offline: boolean,
    ) => Promise<SearchCandidateDetailsResponse>;
  }

  let {
    providerCredentials,
    loading = false,
    hostProblem,
    onSearch,
    onSearchLocal,
    onSearchProviderPage,
    onOpenSettings,
    onRetry,
    onOpenRecord,
    onCandidateAction,
    onCandidateReceiptAction,
    onReadCandidate,
    embedded = false,
    actionLabel = "Create Record",
    pendingLabel = "Creating…",
    completedLabel = "Record ready",
    actionProblemFallback = "Fasti could not create this Record.",
    actionUnavailableLabel = "Record creation unavailable",
    actionUnavailableText = "Search does not create a Record. Record creation is unavailable until the host can save the Record, identifier, and metadata together.",
    selectedProviderId = $bindable(""),
    selectionExplicit = $bindable(false),
    candidateRoute,
    candidateRouteProblem,
    onOpenCandidate,
    onCloseCandidateRoute,
    onReadCandidateRoute,
  }: Props = $props();
  let query = $state("");
  interface ProviderResult {
    candidate: ProviderSearchCandidate;
    receipt?: SearchCandidateReceiptDto;
    cacheState?: "observed" | "fresh" | "stale_on_error";
  }
  interface ProviderPage {
    providerId: string;
    results: ProviderResult[];
    nextPage?: number;
    cacheState?: "observed" | "fresh" | "stale_on_error";
  }
  interface ProviderPages {
    results: ProviderResult[];
    nextPages: Record<string, number>;
    cacheState?: ProviderPage["cacheState"];
    problem?: string;
  }
  interface GroupedProviderResult {
    result: ProviderResult;
    index: number;
    groupIndex: number;
    groupPosition: number;
    groupSize: number;
    providers: string[];
  }
  let results: ProviderResult[] = $state([]);
  let providerNextPages = $state<Record<string, number>>({});
  let providerCacheState: ProviderPage["cacheState"] = $state();
  let cachedOnly = $state(false);
  let detailKey = $state("");
  let candidateDetails = $state<Record<string, ProviderSearchCandidate>>({});
  let candidateDetailProblems = $state<Record<string, string>>({});
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
  let routeCandidate = $state<ProviderSearchCandidate>();
  let routeReceipt = $state<SearchCandidateReceiptDto>();
  let routeLoading = $state(false);
  let routeProblem = $state("");
  let routeGeneration = 0;
  const ALL_PROVIDERS = "all";

  function canonicalCandidatePath(
    providerId: string,
    grain: string,
    receiptId: string,
    title: string,
  ): string {
    return `/explore/${encodeURIComponent(providerId)}/${encodeURIComponent(grain)}/${receiptId}/${routeSlug(title)}`;
  }

  function routeSnapshot(
    response: SearchCandidateDetailsResponse,
  ): SearchCandidateReceiptDto | undefined {
    return response.outcome === "snapshot" ||
      response.outcome === "refetched" ||
      response.outcome === "unavailable"
      ? response.snapshot.receipt
      : undefined;
  }

  function routeDetails(
    response: SearchCandidateDetailsResponse,
  ): ProviderSearchCandidate | undefined {
    if (
      response.outcome === "refetched" ||
      response.outcome === "refetched_without_snapshot"
    )
      return providerCandidate(response.details);
    const receipt = routeSnapshot(response);
    return receipt ? providerCandidate(receipt.candidate) : undefined;
  }

  async function loadCandidateRoute(
    route: SearchCandidateRoute,
    generation: number,
    offline: boolean,
  ): Promise<void> {
    if (!onReadCandidateRoute) {
      routeProblem = "Sign in to read this provider candidate.";
      return;
    }
    routeLoading = true;
    routeProblem = "";
    try {
      const response = await onReadCandidateRoute(route, offline);
      if (generation !== routeGeneration) return;
      routeCandidate = routeDetails(response);
      routeReceipt = routeSnapshot(response);
      if (response.outcome === "missing") {
        routeProblem =
          "This candidate is no longer available. Start a new Search.";
      } else if (
        response.outcome === "unavailable" ||
        response.outcome === "unavailable_without_snapshot"
      ) {
        routeProblem = `Provider details are unavailable (${response.problem_code}).`;
      }
      if (routeCandidate) {
        const receiptId =
          routeReceipt?.candidate_receipt_id ?? route.candidateReceiptId;
        const providerId = routeReceipt?.candidate.provider ?? route.providerId;
        const grain = routeReceipt?.grain ?? route.grain;
        const path = canonicalCandidatePath(
          providerId,
          grain,
          receiptId,
          routeCandidate.title,
        );
        if (window.location.pathname !== path)
          window.history.replaceState(window.history.state, "", path);
      }
    } catch (error) {
      if (generation !== routeGeneration) return;
      routeProblem = hostProblemText(
        error,
        "Fasti could not read the candidate details.",
      );
    } finally {
      if (generation === routeGeneration) routeLoading = false;
    }
  }

  $effect(() => {
    const route = candidateRoute;
    const offline = providerOffline();
    const generation = ++routeGeneration;
    routeCandidate = undefined;
    routeReceipt = undefined;
    routeLoading = false;
    routeProblem = candidateRouteProblem ?? "";
    if (route && !candidateRouteProblem)
      void loadCandidateRoute(route, generation, offline);
    return () => {
      routeGeneration += 1;
    };
  });

  function candidateKey(result: ProviderResult, index: number): string {
    return (
      result.receipt?.candidate_receipt_id ??
      `${result.candidate.provider}:${result.candidate.kind}:${result.candidate.provider_id}:${index}`
    );
  }

  async function runCandidateAction(
    result: ProviderResult,
    index: number,
  ): Promise<void> {
    const key = candidateKey(result, index);
    const action = result.receipt
      ? onCandidateReceiptAction
        ? () =>
            onCandidateReceiptAction(
              result.receipt!,
              providerOffline() ? "cached" : "refetch",
            )
        : undefined
      : onCandidateAction
        ? () => onCandidateAction(result.candidate)
        : undefined;
    if (
      !action ||
      searching ||
      actionKey ||
      detailKey ||
      completedKeys.has(key) ||
      (result.cacheState === "stale_on_error" && providerOffline())
    )
      return;
    const revision = searchRevision;
    actionKey = key;
    actionProblem = "";
    actionProblemKey = "";
    try {
      const outcome = await action();
      if (revision !== searchRevision) return;
      completedKeys = new Set([...completedKeys, key]);
      if (outcome) {
        createdRecordIds = { ...createdRecordIds, [key]: outcome.record_id };
      }
    } catch (error) {
      if (revision !== searchRevision) return;
      actionProblem = hostProblemText(error, actionProblemFallback);
      actionProblemKey = key;
    } finally {
      if (revision === searchRevision && actionKey === key) actionKey = "";
    }
  }

  async function runRoutedCandidateAction(): Promise<void> {
    const candidate = routeCandidate;
    const receipt = routeReceipt;
    if (!candidate || !receipt) return;
    await runCandidateAction({ candidate, receipt }, 0);
    const recordId = createdRecordIds[receipt.candidate_receipt_id];
    if (recordId) onOpenRecord?.(recordId);
  }

  async function readCandidateDetails(
    result: ProviderResult,
    index: number,
  ): Promise<void> {
    if (
      !result.receipt ||
      !onReadCandidate ||
      searching ||
      actionKey ||
      detailKey
    )
      return;
    const key = candidateKey(result, index);
    const revision = searchRevision;
    detailKey = key;
    candidateDetailProblems = { ...candidateDetailProblems, [key]: "" };
    try {
      const response = await onReadCandidate(result.receipt, providerOffline());
      if (revision !== searchRevision) return;
      const details =
        response.outcome === "refetched" ||
        response.outcome === "refetched_without_snapshot"
          ? response.details
          : response.outcome === "snapshot" ||
              response.outcome === "unavailable"
            ? response.snapshot.receipt.candidate
            : undefined;
      if (details) {
        candidateDetails = {
          ...candidateDetails,
          [key]: providerCandidate(details),
        };
      }
      if (
        response.outcome === "missing" ||
        response.outcome === "unavailable" ||
        response.outcome === "unavailable_without_snapshot"
      ) {
        candidateDetailProblems = {
          ...candidateDetailProblems,
          [key]:
            response.outcome === "missing"
              ? "Candidate details are no longer available."
              : `Provider details are unavailable (${response.problem_code}).`,
        };
      }
    } catch (error) {
      if (revision !== searchRevision) return;
      candidateDetailProblems = {
        ...candidateDetailProblems,
        [key]: hostProblemText(
          error,
          "Fasti could not read the candidate details.",
        ),
      };
    } finally {
      if (revision === searchRevision && detailKey === key) detailKey = "";
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
  const selectedProviders = $derived(
    selectedProviderId === ALL_PROVIDERS
      ? supportedProviders.filter(providerAvailable)
      : selectedProvider && providerAvailable(selectedProvider)
        ? [selectedProvider]
        : [],
  );
  const searchAvailable = $derived(
    Boolean(onSearchLocal) || selectedProviders.length > 0,
  );
  const groupedProviderResults = $derived.by(() => {
    const groups = new Map<
      string,
      Array<{ result: ProviderResult; index: number }>
    >();
    results.forEach((result, index) => {
      const title = result.candidate.title
        .normalize("NFKC")
        .toLowerCase()
        .replace(/[^\p{L}\p{N}]+/gu, " ")
        .trim();
      const grain = result.receipt?.grain ?? result.candidate.kind;
      const key =
        title && result.candidate.release_year
          ? `${grain}\u001f${result.candidate.release_year}\u001f${title}`
          : `unique\u001f${candidateKey(result, index)}`;
      const group = groups.get(key) ?? [];
      group.push({ result, index });
      groups.set(key, group);
    });
    return Array.from(groups.values()).flatMap((group, groupIndex) => {
      const providers = Array.from(
        new Set(group.map(({ result }) => result.candidate.provider)),
      );
      return group.map(
        ({ result, index }, groupPosition): GroupedProviderResult => ({
          result,
          index,
          groupIndex,
          groupPosition,
          groupSize: group.length,
          providers,
        }),
      );
    });
  });

  $effect(() => {
    if (supportedProviders.length === 0) return;
    if (
      selectionExplicit &&
      (selectedProviderId === ALL_PROVIDERS ||
        supportedProviders.some(
          (provider) => provider.provider === selectedProviderId,
        ))
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
    providerNextPages = {};
    providerCacheState = undefined;
    localResults = [];
    localNext = undefined;
    localProblem = "";
    problem = "";
    actionProblem = "";
    actionProblemKey = "";
    detailKey = "";
    candidateDetails = {};
    candidateDetailProblems = {};
    searched = false;
    completedQuery = "";
  });

  function selectProvider(provider: string): void {
    selectionExplicit = true;
    selectedProviderId = provider;
  }

  function providerCandidate(
    candidate: SearchCandidateDto,
  ): ProviderSearchCandidate {
    return {
      ...candidate,
      authors: [...candidate.authors],
      image_url: candidate.image_url ?? null,
      original_title: candidate.original_title ?? undefined,
      overview: candidate.overview ?? undefined,
      release_year: candidate.release_year ?? undefined,
    };
  }

  function providerOffline(): boolean {
    return (
      cachedOnly ||
      (typeof navigator !== "undefined" && navigator.onLine === false)
    );
  }

  async function searchProviderResults(
    provider: ProviderCredentialStatus,
    value: string,
    page: number,
  ): Promise<ProviderPage> {
    if (!onSearchProviderPage) {
      return {
        providerId: provider.provider,
        results: (await onSearch(provider.provider, value)).map(
          (candidate) => ({
            candidate,
          }),
        ),
      };
    }
    const response = await onSearchProviderPage(
      provider.provider,
      value,
      page,
      providerOffline(),
    );
    if (response.outcome === "unavailable") {
      throw new Error(
        `${provider.label} is unavailable (${response.problem_code}).`,
      );
    }
    if (response.outcome === "live") {
      return {
        providerId: provider.provider,
        results: response.candidates.map((candidate) => ({
          candidate: providerCandidate(candidate),
        })),
        nextPage: response.next_page ?? undefined,
      };
    }
    return {
      providerId: provider.provider,
      results: response.candidates.map((receipt) => ({
        candidate: providerCandidate(receipt.candidate),
        receipt,
        cacheState: response.cache_state,
      })),
      nextPage: response.next_page ?? undefined,
      cacheState: response.cache_state,
    };
  }

  async function searchProviders(
    providers: ProviderCredentialStatus[],
    value: string,
    pages: Record<string, number>,
  ): Promise<ProviderPages> {
    const settled = await Promise.allSettled(
      providers.map((provider) =>
        searchProviderResults(provider, value, pages[provider.provider] ?? 1),
      ),
    );
    const completed = settled.flatMap((outcome) =>
      outcome.status === "fulfilled" ? [outcome.value] : [],
    );
    const problems = settled.flatMap((outcome, index) =>
      outcome.status === "rejected"
        ? [
            hostProblemText(
              outcome.reason,
              `${providers[index].label} Search is unavailable.`,
            ),
          ]
        : [],
    );
    const nextPages = Object.fromEntries(
      settled.flatMap((outcome, index) =>
        outcome.status === "rejected"
          ? [[providers[index].provider, pages[providers[index].provider] ?? 1]]
          : outcome.value.nextPage
            ? [[outcome.value.providerId, outcome.value.nextPage]]
            : [],
      ),
    );
    const states = completed
      .map((page) => page.cacheState)
      .filter((state): state is NonNullable<typeof state> => Boolean(state));
    return {
      results: completed.flatMap((page) => page.results),
      nextPages,
      cacheState:
        states.length === completed.length &&
        states.length > 0 &&
        states.every((state) => state === states[0])
          ? states[0]
          : undefined,
      problem: problems.length > 0 ? problems.join(" ") : undefined,
    };
  }

  async function search(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const value = query.trim();
    if (!value || searching || actionKey || detailKey) return;
    if (
      /[\u0000-\u001f\u007f]/u.test(value) ||
      new TextEncoder().encode(value).byteLength > 256
    ) {
      problem = "Use 1 to 256 UTF-8 bytes and no control characters.";
      results = [];
      providerNextPages = {};
      providerCacheState = undefined;
      localResults = [];
      localNext = undefined;
      searched = false;
      return;
    }
    const providers = [...selectedProviders];
    const revision = ++searchRevision;
    searching = true;
    problem = "";
    localProblem = "";
    actionProblem = "";
    actionProblemKey = "";
    detailKey = "";
    candidateDetails = {};
    candidateDetailProblems = {};
    searched = false;
    try {
      const [localOutcome, providerOutcome] = await Promise.allSettled([
        onSearchLocal?.(value),
        providers.length > 0
          ? searchProviders(providers, value, {})
          : undefined,
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
        results = providerOutcome.value?.results ?? [];
        providerNextPages = providerOutcome.value?.nextPages ?? {};
        providerCacheState = providerOutcome.value?.cacheState;
        problem = providerOutcome.value?.problem ?? "";
      } else {
        results = [];
        providerNextPages = {};
        providerCacheState = undefined;
        problem = hostProblemText(
          providerOutcome.reason,
          "Provider Search failed. Local results are still available.",
        );
      }
      completedQuery = value;
      searched = true;
    } finally {
      if (revision === searchRevision) searching = false;
    }
  }

  async function loadMoreProvider(): Promise<void> {
    const providers = selectedProviders.filter(
      (provider) => providerNextPages[provider.provider],
    );
    if (
      providers.length === 0 ||
      !onSearchProviderPage ||
      searching ||
      actionKey ||
      detailKey
    )
      return;
    const revision = ++searchRevision;
    searching = true;
    problem = "";
    try {
      const page = await searchProviders(
        providers,
        completedQuery,
        providerNextPages,
      );
      if (revision !== searchRevision) return;
      results = [...results, ...page.results];
      providerNextPages = page.nextPages;
      providerCacheState = page.cacheState;
      problem = page.problem ?? "";
    } catch (error) {
      if (revision !== searchRevision) return;
      problem = hostProblemText(
        error,
        "Fasti could not load the next provider Search page.",
      );
    } finally {
      if (revision === searchRevision) searching = false;
    }
  }

  async function loadMoreLocal(): Promise<void> {
    if (!onSearchLocal || !localNext || searching || actionKey || detailKey)
      return;
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

<div
  class="discover-container"
  class:embedded
  class:candidate-active={Boolean(candidateRoute || candidateRouteProblem)}
>
  {#if candidateRoute || candidateRouteProblem}
    <section class="candidate-detail" aria-busy={routeLoading}>
      <button
        type="button"
        class="btn btn-outline-secondary"
        onclick={onCloseCandidateRoute}
        disabled={!onCloseCandidateRoute || Boolean(actionKey)}
        >Back to Search</button
      >
      <h1 id="candidate-detail-title" tabindex="-1">
        {routeCandidate?.title ?? "Candidate details"}
      </h1>
      {#if routeLoading}
        <p class="alert alert-info" role="status">Loading candidate…</p>
      {/if}
      {#if routeProblem}
        <p class="problem" role="alert">{routeProblem}</p>
      {/if}
      {#if routeCandidate}
        {#if routeCandidate.original_title}
          <p>Original title: {routeCandidate.original_title}</p>
        {/if}
        {#if routeCandidate.authors.length > 0}
          <p>By {routeCandidate.authors.join(", ")}</p>
        {/if}
        {#if routeCandidate.overview}
          <p class="result-overview">{routeCandidate.overview}</p>
        {/if}
        <dl>
          <div>
            <dt>Provider</dt>
            <dd>{routeCandidate.provider}</dd>
          </div>
          <div>
            <dt>Type</dt>
            <dd>{routeCandidate.kind}</dd>
          </div>
          {#if routeCandidate.release_year}
            <div>
              <dt>Year</dt>
              <dd>{routeCandidate.release_year}</dd>
            </div>
          {/if}
          <div>
            <dt>Provider ID</dt>
            <dd><code>{routeCandidate.provider_id}</code></dd>
          </div>
        </dl>
        {#if routeReceipt && onCandidateReceiptAction}
          <button
            type="button"
            class="btn btn-primary"
            aria-disabled={Boolean(actionKey) || routeLoading}
            onclick={runRoutedCandidateAction}
            >{actionKey ? pendingLabel : actionLabel}</button
          >
        {/if}
        {#if actionProblem}
          <p class="problem" role="alert">{actionProblem}</p>
        {/if}
      {:else if !routeLoading && !routeProblem}
        <p>Candidate details are unavailable.</p>
      {/if}
    </section>
  {/if}
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
    <label for="provider-search">
      Search {selectedProvider?.label ?? "your Library and providers"}
    </label>
    <div class="search-row">
      <input
        id="provider-search"
        type="search"
        class="form-control"
        required
        maxlength="256"
        bind:value={query}
        disabled={searching ||
          Boolean(actionKey) ||
          Boolean(detailKey) ||
          !searchAvailable}
        placeholder="Title or provider identifier"
        autocomplete="off"
      />
      <button
        type="submit"
        class="btn btn-primary"
        disabled={searching ||
          Boolean(actionKey) ||
          Boolean(detailKey) ||
          !searchAvailable ||
          !query.trim()}
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
          disabled={searching || Boolean(actionKey) || Boolean(detailKey)}
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
  {:else if supportedProviders.length === 0}
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
        disabled={Boolean(actionKey) || Boolean(detailKey)}
        onchange={(event) => selectProvider(event.currentTarget.value)}
      >
        <option value={ALL_PROVIDERS}>All available providers</option>
        {#each supportedProviders as provider (provider.provider)}
          <option value={provider.provider}>
            {provider.label}{providerAvailable(provider)
              ? ""
              : " — setup required"}
          </option>
        {/each}
      </select>
      {#if onSearchProviderPage}
        <label class="form-check">
          <input
            class="form-check-input"
            type="checkbox"
            bind:checked={cachedOnly}
            disabled={searching || Boolean(actionKey) || Boolean(detailKey)}
          />
          <span class="form-check-label">Use cached provider results only</span>
        </label>
      {/if}
    </div>

    {#if selectedProviderId !== ALL_PROVIDERS && selectedProvider && !providerAvailable(selectedProvider)}
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
          <p role="status">
            Searching {selectedProvider?.label ?? "configured providers"}…
          </p>
        {:else if problem && results.length === 0}
          <p class="problem" role="alert">{problem}</p>
        {:else if searched && results.length === 0}
          <p role="status">
            No compatible titles found for {completedQuery}.
          </p>
        {:else if results.length > 0}
          {#if problem}
            <p class="problem" role="alert">{problem}</p>
          {/if}
          <p role="status">
            {results.length}
            {results.length === 1 ? "result" : "results"} for
            {completedQuery}.
            {#if providerCacheState === "stale_on_error"}
              The provider is unavailable, so these results use retained cache
              evidence.
            {:else if providerCacheState === "fresh"}
              These results came from fresh cache evidence.
            {:else if providerCacheState === "observed"}
              These results were observed from the provider now.
            {/if}
          </p>
          {#if !onCandidateAction && !onCandidateReceiptAction}
            <p id="candidate-action-unavailable" class="result-action-note">
              {actionUnavailableText}
            </p>
          {/if}
          <ol>
            {#each groupedProviderResults as grouped (candidateKey(grouped.result, grouped.index))}
              {@const { result, index } = grouped}
              {@const candidate = result.candidate}
              {@const resultKey = candidateKey(result, index)}
              {#if grouped.groupSize > 1 && grouped.groupPosition === 0}
                <li
                  id={`candidate-group-${grouped.groupIndex}`}
                  class="duplicate-intro"
                >
                  <strong
                    >Possible match across {grouped.groupSize} results.</strong
                  >
                  Sources: {grouped.providers.join(", ")}. Review each source;
                  Fasti has not merged these candidates.
                </li>
              {/if}
              <li
                class:possible-duplicate={grouped.groupSize > 1}
                aria-describedby={grouped.groupSize > 1
                  ? `candidate-group-${grouped.groupIndex}`
                  : undefined}
              >
                <svelte:element
                  this={embedded ? "h5" : "h3"}
                  class="result-title"
                >
                  {candidate.title}
                </svelte:element>
                {#if candidate.original_title}
                  <p>Original title: {candidate.original_title}</p>
                {/if}
                {#if candidate.authors.length > 0}
                  <p>By {candidate.authors.join(", ")}</p>
                {/if}
                {#if candidate.overview}
                  <p class="result-overview">{candidate.overview}</p>
                {/if}
                <dl>
                  <div>
                    <dt>Provider</dt>
                    <dd>{candidate.provider}</dd>
                  </div>
                  <div>
                    <dt>Type</dt>
                    <dd>{candidate.kind}</dd>
                  </div>
                  {#if candidate.release_year}
                    <div>
                      <dt>Year</dt>
                      <dd>{candidate.release_year}</dd>
                    </div>
                  {/if}
                  <div>
                    <dt>Provider ID</dt>
                    <dd><code>{candidate.provider_id}</code></dd>
                  </div>
                </dl>
                {#if result.receipt && onOpenCandidate}
                  <a
                    class="btn btn-outline-secondary"
                    href={canonicalCandidatePath(
                      result.receipt.candidate.provider,
                      result.receipt.grain,
                      result.receipt.candidate_receipt_id,
                      result.receipt.candidate.title,
                    )}
                    onclick={(event) => {
                      event.preventDefault();
                      onOpenCandidate?.(result.receipt!);
                    }}>View details</a
                  >
                {:else if result.receipt && onReadCandidate}
                  <button
                    type="button"
                    class="btn btn-outline-secondary"
                    disabled={searching ||
                      Boolean(actionKey) ||
                      Boolean(detailKey)}
                    onclick={() => readCandidateDetails(result, index)}
                  >
                    {detailKey === resultKey
                      ? "Loading details…"
                      : "View details"}
                  </button>
                  {#if candidateDetails[resultKey]}
                    <div class="result-details" role="status">
                      {#if candidateDetails[resultKey].overview}
                        <p>{candidateDetails[resultKey].overview}</p>
                      {/if}
                      {#if candidateDetails[resultKey].original_title}
                        <p>
                          Original title: {candidateDetails[resultKey]
                            .original_title}
                        </p>
                      {/if}
                    </div>
                  {/if}
                  {#if candidateDetailProblems[resultKey]}
                    <p class="problem" role="alert">
                      {candidateDetailProblems[resultKey]}
                    </p>
                  {/if}
                {/if}
                {#if (result.receipt && onCandidateReceiptAction) || (!result.receipt && onCandidateAction)}
                  <button
                    type="button"
                    class="track-btn"
                    aria-disabled={searching ||
                      Boolean(actionKey) ||
                      Boolean(detailKey) ||
                      completedKeys.has(resultKey) ||
                      (result.cacheState === "stale_on_error" &&
                        providerOffline())}
                    onclick={() => runCandidateAction(result, index)}
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
                  {#if result.cacheState === "stale_on_error" && providerOffline()}
                    <p class="result-action-note">
                      Reconnect before creating a Record from stale provider
                      evidence.
                    </p>
                  {/if}
                {:else}
                  <button type="button" class="track-btn" disabled
                    >{actionUnavailableLabel}</button
                  >
                {/if}
              </li>
            {/each}
          </ol>
          {#if Object.keys(providerNextPages).length > 0 && onSearchProviderPage}
            <button
              type="button"
              class="btn btn-outline-secondary"
              disabled={searching || Boolean(actionKey) || Boolean(detailKey)}
              onclick={loadMoreProvider}
              >Retry or load more provider results</button
            >
          {/if}
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

  .discover-container.candidate-active > :not(.candidate-detail) {
    display: none;
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

  :is(button, input, select, a):focus-visible {
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

  .duplicate-intro,
  .possible-duplicate {
    border-inline-start: 3px solid var(--fasti-state-attention);
    padding-inline-start: 16px;
  }

  .duplicate-intro {
    background: color-mix(
      in srgb,
      var(--fasti-state-attention) 8%,
      transparent
    );
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
