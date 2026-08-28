import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

test("Workbench routes expose one current navigation item", async ({
  page,
}) => {
  await page.setViewportSize({ width: 375, height: 812 });

  for (const [path, name] of [
    ["/", "Overview"],
    ["/discover", "Discover"],
    ["/settings", "Settings"],
    ["/library", "Library"],
    ["/calendar", "Calendar"],
    ["/records", "Media Detail"],
    ["/reviews", "Review Inbox"],
    ["/connections", "Connections"],
  ] as const) {
    await page.goto(path);
    await page.getByRole("button", { name: "Open navigation" }).click();
    const current = page
      .getByRole("dialog", { name: "Main navigation" })
      .locator('[aria-current="page"]');
    await expect(current).toHaveCount(1);
    await expect(current).toHaveAccessibleName(name);
  }
});

test("empty Media Detail preserves a named page and readable recovery", async ({
  page,
}) => {
  await page.goto("/records");

  await expect(
    page.getByRole("heading", { level: 1, name: "Media Detail" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Choose one from Library" }),
  ).toBeVisible();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});
