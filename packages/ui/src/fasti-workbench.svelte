<script lang="ts">
  import type { ActiveNavSection, MediaRecord, WatchStatus } from "./types.js";
  import {
    SAMPLE_RECORDS,
    SAMPLE_CHRONICLE,
    SAMPLE_RECONCILIATION,
    SAMPLE_CUSTOM_FIELDS,
    SAMPLE_TOKENS,
  } from "./mock-data.js";
  import NavSidebar from "./nav-sidebar.svelte";
  import ChronicleView from "./chronicle-view.svelte";
  import LibraryView from "./library-view.svelte";
  import MediaDetailView from "./media-detail-view.svelte";
  import ReconciliationView from "./reconciliation-view.svelte";
  import CalendarView from "./calendar-view.svelte";
  import ConnectionsView from "./connections-view.svelte";
  import SettingsView from "./settings-view.svelte";

  let activeSection: ActiveNavSection = $state("chronicle");
  let records = $state<MediaRecord[]>(SAMPLE_RECORDS);
  let chronicle = $state(SAMPLE_CHRONICLE);
  let reconciliationCases = $state(SAMPLE_RECONCILIATION);
  let selectedRecordId = $state<string | null>(null);

  const selectedRecord = $derived(
    records.find((r) => r.id === selectedRecordId),
  );
  const watchingRecords = $derived(
    records.filter((r) => r.status === "watching"),
  );
  const openReviewCount = $derived(
    reconciliationCases.filter((c) => c.status === "open").length,
  );

  function handleSelectSection(section: ActiveNavSection): void {
    activeSection = section;
    if (section !== "detail") {
      selectedRecordId = null;
    }
  }

  function handleSelectRecord(recordId: string): void {
    selectedRecordId = recordId;
    activeSection = "detail";
  }

  function handleBackToLibrary(): void {
    activeSection = "library";
    selectedRecordId = null;
  }

  function handleUpdateStatus(recordId: string, newStatus: WatchStatus): void {
    records = records.map((r) =>
      r.id === recordId ? { ...r, status: newStatus } : r,
    );
  }

  function handleUpdateRating(recordId: string, newRating: number): void {
    records = records.map((r) =>
      r.id === recordId ? { ...r, userRating: newRating } : r,
    );
  }

  function handleToggleEpisode(recordId: string, episodeId: string): void {
    records = records.map((r) => {
      if (r.id !== recordId || !r.seasons) return r;
      const updatedSeasons = r.seasons.map((s) => ({
        ...s,
        episodes: s.episodes.map((ep) =>
          ep.id === episodeId
            ? {
                ...ep,
                watched: !ep.watched,
                watchedAt: !ep.watched ? new Date().toISOString() : undefined,
              }
            : ep,
        ),
      }));
      const watchedCount = updatedSeasons.reduce(
        (acc, s) => acc + s.episodes.filter((e) => e.watched).length,
        0,
      );
      return { ...r, seasons: updatedSeasons, progressEpisodes: watchedCount };
    });
  }

  function handleAcceptCase(caseId: string): void {
    reconciliationCases = reconciliationCases.filter((c) => c.id !== caseId);
  }

  function handleRejectCase(caseId: string): void {
    reconciliationCases = reconciliationCases.filter((c) => c.id !== caseId);
  }

  function handleDeferCase(caseId: string): void {
    reconciliationCases = reconciliationCases.map((c) =>
      c.id === caseId ? { ...c, status: "deferred" } : c,
    );
  }
</script>

<div class="workbench-layout">
  <NavSidebar
    {activeSection}
    {openReviewCount}
    onSelectSection={handleSelectSection}
  />

  <main id="main-content" class="workbench-viewport" tabindex="-1">
    {#if activeSection === "chronicle"}
      <ChronicleView
        occurrences={chronicle}
        onSelectRecord={handleSelectRecord}
      />
    {:else if activeSection === "library"}
      <LibraryView {records} onSelectRecord={handleSelectRecord} />
    {:else if activeSection === "detail" && selectedRecord}
      <MediaDetailView
        record={selectedRecord}
        onBack={handleBackToLibrary}
        onUpdateStatus={handleUpdateStatus}
        onUpdateRating={handleUpdateRating}
        onToggleEpisode={handleToggleEpisode}
      />
    {:else if activeSection === "reconciliation"}
      <ReconciliationView
        cases={reconciliationCases}
        onAcceptCase={handleAcceptCase}
        onRejectCase={handleRejectCase}
        onDeferCase={handleDeferCase}
      />
    {:else if activeSection === "up_next" || activeSection === "calendar"}
      <CalendarView {watchingRecords} onSelectRecord={handleSelectRecord} />
    {:else if activeSection === "connections"}
      <ConnectionsView />
    {:else if activeSection === "settings"}
      <SettingsView
        customFields={SAMPLE_CUSTOM_FIELDS}
        tokens={SAMPLE_TOKENS}
      />
    {/if}
  </main>
</div>

<style>
  .workbench-layout {
    display: flex;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  .workbench-viewport {
    flex: 1;
    height: 100vh;
    overflow-y: auto;
    background: var(--fasti-surface-archive);
    outline: none;
  }
</style>
