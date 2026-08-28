export { default as NetworkSettingsPanel } from "./network-settings-panel.svelte";
export { default as StatusPanel } from "./status-panel.svelte";
export type { StatusPanelState, StatusProblem } from "./status-types.js";
export { default as SetupPanel } from "./setup-panel.svelte";
export { default as NavSidebar } from "./nav-sidebar.svelte";
export { default as TablerThemeDrawer } from "./tabler-theme-drawer.svelte";
// ChronicleView is deliberately NOT wired into the workbench nav. There is no
// application port or Tauri command that lists occurrence/Chronicle history
// yet, so restoring it would mean presenting fake data again. It stays
// exported (and importable) so it isn't silently dead, but parked pending a
// real occurrence-listing backend.
export { default as ChronicleView } from "./chronicle-view.svelte";
export { default as DiscoverView } from "./discover-view.svelte";
export { default as LibraryView } from "./library-view.svelte";
export { default as MediaDetailView } from "./media-detail-view.svelte";
export { default as ReconciliationView } from "./reconciliation-view.svelte";
export { default as CalendarView } from "./calendar-view.svelte";
export { default as ConnectionsView } from "./connections-view.svelte";
// RuntimeSettingsView is the only exported Settings composition. The older
// settings-view.svelte is preserved as migration reference, but it predates
// the host capability boundary and must not return as a second product path.
export { default as RuntimeSettingsView } from "./runtime-settings-view.svelte";
export { default as AuthModal } from "./auth-modal.svelte";
export { default as FastiWorkbench } from "./fasti-workbench.svelte";
export { projectRecordSummary } from "./record-projection.js";

export * from "./types.js";
export * from "./integration-status.js";
export * from "./setup-types.js";
export * from "./defaults.js";
