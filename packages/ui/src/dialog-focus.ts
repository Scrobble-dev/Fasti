/** Svelte action for a native `<dialog>` opened via `showModal()`.
 *
 * A modal `<dialog>` already traps Tab and closes on Escape natively (the
 * `cancel` event) -- this only adds what the platform doesn't do on its own:
 * focus the first focusable control when the dialog becomes visible, and
 * restore focus to whatever was focused before it opened, once it closes.
 *
 * Reacts to the dialog's own `open` attribute rather than to when the host
 * component happens to call `showModal()`/`close()`, so it works regardless
 * of the surrounding component's effect ordering. Also restores focus on
 * `destroy()`, not just on the native `close` event -- a dialog whose host
 * component is conditionally mounted (`{#if show}<Modal .../>{/if}`) is
 * removed from the DOM directly and never fires `close` at all. */
export function dialogFocus(node: HTMLDialogElement): { destroy(): void } {
  // Capture the opener synchronously when the action is attached, before
  // any showModal() call shifts focus into the dialog.
  let previouslyFocused: HTMLElement | null =
    document.activeElement as HTMLElement | null;

  function focusFirst(): void {
    const target = node.querySelector<HTMLElement>(
      'input, select, textarea, button:not([disabled]), [href], [tabindex]:not([tabindex="-1"])',
    );
    (target ?? node).focus();
  }

  function handleOpened(): void {
    // Don't overwrite the stored opener with a dialog descendant -- the
    // opener was captured synchronously at attachment time.
    focusFirst();
  }

  function handleClosed(): void {
    previouslyFocused?.focus();
    previouslyFocused = null;
  }

  node.addEventListener("close", handleClosed);

  if (node.open) handleOpened();
  const observer = new MutationObserver(() => {
    if (node.open) handleOpened();
  });
  observer.observe(node, { attributes: true, attributeFilter: ["open"] });

  return {
    destroy() {
      node.removeEventListener("close", handleClosed);
      observer.disconnect();
      // Restore focus when destroyed without a close event (e.g. component
      // unmounted), reusing the stored opener.
      if (previouslyFocused) {
        previouslyFocused.focus();
        previouslyFocused = null;
      }
    },
  };
}
