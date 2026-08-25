import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const expectedPackages = new Map([
  ["sdk", "@fasti/sdk"],
  ["schemas", "@fasti/schemas"],
  ["tokens", "@fasti/tokens"],
  ["ui", "@fasti/ui"],
]);
const buildablePackages = new Set(["sdk", "tokens", "ui"]);
const failures = [];

function packageEntryPath(packageRoot, entry) {
  if (typeof entry !== "string" || isAbsolute(entry)) {
    return undefined;
  }
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

assert.equal(packageEntryPath(repoRoot, "/outside"), undefined);
assert.equal(packageEntryPath(repoRoot, "../outside"), undefined);
assert.equal(packageEntryPath(repoRoot, "."), undefined);
assert.equal(
  packageEntryPath(repoRoot, "package.json"),
  join(repoRoot, "package.json"),
);

const actualDirectories = readdirSync(join(repoRoot, "packages"), {
  withFileTypes: true,
})
  .filter(
    (entry) =>
      entry.isDirectory() &&
      existsSync(join(repoRoot, "packages", entry.name, "package.json")),
  )
  .map((entry) => entry.name)
  .sort();

if (
  actualDirectories.join(",") !== [...expectedPackages.keys()].sort().join(",")
) {
  failures.push(
    `package inventory drift: expected ${[...expectedPackages.keys()].sort().join(",")}; found ${actualDirectories.join(",")}`,
  );
}

for (const [directory, expectedName] of expectedPackages) {
  const packageRoot = join(repoRoot, "packages", directory);
  const manifestPath = join(packageRoot, "package.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));

  if (manifest.name !== expectedName) {
    failures.push(`${directory}: expected package name ${expectedName}`);
  }
  if (manifest.private !== true) {
    failures.push(`${directory}: package must remain private before B8`);
  }

  if (!buildablePackages.has(directory)) {
    continue;
  }

  for (const script of ["build", "typecheck"]) {
    if (typeof manifest.scripts?.[script] !== "string") {
      failures.push(`${directory}: missing required ${script} script`);
    }
  }

  const primaryEntry = manifest.main ?? manifest.svelte;
  const primaryEntryPath = packageEntryPath(packageRoot, primaryEntry);
  if (primaryEntryPath === undefined || !existsSync(primaryEntryPath)) {
    failures.push(
      `${directory}: primary entrypoint does not resolve to an existing file`,
    );
  }

  const typesPath = packageEntryPath(packageRoot, manifest.types);
  if (typesPath === undefined || !existsSync(typesPath)) {
    failures.push(`${directory}: types does not resolve to an existing file`);
  }

  const modulePath = packageEntryPath(packageRoot, manifest.module);
  if (
    manifest.module &&
    (modulePath === undefined || !existsSync(modulePath))
  ) {
    failures.push(
      `${directory}: module does not resolve to a built entrypoint`,
    );
  }
}

if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exit(1);
}

console.log(
  "PASS: retained JavaScript package inventory, scripts, privacy, and entrypoints are strict",
);
