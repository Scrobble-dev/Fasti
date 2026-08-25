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
  body: '"Atkinson Hyperlegible Next", -apple-system, BlinkMacSystemFont, sans-serif',
  mono: '"Atkinson Hyperlegible Mono", "IBM Plex Mono", monospace',
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

export const cssVariables = `
:root {
  --fasti-surface-archive: ${colors.surface.archive};
  --fasti-surface-paper: ${colors.surface.paper};
  --fasti-surface-night: ${colors.surface.night};
  --fasti-text-primary: ${colors.text.primary};
  --fasti-text-muted: ${colors.text.muted};
  --fasti-brand-mark: ${colors.brand.mark};
  --fasti-action-primary: ${colors.action.primary};
  --fasti-state-verified: ${colors.state.verified};
  --fasti-state-attention: ${colors.state.attention};
  --fasti-font-display: ${typography.display};
  --fasti-font-body: ${typography.body};
  --fasti-font-mono: ${typography.mono};
  --fasti-touch-target-min: ${touchTargets.minimum};
}
`;
