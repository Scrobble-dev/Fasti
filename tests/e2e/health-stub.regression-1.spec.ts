import { expect, test } from "@playwright/test";

// Regression: ISSUE-001 — the bounded QA stub returned arrays instead of list envelopes.
// Found by /qa on 2026-08-31.
// Report: .gstack/qa-reports/qa-report-127-0-0-1-2026-08-31.md
test("bounded empty pages satisfy the Workbench contracts", async ({
  page,
  request,
}) => {
  const records = await request.get("http://127.0.0.1:18422/api/v1/records");
  await expect(records.json()).resolves.toEqual({
    records: [],
    truncated: false,
  });

  const dispositions = await request.get(
    "http://127.0.0.1:18422/api/v1/profile/record-tracking-dispositions",
  );
  await expect(dispositions.json()).resolves.toEqual({
    states: [],
    truncated: false,
  });

  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
});
