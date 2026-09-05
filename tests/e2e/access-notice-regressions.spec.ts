import { expect, test } from "@playwright/test";
import {
  parseAccessProjectionResponse,
  parseReadTrailBaseContinuationResponse,
  PUBLIC_PROBLEM_CATALOG,
} from "@fasti/sdk";
import { expectNoHorizontalOverflow } from "./test-helpers";

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

for (const theme of ["light", "dark"] as const) {
  for (const scenario of [
    { width: 320, enlarged: false },
    { width: 375, enlarged: false },
    { width: 768, enlarged: false },
    { width: 1440, enlarged: false },
    { width: 320, enlarged: true },
  ]) {
    test(`notice space preserves choice position: ${theme} ${scenario.width}px${scenario.enlarged ? " enlarged text" : ""}`, async ({
      page,
    }, info) => {
      await page.setViewportSize({ width: scenario.width, height: 1024 });
      await page.addInitScript((mode) => {
        localStorage.setItem("fasti-theme-settings", JSON.stringify({ mode }));
      }, theme);
      const continuation = parseReadTrailBaseContinuationResponse({
        candidate_revision: `sha256:${"b".repeat(64)}`,
        expires_at: "2099-08-31T12:05:00Z",
        remembered: false,
        choices: [
          {
            choice_ordinal: 0,
            membership_state: "active",
            profile_created_at: "2026-08-31T12:00:00Z",
            profile_ordinal: 1,
            role: "administrator",
            workspace_created_at: "2026-08-31T12:00:00Z",
            workspace_ordinal: 1,
          },
        ],
      });
      const entry = PUBLIC_PROBLEM_CATALOG.problems.find(
        ({ code, capability_id }) =>
          code === "auth_selection_changed" &&
          capability_id === "browser.session.create",
      );
      if (!entry) throw new Error("Missing canonical selection problem");
      const { param_policy: _policy, ...contract } = entry;
      let reads = 0;
      const mutations: string[] = [];
      await page.route("**/api/access/v1/trailbase/continuation", (route) => {
        const request = route.request();
        if (request.method() === "GET") {
          reads += 1;
          return route.fulfill({
            status: 200,
            contentType: "application/json",
            body: JSON.stringify({
              ...continuation,
              candidate_revision: `sha256:${(reads === 1 ? "b" : "d").repeat(64)}`,
            }),
          });
        }
        mutations.push(
          `${request.method()} ${new URL(request.url()).pathname}`,
        );
        expect(request.postDataJSON()).toEqual({
          candidate_revision: continuation.candidate_revision,
          choice_ordinal: 0,
        });
        return route.fulfill({
          status: 409,
          contentType: "application/problem+json",
          body: JSON.stringify({
            ...contract,
            actual: null,
            violations: [],
            correlation_id: "req_018f0e0e7f7b70008000000000000009",
          }),
        });
      });
      await page.goto("/first-run?auth=continue");
      await expect(page.locator("html")).toHaveAttribute(
        "data-bs-theme",
        theme,
      );
      const choice = page.getByRole("radio", {
        name: /Workspace 1, profile 1/,
      });
      await expect(choice).toBeVisible();
      if (scenario.enlarged) {
        await page.addStyleTag({
          content: `
          html { font-size: 200% !important; }
          * { line-height: 1.5 !important; letter-spacing: .12em !important; word-spacing: .16em !important; }
          p { margin-bottom: 2em !important; }
        `,
        });
      }
      const region = page.locator(".access-notice-region");
      await expect(region).toHaveAttribute("aria-live", "polite");
      await expect(region).toHaveAttribute("aria-atomic", "true");
      await expect(region).not.toHaveAttribute("role");
      await expect(region).toBeEmpty();
      await expect(region).not.toHaveAttribute("tabindex");
      const top = () =>
        choice.evaluate(
          (element) => element.getBoundingClientRect().top + window.scrollY,
        );
      const before = await top();
      await choice.focus();
      await page.keyboard.press("Space");
      await page.getByRole("button", { name: "Confirm access" }).press("Enter");
      const notice = page.locator("#access-notice");
      await expect(notice).toHaveText(
        "Your available access changed. Review the current choices.",
      );
      await expect(region).toHaveAttribute("role", "status");
      await expect(
        page.getByRole("heading", { name: "Secure your Fasti account" }),
      ).toBeFocused();
      await expect(choice).not.toBeChecked();
      await expect(
        page.getByRole("button", { name: "Confirm access" }),
      ).toBeDisabled();
      await expect(region.locator('[role="status"]')).toHaveCount(0);
      const after = await top();
      if (!scenario.enlarged)
        expect(Math.abs(after - before)).toBeLessThanOrEqual(1);
      const bounds = await notice.evaluate((element) => ({
        height: element.clientHeight,
        content: element.scrollHeight,
        region: element.parentElement!.getBoundingClientRect().height,
        notice: element.getBoundingClientRect().height,
      }));
      expect(bounds.content).toBeLessThanOrEqual(bounds.height + 1);
      expect(bounds.notice).toBeLessThanOrEqual(bounds.region + 1);
      await expectNoHorizontalOverflow(page);
      await info.attach("notice-geometry", {
        body: JSON.stringify({ before, after, ...bounds }),
        contentType: "application/json",
      });
      await page.screenshot({
        path: info.outputPath("notice.png"),
        fullPage: true,
      });
      expect(reads).toBe(2);
      expect(mutations).toEqual(["POST /api/access/v1/trailbase/continuation"]);
    });
  }
}
