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
    gold: "#D4AF37", // Horological Gold
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
  --fasti-brand-gold: ${colors.brand.gold};
  --fasti-action-primary: ${colors.action.primary};
  --fasti-state-verified: ${colors.state.verified};
  --fasti-state-attention: ${colors.state.attention};
  --fasti-overlay-contrast: ${colors.surface.paper};
  --fasti-font-display: ${typography.display};
  --fasti-font-body: ${typography.body};
  --fasti-font-mono: ${typography.mono};
  --fasti-touch-target-min: ${touchTargets.minimum};
  --fasti-background: var(--fasti-surface-archive);
  --fasti-panel: var(--fasti-surface-paper);
  --fasti-foreground: var(--fasti-text-primary);
  --fasti-muted: var(--fasti-text-muted);
  --fasti-action: var(--fasti-action-primary);
  --fasti-action-contrast: var(--fasti-surface-paper);
  --fasti-brand-contrast: var(--fasti-surface-paper);
  --fasti-verified-contrast: var(--fasti-surface-paper);
  --fasti-verified: var(--fasti-state-verified);
  --fasti-attention: var(--fasti-state-attention);
  --fasti-border: color-mix(in srgb, var(--fasti-text-muted) 42%, transparent);
  --fasti-focus: var(--fasti-state-attention);
}

[data-bs-theme="dark"] {
  --fasti-background: var(--fasti-surface-night);
  --fasti-panel: color-mix(in srgb, var(--fasti-surface-night) 88%, var(--fasti-surface-paper));
  /* These aliases mix toward the literal near-white paper hex, not
   * var(--fasti-surface-paper) -- that variable is redefined below to a DARK
   * card-background tone for this theme, and since these declarations share
   * its rule, a var() reference to it here would resolve to that same dark
   * value instead of the near-white one every alias below actually wants. */
  --fasti-foreground: var(--fasti-text-primary);
  --fasti-muted: color-mix(in srgb, ${colors.surface.paper} 72%, var(--fasti-surface-night));
  --fasti-action: color-mix(in srgb, var(--fasti-action-primary) 50%, ${colors.surface.paper});
  --fasti-action-contrast: var(--fasti-surface-night);
  --fasti-verified: color-mix(in srgb, var(--fasti-state-verified) 55%, ${colors.surface.paper});
  --fasti-attention: color-mix(in srgb, var(--fasti-state-attention) 45%, ${colors.surface.paper});
  --fasti-border: color-mix(in srgb, ${colors.surface.paper} 35%, transparent);
  --fasti-focus: color-mix(in srgb, var(--fasti-state-attention) 45%, ${colors.surface.paper});

  /* The un-aliased tokens above (--fasti-background, --fasti-panel, etc.)
   * are what the aliases resolve to, but most real components style
   * against the un-aliased surface/text/action/state tokens directly.
   * Without dark values here, those components stayed light-on-light in
   * dark mode. */
  --fasti-surface-archive: ${colors.surface.night};
  --fasti-surface-paper: color-mix(in srgb, ${colors.surface.night} 88%, ${colors.surface.paper});
  --fasti-text-primary: ${colors.surface.paper};
  --fasti-text-muted: color-mix(in srgb, ${colors.surface.paper} 72%, ${colors.surface.night});
  --fasti-brand-mark: color-mix(in srgb, ${colors.brand.mark} 65%, white);
  --fasti-brand-gold: color-mix(in srgb, ${colors.brand.gold} 80%, white);
  --fasti-action-primary: color-mix(in srgb, ${colors.action.primary} 50%, ${colors.surface.paper});
  --fasti-state-verified: color-mix(in srgb, ${colors.state.verified} 55%, ${colors.surface.paper});
  --fasti-state-attention: color-mix(in srgb, ${colors.state.attention} 45%, ${colors.surface.paper});
  --fasti-state-error: color-mix(in srgb, #b42318 55%, ${colors.surface.paper});
  --fasti-brand-contrast: var(--fasti-surface-night);
  --fasti-verified-contrast: var(--fasti-surface-night);
}

[data-bs-theme="dark"][data-fasti-theme="night"] {
  --fasti-background: color-mix(in srgb, var(--fasti-surface-night) 88%, black);
  --fasti-panel: color-mix(in srgb, var(--fasti-surface-night) 94%, ${colors.surface.paper});
  --fasti-surface-archive: color-mix(in srgb, var(--fasti-surface-night) 88%, black);
  --fasti-surface-paper: color-mix(in srgb, var(--fasti-surface-night) 94%, ${colors.surface.paper});
  --fasti-border: color-mix(in srgb, ${colors.surface.paper} 28%, transparent);
}
`;
