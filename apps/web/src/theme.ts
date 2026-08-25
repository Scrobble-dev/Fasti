export type Theme = "light" | "dark";

const STORAGE_KEY = "fasti-theme";

export const themeBootstrapScript = `(() => {
  let stored;
  try { stored = localStorage.getItem(${JSON.stringify(STORAGE_KEY)}); } catch {}
  const theme = stored === "light" || stored === "dark"
    ? stored
    : matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  document.documentElement.dataset.bsTheme = theme;
  document.documentElement.style.colorScheme = theme;
})();`;

export function resolveTheme(): Theme {
  const active = document.documentElement.dataset.bsTheme;
  if (active === "light" || active === "dark") return active;
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export function applyTheme(theme: Theme, persist = true): void {
  document.documentElement.dataset.bsTheme = theme;
  document.documentElement.style.colorScheme = theme;
  if (!persist) return;
  try {
    window.localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // The applied theme remains valid for this session when storage is blocked.
  }
}
