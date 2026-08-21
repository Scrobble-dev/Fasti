export const colors = {
  surface: {
    archive: "#F2EFE6",
    paper: "#FFFDF8",
    night: "#11110F",
  },
  text: {
    primary: "#181716",
    muted: "#625E56",
  },
  brand: {
    mark: "#8B2E2A", // Fasti Oxblood
  },
  action: {
    primary: "#1E4FA3", // Chronicle Blue
  },
  state: {
    verified: "#2E6F63", // Verdigris
    attention: "#8C5A12", // Amber
  },
} as const;

export const typography = {
  display: "Newsreader, Georgia, serif",
  body: "\"Atkinson Hyperlegible Next\", -apple-system, BlinkMacSystemFont, sans-serif",
  mono: "\"Atkinson Hyperlegible Mono\", \"IBM Plex Mono\", monospace",
} as const;

export const touchTargets = {
  minimum: "44px",
} as const;

export const spacing = {
  1: "4px",
  2: "8px",
  3: "12px",
  4: "16px",
  6: "24px",
  8: "32px",
  12: "48px",
  16: "64px",
} as const;

export const radii = {
  sm: "2px",
  md: "6px",
  lg: "10px",
} as const;
