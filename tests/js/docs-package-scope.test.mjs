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

test("the documentation package validates each published contract family", () => {
  for (const validator of [
    "validate-authored-contracts.mjs",
    "validate-generated-contracts.mjs",
    "validate-integration-contracts.mjs",
    "validate-okf-uat.mjs",
  ])
    assert.match(packageSource, new RegExp(validator.replace(".", "\\."), "u"));
});
