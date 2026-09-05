import { expect, test } from "@playwright/test";
import {
  parseAccessProjectionResponse,
  PUBLIC_PROBLEM_CATALOG,
} from "@fasti/sdk";

// Regression: confirmed revocation disappeared during a read-only Retry.
// Found by /qa on 2026-09-05.
// Report: .gstack/qa-reports/qa-report-access-2026-09-05.md
test("confirmed revocation survives failed reads and a held Retry", async ({
  page,
}, info) => {
  const response = await page.request.get(
    "http://127.0.0.1:18422/api/access/v1/projection",
  );
  expect(response.ok()).toBe(true);
  const base = parseAccessProjectionResponse(await response.json());
  if (!base.current_session) throw new Error("Harness needs a current session");
  const other = {
    ...base.current_session,
    browser_session_id: "ses_018f0e0e7f7b70008000000000000001",
    is_current: false,
  };
  const entry = PUBLIC_PROBLEM_CATALOG.problems.find(
    ({ code, capability_id }) =>
      code === "storage_unavailable" &&
      capability_id === "access.projection.read",
  );
  if (!entry) throw new Error("Missing canonical storage problem");
  const { param_policy: _policy, ...contract } = entry;
  let reads = 0;
  const mutations: string[] = [];
  let release!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.context().addCookies([
    {
      name: "__Host-fasti_csrf",
      value: "a".repeat(64),
      url: "https://127.0.0.1:4173",
      secure: true,
      httpOnly: false,
      sameSite: "Strict",
    },
  ]);
  await page.route("**/api/access/v1/**", async (route) => {
    const request = route.request();
    if (request.method() !== "GET") {
      mutations.push(`${request.method()} ${new URL(request.url()).pathname}`);
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ revoked_count: 1 }),
      });
    }
    reads += 1;
    if (reads >= 2 && reads <= 4)
      return route.fulfill({
        status: 503,
        contentType: "application/problem+json",
        body: JSON.stringify({
          ...contract,
          actual: null,
          violations: [],
          correlation_id: "req_018f0e0e7f7b70008000000000000009",
        }),
      });
    if (reads === 5) await held;
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(
        reads === 1 ? { ...base, sessions: [...base.sessions, other] } : base,
      ),
    });
  });
  const notice = page.locator("#access-notice");
  try {
    await page.goto("/settings/account");
    await page
      .locator("summary")
      .filter({ hasText: "Browser sessions" })
      .click();
    page.once("dialog", (dialog) => dialog.accept());
    await page
      .getByRole("button", { name: /^Revoke session last used/ })
      .click();
    await expect(page.getByRole("alert")).toBeFocused();
    await expect(notice).toHaveText(
      "The selected browser session was revoked.",
    );
    await page.screenshot({
      path: info.outputPath("confirmed-with-read-error.png"),
    });
    await page.getByRole("button", { name: "Retry", exact: true }).focus();
    await page.keyboard.press("Enter");
    await expect.poll(() => reads).toBe(5);
    await page.screenshot({ path: info.outputPath("retry-pending.png") });
    await expect(notice).toBeVisible();
    await expect(notice).toHaveText(
      "The selected browser session was revoked.",
    );
  } finally {
    release();
  }
  await expect(page.getByText("1 active browser session.")).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Account and security", exact: true }),
  ).toBeFocused();
  await expect(notice).toHaveText("The selected browser session was revoked.");
  expect(mutations).toEqual([
    `DELETE /api/access/v1/browser-sessions/${other.browser_session_id}`,
  ]);
  expect(reads).toBe(5);
});
