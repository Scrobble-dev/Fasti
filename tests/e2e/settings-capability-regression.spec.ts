import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

test("inactive settings stay visible without reporting false success", async ({
  page,
}) => {
  await page.setViewportSize({ width: 768, height: 1024 });
  await page.goto("/settings");

  const preferences = page.getByRole("button", {
    name: "Preferences & Metadata",
  });
  await preferences.click();
  await expect(preferences).toHaveAttribute("aria-pressed", "true");
  await expect(
    page.getByText("current provider searches and Records do not read"),
  ).toBeVisible();
  for (const control of await page.getByRole("combobox").all()) {
    await expect(control).toBeDisabled();
  }
  await expect(page.getByRole("checkbox")).toBeDisabled();

  const customTypes = page.getByRole("button", {
    name: "Custom Types & Fields",
  });
  await customTypes.click();
  await expect(customTypes).toHaveAttribute("aria-pressed", "true");
  await expect(
    page.getByText("does not apply them to node Records or schemas"),
  ).toBeVisible();
  for (const control of await page
    .locator('section[aria-labelledby="custom-fields-settings-title"]')
    .locator("input, select, button")
    .all()) {
    await expect(control).toBeDisabled();
  }

  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});
