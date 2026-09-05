/** Svelte action for a native `<dialog>` opened via `showModal()`.
 *
 * Keeps keyboard focus inside the dialog, focuses the first useful control
 * when it opens, and restores focus to the opener when it closes.
 *
 * Wraps `showModal()` itself rather than reacting to the `open` attribute
 * via MutationObserver: the browser's own "dialog focusing steps" run
 * synchronously inside `showModal()`, before it returns, so a
 * MutationObserver callback (always a microtask, queued after that
 * synchronous call finishes) is already too late to capture the real
 * opener -- `document.activeElement` at that point is already a dialog
 * descendant. Capturing inside the wrapper, before calling the original
 * `showModal()`, is the only timing that's actually before native focus
 * moves. This also naturally handles a dialog that opens more than once
 * (an always-mounted host, e.g. a settings drawer) as well as one that
 * mounts fresh per open, since the wrapper re-captures on every call. */
export function dialogFocus(node: HTMLDialogElement): { destroy(): void } {
  let previouslyFocused: HTMLElement | null = null;

  function focusableElements(): HTMLElement[] {
    return Array.from(
      node.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
    ).filter((element) => element.getClientRects().length > 0);
  }

  function focusFirst(): void {
    const focusable = focusableElements();
    // Both current dialogs put their dismiss button first in DOM order (it's
    // in the header, above the actual content). Defaulting focus there risks
    // an accidental Enter/Space closing the dialog the user just opened --
    // WAI-ARIA's dialog pattern recommends the first control that isn't the
    // close action. Falls back to it only if nothing else is focusable.
    const first =
      focusable.find(
        (element) => !/close/i.test(element.getAttribute("aria-label") ?? ""),
      ) ?? focusable[0];
    (first ?? node).focus();
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key !== "Tab") return;
    const focusable = focusableElements();
    if (focusable.length === 0) {
      event.preventDefault();
      node.focus();
      return;
    }
    const active = document.activeElement;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && (active === first || !node.contains(active))) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && (active === last || !node.contains(active))) {
      event.preventDefault();
      first.focus();
    }
  }

  const originalShowModal = node.showModal.bind(node);
  node.showModal = () => {
    previouslyFocused = document.activeElement as HTMLElement | null;
    originalShowModal();
    focusFirst();
  };

  function handleClosed(): void {
    if (previouslyFocused?.isConnected) previouslyFocused.focus();
    previouslyFocused = null;
  }

  node.addEventListener("close", handleClosed);
  node.addEventListener("keydown", handleKeydown);

  return {
    destroy() {
      node.removeEventListener("close", handleClosed);
      node.removeEventListener("keydown", handleKeydown);
      node.showModal = originalShowModal;
      // Restore focus when destroyed without a close event (e.g. the host
      // component was conditionally mounted and got torn down directly).
      handleClosed();
    },
  };
}
