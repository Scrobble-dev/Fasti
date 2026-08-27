import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const uiRoots = ["apps/web", "packages/ui"];
const sourceExtensions = new Set([".svelte", ".ts", ".js"]);
const ignoredDirectories = new Set([
  "node_modules",
  "dist",
  "build",
  ".svelte-kit",
]);
const forbiddenIconPackages = [
  "lucide",
  "heroicons",
  "phosphor",
  "fontawesome",
  "react-icons",
  "material-icons",
  "iconify",
];
const inlineSvgExceptions = new Set(["packages/ui/src/nav-sidebar.svelte"]);
const semanticBackgrounds = ["action-primary", "brand-mark", "state-verified"];
const failures = [];

async function sourceFiles(relativeDirectory) {
  const directory = path.join(root, relativeDirectory);
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    if (entry.isDirectory() && ignoredDirectories.has(entry.name)) continue;
    const relativePath = path.join(relativeDirectory, entry.name);
    if (entry.isDirectory()) files.push(...(await sourceFiles(relativePath)));
    else if (sourceExtensions.has(path.extname(entry.name)))
      files.push(relativePath);
  }
  return files;
}

for (const uiRoot of uiRoots) {
  for (const file of await sourceFiles(uiRoot)) {
    const source = await readFile(path.join(root, file), "utf8");
    for (const packageName of forbiddenIconPackages) {
      if (source.includes(packageName)) {
        failures.push(`${file}: use Tabler icons before ${packageName}`);
      }
    }
    if (source.includes("<svg") && !inlineSvgExceptions.has(file)) {
      failures.push(`${file}: raw SVG needs a documented brand-only exception`);
    }
    for (const background of semanticBackgrounds) {
      const hardCodedForeground = new RegExp(
        `\\{[^{}]*background:\\s*var\\(--fasti-${background}\\);[^{}]*color:\\s*(?:white|#fff(?:fff)?);[^{}]*\\}`,
        "i",
      );
      if (hardCodedForeground.test(source)) {
        failures.push(
          `${file}: --fasti-${background} needs its semantic contrast token`,
        );
      }
    }
  }
}

const appSource = await readFile(
  path.join(root, "apps/web/src/App.svelte"),
  "utf8",
);
if (!appSource.includes("@tabler/core/dist/css/tabler.min.css")) {
  failures.push("apps/web/src/App.svelte: Tabler Core stylesheet is required");
}

const agentRules = await readFile(path.join(root, "AGENTS.md"), "utf8");
for (const rule of [
  "FASTI_TABLER_POLICY_START",
  "FASTI_CHESTERTON_POLICY_START",
  "FASTI_AUTH_BOUNDARY_START",
]) {
  if (!agentRules.includes(rule)) failures.push(`AGENTS.md: missing ${rule}`);
}

if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exitCode = 1;
} else {
  console.log(
    "PASS: Tabler-first UI policy and permanent agent rules are present",
  );
}
