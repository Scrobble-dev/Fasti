import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const failures = [];

function gitGrep(arguments_) {
  const result = spawnSync("git", ["grep", ...arguments_], {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0 && result.status !== 1) {
    throw new Error(result.stderr.trim() || "git grep failed");
  }
  return result.status === 0 ? result.stdout.trim() : "";
}

function requireText(file, text, requirement) {
  if (!gitGrep(["-nF", text, "--", file])) {
    failures.push(`${file}: ${requirement}; expected ${JSON.stringify(text)}`);
  }
}

function forbidText(file, text, replacement) {
  if (gitGrep(["-nF", text, "--", file])) {
    failures.push(`${file}: remove ${JSON.stringify(text)}; ${replacement}`);
  }
}

const sourcePaths = [
  ":(glob)apps/web/**/*.js",
  ":(glob)apps/web/**/*.ts",
  ":(glob)apps/web/**/*.svelte",
  ":(glob)packages/ui/**/*.js",
  ":(glob)packages/ui/**/*.ts",
  ":(glob)packages/ui/**/*.svelte",
];
const trackedSources = spawnSync("git", ["ls-files", "--", ...sourcePaths], {
  cwd: root,
  encoding: "utf8",
  stdio: ["ignore", "pipe", "pipe"],
});
if (trackedSources.status !== 0) {
  throw new Error(trackedSources.stderr.trim() || "git ls-files failed");
}
if (!trackedSources.stdout.trim()) {
  failures.push("UI policy source pathspecs must match tracked source files");
}
const untrackedSources = spawnSync(
  "git",
  ["ls-files", "--others", "--exclude-standard", "--", ...sourcePaths],
  {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  },
);
if (untrackedSources.status !== 0) {
  throw new Error(untrackedSources.stderr.trim() || "git ls-files failed");
}
if (untrackedSources.stdout.trim()) {
  failures.push(
    `Stage new UI source files before running the policy gate:\n${untrackedSources.stdout.trim()}`,
  );
}
const forbiddenIcons = gitGrep([
  "-nEI",
  "lucide|heroicons|phosphor|fontawesome|react-icons|material-icons|iconify|bootstrap-icons|radix-icons|@fortawesome|@ant-design/icons|feather-icons",
  "--",
  ...sourcePaths,
]);
if (forbiddenIcons) {
  failures.push("Use Tabler icons before another icon package");
}

// eslint-disable-next-line xss/no-mixed-html -- static repository search; this script does not generate HTML
const rawSvg = gitGrep([
  "-nF",
  "<svg",
  "--",
  ...sourcePaths,
  ":(exclude)packages/ui/src/nav-sidebar.svelte",
  ":(exclude)packages/ui/src/tmdb-attribution.svelte",
]);
if (rawSvg) {
  failures.push("Raw SVG needs a documented brand-only exception");
}
// eslint-disable-next-line xss/no-mixed-html -- static repository search; this script does not generate HTML
const brandSvg = gitGrep([
  "-n",
  "-o",
  "-F",
  "<svg",
  "--",
  "packages/ui/src/nav-sidebar.svelte",
]);
if (brandSvg.split("\n").filter(Boolean).length !== 1) {
  failures.push(
    "packages/ui/src/nav-sidebar.svelte must contain exactly one documented brand SVG",
  );
}
// eslint-disable-next-line xss/no-mixed-html -- static repository search; this script does not generate HTML
const tmdbBrandSvg = gitGrep([
  "-n",
  "-o",
  "-F",
  "<svg",
  "--",
  "packages/ui/src/tmdb-attribution.svelte",
]);
if (
  !gitGrep([
    "-nF",
    "FASTI_BRAND_SVG_EXCEPTION",
    "--",
    "packages/ui/src/tmdb-attribution.svelte",
  ]) ||
  tmdbBrandSvg.split("\n").filter(Boolean).length !== 1
) {
  failures.push(
    "packages/ui/src/tmdb-attribution.svelte must contain one documented provider-brand SVG",
  );
}

const hardCodedLightForeground = gitGrep([
  "-nEI",
  "color:[[:space:]]*(white|#fff(fff)?)[[:space:]]*;",
  "--",
  ...sourcePaths,
]);
if (hardCodedLightForeground) {
  failures.push(
    "Use a governed contrast token instead of a hard-coded light foreground",
  );
}

if (
  !gitGrep([
    "-nF",
    "@tabler/core/dist/css/tabler.min.css",
    "--",
    "apps/web/src/App.svelte",
  ])
) {
  failures.push("apps/web/src/App.svelte: Tabler Core stylesheet is required");
}
if (
  !gitGrep([
    "-nF",
    "@tabler/core/dist/css/tabler-themes.min.css",
    "--",
    "apps/web/src/App.svelte",
  ])
) {
  failures.push("apps/web/src/App.svelte: Tabler theme stylesheet is required");
}

const accentFocus = gitGrep([
  "-nF",
  "outline: 3px solid var(--fasti-action-primary)",
  "--",
  ...sourcePaths,
]);
if (accentFocus) {
  failures.push(
    "Focus indicators must use --fasti-focus so selectable accents cannot reduce contrast",
  );
}

const unscaledRadius = gitGrep([
  "-nEI",
  "border-radius:[[:space:]]*(1|2|3|4|5|6|7|8|9|10|11|12)px[[:space:]]*;",
  "--",
  ...sourcePaths,
]);
if (unscaledRadius) {
  failures.push(
    "Finite component radii must use --tblr-border-radius-scale so Theme settings remain truthful",
  );
}

requireText(
  "packages/ui/src/fasti-workbench.svelte",
  'class="page workbench-shell"',
  "the Workbench root must use Tabler's page primitive",
);
requireText(
  "packages/ui/src/fasti-workbench.svelte",
  "page-wrapper workbench-main-shell",
  "the main shell must be the vertical navbar's adjacent Tabler page-wrapper",
);
requireText(
  "packages/ui/src/nav-sidebar.svelte",
  "navbar navbar-vertical navbar-expand-lg offcanvas-lg offcanvas-start",
  "navigation must use Tabler's responsive vertical-navbar/offcanvas pattern",
);
requireText(
  "packages/ui/src/tabler-theme-drawer.svelte",
  "offcanvas offcanvas-end",
  "theme settings must use Tabler's end offcanvas",
);
requireText(
  "packages/ui/src/runtime-settings-view.svelte",
  "settings-container container-fluid",
  "operational Settings must use the remaining fluid canvas",
);
requireText(
  "packages/ui/src/runtime-settings-view.svelte",
  "list-group-item list-group-item-action",
  "wide Settings navigation must use Tabler list-group links",
);
requireText(
  "packages/ui/src/runtime-settings-view.svelte",
  'class="form-select"',
  "constrained Settings navigation must remain explicitly discoverable",
);

forbidText(
  "packages/ui/src/fasti-workbench.svelte",
  "margin-left: 64px",
  "let the Tabler offcanvas leave the narrow page at full width",
);
forbidText(
  "packages/ui/src/nav-sidebar.svelte",
  "position: fixed !important",
  "use Tabler's responsive navbar/offcanvas positioning",
);
forbidText(
  "packages/ui/src/runtime-settings-view.svelte",
  "max-width: 1080px",
  "keep the application canvas fluid and apply reading measures locally",
);
forbidText(
  "packages/ui/src/tabler-theme-drawer.svelte",
  "transition: all",
  "transition only the property that communicates state",
);
forbidText(
  "packages/ui/src/runtime-settings-view.svelte",
  "aria-pressed={active ===",
  "URL-backed Settings destinations must be links with aria-current",
);
forbidText(
  "packages/ui/src/runtime-settings-view.svelte",
  "activeTab !== active",
  "controlled and uncontrolled Settings tabs must not reset local selection",
);
forbidText(
  "packages/ui/src/runtime-settings-view.svelte",
  "if (host.searchProvider)",
  "required trusted-host provider execution must fail closed",
);
forbidText(
  "packages/ui/src/runtime-settings-view.svelte",
  "image.tmdb.org/t/p/w500/sample",
  "do not persist placeholder artwork URLs",
);
for (const file of [
  "packages/ui/src/library-view.svelte",
  "packages/ui/src/poster-card.svelte",
]) {
  forbidText(
    file,
    "function calculateProgress",
    "use the shared bounded record progress projection",
  );
}
forbidText(
  "packages/ui/src/library-view.svelte",
  "{:else if rec.userRating}",
  "show rating and progress as independent record facts",
);
forbidText(
  "apps/web/src/web-host.ts",
  "await fetch(",
  "route provider requests through the governed trusted host, never the browser host",
);
forbidText(
  "apps/web/src/web-host.ts",
  "fasti-provider-credentials",
  "never place provider credentials in browser storage",
);
requireText(
  "apps/web/src/web-host.ts",
  "The browser host never accepts or stores provider secrets.",
  "the browser host must preserve the provider credential trust boundary",
);

for (const rule of [
  [
    "FASTI_TABLER_POLICY_START",
    "FASTI_TABLER_POLICY_END",
    "Tabler-First Policy",
  ],
  [
    "FASTI_CHESTERTON_POLICY_START",
    "FASTI_CHESTERTON_POLICY_END",
    "Chesterton's fence",
  ],
  [
    "FASTI_AUTH_BOUNDARY_START",
    "FASTI_AUTH_BOUNDARY_END",
    "Authentication boundary",
  ],
]) {
  for (const requiredText of rule) {
    if (!gitGrep(["-nF", requiredText, "--", "AGENTS.md"])) {
      failures.push(`AGENTS.md: missing ${requiredText}`);
    }
  }
}

if (
  !gitGrep([
    "-nF",
    'from "./runtime-settings-view.svelte"',
    "--",
    "packages/ui/src/index.ts",
  ])
) {
  failures.push(
    "packages/ui/src/index.ts: RuntimeSettingsView export is required",
  );
}
if (
  gitGrep([
    "-nF",
    'from "./settings-view.svelte"',
    "--",
    "packages/ui/src/index.ts",
  ])
) {
  failures.push(
    "packages/ui/src/index.ts: legacy SettingsView must stay outside the product API",
  );
}

if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exitCode = 1;
} else {
  console.log(
    "PASS: Tabler-first UI policy and permanent agent rules are present",
  );
}
