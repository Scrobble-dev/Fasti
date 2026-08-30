#!/usr/bin/env node
// Extracts one tag's section from CHANGELOG.md (Keep a Changelog format)
// into a standalone release-notes file. Reads and writes local files only;
// never creates a GitHub Release -- that remains B8's own release action.

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

// The canonical SemVer 2.0.0 grammar (see semver.org's own suggested
// regex): rejects leading zeros in numeric identifiers and separates
// build metadata, which CHANGELOG headings never include. Community-vetted
// as linear-time safe (semver.org tested it explicitly against ReDoS); each
// alternation is evaluated once per dot-separated identifier, with no
// nested quantifier over an ambiguous sub-pattern.
// eslint-disable-next-line security/detect-unsafe-regex
// nosemgrep: generic.regex.security.dos.regex-dos -- vetted linear-time semver.org grammar, see above
const SEMVER_TAG_PATTERN =
  /^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/;
// Fixed pattern, never built from input -- avoids constructing any RegExp
// from tag/version data at all, not just escaping it.
const HEADING_PATTERN = /^##\s*\[([^\]]+)\][^\n]*\n/gm;

/**
 * Extracts the release notes for a semantic version tag from changelog content.
 * @param {string} changelog - The changelog content containing version headings.
 * @param {string} tag - The semantic version tag, optionally prefixed with `v`.
 * @returns {string} The matching release notes section, trimmed and terminated with a newline.
 * @throws {Error} If the tag is invalid or the changelog has no matching version section.
 */
export function extractReleaseNotes(changelog, tag) {
  if (typeof tag !== "string" || !SEMVER_TAG_PATTERN.test(tag)) {
    throw new Error(
      `tag must be a semantic version like "v1.2.3", got: ${JSON.stringify(tag)}`,
    );
  }
  const version = tag.replace(/^v/, "").replace(/\+.*$/, "");
  const headings = [...changelog.matchAll(HEADING_PATTERN)];
  const index = headings.findIndex((heading) => heading[1] === version);
  if (index === -1) {
    throw new Error(
      `CHANGELOG.md has no "## [${version}]" section; rename [Unreleased] to [${version}] before tagging`,
    );
  }
  const match = headings.at(index);
  const start = match.index + match[0].length;
  const end =
    index + 1 < headings.length
      ? headings.at(index + 1).index
      : changelog.length;
  return changelog.slice(start, end).trim() + "\n";
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const tag = process.argv[2];
  if (!tag) {
    console.error("usage: generate-release-notes.mjs vX.Y.Z");
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
