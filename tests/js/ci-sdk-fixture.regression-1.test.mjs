import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { parse } from "yaml";

test("JavaScript CI builds the locked Rust fixture before its bounded SDK tests", () => {
  const workflow = parse(
    readFileSync(
      new URL("../../.github/workflows/ci.yml", import.meta.url),
      "utf8",
    ),
  );
  const steps = workflow.jobs.javascript.steps;
  for (const action of ["dtolnay/rust-toolchain@", "Swatinem/rust-cache@"]) {
    const canonical = workflow.jobs.rust.steps.find((step) =>
      step.uses?.startsWith(action),
    );
    assert.ok(canonical);
    assert.ok(steps.some((step) => step.uses === canonical.uses));
  }
  const build = steps.findIndex(
    (step) =>
      step.run ===
      "cargo build --locked -p fasti-api --features conformance-fixture --bin b1-conformance-server",
  );
  const tests = steps.findIndex((step) => step.run === "pnpm test");
  assert.ok(build >= 0 && tests > build);
  assert.notEqual(steps[build]["continue-on-error"], true);
  assert.equal(steps[build].if, undefined);
});
