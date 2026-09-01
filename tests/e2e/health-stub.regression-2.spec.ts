import { expect, test } from "@playwright/test";

// Regression: ISSUE-002 — the bounded QA stub omitted the provider catalogue route.
// Found by /qa on 2026-08-31.
// Report: .gstack/qa-reports/qa-report-127-0-0-1-2026-08-31.md
test("an empty provider catalogue is a valid Discover state", async ({
  page,
  request,
}) => {
  const providers = await request.get(
    "http://127.0.0.1:18422/api/v1/providers",
  );
  await expect(providers.json()).resolves.toEqual({ providers: [] });

  await page.goto("/discover");
  await expect(page.getByRole("heading", { name: "Discover" })).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "No search provider is available" }),
  ).toBeVisible();
});
