import type {
  ContextMenuItemConfig,
  NavItemConfig,
  ThemeSettings,
  WorkbenchPreferences,
} from "./types.js";

export const DEFAULT_THEME_SETTINGS: ThemeSettings = {
  mode: "light",
  accentColor: "#066fd1",
  fontFamily: "sans-serif",
  themeBase: "slate",
  cornerRadius: 1,
  density: "normal",
  fontSize: "md",
};

/**
 * Only sections with a real, wired backend (or that are pure client-side UI)
 * appear here. Chronicle is deliberately excluded: no backend capability
 * lists occurrence/Chronicle history yet, so it stays parked (see
 * `index.ts`'s barrel export comment) rather than restored with fake data.
 */
export const DEFAULT_NAV_ITEMS: NavItemConfig[] = [
  {
    id: "home",
    label: "Overview",
    category: "primary",
    visible: true,
    pinned: true,
    order: 0,
  },
  {
    id: "discover",
    label: "Discover",
    category: "primary",
    visible: true,
    pinned: true,
    order: 1,
  },
  {
    id: "library",
    label: "Library",
    category: "library",
    visible: true,
    pinned: false,
    order: 2,
  },
  {
    id: "calendar",
    label: "Calendar",
    category: "library",
    visible: true,
    pinned: false,
    order: 3,
  },
  {
    id: "detail",
    label: "Media Detail",
    category: "library",
    visible: true,
    pinned: false,
    order: 4,
  },
  {
    id: "reconciliation",
    label: "Review Inbox",
    category: "utilities",
    visible: true,
    pinned: false,
    order: 5,
  },
  {
    id: "connections",
    label: "Connections",
    category: "utilities",
    visible: true,
    pinned: false,
    order: 6,
  },
  {
    id: "settings",
    label: "Settings",
    category: "utilities",
    visible: true,
    pinned: true,
    order: 7,
  },
];

export const DEFAULT_CONTEXT_MENU_ITEMS: ContextMenuItemConfig[] = [
  { id: "view", label: "View Details", visible: true, order: 0 },
  { id: "watched", label: "Mark as Seen / Unplayed", visible: true, order: 1 },
  {
    id: "progress",
    label: "Update Progress & Episodes",
    visible: true,
    order: 2,
  },
  { id: "watchlist", label: "Watchlist Toggle", visible: true, order: 3 },
  { id: "collection", label: "Add to Collection...", visible: true, order: 4 },
  { id: "review", label: "Rate & Personal Review...", visible: true, order: 5 },
  { id: "edit_tags", label: "Manage Tags...", visible: true, order: 6 },
  {
    id: "manage_ids",
    label: "Inspect External Claim IDs...",
    visible: true,
    order: 7,
  },
  {
    id: "reconcile",
    label: "Review & Reconcile Identity...",
    visible: true,
    order: 8,
  },
];

export function createDefaultWorkbenchPreferences(): WorkbenchPreferences {
  return {
    sidebarCollapsed: false,
    sidebarHidden: false,
    navItems: DEFAULT_NAV_ITEMS.map((item) => ({ ...item })),
    contextMenuItems: DEFAULT_CONTEXT_MENU_ITEMS.map((item) => ({ ...item })),
  };
}
