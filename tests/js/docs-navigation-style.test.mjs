import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const css = await readFile("apps/docs/src/css/custom.css", "utf8");

test("Docusaurus controls when the mobile navigation toggle is displayed", () => {
  const toggleRules = [
    ...css.matchAll(/([^{}]*\.navbar__toggle[^{}]*)\{([^}]*)\}/gu),
  ];

  assert.ok(
    toggleRules.length > 0,
    "the navigation toggle needs a touch-target rule",
  );
  assert.ok(
    toggleRules.some(
      ([, , declarations]) =>
        /min-width:\s*var\(--fasti-touch-target-min\)/u.test(declarations) &&
        /min-height:\s*var\(--fasti-touch-target-min\)/u.test(declarations),
    ),
    "the navigation toggle needs a 44 pixel touch target",
  );
  for (const [, , declarations] of toggleRules)
    assert.doesNotMatch(
      declarations,
      /display\s*:/u,
      "display overrides expose a non-functional toggle above the mobile breakpoint",
    );
});
