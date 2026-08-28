export type Theme = "light" | "dark";

const STORAGE_KEY = "fasti-theme";
const SETTINGS_STORAGE_KEY = "fasti-theme-settings";

export const themeBootstrapScript = `(() => {
  let legacy, settings;
  try {
    legacy = localStorage.getItem(${JSON.stringify(STORAGE_KEY)});
    settings = JSON.parse(localStorage.getItem(${JSON.stringify(SETTINGS_STORAGE_KEY)}) || "null");
  } catch {}
  const mode = settings?.mode === "light" || settings?.mode === "dark" || settings?.mode === "night"
    ? settings.mode
    : legacy === "light" || legacy === "dark"
      ? legacy
    : matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  document.documentElement.dataset.bsTheme = mode === "light" ? "light" : "dark";
  document.documentElement.dataset.fastiTheme = mode;
  if (settings?.themeBase) document.documentElement.dataset.bsThemeBase = settings.themeBase;
  if (settings?.fontFamily) document.documentElement.dataset.bsThemeFont = settings.fontFamily;
  if (settings?.cornerRadius !== undefined) document.documentElement.dataset.bsThemeRadius = String(settings.cornerRadius);
  document.documentElement.style.colorScheme = mode === "light" ? "light" : "dark";
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
  document.documentElement.dataset.fastiTheme = theme;
  document.documentElement.style.colorScheme = theme;
  if (!persist) return;
  try {
    window.localStorage.setItem(STORAGE_KEY, theme);
    const stored = JSON.parse(
      window.localStorage.getItem(SETTINGS_STORAGE_KEY) ?? "{}",
    ) as Record<string, unknown>;
    window.localStorage.setItem(
      SETTINGS_STORAGE_KEY,
      JSON.stringify({ ...stored, mode: theme }),
    );
  } catch {
    // The applied theme remains valid for this session when storage is blocked.
  }
}
