#!/usr/bin/env node
// Emits a plain in-toto/SLSA-shaped provenance STATEMENT (JSON) describing
// which commit, workflow, run, and runner produced the named subject
// artifacts. This script never signs, attests, or publishes the statement
// -- it is deferred CI-artifact evidence only, consumed by
// `cargo xtask test milestone --body B8b` and uploaded via
// actions/upload-artifact. See docs/architecture/b8b-release-readiness.md.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

async function sha256(path) {
  const bytes = await readFile(path);
  return createHash("sha256").update(bytes).digest("hex");
}

function git(root, ...args) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim();
}

export async function generateProvenance(root, subjectPaths, env = {}) {
  if (subjectPaths.length === 0) {
    throw new Error("generateProvenance requires at least one subject path");
  }

  const subject = await Promise.all(
    subjectPaths.map(async (path) => ({
      name: basename(path),
      digest: { sha256: await sha256(path) },
    })),
  );

  return {
    _type: "https://in-toto.io/Statement/v1",
    predicateType: "https://slsa.dev/provenance/v1",
    subject,
    predicate: {
      buildDefinition: {
        buildType:
          "https://github.com/Scrobble-dev/Fasti/blob/dev/.github/workflows/release.yml",
        externalParameters: {
          repository: env.GITHUB_REPOSITORY ?? "",
          ref: env.GITHUB_REF ?? "",
          workflowRef: env.GITHUB_WORKFLOW_REF ?? "",
        },
        internalParameters: {
          runnerOs: env.RUNNER_OS ?? "",
          runnerArch: env.RUNNER_ARCH ?? "",
        },
      },
      runDetails: {
        builder: { id: "https://github.com/actions/runner" },
        metadata: {
          invocationId: env.GITHUB_RUN_ID ?? "local-unpublished",
        },
      },
    },
    // snake_case to match every other receipt's /source/git_commit,
    // /source/git_tree convention in this repo (xtask/src/evidence.rs's
    // ensure_receipt_source), not in-toto/SLSA's own field casing -- this
    // "source" block is a custom addition, not part of either spec.
    source: {
      git_commit: git(root, "rev-parse", "--verify", "HEAD"),
      git_tree: git(root, "rev-parse", "HEAD^{tree}"),
    },
  };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const subjectPaths = process.argv.slice(2);
  if (subjectPaths.length === 0) {
    console.error("usage: generate-provenance.mjs <artifact-path>...");
    process.exit(1);
  }
  const statement = await generateProvenance(
    repositoryRoot,
    subjectPaths,
    process.env,
  );
  const outputDir = resolve(
    repositoryRoot,
    "target/fasti-evidence/b8b/provenance",
  );
  await mkdir(outputDir, { recursive: true });
  await writeFile(
    resolve(outputDir, "provenance-statement.json"),
    JSON.stringify(statement, null, 2) + "\n",
  );
  console.log(
    `PASS: provenance statement written for ${subjectPaths.length} subject(s)`,
  );
}
