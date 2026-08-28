import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

async function undersizedControls(scope: Locator) {
  return scope
    .locator(
      'a[href], button, input:not([type="checkbox"]):not([type="radio"]), select, textarea, summary',
    )
    .evaluateAll((controls) =>
      controls
        .filter((control) => control.getClientRects().length > 0)
        .map((control) => {
          const bounds = control.getBoundingClientRect();
          return {
            tag: control.tagName,
            label:
              control.getAttribute("aria-label") ??
              control.textContent?.trim() ??
              control.tagName,
            width: Math.round(bounds.width),
            height: Math.round(bounds.height),
          };
        })
        .filter((control) => control.width < 44 || control.height < 44),
    );
}

async function bodyBackground(page: Page): Promise<string> {
  return page
    .locator("body")
    .evaluate((body) => getComputedStyle(body).backgroundColor);
}

async function rootVariable(page: Page, name: string): Promise<string> {
  return page
    .locator("html")
    .evaluate(
      (root, variable) =>
        getComputedStyle(root).getPropertyValue(variable).trim(),
      name,
    );
}

test("Workbench navigation and theme controls keep 44 pixel targets", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 900 });
  await page.goto("/library");

  expect(await undersizedControls(page.locator("body"))).toEqual([]);

  await page.getByRole("button", { name: "Open navigation" }).click();
  const navigation = page.getByRole("dialog", { name: "Main navigation" });
  expect(await undersizedControls(navigation)).toEqual([]);
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Theme settings" }).click();
  const drawer = page.getByRole("dialog", { name: "Theme settings" });
  await expect(drawer).toBeVisible();
  expect(await undersizedControls(drawer)).toEqual([]);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test("theme settings apply distinct Tabler and Fasti effects and persist", async ({
  page,
}) => {
  test.slow();
  await page.setViewportSize({ width: 375, height: 900 });
  await page.goto("/library");

  const trigger = page.getByRole("button", { name: "Theme settings" });
  await trigger.click();
  const drawer = page.getByRole("dialog", { name: "Theme settings" });
  await expect(drawer.getByRole("button", { name: "Light" })).toBeFocused();

  const lightBackground = await bodyBackground(page);
  await drawer.getByRole("button", { name: "Dark" }).click();
  const darkBackground = await bodyBackground(page);
  await drawer.getByRole("button", { name: "Night" }).click();
  const nightBackground = await bodyBackground(page);
  expect(new Set([lightBackground, darkBackground, nightBackground]).size).toBe(
    3,
  );
  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "dark");
  await expect(page.locator("html")).toHaveAttribute(
    "data-fasti-theme",
    "night",
  );

  for (const mode of ["Light", "Dark", "Night"]) {
    await drawer.getByRole("button", { name: mode, exact: true }).click();
    for (const scheme of [
      "Tabler Blue",
      "Red",
      "Green",
      "Orange",
      "Purple",
      "Teal",
      "Cyan",
      "Fasti Oxblood",
      "Horological Gold",
    ]) {
      await drawer.getByRole("button", { name: scheme, exact: true }).click();
      await drawer.getByRole("button", { name: "Done" }).hover();
      expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
    }
  }
  await drawer
    .getByRole("button", { name: "Fasti Oxblood", exact: true })
    .click();
  await drawer.getByLabel("Font family").selectOption("serif");

  await drawer.getByRole("button", { name: "Slate", exact: true }).click();
  const slateGray = await rootVariable(page, "--tblr-gray-50");
  await drawer.getByRole("button", { name: "Gray", exact: true }).click();
  const grayGray = await rootVariable(page, "--tblr-gray-50");
  expect(grayGray).not.toBe(slateGray);

  await drawer.getByRole("button", { name: "0", exact: true }).click();
  const squareRadius = await rootVariable(page, "--tblr-border-radius-scale");
  await expect(page.locator(".global-search input")).toHaveCSS(
    "border-radius",
    "0px",
  );
  await drawer.getByRole("button", { name: "2", exact: true }).click();
  const doubleRadius = await rootVariable(page, "--tblr-border-radius-scale");
  await expect(page.locator(".global-search input")).toHaveCSS(
    "border-radius",
    "12px",
  );
  expect(squareRadius).toBe("0");
  expect(doubleRadius).toBe("2");

  const html = page.locator("html");
  await expect(html).toHaveAttribute("data-bs-theme-base", "gray");
  await expect(html).toHaveAttribute("data-bs-theme-font", "serif");
  await expect(html).toHaveAttribute("data-bs-theme-primary", "red");
  await expect(html).toHaveAttribute("data-bs-theme-radius", "2");
  expect(
    (await rootVariable(page, "--fasti-action-primary")).toLowerCase(),
  ).toBe("#8b2e2a");
  expect(
    (
      await page
        .locator("body")
        .evaluate((body) => getComputedStyle(body).fontFamily)
    ).toLowerCase(),
  ).toContain("newsreader");

  await drawer.getByRole("button", { name: "Done" }).click();
  await expect(trigger).toBeFocused();
  await page.reload();

  await expect(html).toHaveAttribute("data-fasti-theme", "night");
  await expect(html).toHaveAttribute("data-bs-theme-base", "gray");
  await expect(html).toHaveAttribute("data-bs-theme-font", "serif");
  await expect(html).toHaveAttribute("data-bs-theme-primary", "red");
  await expect(html).toHaveAttribute("data-bs-theme-radius", "2");
  expect((await rootVariable(page, "--tblr-border-radius-scale")).trim()).toBe(
    "2",
  );
  expect(await bodyBackground(page)).toBe(nightBackground);

  await trigger.click();
  await expect(drawer.getByRole("button", { name: "Night" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect(
    drawer.getByRole("button", { name: "Fasti Oxblood", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(drawer.getByLabel("Font family")).toHaveValue("serif");
  await expect(
    drawer.getByRole("button", { name: "Gray", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(
    drawer.getByRole("button", { name: "2", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test("theme settings recover malformed storage and keep session changes when storage is blocked", async ({
  page,
}) => {
  await page.goto("/library");

  for (const stored of ["null", "{broken", JSON.stringify({ mode: "night" })]) {
    await page.evaluate(
      ({ value }) => localStorage.setItem("fasti-theme-settings", value),
      { value: stored },
    );
    await page.reload();
    await expect(page.getByRole("heading", { name: "Library" })).toBeVisible();
  }

  const trigger = page.getByRole("button", { name: "Theme settings" });
  await trigger.click();
  const drawer = page.getByRole("dialog", { name: "Theme settings" });
  await expect(
    drawer.getByRole("button", { name: "Night", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(
    drawer.getByRole("button", { name: "Slate", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(drawer.getByLabel("Font family")).toHaveValue("sans-serif");
  await expect(
    drawer.getByRole("button", { name: "1", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await drawer.getByRole("button", { name: "Done" }).click();

  await page.evaluate(() => {
    Storage.prototype.setItem = () => {
      throw new DOMException("Storage is blocked", "SecurityError");
    };
  });
  await trigger.click();
  await drawer.getByRole("button", { name: "Dark", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute(
    "data-fasti-theme",
    "dark",
  );
  await expect(
    drawer.getByRole("button", { name: "Dark", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
});

test("theme reset, Escape, and backdrop dismissal restore the trigger", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/");
  const trigger = page.getByRole("button", { name: "Theme settings" });
  await trigger.click();
  const drawer = page.getByRole("dialog", { name: "Theme settings" });
  await drawer.getByRole("button", { name: "Night", exact: true }).click();
  await drawer.getByRole("button", { name: "Gray", exact: true }).click();
  await drawer.getByLabel("Font family").selectOption("serif");
  await drawer.getByRole("button", { name: "2", exact: true }).click();
  await drawer.getByRole("button", { name: "Reset changes" }).click();

  await expect(
    drawer.getByRole("button", { name: "Light", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(
    drawer.getByRole("button", { name: "Tabler Blue", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(
    drawer.getByRole("button", { name: "Slate", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(drawer.getByLabel("Font family")).toHaveValue("sans-serif");
  await expect(
    drawer.getByRole("button", { name: "1", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");

  await page.keyboard.press("Escape");
  await expect(drawer).toBeHidden();
  await expect(trigger).toBeFocused();

  await trigger.click();
  await page.mouse.click(8, 8);
  await expect(drawer).toBeHidden();
  await expect(trigger).toBeFocused();
});

test("theme drawer reflows with enlarged text and WCAG text spacing", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await page.goto("/library");
  await page.getByRole("button", { name: "Theme settings" }).click();
  const drawer = page.getByRole("dialog", { name: "Theme settings" });

  await page.locator("html").evaluate((element) => {
    element.style.fontSize = "200%";
  });
  expect(
    await drawer.evaluate(
      (element) => element.scrollWidth - element.clientWidth,
    ),
  ).toBeLessThanOrEqual(0);
  await expect(
    drawer.getByRole("button", { name: "Stone", exact: true }),
  ).toBeVisible();
  await expect(
    drawer.getByRole("button", { name: "2", exact: true }),
  ).toBeVisible();

  await page.locator("html").evaluate((element) => {
    element.style.fontSize = "100%";
    const style = document.createElement("style");
    style.textContent =
      "* { line-height: 1.5 !important; letter-spacing: 0.12em !important; word-spacing: 0.16em !important; }";
    document.head.append(style);
  });
  expect(
    await drawer.evaluate(
      (element) => element.scrollWidth - element.clientWidth,
    ),
  ).toBeLessThanOrEqual(0);
});

test("Workbench and status share one persisted theme mode", async ({
  page,
}) => {
  await page.goto("/library");
  await page.getByRole("button", { name: "Theme settings" }).click();
  const drawer = page.getByRole("dialog", { name: "Theme settings" });
  await drawer.getByRole("button", { name: "Night", exact: true }).click();
  await drawer.getByRole("button", { name: "Done" }).click();

  await page.getByRole("link", { name: "Service status" }).click();
  await expect(
    page.getByRole("button", { name: "Use light theme" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Use light theme" }).click();
  await page
    .getByRole("button", { name: "Open Media Workbench" })
    .first()
    .click();

  await page.getByRole("button", { name: "Theme settings" }).click();
  await expect(
    page
      .getByRole("dialog", { name: "Theme settings" })
      .getByRole("button", { name: "Light", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
});

test("theme dialog wraps focus and restores its trigger", async ({ page }) => {
  await page.goto("/");

  const trigger = page.getByRole("button", { name: "Theme settings" });
  await trigger.click();
  const drawer = page.getByRole("dialog", { name: "Theme settings" });
  const close = drawer.getByRole("button", { name: "Close theme settings" });
  const done = drawer.getByRole("button", { name: "Done" });

  await expect(drawer.getByRole("button", { name: "Light" })).toBeFocused();
  await done.focus();
  await page.keyboard.press("Tab");
  await expect(close).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(done).toBeFocused();
  await done.click();
  await expect(trigger).toBeFocused();
});

test("global search closes on Escape and when focus leaves", async ({
  page,
}) => {
  await page.goto("/");

  const search = page.getByRole("combobox", {
    name: "Search records or commands",
  });
  await search.focus();
  await expect(page.getByRole("listbox")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("listbox")).toHaveCount(0);
  await expect(search).toBeFocused();

  await page.getByRole("button", { name: "Theme settings" }).focus();
  await search.focus();
  await expect(page.getByRole("listbox")).toBeVisible();
  await page.getByRole("button", { name: "Theme settings" }).focus();
  await expect(page.getByRole("listbox")).toHaveCount(0);
});
