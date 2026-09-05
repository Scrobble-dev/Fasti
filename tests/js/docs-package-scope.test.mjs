import assert from "node:assert/strict";
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  realpath,
  rm,
  symlink,
} from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const source = await readFile("xtask/src/docs.rs", "utf8");
const start = source.indexOf("pub(crate) fn package");
const end = source.indexOf("\nfn generate_to", start);
const packageSource = source.slice(start, end);

test("the documentation package does not run product or runtime gates", () => {
  assert.ok(start >= 0 && end > start, "documentation package body is missing");
  assert.doesNotMatch(packageSource, /verify_contracts|cargo\s+test/u);
});

test("generated docs resolve symlinked content and reject missing content", async () => {
  const docs = new URL("../../apps/docs/", import.meta.url);
  const require = createRequire(new URL("package.json", docs));
  const { loadSiteConfig } = require("@docusaurus/core/lib/server/config.js");
  const fixture = await mkdtemp(join(tmpdir(), "fasti-docs-config-"));
  try {
    const siteDir = join(fixture, "checkout/apps/docs");
    const target = join(fixture, "physical-target");
    const content = join(target, "docs-site/content");
    await mkdir(siteDir, { recursive: true });
    await mkdir(content, { recursive: true });
    await copyFile(
      new URL("docusaurus.config.ts", docs),
      join(siteDir, "docusaurus.config.ts"),
    );
    await symlink(target, join(fixture, "checkout/target"), "dir");
    await symlink(
      fileURLToPath(new URL("node_modules", docs)),
      join(siteDir, "node_modules"),
      "dir",
    );

    const { siteConfig } = await loadSiteConfig({ siteDir });
    assert.equal(siteConfig.presets[0][1].docs.path, await realpath(content));
    await rm(content, { recursive: true });
    await assert.rejects(
      loadSiteConfig({ siteDir }),
      (error) => error.cause?.code === "ENOENT",
    );
  } finally {
    await rm(fixture, { recursive: true, force: true });
  }
});
