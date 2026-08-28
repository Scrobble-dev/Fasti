import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";

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
  const preferencePanel = page.locator(
    'section[aria-labelledby="preferences-settings-title"]',
  );
  for (const control of await preferencePanel.getByRole("combobox").all()) {
    await expect(control).toBeDisabled();
  }
  await expect(preferencePanel.getByRole("checkbox")).toBeDisabled();

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

  await page.getByRole("button", { name: "Capability status" }).click();
  const downloadStarted = page.waitForEvent("download");
  await page
    .getByRole("button", { name: "Download diagnostic summary" })
    .click();
  const download = await downloadStarted;
  expect(download.suggestedFilename()).toMatch(
    /^fasti-diagnostic-summary-\d+\.json$/,
  );
  const summaryPath = await download.path();
  expect(summaryPath).not.toBeNull();
  const summary = JSON.parse(await readFile(summaryPath!, "utf8"));
  expect(summary).toHaveProperty("generatedAt");
  expect(summary).toHaveProperty("network");
  expect(summary).toHaveProperty("providers");

  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});
