export function hostProblemText(error: unknown, fallback: string): string {
  if (typeof error === "string") return safeText(error) || fallback;
  if (!error || typeof error !== "object") return fallback;
  const candidate = error as { detail?: unknown; next_action?: unknown };
  const detail = safeText(candidate.detail);
  const action = safeText(candidate.next_action);
  return [detail, action].filter(Boolean).join(" ") || fallback;
}

function safeText(value: unknown): string {
  return typeof value === "string" &&
    value.length <= 1_000 &&
    !value.match(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/)
    ? value.trim()
    : "";
}
