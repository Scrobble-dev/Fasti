import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Locator, type Page } from "@playwright/test";

const mobileViewports = [
  { width: 320, height: 900 },
  { width: 375, height: 812 },
  { width: 768, height: 1024 },
] as const;

const desktopViewports = [
  { width: 1024, height: 768 },
  { width: 1440, height: 900 },
  { width: 1920, height: 1080 },
] as const;

async function expectNoHorizontalOverflow(page: Page): Promise<void> {
  expect(
    await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    ),
  ).toBeLessThanOrEqual(0);
}

async function expectAxeClean(page: Page): Promise<void> {
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
}

async function expectAdjacent(
  first: Locator,
  second: Locator,
  maximumGap = 16,
): Promise<void> {
  const firstBox = await first.boundingBox();
  const secondBox = await second.boundingBox();
  expect(firstBox).not.toBeNull();
  expect(secondBox).not.toBeNull();
  expect(Math.abs(firstBox!.y - secondBox!.y)).toBeLessThanOrEqual(1);
  expect(secondBox!.x - (firstBox!.x + firstBox!.width)).toBeLessThanOrEqual(
    maximumGap,
  );
}

for (const viewport of mobileViewports) {
  test(`${viewport.width}px Workbench uses a full-canvas navigation offcanvas`, async ({
    page,
  }) => {
    await page.setViewportSize(viewport);
    await page.goto("/");

    const navigation = page.locator("#fasti-main-navigation");
    const navigationTrigger = page.getByRole("button", {
      name: "Open navigation",
    });
    const pageWrapper = page.locator(".workbench-main-shell");

    await expect(navigation).toHaveAttribute("aria-hidden", "true");
    await expect(navigationTrigger).toHaveAttribute("aria-expanded", "false");
    const closedWrapper = await pageWrapper.boundingBox();
    expect(closedWrapper).not.toBeNull();
    expect(closedWrapper!.x).toBeLessThanOrEqual(1);
    expect(closedWrapper!.width).toBeGreaterThanOrEqual(viewport.width - 1);

    await navigationTrigger.click();
    const dialog = page.getByRole("dialog", { name: "Main navigation" });
    await expect(dialog).toBeVisible();
    await expect(navigationTrigger).toHaveAttribute("aria-expanded", "true");
    await expect(dialog.getByRole("link", { name: "Overview" })).toBeFocused();
    await page.keyboard.press("Control+K");
    await expect(dialog.getByRole("link", { name: "Overview" })).toBeFocused();
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await page.keyboard.press("/");
    await expect(dialog.getByRole("link", { name: "Overview" })).toBeFocused();
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await expectAxeClean(page);

    await page.keyboard.press("Escape");
    await expect(navigation).toHaveAttribute("aria-hidden", "true");
    await expect(navigationTrigger).toBeFocused();

    await navigationTrigger.click();
    await dialog.getByRole("link", { name: "Settings" }).click();
    await expect(page).toHaveURL(/\/settings$/);
    await expect(navigation).toHaveAttribute("aria-hidden", "true");
    await expect(page.locator("#main-content")).toBeFocused();

    await expectNoHorizontalOverflow(page);
    await expectAxeClean(page);
  });
}

for (const viewport of desktopViewports) {
  test(`${viewport.width}px Workbench preserves expanded, collapsed, and hidden geometry`, async ({
    page,
  }) => {
    await page.setViewportSize(viewport);
    await page.goto("/");

    const navigation = page.getByRole("navigation", {
      name: "Main navigation",
    });
    const pageWrapper = page.locator(".workbench-main-shell");
    const expandedNavigation = await navigation.boundingBox();
    const expandedWrapper = await pageWrapper.boundingBox();
    expect(expandedNavigation).not.toBeNull();
    expect(expandedWrapper).not.toBeNull();
    expect(expandedNavigation!.width).toBeGreaterThanOrEqual(230);
    expect(expandedNavigation!.width).toBeLessThanOrEqual(250);
    expect(
      Math.abs(
        expandedWrapper!.x -
          (expandedNavigation!.x + expandedNavigation!.width),
      ),
    ).toBeLessThanOrEqual(1);
    expect(expandedWrapper!.width).toBeGreaterThanOrEqual(
      viewport.width - expandedNavigation!.width - 1,
    );

    await page.getByRole("button", { name: "Collapse navigation" }).click();
    const expand = page.getByRole("button", { name: "Expand navigation" });
    await expect(expand).toBeFocused();
    const collapsedNavigation = await navigation.boundingBox();
    const collapsedWrapper = await pageWrapper.boundingBox();
    expect(collapsedNavigation).not.toBeNull();
    expect(collapsedWrapper).not.toBeNull();
    expect(collapsedNavigation!.width).toBeLessThan(expandedNavigation!.width);
    expect(collapsedNavigation!.width).toBeGreaterThanOrEqual(44);
    expect(
      Math.abs(
        collapsedWrapper!.x -
          (collapsedNavigation!.x + collapsedNavigation!.width),
      ),
    ).toBeLessThanOrEqual(1);

    await expand.click();
    await expect(
      page.getByRole("button", { name: "Collapse navigation" }),
    ).toBeFocused();

    await page.getByRole("button", { name: "Hide navigation" }).click();
    const show = page.getByRole("button", { name: "Show navigation" });
    await expect(navigation).toBeHidden();
    await expect(show).toBeFocused();
    const hiddenWrapper = await pageWrapper.boundingBox();
    expect(hiddenWrapper).not.toBeNull();
    expect(hiddenWrapper!.x).toBeLessThanOrEqual(1);
    expect(hiddenWrapper!.width).toBeGreaterThanOrEqual(viewport.width - 1);

    await show.click();
    await expect(
      navigation.getByRole("link", { name: "Overview" }),
    ).toBeFocused();
    await expectNoHorizontalOverflow(page);
    await expectAxeClean(page);
  });
}

test("persisted navigation state survives reload and yields to the Tabler breakpoint", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1024, height: 768 });
  await page.goto("/");

  await page.getByRole("button", { name: "Collapse navigation" }).click();
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Expand navigation" }),
  ).toBeVisible();

  await page.setViewportSize({ width: 768, height: 900 });
  const trigger = page.getByRole("button", { name: "Open navigation" });
  await expect(trigger).toBeVisible();
  await expect(page.locator("#fasti-main-navigation")).toHaveAttribute(
    "aria-hidden",
    "true",
  );
  await trigger.click();
  await expect(
    page.getByRole("dialog", { name: "Main navigation" }),
  ).toBeVisible();

  await page.setViewportSize({ width: 1024, height: 768 });
  await expect(
    page.getByRole("navigation", { name: "Main navigation" }),
  ).toBeVisible();
  await expect(page.locator("body")).not.toHaveClass(/fasti-navigation-open/);
  await expect(
    page.getByRole("button", { name: "Expand navigation" }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Expand navigation" }).click();
  await page.getByRole("button", { name: "Hide navigation" }).click();
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Show navigation" }),
  ).toBeVisible();

  await page.setViewportSize({ width: 768, height: 900 });
  await expect(trigger).toBeVisible();
  const pageWrapper = await page.locator(".workbench-main-shell").boundingBox();
  expect(pageWrapper).not.toBeNull();
  expect(pageWrapper!.x).toBeLessThanOrEqual(1);
  expect(pageWrapper!.width).toBeGreaterThanOrEqual(767);

  await page.setViewportSize({ width: 1024, height: 768 });
  await expect(
    page.getByRole("button", { name: "Show navigation" }),
  ).toBeVisible();
});

test("malformed navigation preferences fall back and remove duplicate routes", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1024, height: 768 });
  await page.goto("/");
  await page.evaluate(() => {
    const key = "fasti-workbench-preferences";
    const preferences = JSON.parse(localStorage.getItem(key) ?? "{}") as {
      navItems?: Array<Record<string, unknown>>;
    };
    const overview = preferences.navItems?.find((item) => item.id === "home");
    localStorage.setItem(
      key,
      JSON.stringify({
        ...preferences,
        sidebarCollapsed: "yes",
        sidebarHidden: 1,
        navItems: overview
          ? [overview, overview, ...(preferences.navItems ?? [])]
          : preferences.navItems,
      }),
    );
  });
  await page.reload();

  const navigation = page.getByRole("navigation", {
    name: "Main navigation",
  });
  await expect(navigation).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Collapse navigation" }),
  ).toBeVisible();
  await expect(navigation.locator('[aria-current="page"]')).toHaveCount(1);
  await expect(navigation.getByRole("link", { name: "Overview" })).toHaveCount(
    1,
  );
});

test("mobile navigation traps Tab and closes from its controls and backdrop", async ({
  page,
}) => {
  await page.setViewportSize({ width: 375, height: 900 });
  await page.goto("/");
  const trigger = page.getByRole("button", { name: "Open navigation" });
  await trigger.click();

  const dialog = page.getByRole("dialog", { name: "Main navigation" });
  const close = dialog.getByRole("button", { name: "Close navigation" });
  const controls = dialog.locator("a[href], button:not([disabled])");
  await expect(dialog.getByRole("link", { name: "Overview" })).toBeFocused();
  await controls.last().focus();
  await page.keyboard.press("Tab");
  await expect(close).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(controls.last()).toBeFocused();

  await close.click();
  await expect(dialog).toBeHidden();
  await expect(trigger).toBeFocused();

  await trigger.click();
  await expect(page.locator(".navigation-backdrop")).toBeVisible();
  await page.mouse.click(370, 450);
  await expect(dialog).toBeHidden();
  await expect(trigger).toBeFocused();
});

test.describe("reduced motion", () => {
  test.use({ reducedMotion: "reduce" });

  test("mobile navigation opens visibly and moves focus without animation", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 320, height: 900 });
    await page.goto("/settings");

    await page.getByRole("button", { name: "Open navigation" }).click();
    const navigation = page.getByRole("dialog", {
      name: "Main navigation",
    });
    await expect(navigation).toBeVisible();
    await expect(
      navigation.getByRole("link", { name: "Settings" }),
    ).toBeFocused();
    await expectAxeClean(page);
  });
});

test("forced colors preserves Workbench navigation and theme controls", async ({
  page,
}) => {
  await page.emulateMedia({ forcedColors: "active" });
  await page.setViewportSize({ width: 320, height: 900 });
  await page.goto("/settings");

  await page.getByRole("button", { name: "Open navigation" }).click();
  const navigation = page.getByRole("dialog", { name: "Main navigation" });
  await expect(navigation).toBeVisible();
  await expect(
    navigation.getByRole("link", { name: "Settings" }),
  ).toBeFocused();
  await expectAxeClean(page);

  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Theme settings" }).click();
  await expect(
    page.getByRole("dialog", { name: "Theme settings" }),
  ).toBeVisible();
  await expectAxeClean(page);
});

test("Settings uses responsive native navigation and keeps browser policy subordinate", async ({
  page,
}) => {
  await page.setViewportSize({ width: 768, height: 1024 });
  await page.goto("/settings");

  const selector = page.getByLabel("Settings section", { exact: true });
  const desktopNavigation = page.getByRole("navigation", {
    name: "Settings sections",
  });
  await expect(selector).toBeVisible();
  await expect(desktopNavigation).toBeHidden();
  await expect(selector).toHaveValue("network");

  await selector.selectOption("providers");
  await expect(page).toHaveURL(/\/settings\/metadata$/);
  await expect(selector).toHaveValue("providers");
  await page.goBack();
  await expect(page).toHaveURL(/\/settings$/);
  await expect(selector).toHaveValue("network");

  const save = page.getByRole("button", { name: "Save service URL" });
  const testConnection = page.getByRole("button", {
    name: "Test service URL",
  });
  await expectAdjacent(save, testConnection, 12);

  const managedPolicy = page.locator("details.managed-policy");
  await expect(managedPolicy).not.toHaveAttribute("open", "");
  await managedPolicy.locator("summary").click();
  await expect(managedPolicy).toHaveAttribute("open", "");
  await expect(
    page.getByText("This browser cannot read or change the node's provider"),
  ).toBeVisible();
  await expect(page.getByLabel("Allowed providers")).toBeDisabled();

  await page.getByLabel("Service URL").fill("http://localhost:8420");
  await save.click();
  await expect(page.getByRole("status")).toHaveText("Settings saved.");
  await page.reload();
  await expect(page.getByLabel("Service URL")).toHaveValue(
    "http://localhost:8420",
  );

  await expectNoHorizontalOverflow(page);
  await expectAxeClean(page);
});

test("wide Settings links preserve URL state and browser history", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/settings");

  const navigation = page.getByRole("navigation", {
    name: "Settings sections",
  });
  await expect(navigation).toBeVisible();
  await expect(
    page.getByLabel("Settings section", { exact: true }),
  ).toBeHidden();
  await expect(
    navigation.getByRole("link", { name: "Network" }),
  ).toHaveAttribute("aria-current", "page");

  const preferences = navigation.getByRole("link", {
    name: "Preferences & Metadata",
  });
  await preferences.click();
  await expect(page).toHaveURL(/\/settings\/preferences$/);
  await expect(preferences).toHaveAttribute("aria-current", "page");

  await page.goBack();
  await expect(page).toHaveURL(/\/settings$/);
  await expect(
    navigation.getByRole("link", { name: "Network" }),
  ).toHaveAttribute("aria-current", "page");
  await page.goForward();
  await expect(page).toHaveURL(/\/settings\/preferences$/);
  await expect(preferences).toHaveAttribute("aria-current", "page");

  expect(
    await preferences.evaluate((link) => {
      let prevented = false;
      link.addEventListener(
        "click",
        (event) => {
          prevented = event.defaultPrevented;
          event.preventDefault();
        },
        { once: true },
      );
      link.dispatchEvent(
        new MouseEvent("click", {
          bubbles: true,
          cancelable: true,
          ctrlKey: true,
        }),
      );
      return prevented;
    }),
  ).toBe(false);
  await expectNoHorizontalOverflow(page);
});

test("direct canonical and compatibility Settings routes preserve one active section", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  for (const [path, name] of [
    ["/settings", "Network"],
    ["/settings/metadata", "Metadata credentials"],
    ["/settings/providers", "Metadata credentials"],
    ["/settings/preferences", "Preferences & Metadata"],
    ["/settings/custom-fields", "Custom Types & Fields"],
    ["/settings/custom_fields", "Custom Types & Fields"],
    ["/settings/collections", "Nuvio Collections"],
    ["/settings/nuvio_collections", "Nuvio Collections"],
    ["/settings/status", "Capability status"],
    ["/settings/system", "Capability status"],
  ] as const) {
    await page.goto(path);
    await expect(page).toHaveURL(new RegExp(`${path.replace("-", "\\-")}$`));
    const navigation = page.getByRole("navigation", {
      name: "Settings sections",
    });
    await expect(navigation.locator('[aria-current="page"]')).toHaveCount(1);
    await expect(
      navigation.getByRole("link", { name, exact: true }),
    ).toHaveAttribute("aria-current", "page");
  }

  await page.setViewportSize({ width: 320, height: 900 });
  for (const path of [
    "/settings/preferences",
    "/settings/custom-fields",
    "/settings/collections",
    "/settings/status",
  ] as const) {
    await page.goto(path);
    await expectNoHorizontalOverflow(page);
    expect(
      await page
        .locator(".settings-panel")
        .evaluate((panel) => panel.scrollWidth - panel.clientWidth),
    ).toBeLessThanOrEqual(0);
  }
});

for (const viewport of [...mobileViewports, ...desktopViewports]) {
  test(`${viewport.width}px shell and Settings do not overflow horizontally`, async ({
    page,
  }) => {
    await page.setViewportSize(viewport);
    for (const path of ["/", "/settings"] as const) {
      await page.goto(path);
      await expectNoHorizontalOverflow(page);
      if (path === "/settings") {
        const main = await page.locator("#main-content").boundingBox();
        const settings = await page
          .locator(".settings-container")
          .boundingBox();
        expect(main).not.toBeNull();
        expect(settings).not.toBeNull();
        expect(Math.abs(settings!.x - main!.x)).toBeLessThanOrEqual(1);
        expect(Math.abs(settings!.width - main!.width)).toBeLessThanOrEqual(1);
      }
    }
  });
}
