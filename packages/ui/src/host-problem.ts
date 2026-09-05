export function hostProblemText(error: unknown, fallback: string): string {
  if (typeof error === "string") return safeText(error) || fallback;
  if (!error || typeof error !== "object") return fallback;
  const candidate = error as {
    detail?: unknown;
    next_action?: unknown;
    message?: unknown;
  };
  const detail = safeText(candidate.detail);
  const action = safeText(candidate.next_action);
  const structured = [detail, action].filter(Boolean).join(" ");
  if (structured) return structured;
  // The desktop host throws structured DesktopProblem objects (detail/
  // next_action, handled above); the browser host throws plain Errors --
  // without this, every browser-host failure showed only the caller's
  // generic fallback text instead of the specific reason.
  if (error instanceof Error) return safeText(candidate.message) || fallback;
  return fallback;
}

function safeText(value: unknown): string {
  return typeof value === "string" &&
    value.length <= 1_000 &&
    !value.match(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/)
    ? value.trim()
    : "";
}
