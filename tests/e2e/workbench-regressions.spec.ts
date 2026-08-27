import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

test("signed-out Workbench keeps the product and compact rail usable", async ({
  page,
}) => {
  await page.setViewportSize({ width: 375, height: 812 });
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "In progress" }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toContainText(
    "Records are unavailable",
  );

  const connect = await page
    .getByRole("button", { name: "Connect records" })
    .boundingBox();
  expect(connect?.width).toBeGreaterThanOrEqual(44);
  expect(connect?.height).toBeGreaterThanOrEqual(44);

  const rail = await page
    .getByRole("complementary", { name: "Main Navigation" })
    .boundingBox();
  const main = await page.locator("main").boundingBox();
  const overview = await page
    .getByRole("button", { name: "Overview" })
    .boundingBox();
  expect(rail).not.toBeNull();
  expect(main).not.toBeNull();
  expect(overview).not.toBeNull();
  expect(overview!.x).toBeGreaterThanOrEqual(rail!.x);
  expect(overview!.x + overview!.width).toBeLessThanOrEqual(
    rail!.x + rail!.width,
  );
  expect(main!.x).toBeGreaterThanOrEqual(rail!.x + rail!.width);

  await page.getByRole("button", { name: "Hide sidebar" }).click();
  await expect(
    page.getByRole("complementary", { name: "Main Navigation" }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Show sidebar" }).click();
  await expect(
    page.getByRole("complementary", { name: "Main Navigation" }),
  ).toBeVisible();

  expect(
    await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    ),
  ).toBeLessThanOrEqual(0);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test("browser Settings reflows and saves only the real client endpoint", async ({
  page,
}) => {
  await page.setViewportSize({ width: 768, height: 1024 });
  await page.goto("/settings");

  const settingsNavigation = await page
    .getByRole("navigation", { name: "Settings sections" })
    .boundingBox();
  const settingsPanel = await page.locator(".settings-panel").boundingBox();
  expect(settingsNavigation).not.toBeNull();
  expect(settingsPanel).not.toBeNull();
  expect(settingsPanel!.y).toBeGreaterThanOrEqual(
    settingsNavigation!.y + settingsNavigation!.height,
  );

  await expect(page.getByLabel("Public URL (optional)")).toBeDisabled();
  await expect(page.getByLabel("Allowed providers")).toBeDisabled();
  await expect(
    page.getByText("This browser cannot read or change the node's provider"),
  ).toBeVisible();

  await page.getByLabel("Service URL").fill("http://localhost:8420");
  const save = page.getByRole("button", { name: "Save service URL" });
  const saveBox = await save.boundingBox();
  expect(saveBox?.height).toBeGreaterThanOrEqual(44);
  await save.click();
  await expect(page.getByRole("status")).toHaveText("Settings saved.");
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("fasti-network-config") ?? "null"),
    ),
  ).toEqual({ service_url: "http://localhost:8420" });

  await page
    .getByLabel("Service URL")
    .fill("http://localhost:8420/not-an-api-root");
  await save.click();
  await expect(page.getByRole("alert")).toContainText(
    "only a scheme, host, and optional port",
  );
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("fasti-network-config") ?? "null"),
    ),
  ).toEqual({ service_url: "http://localhost:8420" });

  expect(
    await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    ),
  ).toBeLessThanOrEqual(0);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});
