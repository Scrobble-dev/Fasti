import assert from "node:assert/strict";
import { globSync, readFileSync, realpathSync } from "node:fs";
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

/**
 * Determines whether a path identifies a descendant within a root directory.
 * @param {string} root - The root directory path.
 * @param {string} path - The path to check.
 * @return {boolean} `true` if the path is within the root and differs from the root itself, `false` otherwise.
 */
function isConfinedPath(root, path) {
  const fromRoot = relative(root, path);
  return !(
    fromRoot === "" ||
    fromRoot === ".." ||
    fromRoot.startsWith(`..${sep}`) ||
    isAbsolute(fromRoot)
  );
}

/**
 * Resolves a package entry path and validates it is confined within the package root.
 * @param {string} packageRoot - The package root directory.
 * @param {string} entry - The entry path from package.json.
 * @returns {string|undefined} The resolved entry path, or undefined if invalid.
 */
function packageEntryPath(packageRoot, entry) {
  if (typeof entry !== "string" || isAbsolute(entry)) return undefined;
  const path = resolve(packageRoot, entry);
  return isConfinedPath(packageRoot, path) ? path : undefined;
}

/**
 * Verifies that a package entry exists within the package directory.
 * @param {string} packageRoot - The package root directory.
 * @param {string} entry - The entry path from package metadata.
 * @returns {boolean} `true` if the entry exists and is physically confined, `false` otherwise.
 */
function packageEntryExists(packageRoot, entry) {
  const entryPath = packageEntryPath(packageRoot, entry);
  if (entryPath === undefined) return false;
  try {
    // The root is fixed by expectedWorkspaces and entryPath passed lexical confinement.
    /* eslint-disable security/detect-non-literal-fs-filename */
    const physicalRoot = realpathSync(packageRoot);
    const physicalEntry = realpathSync(entryPath);
    /* eslint-enable security/detect-non-literal-fs-filename */
    return isConfinedPath(physicalRoot, physicalEntry);
  } catch {
    return false;
  }
}

assert.equal(packageEntryPath(repoRoot, "/outside"), undefined);
assert.equal(packageEntryPath(repoRoot, "../outside"), undefined);
assert.equal(packageEntryPath(repoRoot, "."), undefined);
assert.equal(
  packageEntryPath(repoRoot, "package.json"),
  join(repoRoot, "package.json"),
);
// A symlinked entry resolves to its physical target before this check.
assert.equal(
  isConfinedPath(repoRoot, resolve(repoRoot, "../outside/transport.js")),
  false,
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

  if (
    manifest.module !== undefined &&
    !packageEntryExists(packageRoot, manifest.module)
  ) {
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
