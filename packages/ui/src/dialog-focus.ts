/** Svelte action for a native `<dialog>` opened via `showModal()`.
 *
 * A modal `<dialog>` already traps Tab and closes on Escape natively (the
 * `cancel` event) -- this only adds what the platform doesn't do on its own:
 * focus the first focusable control when the dialog becomes visible, and
 * restore focus to whatever was focused before it opened, once it closes.
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

  function focusFirst(): void {
    const focusable = node.querySelectorAll<HTMLElement>(
      'input, select, textarea, button:not([disabled]), [href], [tabindex]:not([tabindex="-1"])',
    );
    // Both current dialogs put their dismiss button first in DOM order (it's
    // in the header, above the actual content). Defaulting focus there risks
    // an accidental Enter/Space closing the dialog the user just opened --
    // WAI-ARIA's dialog pattern recommends the first control that isn't the
    // close action. Falls back to it only if nothing else is focusable.
    const first =
      Array.from(focusable).find(
        (element) => !/close/i.test(element.getAttribute("aria-label") ?? ""),
      ) ?? focusable[0];
    (first ?? node).focus();
  }

  const originalShowModal = node.showModal.bind(node);
  node.showModal = () => {
    previouslyFocused = document.activeElement as HTMLElement | null;
    originalShowModal();
    focusFirst();
  };

  function handleClosed(): void {
    previouslyFocused?.focus();
    previouslyFocused = null;
  }

  node.addEventListener("close", handleClosed);

  return {
    destroy() {
      node.removeEventListener("close", handleClosed);
      node.showModal = originalShowModal;
      // Restore focus when destroyed without a close event (e.g. the host
      // component was conditionally mounted and got torn down directly).
      handleClosed();
    },
  };
}
