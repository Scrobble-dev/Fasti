import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { execFileSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { generateProvenance } from "../../scripts/generate-provenance.mjs";

const withGitFixture = async (run) => {
  const root = await mkdtemp(join(tmpdir(), "fasti-provenance-"));
  try {
    execFileSync("git", ["init", "--quiet"], { cwd: root });
    execFileSync("git", ["config", "user.email", "test@example.com"], {
      cwd: root,
    });
    execFileSync("git", ["config", "user.name", "Test"], { cwd: root });
    await run(root);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
};

test("binds the statement to the exact content and commit of its subjects", async () => {
  await withGitFixture(async (root) => {
    const subjectPath = join(root, "artifact.txt");
    await writeFile(subjectPath, "hello world");
    execFileSync("git", ["add", "."], { cwd: root });
    execFileSync("git", ["commit", "--quiet", "-m", "init"], { cwd: root });
    const commit = execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: root,
      encoding: "utf8",
    }).trim();

    const statement = await generateProvenance(root, [subjectPath], {
      GITHUB_REPOSITORY: "Scrobble-dev/Fasti",
      GITHUB_RUN_ID: "12345",
    });

    assert.equal(statement._type, "https://in-toto.io/Statement/v1");
    assert.equal(statement.subject.length, 1);
    assert.equal(statement.subject[0].name, "artifact.txt");
    assert.equal(statement.source.git_commit, commit);
    assert.equal(statement.predicate.runDetails.metadata.invocationId, "12345");
  });
});

test("changing a subject's bytes changes its recorded digest", async () => {
  await withGitFixture(async (root) => {
    const subjectPath = join(root, "artifact.txt");
    await writeFile(subjectPath, "hello world");
    execFileSync("git", ["add", "."], { cwd: root });
    execFileSync("git", ["commit", "--quiet", "-m", "init"], { cwd: root });

    const before = await generateProvenance(root, [subjectPath], {});

    await writeFile(subjectPath, "hello world!");
    const after = await generateProvenance(root, [subjectPath], {});

    assert.notEqual(
      before.subject[0].digest.sha256,
      after.subject[0].digest.sha256,
      "flipping the subject's bytes must change the recorded digest",
    );
  });
});

test("rejects being called with no subjects", async () => {
  await withGitFixture(async (root) => {
    await assert.rejects(() => generateProvenance(root, [], {}));
  });
});

const repositoryRoot = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../..",
);
const scriptPath = resolve(repositoryRoot, "scripts/generate-provenance.mjs");

test("CLI entrypoint writes a provenance statement for a real subject file", () => {
  const outputPath = resolve(
    repositoryRoot,
    "target/fasti-evidence/b8b/provenance/provenance-statement.json",
  );
  execFileSync("node", [scriptPath, resolve(repositoryRoot, "package.json")], {
    cwd: repositoryRoot,
  });
  return readFile(outputPath, "utf8").then((contents) => {
    const written = JSON.parse(contents);
    assert.equal(written._type, "https://in-toto.io/Statement/v1");
    assert.equal(written.subject[0].name, "package.json");
  });
});

test("CLI entrypoint exits nonzero with usage text when called without a subject", () => {
  assert.throws(() => execFileSync("node", [scriptPath], { stdio: "pipe" }));
});
