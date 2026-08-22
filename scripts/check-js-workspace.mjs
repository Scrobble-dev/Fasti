import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const expectedPackages = new Map([
  ["sdk", "@fasti/sdk"],
  ["schemas", "@fasti/schemas"],
  ["tokens", "@fasti/tokens"],
]);
const buildablePackages = new Set(["sdk", "tokens"]);
const failures = [];

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

  for (const field of ["main", "types"]) {
    const target = manifest[field];
    if (typeof target !== "string" || !existsSync(join(packageRoot, target))) {
      failures.push(
        `${directory}: ${field} does not resolve to a built entrypoint`,
      );
    }
  }

  if (manifest.module && !existsSync(join(packageRoot, manifest.module))) {
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
