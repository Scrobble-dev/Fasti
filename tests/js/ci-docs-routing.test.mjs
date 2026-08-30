import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { parse as parseYaml } from "yaml";

import { requiresRuntime } from "../../scripts/classify-ci-scope.mjs";

const ignoredPortalPaths = [
  ".github/workflows/ci.yml",
  ".github/workflows/docs-pages.yml",
  "apps/docs/**",
  "brand/logos/**",
  "diagrams/documentation-publication.svg",
  "docs/**",
  "packages/deploy-plan/**",
  "scripts/classify-ci-scope.mjs",
  "scripts/validate-docs*.mjs",
  "tests/js/ci-docs-routing.test.mjs",
  "tests/js/docs-*.test.mjs",
  "xtask/src/docs.rs",
];

test("portal-only changes do not start runtime CI jobs", () => {
  assert.equal(
    requiresRuntime([
      ".github/workflows/ci.yml",
      ".github/workflows/conformance.yml",
      ".github/workflows/security.yml",
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

test("independent runtime workflows ignore the same portal-only paths", async () => {
  for (const path of [
    ".github/workflows/conformance.yml",
    ".github/workflows/security.yml",
  ]) {
    const workflow = parseYaml(await readFile(path, "utf8"));
    assert.deepEqual(
      workflow.on.push["paths-ignore"],
      ignoredPortalPaths,
      path,
    );
    assert.deepEqual(
      workflow.on.pull_request["paths-ignore"],
      ignoredPortalPaths,
      path,
    );
  }
});
