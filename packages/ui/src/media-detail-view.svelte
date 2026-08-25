<script lang="ts">
  import type { MediaRecord, WatchStatus, ExternalId } from "./types.js";
  import {
    IconArrowLeft,
    IconStarFilled,
    IconPlayerPlay,
    IconCheck,
    IconBookmark,
    IconHeart,
    IconCalendar,
    IconRepeat,
    IconExternalLink,
    IconShieldCheck,
    IconNotes,
    IconTags,
    IconListNumbers,
    IconPlus,
    IconX,
    IconEdit,
  } from "@tabler/icons-svelte";

  interface Props {
    record: MediaRecord;
    onBack: () => void;
    onUpdateStatus?: (recordId: string, status: WatchStatus) => void;
    onUpdateRating?: (recordId: string, rating: number) => void;
    onToggleEpisode?: (recordId: string, episodeId: string) => void;
    onUpdateNotes?: (recordId: string, notes: string) => void;
    onAddTag?: (recordId: string, tag: string) => void;
    onRemoveTag?: (recordId: string, tag: string) => void;
  }

  let {
    record,
    onBack,
    onUpdateStatus,
    onUpdateRating,
    onToggleEpisode,
    onUpdateNotes,
    onAddTag,
    onRemoveTag,
  }: Props = $props();

  let activeTab: "seasons" | "cast" | "sources" | "notes" = $state("seasons");
  let selectedSeasonIndex = $state(0);
  let isEditingNotes = $state(false);
  let editedNotesText = $state("");
  let newTagInput = $state("");
  let isFavorite = $state(false);
  let activeDisplaySource = $state("tmdb_tv");

  $effect(() => {
    editedNotesText = record.userNotes ?? "";
    activeDisplaySource = record.displaySource;
  });

  const statusOptions: Array<{ id: WatchStatus; label: string }> = [
    { id: "watching", label: "In Progress / Watching" },
    { id: "completed", label: "Completed" },
    { id: "plan_to_watch", label: "Plan to Watch" },
    { id: "on_hold", label: "On Hold" },
    { id: "dropped", label: "Dropped" },
  ];

  function handleSaveNotes(): void {
    onUpdateNotes?.(record.id, editedNotesText);
    isEditingNotes = false;
  }

  function handleAddTagSubmit(e: Event): void {
    e.preventDefault();
    if (newTagInput.trim().length > 0) {
      onAddTag?.(record.id, newTagInput.trim());
      newTagInput = "";
    }
  }
</script>

<div class="detail-container">
  <!-- Top Navigation Bar -->
  <div class="top-nav-bar">
    <button type="button" class="back-btn" onclick={onBack}>
      <IconArrowLeft size={16} stroke={2} />
      <span>Back to Library</span>
    </button>

    <div class="top-id-badge">
      <span class="id-label">Record ID:</span>
      <code>{record.id}</code>
    </div>
  </div>

  <!-- Main Media Header (Floppy / Yamtrack Editorial Style) -->
  <header class="media-main-header">
    <!-- Left Poster Column -->
    <div class="poster-column">
      <div class="poster-frame">
        {#if record.posterUrl}
          <img
            src={record.posterUrl}
            alt="{record.title} Poster"
            class="main-poster"
          />
        {:else}
          <div class="fallback-poster">{record.mediaKind}</div>
        {/if}
      </div>
    </div>

    <!-- Center/Right Details Column -->
    <div class="header-details-column">
      <div class="title-row">
        <h1 class="main-title">{record.title}</h1>
      </div>

      {#if record.originalTitle}
        <h2 class="original-title">{record.originalTitle}</h2>
      {/if}

      <!-- Progress & Watched Stats Meta Row -->
      <div class="meta-strip">
        {#if record.progressEpisodes !== undefined && record.totalEpisodes !== undefined}
          <span class="meta-item">
            <strong>Progress:</strong>
            {record.progressEpisodes} of {record.totalEpisodes} eps
          </span>
          <span class="bullet">·</span>
        {/if}
        {#if record.airDates}
          <span class="meta-item">{record.airDates}</span>
          <span class="bullet">·</span>
        {/if}
        {#if record.runtimeMinutes}
          <span class="meta-item"
            >{Math.floor(record.runtimeMinutes / 60)}h {record.runtimeMinutes %
              60}m</span
          >
        {/if}
      </div>

      <!-- Ratings & Action Button Row -->
      <div class="action-strip">
        <!-- Community Score Badge -->
        {#if record.communityRating}
          <div class="score-badge community">
            <span class="score-num">{record.communityRating.score}</span>
            <span class="score-meta"
              >{record.communityRating.votes.toLocaleString()}
              {record.communityRating.source} ratings</span
            >
          </div>
        {/if}

        <!-- User Rating Picker -->
        <div class="user-rating-box">
          <span class="user-star">★</span>
          <select
            class="rating-select"
            value={record.userRating ?? 0}
            onchange={(e) =>
              onUpdateRating?.(record.id, Number(e.currentTarget.value))}
            aria-label="User Rating"
          >
            <option value="0">Unrated</option>
            {#each [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] as num}
              <option value={num}>{num} ★ ({num}/10)</option>
            {/each}
          </select>
        </div>

        <!-- Status Selector Dropdown -->
        <div class="status-select-wrap">
          <select
            class="status-select {record.status}"
            value={record.status}
            onchange={(e) =>
              onUpdateStatus?.(record.id, e.currentTarget.value as WatchStatus)}
            aria-label="Watch Status"
          >
            {#each statusOptions as opt}
              <option value={opt.id}>{opt.label}</option>
            {/each}
          </select>
        </div>

        <!-- Quick Action Buttons -->
        <button
          type="button"
          class="icon-action-btn"
          class:active={isFavorite}
          onclick={() => (isFavorite = !isFavorite)}
          title="Favorite / Bookmark"
        >
          <IconHeart size={18} fill={isFavorite ? "currentColor" : "none"} />
        </button>

        <button type="button" class="icon-action-btn" title="Add to List">
          <IconBookmark size={18} />
        </button>

        <button type="button" class="icon-action-btn" title="View Calendar">
          <IconCalendar size={18} />
        </button>

        <button
          type="button"
          class="icon-action-btn"
          title="Log Rewatch Occurrence"
        >
          <IconRepeat size={18} />
        </button>

        <button type="button" class="play-with-btn">
          <IconPlayerPlay size={16} stroke={2.5} /> Play With...
        </button>
      </div>

      <!-- Synopsis -->
      <p class="synopsis-prose">
        {record.overview ?? "No synopsis available for this record."}
      </p>
    </div>
  </header>

  <!-- Two-Column Layout: Left Details Sidebar | Right Tabbed Content -->
  <div class="details-body-grid">
    <!-- Left Metadata Sidebar -->
    <aside class="sidebar-details-card">
      <h3 class="details-sidebar-heading">Details</h3>

      <dl class="sidebar-meta-list">
        {#if record.format}
          <div class="meta-pair">
            <dt>Format</dt>
            <dd>{record.format}</dd>
          </div>
        {/if}

        {#if record.airDates}
          <div class="meta-pair">
            <dt>Air Dates</dt>
            <dd>{record.airDates}</dd>
          </div>
        {/if}

        {#if record.statusText}
          <div class="meta-pair">
            <dt>Status</dt>
            <dd><span class="status-pill">{record.statusText}</span></dd>
          </div>
        {/if}

        {#if record.runtimeMinutes}
          <div class="meta-pair">
            <dt>Runtime</dt>
            <dd>
              {Math.floor(record.runtimeMinutes / 60)}h {record.runtimeMinutes %
                60}m ({record.totalEpisodes ?? 1} total)
            </dd>
          </div>
        {/if}

        {#if record.country}
          <div class="meta-pair">
            <dt>Country</dt>
            <dd>{record.country}</dd>
          </div>
        {/if}

        {#if record.languages && record.languages.length > 0}
          <div class="meta-pair">
            <dt>Languages</dt>
            <dd>{record.languages.join(", ")}</dd>
          </div>
        {/if}
      </dl>

      <!-- Genres -->
      {#if record.genres && record.genres.length > 0}
        <div class="sidebar-section">
          <h4 class="sidebar-subheading">Genres</h4>
          <div class="chips-row">
            {#each record.genres as g}
              <span class="genre-chip">{g}</span>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Studios & Production -->
      {#if record.studios && record.studios.length > 0}
        <div class="sidebar-section">
          <h4 class="sidebar-subheading">Studios & Networks</h4>
          <ul class="studios-list">
            {#each record.studios as st}
              <li>{st}</li>
            {/each}
          </ul>
        </div>
      {/if}

      <!-- External Links -->
      <div class="sidebar-section">
        <h4 class="sidebar-subheading">External Identifiers</h4>
        <div class="xid-links">
          {#each record.externalIds as xid}
            <a
              href={xid.url ?? "#"}
              target="_blank"
              rel="noopener noreferrer"
              class="xid-link"
              title="Open in {xid.namespace}"
            >
              <span class="ns-tag">{xid.namespace}</span>
              <span class="ns-val">{xid.value}</span>
              <IconExternalLink size={12} class="link-icon" />
            </a>
          {/each}
        </div>
      </div>
    </aside>

    <!-- Right Main Tabbed Content Area -->
    <main class="main-content-pane">
      <!-- Section Tabs -->
      <nav class="content-tabs" aria-label="Media section tabs">
        {#if record.seasons && record.seasons.length > 0}
          <button
            type="button"
            class="tab-btn"
            class:active={activeTab === "seasons"}
            onclick={() => (activeTab = "seasons")}
          >
            <IconListNumbers size={16} /> Seasons & Episodes ({record.seasons
              .length})
          </button>
        {/if}

        {#if record.cast && record.cast.length > 0}
          <button
            type="button"
            class="tab-btn"
            class:active={activeTab === "cast"}
            onclick={() => (activeTab = "cast")}
          >
            Cast & Crew ({record.cast.length})
          </button>
        {/if}

        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === "sources"}
          onclick={() => (activeTab = "sources")}
        >
          <IconShieldCheck size={16} /> Sources & Identity
        </button>

        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === "notes"}
          onclick={() => (activeTab = "notes")}
        >
          <IconNotes size={16} /> Notes & Custom Tags
        </button>
      </nav>

      <!-- Tab 1: Seasons & Episodes Accordion -->
      {#if activeTab === "seasons" && record.seasons}
        <section class="tab-pane">
          <!-- Horizontal Season Selector Cards -->
          <div class="seasons-cards-row">
            {#each record.seasons as season, sIdx}
              <button
                type="button"
                class="season-card-btn"
                class:active={selectedSeasonIndex === sIdx}
                onclick={() => (selectedSeasonIndex = sIdx)}
              >
                {#if season.posterUrl}
                  <img src={season.posterUrl} alt="" class="season-thumb" />
                {/if}
                <div class="season-card-info">
                  <span class="season-name">{season.title}</span>
                  <span class="season-count"
                    >{season.episodeCount} Episodes</span
                  >
                </div>
              </button>
            {/each}
          </div>

          <!-- Episode Checklist -->
          <div class="episodes-deck">
            <h3 class="deck-title">
              Episodes — {record.seasons[selectedSeasonIndex].title}
            </h3>
            <div class="episodes-table-wrap">
              {#each record.seasons[selectedSeasonIndex].episodes as ep (ep.id)}
                <div class="episode-item-row" class:watched={ep.watched}>
                  <button
                    type="button"
                    class="ep-check-btn"
                    class:checked={ep.watched}
                    onclick={() => onToggleEpisode?.(record.id, ep.id)}
                    aria-label="Toggle watched for Episode {ep.number}"
                  >
                    {#if ep.watched}
                      <IconCheck size={16} stroke={3} />
                    {/if}
                  </button>

                  <span class="ep-num">#{ep.number}</span>

                  <div class="ep-main-details">
                    <div class="ep-header-line">
                      <h4 class="ep-title">{ep.title}</h4>
                      {#if ep.durationSeconds}
                        <span class="ep-duration"
                          >{Math.round(ep.durationSeconds / 60)} min</span
                        >
                      {/if}
                      {#if ep.airDate}
                        <span class="ep-air-date">{ep.airDate}</span>
                      {/if}
                    </div>
                    {#if ep.overview}
                      <p class="ep-overview">{ep.overview}</p>
                    {/if}
                  </div>

                  {#if ep.watchedAt}
                    <div class="ep-watched-pill">
                      Watched {new Date(ep.watchedAt).toLocaleDateString(
                        "en-IE",
                        { month: "short", day: "numeric" },
                      )}
                    </div>
                  {/if}
                </div>
              {/each}
            </div>
          </div>
        </section>

        <!-- Tab 2: Cast & Crew Avatars -->
      {:else if activeTab === "cast"}
        <section class="tab-pane">
          <h3 class="pane-heading">Top Billed Cast</h3>
          <div class="cast-grid">
            {#each record.cast ?? [] as actor (actor.id)}
              <div class="cast-card">
                <div class="cast-avatar">
                  {#if actor.profileUrl}
                    <img
                      src={actor.profileUrl}
                      alt={actor.name}
                      class="avatar-img"
                    />
                  {:else}
                    <div class="avatar-fallback">{actor.name.charAt(0)}</div>
                  {/if}
                </div>
                <div class="cast-text">
                  <h4 class="actor-name">{actor.name}</h4>
                  <p class="character-name">{actor.characterName}</p>
                </div>
              </div>
            {/each}
          </div>

          {#if record.crew && record.crew.length > 0}
            <h3 class="pane-heading mt-4">Key Crew & Creators</h3>
            <div class="crew-grid">
              {#each record.crew as c (c.id)}
                <div class="crew-card">
                  <h4 class="crew-name">{c.name}</h4>
                  <p class="crew-role">{c.role}</p>
                </div>
              {/each}
            </div>
          {/if}
        </section>

        <!-- Tab 3: Sources & Provider Identity -->
      {:else if activeTab === "sources"}
        <section class="tab-pane">
          <div class="provider-banner">
            <IconShieldCheck size={24} class="verified-icon" />
            <div>
              <h4 class="banner-title">
                Provider-Neutral Identity Architecture
              </h4>
              <p class="banner-desc">
                Fasti maintains a stable, immutable identity (`{record.id}`)
                independent of TMDB, TVDB, or MyAnimeList. You can switch
                primary metadata projection without breaking your history logs.
              </p>
            </div>
          </div>

          <div class="provider-switcher-row">
            <label for="display-source-select" class="provider-label"
              >Active Metadata Projection Provider:</label
            >
            <select
              id="display-source-select"
              class="provider-select"
              bind:value={activeDisplaySource}
            >
              <option value="tmdb_tv">TheMovieDatabase (TMDB)</option>
              <option value="tvdb">TheTVDB v4</option>
              <option value="mal_anime">MyAnimeList (MAL)</option>
              <option value="anilist_anime">AniList GraphQL</option>
              <option value="simkl">SIMKL Multi-Crosswalk</option>
            </select>
          </div>

          <table class="assertions-table">
            <thead>
              <tr>
                <th scope="col">Namespace</th>
                <th scope="col">Identifier</th>
                <th scope="col">Status</th>
                <th scope="col">Provenance Route</th>
              </tr>
            </thead>
            <tbody>
              {#each record.externalIds as xid}
                <tr>
                  <td class="mono">{xid.namespace}</td>
                  <td class="mono"><strong>{xid.value}</strong></td>
                  <td
                    ><span class="status-pill matched"
                      >{xid.status.replace("_", " ")}</span
                    ></td
                  >
                  <td>{xid.source}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </section>

        <!-- Tab 4: Personal Notes & Tags Editor -->
      {:else if activeTab === "notes"}
        <section class="tab-pane">
          <div class="notes-header-row">
            <h3 class="pane-heading">Personal Reflections & Review</h3>
            {#if !isEditingNotes}
              <button
                type="button"
                class="edit-notes-btn"
                onclick={() => {
                  editedNotesText = record.userNotes ?? "";
                  isEditingNotes = true;
                }}
              >
                <IconEdit size={14} /> Edit Notes
              </button>
            {/if}
          </div>

          {#if isEditingNotes}
            <div class="notes-edit-box">
              <textarea
                bind:value={editedNotesText}
                class="notes-textarea"
                rows="6"
                placeholder="Write markdown reflections, favorite scenes, or analysis..."
              ></textarea>
              <div class="notes-actions">
                <button
                  type="button"
                  class="btn-primary"
                  onclick={handleSaveNotes}>Save Reflections</button
                >
                <button
                  type="button"
                  class="btn-secondary"
                  onclick={() => (isEditingNotes = false)}>Cancel</button
                >
              </div>
            </div>
          {:else}
            <div class="notes-display-box">
              {#if record.userNotes}
                <p class="notes-text">{record.userNotes}</p>
              {:else}
                <p class="empty-notes-hint">
                  No personal notes recorded yet. Click 'Edit Notes' to add your
                  thoughts.
                </p>
              {/if}
            </div>
          {/if}

          <!-- Tags Editor -->
          <h3 class="pane-heading mt-4">Personal Tags & Organizers</h3>
          <div class="tags-editor-row">
            {#each record.tags as tag}
              <span class="tag-chip">
                <span>{tag}</span>
                <button
                  type="button"
                  class="tag-delete-btn"
                  onclick={() => onRemoveTag?.(record.id, tag)}
                  aria-label="Remove tag {tag}"
                >
                  <IconX size={12} />
                </button>
              </span>
            {/each}

            <form onsubmit={handleAddTagSubmit} class="add-tag-form">
              <input
                type="text"
                placeholder="+ Add tag..."
                bind:value={newTagInput}
                class="add-tag-input"
                aria-label="New tag name"
              />
            </form>
          </div>
        </section>
      {/if}
    </main>
  </div>
</div>

<style>
  .detail-container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .top-nav-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .back-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: none;
    color: var(--fasti-action-primary);
    font-weight: 600;
    font-size: 0.92rem;
    cursor: pointer;
    padding: 6px 0;
  }

  .top-id-badge {
    font-family: var(--fasti-font-mono);
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
  }

  .media-main-header {
    display: grid;
    grid-template-columns: 240px 1fr;
    gap: 32px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 8px;
    padding: 28px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.04);
  }

  .poster-frame {
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: 6px;
    overflow: hidden;
    background: var(--fasti-surface-archive);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.12);
  }

  .main-poster {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .header-details-column {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .main-title {
    font-family: var(--fasti-font-display);
    font-size: 2.6rem;
    font-weight: 600;
    margin: 0;
    line-height: 1.1;
    color: var(--fasti-text-primary);
  }

  .original-title {
    font-family: var(--fasti-font-display);
    font-size: 1.25rem;
    font-style: italic;
    color: var(--fasti-text-muted);
    margin: -4px 0 0;
  }

  .meta-strip {
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: var(--fasti-font-mono);
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
  }

  .action-strip {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    padding: 12px 0;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }

  .score-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 4px;
    background: #eab308;
    color: #1c1917;
    font-family: var(--fasti-font-mono);
    font-weight: 700;
  }

  .score-num {
    font-size: 1.05rem;
  }
  .score-meta {
    font-size: 0.72rem;
    font-weight: 500;
  }

  .user-rating-box {
    display: flex;
    align-items: center;
    background: var(--fasti-surface-archive);
    border: 1px solid var(--fasti-brand-gold);
    border-radius: 4px;
    padding: 4px 8px;
  }

  .user-star {
    color: var(--fasti-brand-gold);
    margin-right: 4px;
  }

  .rating-select {
    border: none;
    background: transparent;
    font-family: var(--fasti-font-mono);
    font-size: 0.88rem;
    font-weight: 700;
    color: var(--fasti-text-primary);
    cursor: pointer;
  }

  .status-select {
    padding: 7px 12px;
    border-radius: 4px;
    font-weight: 600;
    font-size: 0.88rem;
    cursor: pointer;
    border: 1px solid transparent;
  }

  .status-select.watching {
    background: var(--fasti-action-primary);
    color: white;
  }
  .status-select.completed {
    background: var(--fasti-state-verified);
    color: white;
  }

  .icon-action-btn {
    width: 36px;
    height: 36px;
    border-radius: 4px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    background: var(--fasti-surface-archive);
    display: grid;
    place-items: center;
    color: var(--fasti-text-primary);
    cursor: pointer;
  }

  .icon-action-btn.active {
    color: #e11d48;
    border-color: #e11d48;
  }

  .play-with-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    border-radius: 4px;
    background: var(--fasti-brand-mark);
    color: white;
    font-weight: 600;
    font-size: 0.9rem;
    border: none;
    cursor: pointer;
    margin-left: auto;
  }

  .synopsis-prose {
    font-size: 0.95rem;
    line-height: 1.7;
    color: var(--fasti-text-primary);
    margin: 4px 0 0;
    max-width: 85ch;
  }

  .details-body-grid {
    display: grid;
    grid-template-columns: 280px 1fr;
    gap: 24px;
  }

  .sidebar-details-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    height: fit-content;
  }

  .details-sidebar-heading {
    font-family: var(--fasti-font-display);
    font-size: 1.3rem;
    margin: 0;
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
    padding-bottom: 8px;
  }

  .sidebar-meta-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
  }

  .meta-pair dt {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    text-transform: uppercase;
    color: var(--fasti-text-muted);
  }

  .meta-pair dd {
    font-size: 0.9rem;
    font-weight: 600;
    margin: 2px 0 0;
  }

  .sidebar-subheading {
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    text-transform: uppercase;
    color: var(--fasti-text-muted);
    margin: 0 0 8px;
  }

  .chips-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .genre-chip {
    padding: 3px 8px;
    border-radius: 4px;
    background: var(--fasti-surface-archive);
    font-size: 0.8rem;
    font-weight: 500;
  }

  .studios-list {
    margin: 0;
    padding-left: 18px;
    font-size: 0.85rem;
    color: var(--fasti-text-primary);
  }

  .xid-links {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .xid-link {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    background: var(--fasti-surface-archive);
    border-radius: 4px;
    font-family: var(--fasti-font-mono);
    font-size: 0.78rem;
    color: var(--fasti-text-primary);
    text-decoration: none;
  }

  .xid-link:hover {
    color: var(--fasti-action-primary);
  }

  .main-content-pane {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
    overflow: hidden;
  }

  .content-tabs {
    display: flex;
    background: var(--fasti-surface-archive);
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .tab-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 12px 18px;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--fasti-text-muted);
    cursor: pointer;
  }

  .tab-btn.active {
    color: var(--fasti-action-primary);
    border-bottom-color: var(--fasti-action-primary);
    background: var(--fasti-surface-paper);
  }

  .tab-pane {
    padding: 24px;
  }

  .seasons-cards-row {
    display: flex;
    gap: 12px;
    margin-bottom: 24px;
    overflow-x: auto;
  }

  .season-card-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 14px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
    cursor: pointer;
  }

  .season-card-btn.active {
    border-color: var(--fasti-action-primary);
    background: color-mix(
      in srgb,
      var(--fasti-action-primary) 10%,
      transparent
    );
  }

  .season-thumb {
    width: 36px;
    height: 52px;
    object-fit: cover;
    border-radius: 3px;
  }

  .season-name {
    font-weight: 700;
    font-size: 0.92rem;
    display: block;
  }
  .season-count {
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
  }

  .episodes-table-wrap {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .episode-item-row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 16px;
    background: var(--fasti-surface-archive);
    border-radius: 4px;
  }

  .episode-item-row.watched {
    opacity: 0.75;
  }

  .ep-check-btn {
    width: 28px;
    height: 28px;
    border-radius: 4px;
    border: 2px solid var(--fasti-text-muted);
    background: transparent;
    display: grid;
    place-items: center;
    cursor: pointer;
    padding: 0;
  }

  .ep-check-btn.checked {
    background: var(--fasti-state-verified);
    border-color: var(--fasti-state-verified);
    color: white;
  }

  .ep-num {
    font-family: var(--fasti-font-mono);
    font-weight: 700;
    color: var(--fasti-text-muted);
  }
  .ep-main-details {
    flex: 1;
  }
  .ep-header-line {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .ep-title {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
  }
  .ep-duration,
  .ep-air-date {
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
  }
  .ep-overview {
    font-size: 0.82rem;
    color: var(--fasti-text-muted);
    margin: 4px 0 0;
  }

  .cast-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
  }

  .cast-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 12px;
    background: var(--fasti-surface-archive);
    border-radius: 6px;
  }

  .cast-avatar {
    width: 72px;
    height: 72px;
    border-radius: 50%;
    overflow: hidden;
    background: color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    margin-bottom: 8px;
  }

  .avatar-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .actor-name {
    font-size: 0.88rem;
    font-weight: 600;
    margin: 0 0 2px;
  }
  .character-name {
    font-size: 0.75rem;
    color: var(--fasti-text-muted);
    margin: 0;
  }

  .provider-banner {
    display: flex;
    gap: 14px;
    padding: 14px 18px;
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 12%,
      transparent
    );
    border-left: 4px solid var(--fasti-state-verified);
    border-radius: 4px;
    margin-bottom: 20px;
  }

  .banner-title {
    font-size: 0.95rem;
    font-weight: 700;
    margin: 0 0 2px;
  }
  .banner-desc {
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
    margin: 0;
  }

  .provider-switcher-row {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 20px;
  }

  .provider-label {
    font-weight: 600;
    font-size: 0.9rem;
  }
  .provider-select {
    padding: 8px 12px;
    border-radius: 4px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    background: var(--fasti-surface-paper);
    font-size: 0.9rem;
    font-weight: 600;
  }

  .assertions-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.88rem;
    text-align: left;
  }

  .assertions-table th,
  .assertions-table td {
    padding: 10px 14px;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }

  .mono {
    font-family: var(--fasti-font-mono);
  }
  .status-pill.matched {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 3px;
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 15%,
      transparent
    );
    color: var(--fasti-state-verified);
  }

  .notes-header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }

  .edit-notes-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 6px 12px;
    border-radius: 4px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
  }

  .notes-textarea {
    width: 100%;
    padding: 12px;
    border-radius: 4px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    font-family: var(--fasti-font-body);
    font-size: 0.95rem;
    background: var(--fasti-surface-paper);
    box-sizing: border-box;
  }

  .notes-actions {
    display: flex;
    gap: 10px;
    margin-top: 10px;
  }
  .btn-primary {
    padding: 8px 16px;
    background: var(--fasti-action-primary);
    color: white;
    border: none;
    border-radius: 4px;
    font-weight: 600;
    cursor: pointer;
  }
  .btn-secondary {
    padding: 8px 16px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    cursor: pointer;
  }

  .notes-display-box {
    padding: 18px;
    background: var(--fasti-surface-archive);
    border-radius: 6px;
    font-size: 0.95rem;
    line-height: 1.6;
  }

  .tags-editor-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }

  .tag-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 20px;
    font-size: 0.85rem;
  }

  .tag-delete-btn {
    background: transparent;
    border: none;
    padding: 0;
    cursor: pointer;
    display: grid;
    place-items: center;
    color: var(--fasti-text-muted);
  }

  .tag-delete-btn:hover {
    color: #e11d48;
  }

  .add-tag-input {
    height: 30px;
    padding: 4px 10px;
    border: 1px dashed
      color-mix(in srgb, var(--fasti-text-muted) 40%, transparent);
    border-radius: 20px;
    background: transparent;
    font-size: 0.85rem;
  }
</style>
