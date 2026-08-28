import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

test("an unavailable review API preserves the Review Inbox", async ({
  page,
}) => {
  await page.goto("/reviews");

  await expect(
    page.getByRole("heading", { level: 1, name: "Review Inbox" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Review listing is unavailable" }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toContainText(
    "This host does not support review listing yet.",
  );
  await expect(
    page.getByRole("heading", { name: "No open reviews" }),
  ).toHaveCount(0);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});
