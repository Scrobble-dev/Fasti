import assert from "node:assert/strict";
import { existsSync, globSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const expectedWorkspaces = new Map([
  ["apps/web", "@fasti/web"],
  ["packages/sdk", "@fasti/sdk"],
  ["packages/schemas", "@fasti/schemas"],
  ["packages/tokens", "@fasti/tokens"],
  ["packages/ui", "@fasti/ui"],
]);
const buildableWorkspaces = new Set([
  "apps/web",
  "packages/sdk",
  "packages/tokens",
  "packages/ui",
]);
const entrypointWorkspaces = new Set([
  "packages/sdk",
  "packages/tokens",
  "packages/ui",
]);
const failures = [];

function packageEntryPath(packageRoot, entry) {
  if (typeof entry !== "string" || isAbsolute(entry)) return undefined;
  const path = resolve(packageRoot, entry);
  const fromRoot = relative(packageRoot, path);
  if (
    fromRoot === "" ||
    fromRoot === ".." ||
    fromRoot.startsWith(`..${sep}`) ||
    isAbsolute(fromRoot)
  ) {
    return undefined;
  }
  return path;
}

function packageEntryExists(packageRoot, entry) {
  const entryPath = packageEntryPath(packageRoot, entry);
  return entryPath !== undefined && existsSync(entryPath);
}

assert.equal(packageEntryPath(repoRoot, "/outside"), undefined);
assert.equal(packageEntryPath(repoRoot, "../outside"), undefined);
assert.equal(packageEntryPath(repoRoot, "."), undefined);
assert.equal(
  packageEntryPath(repoRoot, "package.json"),
  join(repoRoot, "package.json"),
);

const actualWorkspaces = globSync("{apps,packages}/*/package.json", {
  cwd: repoRoot,
})
  .map((manifestPath) => dirname(manifestPath))
  .sort();

if (
  actualWorkspaces.join(",") !== [...expectedWorkspaces.keys()].sort().join(",")
) {
  failures.push(
    `workspace inventory drift: expected ${[...expectedWorkspaces.keys()].sort().join(",")}; found ${actualWorkspaces.join(",")}`,
  );
}

for (const [directory, expectedName] of expectedWorkspaces) {
  const packageRoot = join(repoRoot, directory);
  const manifestPath = join(packageRoot, "package.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));

  if (manifest.name !== expectedName) {
    failures.push(`${directory}: expected package name ${expectedName}`);
  }
  if (manifest.private !== true) {
    failures.push(`${directory}: package must remain private before B8`);
  }

  if (!buildableWorkspaces.has(directory)) {
    continue;
  }

  for (const script of ["build", "typecheck"]) {
    if (typeof manifest.scripts?.[script] !== "string") {
      failures.push(`${directory}: missing required ${script} script`);
    }
  }

  if (!entrypointWorkspaces.has(directory)) continue;

  for (const [field, target] of [
    ["primary", manifest.main ?? manifest.svelte],
    ["types", manifest.types],
  ]) {
    if (!packageEntryExists(packageRoot, target)) {
      failures.push(
        `${directory}: ${field} does not resolve to a confined built entrypoint`,
      );
    }
  }

  if (manifest.module && !packageEntryExists(packageRoot, manifest.module)) {
    failures.push(
      `${directory}: module does not resolve to a confined built entrypoint`,
    );
  }
}

if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exit(1);
}

console.log(
  "PASS: retained JavaScript workspace inventory, scripts, privacy, and entrypoints are strict",
);
