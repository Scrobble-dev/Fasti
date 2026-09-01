import { expect, test } from "@playwright/test";

// Regression: ISSUE-003 — the bounded QA stub returned an array for Collections state.
// Found by /qa on 2026-08-31.
// Report: .gstack/qa-reports/qa-report-127-0-0-1-2026-08-31.md
test("an absent Nuvio Collections document is contract-valid", async ({
  page,
  request,
}) => {
  const collections = await request.get(
    "http://127.0.0.1:18422/api/v1/profile/nuvio-collections",
  );
  await expect(collections.json()).resolves.toEqual({ document: null });

  await page.goto("/settings/collections");
  await expect(
    page.getByRole("heading", { name: "Nuvio custom Collections" }),
  ).toBeVisible();
  await expect(page.getByText("Not imported")).toBeVisible();
  await expect(
    page.getByText(
      "Nuvio Collections response violates the generated contract",
    ),
  ).toHaveCount(0);
});
