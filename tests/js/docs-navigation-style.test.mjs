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

test(
  "the built navigation opens only at the mobile breakpoint",
  { skip: !process.env.FASTI_DOCS_BASE_URL },
  async () => {
    const { chromium } = await import("@playwright/test");
    const browser = await chromium.launch({ headless: true });
    const page = await browser.newPage();
    const errors = [];
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    try {
      await page.setViewportSize({ width: 997, height: 700 });
      await page.goto(
        new URL("/start/choose-a-path/", process.env.FASTI_DOCS_BASE_URL).href,
      );
      const toggle = page.getByRole("button", {
        name: "Toggle navigation bar",
      });
      assert.equal(await toggle.isVisible(), false);

      await page.setViewportSize({ width: 996, height: 700 });
      assert.equal(await toggle.isVisible(), true);
      await toggle.click();
      await page
        .getByRole("button", { name: "Close navigation bar" })
        .waitFor({ state: "visible" });

      await page.getByRole("button", { name: "Close navigation bar" }).click();
      await page.setViewportSize({ width: 320, height: 695 });
      await toggle.focus();
      await toggle.press("Enter");
      await page
        .getByRole("button", { name: "Close navigation bar" })
        .waitFor({ state: "visible" });
      assert.deepEqual(errors, []);
    } finally {
      await browser.close();
    }
  },
);
