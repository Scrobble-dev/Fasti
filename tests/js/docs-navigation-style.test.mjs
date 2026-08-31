import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const css = await readFile(
  new URL("../../apps/docs/src/css/custom.css", import.meta.url),
  "utf8",
);
const searchPage = await readFile(
  new URL("../../apps/docs/src/pages/search.tsx", import.meta.url),
  "utf8",
);
const statusPage = await readFile(
  new URL("../../apps/docs/src/pages/status.tsx", import.meta.url),
  "utf8",
);
const tokensSource = await readFile(
  new URL("../../packages/tokens/src/index.ts", import.meta.url),
  "utf8",
);

test("Docusaurus controls when the mobile navigation toggle is displayed", () => {
  const toggleRules = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/gu)].filter(
    ([, selectors]) =>
      selectors
        .split(",")
        .map((selector) => selector.trim())
        .includes(".navbar__toggle"),
  );

  assert.ok(
    toggleRules.length > 0,
    "the navigation toggle needs a touch-target rule",
  );
  assert.ok(
    toggleRules.some(
      ([, selectors, declarations]) =>
        selectors.trim() === ".navbar__toggle" &&
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

test("Docusaurus documentation controls keep a 44 pixel target", () => {
  assert.match(tokensSource, /minimum:\s*"44px"/u);
  assert.match(
    tokensSource,
    /--fasti-touch-target-min:\s*\$\{touchTargets\.minimum\}/u,
  );
  assert.match(
    css,
    /\.breadcrumbs__link,\s*\.theme-doc-toc-mobile button,\s*\.theme-code-block button\s*\{[^}]*min-width:\s*var\(--fasti-touch-target-min\);[^}]*min-height:\s*var\(--fasti-touch-target-min\);/u,
  );
});

test("local search keeps an accessible fallback and waits for both assets", () => {
  assert.match(searchPage, /aria-busy=\{!loadError\}/u);
  assert.match(searchPage, /setAttribute\("role", "searchbox"\)/u);
  assert.match(searchPage, /Local search could not load\./u);
  assert.match(searchPage, /let mounted = true;/u);
  assert.match(searchPage, /!scriptReady \|\| !stylesheetReady/u);
  assert.match(searchPage, /mounted = false;/u);
});

test("status keeps its table visible while data loads or fails", () => {
  assert.match(statusPage, /aria-busy=\{!error && capabilities === null\}/u);
  assert.equal(
    [...statusPage.matchAll(/capabilities === null\s*\? "—"/gu)].length,
    3,
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

      const close = page.getByRole("button", { name: "Close navigation bar" });
      await close.click();
      await close.waitFor({ state: "hidden" });
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
