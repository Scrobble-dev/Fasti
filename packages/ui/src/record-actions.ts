import IconAdjustments from "@tabler/icons-svelte/icons/adjustments";
import IconBan from "@tabler/icons-svelte/icons/ban";
import IconBookmark from "@tabler/icons-svelte/icons/bookmark";
import IconCircleCheck from "@tabler/icons-svelte/icons/circle-check";
import IconCopy from "@tabler/icons-svelte/icons/copy";
import IconEye from "@tabler/icons-svelte/icons/eye";
import IconFolderPlus from "@tabler/icons-svelte/icons/folder-plus";
import IconHistoryToggle from "@tabler/icons-svelte/icons/history-toggle";
import IconMessage from "@tabler/icons-svelte/icons/message";
import IconPlayerPlay from "@tabler/icons-svelte/icons/player-play";
import IconRefresh from "@tabler/icons-svelte/icons/refresh";
import IconScale from "@tabler/icons-svelte/icons/scale";
import IconTags from "@tabler/icons-svelte/icons/tags";
import { DEFAULT_CONTEXT_MENU_ITEMS } from "./defaults.js";
import type { ContextMenuItem } from "./context-menu.svelte";
import type {
  ContextMenuItemConfig,
  MediaRecord,
  TrackingDispositionUpdate,
} from "./types.js";

export interface RecordActionHandlers {
  onView: () => void;
  onSetTrackingDisposition?: (disposition: TrackingDispositionUpdate) => void;
  onMarkCompleted?: () => void;
  onUpdateProgress?: () => void;
  onToggleWatchlist?: () => void;
  onOpenCollection?: () => void;
  onOpenReview?: () => void;
  onEditTags?: () => void;
  onInspectIds?: () => void;
  onReconcile?: () => void;
  onCopyId?: () => void;
}

function action(
  item: Omit<ContextMenuItem, "action" | "disabled" | "description">,
  handler: (() => void) | undefined,
  unavailable: string,
): ContextMenuItem {
  return {
    ...item,
    action: handler ?? (() => {}),
    disabled: !handler,
    description: handler ? undefined : unavailable,
  };
}

export function recordContextMenuItems(
  record: MediaRecord,
  handlers: RecordActionHandlers,
  configs: ContextMenuItemConfig[] = DEFAULT_CONTEXT_MENU_ITEMS,
): ContextMenuItem[] {
  const tracking = handlers.onSetTrackingDisposition;
  const all: Record<string, ContextMenuItem> = {
    view: action(
      {
        id: "view",
        label: "View media details",
        group: "Playback & tracking",
        icon: IconEye,
      },
      handlers.onView,
      "Record details are unavailable.",
    ),
    tracking_watching: action(
      {
        id: "tracking_watching",
        label: "Mark as in progress",
        group: "Playback & tracking",
        icon: IconPlayerPlay,
      },
      tracking ? () => tracking("watching") : undefined,
      "Profile tracking state is unavailable on this host.",
    ),
    tracking_on_hold: action(
      {
        id: "tracking_on_hold",
        label: "Mark as on hold",
        group: "Playback & tracking",
        icon: IconHistoryToggle,
      },
      tracking ? () => tracking("on_hold") : undefined,
      "Profile tracking state is unavailable on this host.",
    ),
    tracking_dropped: action(
      {
        id: "tracking_dropped",
        label: "Mark as dropped",
        group: "Playback & tracking",
        icon: IconBan,
      },
      tracking ? () => tracking("dropped") : undefined,
      "Profile tracking state is unavailable on this host.",
    ),
    tracking_clear: action(
      {
        id: "tracking_clear",
        label: "Use automatic tracking state",
        group: "Playback & tracking",
        icon: IconRefresh,
      },
      tracking ? () => tracking("unset") : undefined,
      "Profile tracking state is unavailable on this host.",
    ),
    watched: action(
      {
        id: "watched",
        label:
          record.status === "completed"
            ? "Mark as not completed"
            : "Mark as completed",
        group: "Playback & tracking",
        icon: IconCircleCheck,
      },
      handlers.onMarkCompleted,
      "Completion needs Chronicle progress history, which is not active yet.",
    ),
    progress: action(
      {
        id: "progress",
        label: "Update progress and episodes",
        group: "Playback & tracking",
        icon: IconAdjustments,
      },
      handlers.onUpdateProgress,
      "Progress editing is not active on this host.",
    ),
    watchlist: action(
      {
        id: "watchlist",
        label:
          handlers.onToggleWatchlist && record.status === "plan_to_watch"
            ? "Remove from watchlist"
            : "Add to watchlist",
        group: "Library & lists",
        icon: IconBookmark,
      },
      handlers.onToggleWatchlist,
      "Watchlist membership is not active on this host.",
    ),
    collection: action(
      {
        id: "collection",
        label: "Add to collection",
        group: "Library & lists",
        icon: IconFolderPlus,
      },
      handlers.onOpenCollection,
      "Collection membership is not active on this host.",
    ),
    review: action(
      {
        id: "review",
        label: "Rate and review",
        group: "Library & lists",
        icon: IconMessage,
      },
      handlers.onOpenReview,
      "Personal ratings and reviews are not active on this host.",
    ),
    edit_tags: action(
      {
        id: "edit_tags",
        label: "Manage tags",
        group: "Library & lists",
        icon: IconTags,
      },
      handlers.onEditTags,
      "Tag editing is not active on this host.",
    ),
    manage_ids: action(
      {
        id: "manage_ids",
        label: "Inspect external claims and IDs",
        group: "Identity & metadata",
        icon: IconScale,
      },
      handlers.onInspectIds,
      "Identity details are unavailable.",
    ),
    reconcile: action(
      {
        id: "reconcile",
        label: "Open review and reconciliation",
        group: "Identity & metadata",
        icon: IconScale,
      },
      handlers.onReconcile,
      "The review inbox is unavailable.",
    ),
    copy_id: action(
      {
        id: "copy_id",
        label: "Copy Fasti entity ID",
        group: "Identity & metadata",
        icon: IconCopy,
      },
      handlers.onCopyId,
      "Clipboard access is unavailable.",
    ),
  };

  return [...configs]
    .filter((config) => config.visible && all[config.id])
    .sort((left, right) => left.order - right.order)
    .map((config) => all[config.id]);
}
