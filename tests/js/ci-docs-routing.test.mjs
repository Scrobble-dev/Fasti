import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { requiresRuntime } from "../../scripts/classify-ci-scope.mjs";

const ignoredPortalPaths = [
  ".github/workflows/docs-pages.yml",
  "apps/docs/**",
  "diagrams/documentation-publication.svg",
  "docs/**",
  "*.md",
  "**/*.md",
  "packages/deploy-plan/**",
  "scripts/validate-docs.mjs",
  "scripts/validate-docs-build.mjs",
  "tests/js/deploy-plan.test.mjs",
  "tests/js/docs-*.test.mjs",
  "xtask/src/docs.rs",
];

test("portal-only changes do not start runtime CI jobs", () => {
  assert.equal(
    requiresRuntime([
      "apps/docs/src/css/custom.css",
      "README.md",
      "crates/fasti-core/README.md",
      "packages/deploy-plan/src/index.ts",
      "tests/js/deploy-plan.test.mjs",
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

test("routing changes and empty comparisons fail closed", () => {
  assert.equal(requiresRuntime([]), true);
  for (const path of [
    ".github/workflows/ci.yml",
    ".github/workflows/conformance.yml",
    ".github/workflows/security.yml",
    "scripts/classify-ci-scope.mjs",
    "tests/js/ci-docs-routing.test.mjs",
  ])
    assert.equal(
      requiresRuntime([path, "apps/docs/src/css/custom.css"]),
      true,
      path,
    );
});

test("shared brand assets keep every runtime CI gate", () => {
  assert.equal(requiresRuntime(["brand/logos/fasti-mark.svg"]), true);
});

test("documentation CI keeps its exact relevant gates", async () => {
  const workflow = await readFile(".github/workflows/ci.yml", "utf8");
  assert.match(workflow, /git diff --no-renames --name-only/gu);
  assert.match(workflow, /PUSH_BASE_SHA" =~ \^0\+\$/u);
  assert.match(workflow, /cargo test --package xtask docs::tests --locked/u);
  assert.match(workflow, /node --test tests\/js\/deploy-plan\.test\.mjs/u);
});

test("independent runtime workflows ignore the same portal-only paths", async () => {
  for (const path of [
    ".github/workflows/conformance.yml",
    ".github/workflows/security.yml",
  ]) {
    const workflow = await readFile(path, "utf8");
    for (const event of ["push", "pull_request"]) {
      const start = workflow.indexOf(`  ${event}:`);
      const next = workflow.slice(start + 1).search(/^  [a-z_]+:/mu);
      const section = workflow.slice(
        start,
        next < 0 ? undefined : start + 1 + next,
      );
      for (const ignored of ignoredPortalPaths)
        assert.equal(
          section.includes(`      - "${ignored}"`),
          true,
          `${path}:${event}:${ignored}`,
        );
    }
  }
});

test("dev-push heavyweight workflows ignore the same portal-only paths", async () => {
  for (const path of [
    ".github/workflows/dev-nightly.yml",
    ".github/workflows/scorecard.yml",
  ]) {
    const workflow = await readFile(path, "utf8");
    const start = workflow.indexOf("  push:");
    const next = workflow.slice(start + 1).search(/^  [a-z_]+:/mu);
    const section = workflow.slice(
      start,
      next < 0 ? undefined : start + 1 + next,
    );
    for (const ignored of ignoredPortalPaths)
      assert.equal(
        section.includes(`      - "${ignored}"`),
        true,
        `${path}:push:${ignored}`,
      );
  }
});
