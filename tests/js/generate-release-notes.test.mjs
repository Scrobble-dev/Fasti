import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { extractReleaseNotes } from "../../scripts/generate-release-notes.mjs";

const repositoryRoot = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../..",
);
const scriptPath = resolve(
  repositoryRoot,
  "scripts/generate-release-notes.mjs",
);

const fixtureChangelog = `# Changelog

## [Unreleased]

### Added

* Something not yet released.

## [0.2.0] - 2026-08-20

### Added

* Second release feature.

### Fixed

* Second release fix.

## [0.1.0] - 2026-08-01

### Added

* First release feature.
`;

test("extracts exactly the requested version's section", () => {
  const notes = extractReleaseNotes(fixtureChangelog, "v0.2.0");
  assert.match(notes, /Second release feature\./);
  assert.match(notes, /Second release fix\./);
  assert.doesNotMatch(notes, /First release feature\./);
  assert.doesNotMatch(notes, /Something not yet released\./);
});

test("extracts the oldest section without bleeding into a next one that doesn't exist", () => {
  const notes = extractReleaseNotes(fixtureChangelog, "v0.1.0");
  assert.match(notes, /First release feature\./);
  assert.doesNotMatch(notes, /Second release/);
});

test("accepts a tag without the leading v", () => {
  const notes = extractReleaseNotes(fixtureChangelog, "0.2.0");
  assert.match(notes, /Second release feature\./);
});

test("errors on a version with no matching section", () => {
  assert.throws(
    () => extractReleaseNotes(fixtureChangelog, "v9.9.9"),
    /9\.9\.9/,
  );
});

test("rejects a tag that isn't a semantic version before it ever reaches RegExp", () => {
  assert.throws(
    () => extractReleaseNotes(fixtureChangelog, "v1.0.0)(.*"),
    /must be a semantic version/,
  );
});

test("accepts a tag with build metadata", () => {
  const notes = extractReleaseNotes(fixtureChangelog, "v0.2.0+build.5");
  assert.match(notes, /Second release feature\./);
  assert.match(notes, /Second release fix\./);
});

test("rejects a tag with a leading zero in the major version", () => {
  assert.throws(
    () => extractReleaseNotes(fixtureChangelog, "v01.2.3"),
    /must be a semantic version/,
  );
});

test("rejects a tag with a leading zero in a numeric prerelease identifier", () => {
  assert.throws(
    () => extractReleaseNotes(fixtureChangelog, "v1.2.3-01"),
    /must be a semantic version/,
  );
});

test("CLI entrypoint prints usage and exits nonzero when called without a tag", () => {
  assert.throws(() => execFileSync("node", [scriptPath], { stdio: "pipe" }));
});

test("CLI entrypoint exits nonzero for a tag with no matching CHANGELOG section", () => {
  assert.throws(() =>
    execFileSync("node", [scriptPath, "v9.9.9"], {
      cwd: repositoryRoot,
      stdio: "pipe",
    }),
  );
});
