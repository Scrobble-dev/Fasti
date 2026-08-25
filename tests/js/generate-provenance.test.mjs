import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { execFileSync } from "node:child_process";
import { join } from "node:path";
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
