import { spawnSync } from "node:child_process";

const failures = [];

function gitGrep(arguments_) {
  const result = spawnSync("git", ["grep", ...arguments_], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0 && result.status !== 1) {
    throw new Error(result.stderr.trim() || "git grep failed");
  }
  return result.status === 0 ? result.stdout.trim() : "";
}

const sourcePaths = [
  ":(glob)apps/web/**/*.{js,ts,svelte}",
  ":(glob)packages/ui/**/*.{js,ts,svelte}",
];
const forbiddenIcons = gitGrep([
  "-nEI",
  "lucide|heroicons|phosphor|fontawesome|react-icons|material-icons|iconify",
  "--",
  ...sourcePaths,
]);
if (forbiddenIcons) {
  failures.push(
    `Use Tabler icons before another icon package:\n${forbiddenIcons}`,
  );
}

const rawSvg = gitGrep([
  "-nF",
  "<svg",
  "--",
  ...sourcePaths,
  ":(exclude)packages/ui/src/nav-sidebar.svelte",
]);
if (rawSvg) {
  failures.push(`Raw SVG needs a documented brand-only exception:\n${rawSvg}`);
}

const hardCodedSemanticContrast = gitGrep([
  "-nEI",
  "background:[^;]*var\\(--fasti-(action-primary|brand-mark|state-verified)\\)[^;]*;.*color:[[:space:]]*(white|#fff|#ffffff)",
  "--",
  ...sourcePaths,
]);
if (hardCodedSemanticContrast) {
  failures.push(
    `Semantic backgrounds need their matching contrast token:\n${hardCodedSemanticContrast}`,
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
    `Use a governed contrast token instead of a hard-coded light foreground:\n${hardCodedLightForeground}`,
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
  "FASTI_TABLER_POLICY_START",
  "FASTI_CHESTERTON_POLICY_START",
  "FASTI_AUTH_BOUNDARY_START",
]) {
  if (!gitGrep(["-nF", rule, "--", "AGENTS.md"])) {
    failures.push(`AGENTS.md: missing ${rule}`);
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
