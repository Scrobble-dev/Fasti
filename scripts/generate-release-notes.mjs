#!/usr/bin/env node
// Extracts one tag's section from CHANGELOG.md (Keep a Changelog format)
// into a standalone release-notes file. Reads and writes local files only;
// never creates a GitHub Release -- that remains B8's own release action.

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const SEMVER_TAG_PATTERN =
  /^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/;

export function extractReleaseNotes(changelog, tag) {
  if (typeof tag !== "string" || !SEMVER_TAG_PATTERN.test(tag)) {
    throw new Error(
      `tag must be a semantic version like "v1.2.3", got: ${JSON.stringify(tag)}`,
    );
  }
  const version = tag.replace(/^v/, "").replace(/\+.*$/, "");
  const escapedVersion = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const headingPattern = new RegExp(
    `^##\\s*\\[${escapedVersion}\\][^\\n]*\\n`,
    "m",
  );
  const startMatch = headingPattern.exec(changelog);
  if (!startMatch) {
    throw new Error(
      `CHANGELOG.md has no "## [${version}]" section; rename [Unreleased] to [${version}] before tagging`,
    );
  }
  const start = startMatch.index + startMatch[0].length;
  const rest = changelog.slice(start);
  const nextHeadingMatch = /^##\s*\[/m.exec(rest);
  const section = nextHeadingMatch
    ? rest.slice(0, nextHeadingMatch.index)
    : rest;
  return section.trim() + "\n";
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const tag = process.argv[2];
  if (!tag) {
    console.error("usage: generate-release-notes.mjs <vX.Y.Z>");
    process.exit(1);
  }
  const changelog = await readFile(
    resolve(repositoryRoot, "CHANGELOG.md"),
    "utf8",
  );
  const notes = extractReleaseNotes(changelog, tag);
  const outputDir = resolve(repositoryRoot, "target/fasti-evidence/b8b");
  await mkdir(outputDir, { recursive: true });
  await writeFile(resolve(outputDir, "release-notes.md"), notes);
  console.log(`PASS: release notes extracted for ${tag}`);
}
