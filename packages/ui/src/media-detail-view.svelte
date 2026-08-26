<script lang="ts">
  import type {
    MediaRecord,
    WatchStatus,
    ExternalId,
    ChronicleOccurrence,
  } from "./types.js";
  import {
    IconArrowLeft,
    IconStarFilled,
    IconCheck,
    IconBookmark,
    IconHeart,
    IconCalendar,
    IconRepeat,
    IconExternalLink,
    IconShieldCheck,
    IconNotes,
    IconListNumbers,
    IconHistory,
    IconAdjustments,
    IconFolderPlus,
    IconMessage,
    IconDotsVertical,
    IconX,
    IconEdit,
    IconClock,
    IconDeviceTv,
    IconPhoto,
    IconRefresh,
    IconPlus,
  } from "@tabler/icons-svelte";
  import ProgressModal from "./progress-modal.svelte";
  import RatingReviewModal from "./rating-review-modal.svelte";
  import CollectionModal from "./collection-modal.svelte";
  import ContextMenu, { type ContextMenuItem } from "./context-menu.svelte";

  interface Props {
    record: MediaRecord;
    occurrences?: ChronicleOccurrence[];
    onBack: () => void;
    onUpdateStatus?: (recordId: string, status: WatchStatus) => void;
    onUpdateRating?: (recordId: string, rating: number) => void;
    onToggleEpisode?: (recordId: string, episodeId: string) => void;
    onUpdateProgress?: (
      recordId: string,
      episodes: number,
      seconds: number,
      status: WatchStatus,
    ) => void;
    onSaveReview?: (recordId: string, rating: number, notes: string) => void;
    onSaveCollection?: (recordId: string, collections: string[]) => void;
    onUpdateNotes?: (recordId: string, notes: string) => void;
    onAddTag?: (recordId: string, tag: string) => void;
    onRemoveTag?: (recordId: string, tag: string) => void;
  }

  let {
    record,
    occurrences = [],
    onBack,
    onUpdateStatus,
    onUpdateRating,
    onToggleEpisode,
    onUpdateProgress,
    onSaveReview,
    onSaveCollection,
    onUpdateNotes,
    onAddTag,
    onRemoveTag,
  }: Props = $props();

  let activeTab: "overview" | "actions" | "history" | "sources" | "reviews" =
    $state("overview");
  let selectedSeasonIndex = $state(0);
  let isEditingNotes = $state(false);
  let editedNotesText = $state("");
  let newTagInput = $state("");
  let isFavorite = $state(false);
  let activeDisplaySource = $state("tmdb_tv");

  // Artwork & Metadata Editor State
  let showArtworkModal = $state(false);
  let editingPosterUrl = $state("");
  let editingBackdropUrl = $state("");
  let editingTitle = $state("");
  let editingOverview = $state("");
  let newClaimNamespace = $state("tmdb_tv");
  let newClaimValue = $state("");
  let showAddClaimInline = $state(false);

  // Modal Dialog States
  let showProgressModal = $state(false);
  let showReviewModal = $state(false);
  let showCollectionModal = $state(false);

  // Context Menu State
  let contextMenuState = $state<{
    x: number;
    y: number;
    items: ContextMenuItem[];
  } | null>(null);

  $effect(() => {
    editedNotesText = record.userNotes ?? "";
    activeDisplaySource = record.displaySource;
    editingPosterUrl = record.posterUrl ?? "";
    editingBackdropUrl = record.backdropUrl ?? "";
    editingTitle = record.title;
    editingOverview = record.overview ?? "";
    const seasonCount = record.seasons?.length ?? 0;
    if (seasonCount === 0) {
      selectedSeasonIndex = 0;
    } else if (selectedSeasonIndex >= seasonCount) {
      selectedSeasonIndex = seasonCount - 1;
    }
  });

  const candidatePosters = $derived.by(() => {
    const list: Array<{ source: string; url: string }> = [];
    if (record.posterUrl) {
      list.push({ source: "Current", url: record.posterUrl });
    }
    if (record.id === "rec_01K89Z01FrierenAnime") {
      list.push(
        {
          source: "AniList Cover",
          url: "https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/bx154587-n2bGQYLiqcQE.jpg",
        },
        {
          source: "MAL Key Visual",
          url: "https://cdn.myanimelist.net/images/anime/1015/138006.jpg",
        },
        {
          source: "Unsplash Aesthetic",
          url: "https://images.unsplash.com/photo-1578632767115-351597cf2477?w=600&q=80",
        },
      );
    } else if (record.id === "rec_01K89Z06HikaruNoGoManga") {
      list.push(
        {
          source: "AniList Manga",
          url: "https://s4.anilist.co/file/anilistcdn/media/manga/cover/large/bx30023-FmO9hV838P0F.jpg",
        },
        {
          source: "MAL Manga Cover",
          url: "https://cdn.myanimelist.net/images/manga/2/253419.jpg",
        },
        {
          source: "Jump Classic",
          url: "https://images.unsplash.com/photo-1529699211952-734e80c4d42b?w=600&q=80",
        },
      );
    } else if (record.mediaKind === "anime") {
      list.push(
        {
          source: "Kitsu Artwork",
          url: "https://media.kitsu.app/anime/poster_images/46001/medium.jpg",
        },
        {
          source: "AniList Artwork",
          url: "https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/bx154587-n2bGQYLiqcQE.jpg",
        },
      );
    }
    return list;
  });

  function handleSetCanonical(xid: ExternalId): void {
    record.displaySource = xid.namespace;
    activeDisplaySource = xid.namespace;
    xid.status = "matched";
  }

  function handleAddExternalId(): void {
    if (newClaimValue.trim().length > 0) {
      if (!record.externalIds) record.externalIds = [];
      record.externalIds.push({
        namespace: newClaimNamespace,
        value: newClaimValue.trim(),
        status: "matched",
        source: "user_override",
      });
      newClaimValue = "";
      showAddClaimInline = false;
    }
  }

  const recordOccurrences = $derived(
    occurrences.filter((occ) => occ.recordId === record.id),
  );

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

  function handleMarkSeasonWatched(seasonIndex: number): void {
    if (!record.seasons?.[seasonIndex]) return;
    const season = record.seasons[seasonIndex];
    for (const ep of season.episodes) {
      if (!ep.watched) {
        onToggleEpisode?.(record.id, ep.id);
      }
    }
  }

  function handleMarkSeasonUnwatched(seasonIndex: number): void {
    if (!record.seasons?.[seasonIndex]) return;
    const season = record.seasons[seasonIndex];
    for (const ep of season.episodes) {
      if (ep.watched) {
        onToggleEpisode?.(record.id, ep.id);
      }
    }
  }

  function handleMarkPreviousEpisodesWatched(
    seasonIndex: number,
    upToEpisodeNumber: number,
  ): void {
    if (!record.seasons?.[seasonIndex]) return;
    const season = record.seasons[seasonIndex];
    for (const ep of season.episodes) {
      if (ep.number <= upToEpisodeNumber && !ep.watched) {
        onToggleEpisode?.(record.id, ep.id);
      }
    }
  }

  function handleAddTagSubmit(e: Event): void {
    e.preventDefault();
    if (newTagInput.trim().length > 0) {
      onAddTag?.(record.id, newTagInput.trim());
      newTagInput = "";
    }
  }

  function handleOpenContextMenu(e: MouseEvent): void {
    e.preventDefault();
    contextMenuState = {
      x: e.clientX,
      y: e.clientY,
      items: [
        {
          id: "prog",
          label: "Update Progress...",
          icon: IconAdjustments,
          action: () => (showProgressModal = true),
        },
        {
          id: "review",
          label: "Post a Review...",
          icon: IconMessage,
          action: () => (showReviewModal = true),
        },
        {
          id: "coll",
          label: "Add to Collection...",
          icon: IconFolderPlus,
          action: () => (showCollectionModal = true),
        },
        {
          id: "rewatch",
          label: "Log Occurrence (Rewatch)",
          icon: IconRepeat,
          action: () => onUpdateStatus?.(record.id, "completed"),
        },
        { id: "d1", label: "", divider: true, action: () => {} },
        {
          id: "copy_id",
          label: `Copy Fasti ID (${record.id})`,
          action: () => navigator.clipboard.writeText(record.id),
        },
      ],
    };
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
      <span class="id-label">Fasti Entity ID:</span>
      <code>{record.id}</code>
    </div>
  </div>

  <!-- Main Media Header (Floppy / Yamtrack / Ryot Editorial Style) -->
  <header class="media-main-header">
    <!-- Left Poster Column -->
    <div class="poster-column">
      <div class="poster-frame">
        {#if record.posterUrl}
          <img
            src={record.posterUrl}
            alt="{record.title} Poster"
            class="main-poster"
            referrerpolicy="no-referrer"
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
        {#if record.collectionName}
          <span class="collection-pill">{record.collectionName}</span>
          <span class="bullet">·</span>
        {/if}
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

        <button
          type="button"
          class="icon-action-btn"
          onclick={() => (showCollectionModal = true)}
          title="Add to Collection"
        >
          <IconBookmark size={18} />
        </button>

        <button
          type="button"
          class="icon-action-btn"
          onclick={() => (showProgressModal = true)}
          title="Update Progress"
        >
          <IconAdjustments size={18} />
        </button>

        <button
          type="button"
          class="btn btn-sm btn-outline-secondary d-inline-flex align-items-center gap-1"
          onclick={() => {
            editingPosterUrl = record.posterUrl ?? "";
            editingBackdropUrl = record.backdropUrl ?? "";
            editingTitle = record.title;
            editingOverview = record.overview ?? "";
            showArtworkModal = true;
          }}
          title="Edit Artwork, Poster & Metadata"
        >
          <IconPhoto size={16} />
          <span>Edit Artwork & Poster</span>
        </button>

        <button
          type="button"
          class="icon-action-btn"
          onclick={handleOpenContextMenu}
          title="More Actions..."
        >
          <IconDotsVertical size={18} />
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

      <!-- Custom Fields & Attributes -->
      <div class="sidebar-section">
        <h4 class="sidebar-subheading">Custom Fields</h4>
        {#if record.customFields && Object.keys(record.customFields).length > 0}
          <dl class="meta-list">
            {#each Object.entries(record.customFields) as [key, val]}
              <div class="meta-pair">
                <dt class="text-capitalize">{key.replace(/_/g, " ")}</dt>
                <dd>{val === true ? "Yes" : val === false ? "No" : val}</dd>
              </div>
            {/each}
          </dl>
        {:else}
          <p class="text-muted small mb-0" style="font-size: 0.75rem;">
            No custom field values set. Configure fields in Settings &rarr; Custom Fields.
          </p>
        {/if}
      </div>
    </aside>

    <!-- Right Main Tabbed Content Area (Ryot 5-Tab System) -->
    <main class="main-content-pane">
      <!-- Section Tabs -->
      <nav class="content-tabs" aria-label="Media section tabs">
        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === "overview"}
          onclick={() => (activeTab = "overview")}
        >
          <IconListNumbers size={16} /> Overview & Seasons
        </button>

        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === "actions"}
          onclick={() => (activeTab = "actions")}
        >
          <IconAdjustments size={16} /> Actions & Progress
        </button>

        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === "history"}
          onclick={() => (activeTab = "history")}
        >
          <IconHistory size={16} /> History ({recordOccurrences.length})
        </button>

        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === "sources"}
          onclick={() => (activeTab = "sources")}
        >
          <IconShieldCheck size={16} /> Sources & Identity ({record.externalIds
            .length})
        </button>

        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === "reviews"}
          onclick={() => (activeTab = "reviews")}
        >
          <IconNotes size={16} /> Notes & Reviews
        </button>
      </nav>

      <!-- TAB 1: OVERVIEW & SEASONS -->
      {#if activeTab === "overview"}
        <section class="tab-pane">
          {#if record.seasons && record.seasons.length > 0}
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
                    <img
                      src={season.posterUrl}
                      alt=""
                      class="season-thumb"
                      referrerpolicy="no-referrer"
                    />
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
            {#if record.seasons[selectedSeasonIndex]}
              <div class="episodes-deck">
                <div class="d-flex align-items-center justify-content-between flex-wrap gap-2 mb-3">
                  <h3 class="deck-title mb-0">
                    Episodes — {record.seasons[selectedSeasonIndex].title}
                  </h3>
                  <div class="d-flex align-items-center gap-2">
                    <button
                      type="button"
                      class="btn btn-sm btn-outline-success d-flex align-items-center gap-1"
                      onclick={() => handleMarkSeasonWatched(selectedSeasonIndex)}
                      title="Mark all episodes in this season as watched"
                    >
                      <IconCheck size={14} /> Mark Season Watched
                    </button>
                    <button
                      type="button"
                      class="btn btn-sm btn-outline-secondary d-flex align-items-center gap-1"
                      onclick={() => handleMarkSeasonUnwatched(selectedSeasonIndex)}
                      title="Mark all episodes in this season as unwatched"
                    >
                      <IconX size={14} /> Mark Season Unwatched
                    </button>
                  </div>
                </div>

                <div class="episodes-table-wrap">
                  {#each record.seasons[selectedSeasonIndex].episodes ?? [] as ep (ep.id)}
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

                      <div class="d-flex align-items-center gap-2 flex-shrink-0">
                        {#if !ep.watched && ep.number > 1}
                          <button
                            type="button"
                            class="btn btn-outline-secondary btn-sm py-0 px-2 font-monospace"
                            style="font-size: 0.72rem;"
                            onclick={() =>
                              handleMarkPreviousEpisodesWatched(
                                selectedSeasonIndex,
                                ep.number,
                              )}
                            title="Mark all previous episodes 1 to {ep.number} as watched"
                          >
                            Mark 1..#{ep.number} Seen
                          </button>
                        {/if}

                        {#if ep.watchedAt}
                          <div class="ep-watched-pill">
                            Watched {new Date(ep.watchedAt).toLocaleDateString(
                              "en-IE",
                              { month: "short", day: "numeric" },
                            )}
                          </div>
                        {/if}
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
          {/if}

          <!-- Cast & Crew Section -->
          {#if record.cast && record.cast.length > 0}
            <h3 class="pane-heading mt-4">Top Billed Cast</h3>
            <div class="cast-grid">
              {#each record.cast as actor (actor.id)}
                <div class="cast-card">
                  <div class="cast-avatar">
                    {#if actor.profileUrl}
                      <img
                        src={actor.profileUrl}
                        alt={actor.name}
                        class="avatar-img"
                        referrerpolicy="no-referrer"
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
          {/if}
        </section>

        <!-- TAB 2: RYOT-STYLE ACTIONS DASHBOARD -->
      {:else if activeTab === "actions"}
        <section class="tab-pane">
          <h3 class="pane-heading">Media Action Center</h3>
          <p class="pane-sub">
            Execute state changes, log manual occurrences, or re-organize
            collections.
          </p>

          <div class="ryot-actions-grid">
            <button
              type="button"
              class="ryot-action-btn"
              onclick={() => (showProgressModal = true)}
            >
              <div class="action-btn-icon"><IconAdjustments size={22} /></div>
              <div class="action-btn-text">
                <strong>Update Progress</strong>
                <span>Set current episode, timestamp, or percentage</span>
              </div>
            </button>

            <button
              type="button"
              class="ryot-action-btn"
              onclick={() => (showReviewModal = true)}
            >
              <div class="action-btn-icon"><IconMessage size={22} /></div>
              <div class="action-btn-text">
                <strong>Post a Review</strong>
                <span>Submit 10-star rating and personal critique</span>
              </div>
            </button>

            <button
              type="button"
              class="ryot-action-btn"
              onclick={() => (showCollectionModal = true)}
            >
              <div class="action-btn-icon"><IconFolderPlus size={22} /></div>
              <div class="action-btn-text">
                <strong>Add to Collection</strong>
                <span>Add to custom lists, favorites, or franchise</span>
              </div>
            </button>

            <button
              type="button"
              class="ryot-action-btn"
              onclick={() => onUpdateStatus?.(record.id, "completed")}
            >
              <div class="action-btn-icon"><IconRepeat size={22} /></div>
              <div class="action-btn-text">
                <strong>Log Rewatch Occurrence</strong>
                <span>Record a new chronological consumption timestamp</span>
              </div>
            </button>
          </div>
        </section>

        <!-- TAB 3: OCCURRENCE & PROGRESS HISTORY -->
      {:else if activeTab === "history"}
        <section class="tab-pane">
          <h3 class="pane-heading">Chronicle History for this Entity</h3>
          <p class="pane-sub">
            Every recorded observation, rewatch, and device scrobble for {record.title}.
          </p>

          {#if recordOccurrences.length === 0}
            <div class="empty-history-box">
              <IconClock size={32} class="empty-icon" />
              <h4>No occurrences recorded yet</h4>
              <p>
                When you watch or scrobble this title, every occurrence will be
                preserved here.
              </p>
              <button
                type="button"
                class="btn-primary"
                onclick={() => onUpdateStatus?.(record.id, "completed")}
              >
                Log First Occurrence
              </button>
            </div>
          {:else}
            <div class="history-timeline">
              {#each recordOccurrences as occ (occ.id)}
                <div class="history-item-card">
                  <div class="history-left">
                    <span class="hist-time"
                      >{new Date(occ.timestamp).toLocaleDateString("en-IE", {
                        month: "short",
                        day: "numeric",
                        hour: "2-digit",
                        minute: "2-digit",
                      })}</span
                    >
                    <span class="hist-client">{occ.clientName}</span>
                  </div>
                  <div class="history-center">
                    <h4 class="hist-title">{occ.episodeTitle ?? occ.title}</h4>
                    <span class="hist-device"
                      ><IconDeviceTv size={14} /> {occ.deviceName}</span
                    >
                  </div>
                  <div class="history-right">
                    <span class="hist-dur">{occ.durationMinutes} min</span>
                    {#if occ.isRewatch}
                      <span class="rewatch-pill">Rewatch</span>
                    {/if}
                    {#if occ.userRating}
                      <span class="hist-star">★ {occ.userRating}/10</span>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </section>

        <!-- TAB 4: SOURCES & PROVIDER IDENTITY (UAT ID-001–ID-030) -->
      {:else if activeTab === "sources"}
        <section class="tab-pane">
          <div class="provider-banner">
            <IconShieldCheck size={24} class="verified-icon" />
            <div>
              <h4 class="banner-title">
                Provider-Neutral Identity Architecture
              </h4>
              <p class="banner-desc">
                Fasti maintains a stable, immutable identity (<code
                  >{record.id}</code
                >) independent of TMDB, TVDB, or MyAnimeList. You can switch
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

          <div class="d-flex justify-content-between align-items-center mb-2">
            <h4 class="m-0 font-display fw-bold" style="font-size: 1.1rem;">
              Mapped External Claims & IDs
            </h4>
            <button
              type="button"
              class="btn btn-sm btn-outline-primary d-inline-flex align-items-center gap-1"
              onclick={() => (showAddClaimInline = !showAddClaimInline)}
            >
              <IconPlus size={14} />
              <span>Add Identifier Claim</span>
            </button>
          </div>

          {#if showAddClaimInline}
            <div
              class="p-3 mb-3 border rounded"
              style="background: var(--fasti-surface-archive);"
            >
              <h5 class="mb-2 font-display">Add External Entity Mapping</h5>
              <div class="row g-2 align-items-end">
                <div class="col-md-4">
                  <label
                    for="claim-provider-select"
                    class="form-label small fw-bold">Provider / Namespace</label
                  >
                  <select
                    id="claim-provider-select"
                    class="form-select form-select-sm"
                    bind:value={newClaimNamespace}
                  >
                    <option value="tmdb_tv">TheMovieDatabase (TMDB)</option>
                    <option value="tvdb_series">TheTVDB v4 Series</option>
                    <option value="mal_anime">MyAnimeList (MAL)</option>
                    <option value="anilist_anime">AniList GraphQL</option>
                    <option value="kitsu_anime">Kitsu.io</option>
                    <option value="simkl">SIMKL Media Crosswalk</option>
                    <option value="imdb_title">IMDb Title ID</option>
                    <option value="google_books">Google Books</option>
                    <option value="open_library">Open Library</option>
                    <option value="steam_app">Steam Web API</option>
                    <option value="rawg_game">RAWG Video Game</option>
                    <option value="igdb">IGDB</option>
                    <option value="comicvine">ComicVine</option>
                    <option value="podcast_index">Podcast Index</option>
                  </select>
                </div>
                <div class="col-md-5">
                  <label
                    for="claim-value-input"
                    class="form-label small fw-bold">ID / Slug / ISBN</label
                  >
                  <input
                    id="claim-value-input"
                    type="text"
                    class="form-control form-control-sm"
                    placeholder="e.g. 52991 or tt22238804"
                    bind:value={newClaimValue}
                  />
                </div>
                <div class="col-md-3 d-flex gap-2">
                  <button
                    type="button"
                    class="btn btn-sm btn-primary flex-grow-1"
                    onclick={handleAddExternalId}
                  >
                    Save Claim
                  </button>
                  <button
                    type="button"
                    class="btn btn-sm btn-secondary"
                    onclick={() => (showAddClaimInline = false)}
                  >
                    Cancel
                  </button>
                </div>
              </div>
            </div>
          {/if}

          <table class="assertions-table">
            <thead>
              <tr>
                <th scope="col">Namespace</th>
                <th scope="col">Identifier</th>
                <th scope="col">Status</th>
                <th scope="col">Provenance Route</th>
                <th scope="col">Canonical Mapping</th>
              </tr>
            </thead>
            <tbody>
              {#each record.externalIds as xid}
                <tr>
                  <td class="mono">{xid.namespace}</td>
                  <td class="mono">
                    {#if xid.url}
                      <a
                        href={xid.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        class="d-inline-flex align-items-center gap-1"
                      >
                        <strong>{xid.value}</strong>
                        <IconExternalLink size={12} />
                      </a>
                    {:else}
                      <strong>{xid.value}</strong>
                    {/if}
                  </td>
                  <td>
                    <span class="status-pill {xid.status}">
                      {xid.status.replace("_", " ")}
                    </span>
                  </td>
                  <td>{xid.source}</td>
                  <td>
                    {#if activeDisplaySource === xid.namespace}
                      <span
                        class="badge bg-success-lt font-monospace d-inline-flex align-items-center gap-1"
                      >
                        <IconCheck size={12} stroke={3} /> Primary Canonical
                      </span>
                    {:else}
                      <button
                        type="button"
                        class="btn btn-sm btn-outline-secondary py-0 px-2 font-monospace"
                        style="font-size: 0.75rem;"
                        onclick={() => handleSetCanonical(xid)}
                        title="Set {xid.namespace} as canonical projection source"
                      >
                        Set Canonical
                      </button>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </section>

        <!-- TAB 5: PERSONAL REVIEWS & NOTES -->
      {:else if activeTab === "reviews"}
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

<!-- Modal Dialogs -->
{#if showProgressModal}
  <ProgressModal
    {record}
    onClose={() => (showProgressModal = false)}
    onSaveProgress={(recId, eps, sec, st) =>
      onUpdateProgress?.(recId, eps, sec, st)}
  />
{/if}

{#if showReviewModal}
  <RatingReviewModal
    {record}
    onClose={() => (showReviewModal = false)}
    onSaveReview={(recId, r, n) => onSaveReview?.(recId, r, n)}
  />
{/if}

{#if showCollectionModal}
  <CollectionModal
    {record}
    onClose={() => (showCollectionModal = false)}
    onSaveCollection={(recId, colls) => onSaveCollection?.(recId, colls)}
  />
{/if}

{#if showArtworkModal}
  <div
    class="modal show d-block fasti-modal-backdrop"
    tabindex="-1"
    role="dialog"
    aria-modal="true"
  >
    <div class="modal-dialog modal-lg modal-dialog-centered" role="document">
      <div
        class="modal-content"
        style="background: var(--fasti-surface-paper); color: var(--fasti-text-primary); border: 1px solid color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);"
      >
        <div class="modal-header border-bottom">
          <h5 class="modal-title d-flex align-items-center gap-2 font-display">
            <IconPhoto size={20} class="text-primary" />
            <span>Edit Artwork, Posters & Metadata</span>
          </h5>
          <button
            type="button"
            class="btn-close"
            aria-label="Close"
            onclick={() => (showArtworkModal = false)}
          ></button>
        </div>
        <div class="modal-body p-4">
          <div class="row g-4">
            <!-- Left: Poster Preview & URL -->
            <div class="col-md-5">
              <span class="form-label fw-bold d-block"
                >Active Poster Preview</span
              >
              <div
                class="poster-preview-box rounded overflow-hidden mb-3 border shadow-sm"
                style="aspect-ratio: 2/3; max-height: 280px; background: var(--fasti-surface-archive);"
              >
                {#if editingPosterUrl}
                  <img
                    src={editingPosterUrl}
                    alt="Preview"
                    class="w-100 h-100 object-fit-cover"
                    referrerpolicy="no-referrer"
                  />
                {:else}
                  <div
                    class="d-flex align-items-center justify-content-center h-100 text-muted"
                  >
                    No Poster Selected
                  </div>
                {/if}
              </div>
              <label for="edit-poster-url" class="form-label small fw-bold"
                >Poster Image URL</label
              >
              <input
                id="edit-poster-url"
                type="url"
                class="form-control form-control-sm mb-3"
                bind:value={editingPosterUrl}
                placeholder="https://..."
              />

              <label for="edit-backdrop-url" class="form-label small fw-bold"
                >Backdrop Banner URL</label
              >
              <input
                id="edit-backdrop-url"
                type="url"
                class="form-control form-control-sm"
                bind:value={editingBackdropUrl}
                placeholder="https://..."
              />
            </div>

            <!-- Right: Candidate Posters & Metadata fields -->
            <div class="col-md-7">
              <span class="form-label fw-bold mb-1 d-block"
                >Pick Alternative Cover Artwork</span
              >
              <p class="text-muted small mb-2">
                Click any candidate artwork below to apply it to this title:
              </p>

              <div class="d-flex gap-2 mb-4 overflow-x-auto pb-2">
                {#each candidatePosters as cand}
                  <button
                    type="button"
                    class="btn p-1 border rounded text-start flex-shrink-0"
                    class:border-primary={editingPosterUrl === cand.url}
                    style="width: 80px;"
                    onclick={() => (editingPosterUrl = cand.url)}
                    title={cand.source}
                  >
                    <img
                      src={cand.url}
                      alt={cand.source}
                      class="w-100 rounded mb-1"
                      style="aspect-ratio: 2/3; object-fit: cover;"
                      referrerpolicy="no-referrer"
                    />
                    <div
                      class="text-truncate text-muted font-monospace"
                      style="font-size: 0.65rem;"
                    >
                      {cand.source}
                    </div>
                  </button>
                {/each}
              </div>

              <label for="edit-title-input" class="form-label small fw-bold"
                >Title</label
              >
              <input
                id="edit-title-input"
                type="text"
                class="form-control form-control-sm mb-3"
                bind:value={editingTitle}
              />

              <label for="edit-overview-input" class="form-label small fw-bold"
                >Overview / Synopsis</label
              >
              <textarea
                id="edit-overview-input"
                class="form-control form-control-sm"
                rows="4"
                bind:value={editingOverview}></textarea>
            </div>
          </div>
        </div>
        <div class="modal-footer border-top">
          <button
            type="button"
            class="btn btn-secondary"
            onclick={() => (showArtworkModal = false)}
          >
            Cancel
          </button>
          <button
            type="button"
            class="btn btn-primary"
            onclick={() => {
              record.posterUrl = editingPosterUrl;
              record.backdropUrl = editingBackdropUrl;
              record.title = editingTitle;
              record.overview = editingOverview;
              showArtworkModal = false;
            }}
          >
            Apply & Save Artwork
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if contextMenuState}
  <ContextMenu
    x={contextMenuState.x}
    y={contextMenuState.y}
    items={contextMenuState.items}
    onClose={() => (contextMenuState = null)}
  />
{/if}

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
  .collection-pill {
    padding: 2px 8px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--fasti-brand-gold) 20%, transparent);
    color: var(--fasti-brand-gold);
    font-weight: 700;
    font-size: 0.75rem;
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
  .rating-select option {
    background-color: var(--fasti-surface-paper) !important;
    color: var(--fasti-text-primary) !important;
  }

  .status-select {
    padding: 7px 12px;
    border-radius: 4px;
    font-weight: 600;
    font-size: 0.88rem;
    cursor: pointer;
    border: 1px solid transparent;
    color: #ffffff;
  }
  .status-select option {
    background-color: var(--fasti-surface-paper) !important;
    color: var(--fasti-text-primary) !important;
  }
  .status-select.watching {
    background: var(--fasti-action-primary);
    color: #ffffff;
  }
  .status-select.completed {
    background: var(--fasti-state-verified);
    color: #ffffff;
  }
  .status-select.plan_to_watch {
    background: #6366f1;
    color: #ffffff;
  }
  .status-select.on_hold {
    background: #d97706;
    color: #ffffff;
  }
  .status-select.dropped {
    background: #dc2626;
    color: #ffffff;
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
    overflow-x: auto;
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
    white-space: nowrap;
  }
  .tab-btn.active {
    color: var(--fasti-action-primary);
    border-bottom-color: var(--fasti-action-primary);
    background: var(--fasti-surface-paper);
  }
  .tab-pane {
    padding: 24px;
  }

  /* Ryot Actions Grid */
  .ryot-actions-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 14px;
    margin-top: 14px;
  }
  .ryot-action-btn {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 16px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
    transition: all 120ms ease;
  }
  .ryot-action-btn:hover {
    border-color: var(--fasti-action-primary);
    background: color-mix(in srgb, var(--fasti-action-primary) 8%, transparent);
  }
  .action-btn-icon {
    width: 44px;
    height: 44px;
    border-radius: 6px;
    background: var(--fasti-surface-paper);
    display: grid;
    place-items: center;
    color: var(--fasti-action-primary);
  }
  .action-btn-text strong {
    display: block;
    font-size: 0.95rem;
    margin-bottom: 2px;
  }
  .action-btn-text span {
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
  }

  /* History Timeline */
  .history-timeline {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 14px;
  }
  .history-item-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    background: var(--fasti-surface-archive);
    border-radius: 6px;
  }
  .history-left {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 140px;
  }
  .hist-time {
    font-family: var(--fasti-font-mono);
    font-size: 0.8rem;
    font-weight: 700;
  }
  .hist-client {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    color: var(--fasti-text-muted);
  }
  .history-center {
    flex: 1;
    margin: 0 16px;
  }
  .hist-title {
    margin: 0;
    font-size: 0.92rem;
  }
  .hist-device {
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-top: 2px;
  }
  .history-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .hist-dur {
    font-family: var(--fasti-font-mono);
    font-size: 0.8rem;
    font-weight: 600;
  }
  .rewatch-pill {
    font-size: 0.7rem;
    padding: 2px 6px;
    background: var(--fasti-brand-gold);
    color: black;
    border-radius: 3px;
    font-weight: 700;
  }
  .hist-star {
    font-size: 0.8rem;
    color: var(--fasti-brand-gold);
    font-weight: 700;
  }

  .empty-history-box {
    text-align: center;
    padding: 40px 20px;
    background: var(--fasti-surface-archive);
    border-radius: 6px;
  }
  :global(.empty-icon) {
    color: var(--fasti-text-muted);
    margin-bottom: 8px;
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
    margin-top: 14px;
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
