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
if (
  !gitGrep([
    "-nF",
    "FASTI_BRAND_SVG_EXCEPTION",
    "--",
    "packages/ui/src/tmdb-attribution.svelte",
  ]) ||
  gitGrep([
    "-n",
    "-o",
    "-F",
    "<svg",
    "--",
    "packages/ui/src/tmdb-attribution.svelte",
  ])
    .split("\n")
    .filter(Boolean).length !== 1
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
