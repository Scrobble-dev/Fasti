import assert from "node:assert/strict";
import test from "node:test";

import { requiresRuntime } from "../../scripts/classify-ci-scope.mjs";

test("portal-only changes do not start runtime CI jobs", () => {
  assert.equal(
    requiresRuntime([
      ".github/workflows/ci.yml",
      "apps/docs/src/css/custom.css",
      "scripts/classify-ci-scope.mjs",
      "tests/js/ci-docs-routing.test.mjs",
      "tests/js/docs-navigation-style.test.mjs",
    ]),
    false,
  );
});

test("unclassified changes keep every runtime CI gate", () => {
  for (const path of [
    "Cargo.lock",
    "crates/fasti-core/src/lib.rs",
    "packages/ui/src/index.ts",
    "pnpm-lock.yaml",
    "contracts/generated/v1/openapi.json",
  ])
    assert.equal(requiresRuntime([path]), true, path);
});

test("routing-only and empty comparisons fail closed", () => {
  assert.equal(requiresRuntime([]), true);
  assert.equal(requiresRuntime([".github/workflows/ci.yml"]), true);
});
