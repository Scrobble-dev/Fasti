<script lang="ts">
  import type {
    MediaRecord,
    MediaKind,
    OutboundAccessPolicy,
    ProviderCandidate,
  } from "./types.js";
  import {
    IconCompass,
    IconSearch,
    IconStarFilled,
    IconFlame,
    IconAward,
  } from "@tabler/icons-svelte";

  interface Props {
    trendingRecords: MediaRecord[];
    providerPolicy: OutboundAccessPolicy;
    onSearchProvider: (
      provider: string,
      query: string,
      policy: OutboundAccessPolicy,
    ) => Promise<ProviderCandidate[]>;
  }

  let { trendingRecords, providerPolicy, onSearchProvider }: Props = $props();

  let selectedCategory: MediaKind | "all" = $state("all");
  let searchQuery = $state("");
  let providerResults = $state<ProviderCandidate[]>([]);
  let providerSearchState = $state<
    "idle" | "loading" | "results" | "empty" | "error"
  >("idle");
  let providerSearchMessage = $state("");

  const categories: Array<{ id: MediaKind | "all"; label: string }> = [
    { id: "all", label: "All Media" },
    { id: "show", label: "TV Shows" },
    { id: "movie", label: "Movies" },
    { id: "anime", label: "Anime" },
    { id: "manga", label: "Manga" },
    { id: "game", label: "Games" },
    { id: "book", label: "Books" },
    { id: "comic", label: "Comics" },
    { id: "music", label: "Music" },
    { id: "podcast", label: "Podcasts" },
  ];

  const filteredTrending = $derived(
    trendingRecords.filter((rec) => {
      const matchCat =
        selectedCategory === "all" || rec.mediaKind === selectedCategory;
      return matchCat;
    }),
  );

  async function searchProvider(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const query = searchQuery.trim();
    if (!query || providerSearchState === "loading") return;
    providerResults = [];
    providerSearchMessage = "";
    providerSearchState = "loading";
    try {
      providerResults = await onSearchProvider(
        "google-books",
        query,
        providerPolicy,
      );
      providerSearchState = providerResults.length > 0 ? "results" : "empty";
      providerSearchMessage =
        providerResults.length > 0
          ? `${providerResults.length} Google Books results.`
          : "Google Books returned no matching titles.";
    } catch (error) {
      const value =
        error !== null && typeof error === "object"
          ? (error as { title?: string; detail?: string; next_action?: string })
          : {};
      providerSearchState = "error";
      providerSearchMessage =
        [value.title, value.detail, value.next_action]
          .filter(Boolean)
          .join(" ") || "Fasti could not search Google Books.";
    }
  }
</script>

<div class="discover-container">
  <header class="discover-header">
    <div>
      <div class="heading-row">
        <IconCompass size={28} class="discover-icon" />
        <h1 class="view-title">Discover</h1>
      </div>
      <p class="view-subtitle">
        Search Google Books. Provider results stay separate from local records.
      </p>
    </div>
  </header>

  <div class="search-section">
    <form class="search-bar-wrap" onsubmit={searchProvider}>
      <IconSearch size={18} class="search-icon" />
      <input
        type="search"
        placeholder="Search Google Books"
        bind:value={searchQuery}
        class="online-search-input"
        aria-label="Search Google Books"
        maxlength="256"
      />
      <button
        type="submit"
        class="provider-search-btn"
        disabled={!searchQuery.trim() || providerSearchState === "loading"}
        >{providerSearchState === "loading" ? "Searching…" : "Search"}</button
      >
    </form>

    {#if providerSearchState !== "idle"}
      <p
        class="provider-search-status"
        class:error={providerSearchState === "error"}
        role={providerSearchState === "error" ? "alert" : "status"}
        aria-live="polite"
      >
        {providerSearchState === "loading"
          ? "Searching Google Books…"
          : providerSearchMessage}
      </p>
    {/if}

    {#if providerResults.length > 0}
      <ul class="provider-results" aria-label="Google Books results">
        {#each providerResults as result (`${result.provider}:${result.provider_id}`)}
          <li>
            <div>
              <h2>{result.title}</h2>
              {#if result.description}<p>{result.description}</p>{/if}
            </div>
            <span>{result.kind}</span>
          </li>
        {/each}
      </ul>
    {/if}

    <h2 class="local-heading">Demonstration library</h2>
    <p class="section-tagline">
      These local sample records demonstrate the interface. They are not live
      recommendations.
    </p>
    <div
      class="categories-bar"
      role="radiogroup"
      aria-label="Filter demonstration records by media kind"
    >
      {#each categories as cat}
        <button
          type="button"
          role="radio"
          aria-checked={selectedCategory === cat.id}
          class="cat-pill"
          class:active={selectedCategory === cat.id}
          onclick={() => (selectedCategory = cat.id)}
        >
          {cat.label}
        </button>
      {/each}
    </div>
  </div>

  <section class="feed-section" aria-label="Demonstration media">
    <div class="section-header-row">
      <div class="title-with-icon">
        <IconFlame size={20} class="trend-flame-icon" />
        <h2 class="section-title">Sample records</h2>
      </div>
      <span class="section-tagline">Local demonstration data</span>
    </div>

    <div class="media-carousel">
      {#each filteredTrending as item (item.id)}
        <div class="discover-card" role="group" aria-label="{item.title} card">
          <div class="card-art-btn">
            <div class="poster-frame">
              {#if item.posterUrl}
                <img
                  src={item.posterUrl}
                  alt=""
                  class="poster-img"
                  loading="lazy"
                />
              {:else}
                <div class="poster-fallback">{item.mediaKind}</div>
              {/if}

              <span class="kind-chip {item.mediaKind}">{item.mediaKind}</span>

              {#if item.communityRating}
                <div class="rating-chip">
                  <IconStarFilled size={12} class="star-icon" />
                  <span>{item.communityRating.score}</span>
                </div>
              {/if}
            </div>
          </div>

          <div class="card-details">
            <h3 class="item-title">{item.title}</h3>
            <div class="item-meta-row">
              <span class="item-year">{item.releaseYear ?? "—"}</span>
              <span class="item-format">{item.format ?? item.mediaKind}</span>
            </div>
          </div>
        </div>
      {/each}
    </div>
  </section>

  <section class="feed-section" aria-label="More demonstration media">
    <div class="section-header-row">
      <div class="title-with-icon">
        <IconAward size={20} class="award-icon" />
        <h2 class="section-title">More sample records</h2>
      </div>
      <span class="section-tagline">Local demonstration data</span>
    </div>

    <div class="media-carousel">
      {#each filteredTrending.slice().reverse() as item (item.id + "_great")}
        <div class="discover-card" role="group" aria-label="{item.title} card">
          <div class="card-art-btn">
            <div class="poster-frame">
              {#if item.posterUrl}
                <img
                  src={item.posterUrl}
                  alt=""
                  class="poster-img"
                  loading="lazy"
                />
              {:else}
                <div class="poster-fallback">{item.mediaKind}</div>
              {/if}
              <span class="kind-chip {item.mediaKind}">{item.mediaKind}</span>
              {#if item.communityRating}
                <div class="rating-chip">
                  <IconStarFilled size={12} class="star-icon" />
                  <span>{item.communityRating.score}</span>
                </div>
              {/if}
            </div>
          </div>

          <div class="card-details">
            <h3 class="item-title">{item.title}</h3>
            <div class="item-meta-row">
              <span class="item-year">{item.releaseYear ?? "—"}</span>
              <span class="item-format">{item.format ?? item.mediaKind}</span>
            </div>
          </div>
        </div>
      {/each}
    </div>
  </section>
</div>

<style>
  .discover-container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .discover-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
    padding-bottom: 16px;
  }

  .heading-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  :global(.discover-icon) {
    color: var(--fasti-brand-mark);
  }
  .view-title {
    font-family: var(--fasti-font-display);
    font-size: 2.4rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
  }
  .view-subtitle {
    margin: 4px 0 0;
    color: var(--fasti-text-muted);
    font-size: 0.95rem;
  }

  .refresh-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
  }

  .search-section {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .search-bar-wrap {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  :global(.search-icon) {
    position: absolute;
    left: 16px;
    color: var(--fasti-text-muted);
  }
  .online-search-input {
    flex: 1;
    min-width: 0;
    height: 48px;
    padding: 12px 16px 12px 46px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-brand-mark) 40%, transparent);
    border-radius: 6px;
    font-size: 1rem;
    color: var(--fasti-text-primary);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.03);
  }
  .online-search-input:focus {
    outline: 2px solid var(--fasti-action-primary);
  }
  .provider-search-btn {
    min-height: 48px;
    padding: 0 20px;
    border: 1px solid var(--fasti-action-primary);
    border-radius: 4px;
    background: var(--fasti-action-primary);
    color: white;
    font-weight: 700;
  }
  .provider-search-btn:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }
  .provider-search-status {
    margin: 0;
    padding: 10px 12px;
    border: 1px solid var(--fasti-state-verified);
    background: var(--fasti-surface-paper);
  }
  .provider-search-status.error {
    border-color: var(--fasti-brand-mark);
  }
  .provider-results {
    display: grid;
    gap: 1px;
    margin: 0;
    padding: 0;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    background: color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    list-style: none;
  }
  .provider-results li {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    padding: 14px;
    background: var(--fasti-surface-paper);
  }
  .provider-results h2,
  .provider-results p {
    margin: 0;
  }
  .provider-results h2 {
    font-size: 1rem;
  }
  .provider-results p,
  .provider-results span {
    margin-top: 4px;
    color: var(--fasti-text-muted);
    font-size: 0.85rem;
  }
  .provider-results span {
    font-family: var(--fasti-font-mono);
    text-transform: uppercase;
  }
  .local-heading {
    margin: 14px 0 -10px;
    font-family: var(--fasti-font-display);
    font-size: 1.45rem;
  }

  .categories-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .cat-pill {
    padding: 6px 14px;
    border-radius: 20px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    background: var(--fasti-surface-paper);
    font-size: 0.82rem;
    font-weight: 500;
    color: var(--fasti-text-muted);
    cursor: pointer;
    transition: all 120ms ease;
  }
  .cat-pill:hover {
    color: var(--fasti-text-primary);
    border-color: var(--fasti-text-primary);
  }
  .cat-pill.active {
    background: var(--fasti-brand-mark);
    border-color: var(--fasti-brand-mark);
    color: white;
    font-weight: 600;
  }

  .feed-section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .section-header-row {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title-with-icon {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  :global(.trend-flame-icon) {
    color: #e11d48;
  }
  :global(.award-icon) {
    color: var(--fasti-brand-gold);
  }
  .section-title {
    font-family: var(--fasti-font-display);
    font-size: 1.45rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
  }
  .section-tagline {
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
  }

  .media-carousel {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 18px;
  }
  .discover-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .card-art-btn {
    background: transparent;
    border: none;
    padding: 0;
    cursor: default;
    text-align: left;
    display: block;
  }
  .poster-frame {
    position: relative;
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: 6px;
    overflow: hidden;
    background: var(--fasti-surface-archive);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
    transition: transform 120ms ease;
  }
  .poster-frame:hover {
    transform: translateY(-4px);
  }
  .poster-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .poster-fallback {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    font-family: var(--fasti-font-mono);
    text-transform: uppercase;
    color: var(--fasti-text-muted);
  }
  .kind-chip {
    position: absolute;
    top: 8px;
    left: 8px;
    font-family: var(--fasti-font-mono);
    font-size: 0.65rem;
    font-weight: 700;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.75);
    color: white;
  }
  .rating-chip {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    align-items: center;
    gap: 3px;
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.8);
    color: var(--fasti-brand-gold);
  }

  .item-title {
    font-family: var(--fasti-font-display);
    font-size: 1.05rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
    line-height: 1.2;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .item-meta-row {
    display: flex;
    justify-content: space-between;
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    color: var(--fasti-text-muted);
    margin-top: 2px;
  }
</style>
