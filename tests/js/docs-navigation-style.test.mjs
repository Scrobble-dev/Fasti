import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const css = await readFile(
  new URL("../../apps/docs/src/css/custom.css", import.meta.url),
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

test(
  "built search and status pages keep usable loading and failure states",
  { skip: !process.env.FASTI_DOCS_BASE_URL },
  async () => {
    const { chromium } = await import("@playwright/test");
    const browser = await chromium.launch({ headless: true });
    try {
      const search = await browser.newPage();
      let scriptRoute;
      let stylesheetRoute;
      await search.route("**/pagefind/pagefind-ui.js", (route) => {
        scriptRoute = route;
      });
      await search.route("**/pagefind/pagefind-ui.css", (route) => {
        stylesheetRoute = route;
      });
      await search.goto(
        new URL("/search/", process.env.FASTI_DOCS_BASE_URL).href,
        { waitUntil: "domcontentloaded" },
      );
      const fallback = search.locator("#fasti-search-fallback");
      await fallback.waitFor({ state: "visible" });
      assert.equal(await fallback.isDisabled(), true);
      assert.equal(
        await fallback.getAttribute("aria-describedby"),
        "fasti-search-loading",
      );
      assert.ok(scriptRoute);
      assert.ok(stylesheetRoute);
      await Promise.all([scriptRoute.continue(), stylesheetRoute.continue()]);
      await search
        .getByRole("searchbox", { name: "Search documentation" })
        .waitFor({ state: "visible" });

      const failedSearch = await browser.newPage();
      await failedSearch.route("**/pagefind/pagefind-ui.*", (route) =>
        route.abort(),
      );
      await failedSearch.goto(
        new URL("/search/", process.env.FASTI_DOCS_BASE_URL).href,
      );
      await failedSearch
        .getByRole("alert")
        .filter({ hasText: "Local search could not load." })
        .waitFor({ state: "visible" });
      assert.equal(
        await failedSearch.locator("#fasti-search-fallback").isVisible(),
        true,
      );

      const status = await browser.newPage();
      let capabilityRoute;
      await status.route("**/capabilities.json", (route) => {
        capabilityRoute = route;
      });
      await status.goto(
        new URL("/status/", process.env.FASTI_DOCS_BASE_URL).href,
        { waitUntil: "domcontentloaded" },
      );
      const table = status.getByRole("table", {
        name: "Current generated capability states",
      });
      await table.waitFor({ state: "visible" });
      assert.equal(await table.getAttribute("aria-busy"), "true");
      assert.deepEqual(
        await table.locator("tbody tr td:last-child").allTextContents(),
        ["—", "—", "—"],
      );
      assert.ok(capabilityRoute);
      await capabilityRoute.abort();
      await status
        .getByRole("alert")
        .filter({ hasText: "The generated capability data could not load." })
        .waitFor({ state: "visible" });
      assert.equal(await table.isVisible(), true);
      assert.equal(await table.getAttribute("aria-busy"), "false");
    } finally {
      await browser.close();
    }
  },
);

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
