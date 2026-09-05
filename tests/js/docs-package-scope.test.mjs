import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const source = await readFile("xtask/src/docs.rs", "utf8");
const start = source.indexOf("pub(crate) fn package");
const end = source.indexOf("\nfn generate_to", start);
const packageSource = source.slice(start, end);

test("the documentation package does not run product or runtime gates", () => {
  assert.ok(start >= 0 && end > start, "documentation package body is missing");
  assert.doesNotMatch(packageSource, /verify_contracts|cargo\s+test/u);
});

test("generated docs use the physical path required by webpack's MDX include", async () => {
  const config = await readFile("apps/docs/docusaurus.config.ts", "utf8");
  assert.match(config, /import \{ realpathSync \} from "node:fs"/u);
  assert.match(
    config,
    /path: realpathSync\(\s*resolve\(__dirname, "\.\.\/\.\.\/target\/docs-site\/content"\),?\s*\)/u,
  );
});
