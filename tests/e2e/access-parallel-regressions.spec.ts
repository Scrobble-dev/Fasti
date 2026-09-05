import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page, type Route } from "@playwright/test";
import {
  parseAccessProjectionResponse,
  parseReadTrailBaseContinuationResponse,
  PUBLIC_PROBLEM_CATALOG,
  type ProblemDetails,
} from "@fasti/sdk";
import { expectNoHorizontalOverflow } from "./test-helpers";

// Browser-only fixtures. Reuse the existing harness projection, not a second DTO.
async function projection(page: Page) {
  const response = await page.request.get(
    "http://127.0.0.1:18422/api/access/v1/projection",
  );
  expect(response.ok()).toBe(true);
  return parseAccessProjectionResponse(await response.json());
}

async function json(route: Route, body: unknown, status = 200) {
  await route.fulfill({
    status,
    contentType:
      status >= 400 ? "application/problem+json" : "application/json",
    body: JSON.stringify(body),
  });
}

function problem(
  code: ProblemDetails["code"],
  capability: ProblemDetails["capability_id"],
) {
  const canonical = PUBLIC_PROBLEM_CATALOG.problems.find(
    (entry) => entry.code === code && entry.capability_id === capability,
  );
  if (!canonical) throw new Error(`Missing problem: ${capability}.${code}`);
  const { param_policy: _policy, ...contract } = canonical;
  return {
    ...contract,
    actual: null,
    violations: [],
    correlation_id: "req_018f0e0e7f7b70008000000000000009",
  };
}

function continuation(revised = false) {
  const choice = {
    choice_ordinal: 0,
    membership_state: "active",
    profile_created_at: "2026-08-31T12:00:00Z",
    profile_ordinal: 1,
    role: "administrator",
    workspace_created_at: "2026-08-31T12:00:00Z",
    workspace_ordinal: 1,
  };
  return parseReadTrailBaseContinuationResponse({
    candidate_revision: `sha256:${(revised ? "d" : "b").repeat(64)}`,
    expires_at: "2099-08-31T12:05:00Z",
    remembered: false,
    choices: revised
      ? [
          choice,
          {
            ...choice,
            choice_ordinal: 1,
            profile_ordinal: 2,
            workspace_ordinal: 2,
            role: "member",
          },
        ]
      : [choice],
  });
}

test("resume uses newly confirmed steps without sending a mutation", async ({
  page,
}) => {
  let current = await projection(page);
  const mutations: string[] = [];
  await page.route("**/api/access/v1/**", (route) => {
    if (route.request().method() !== "GET")
      mutations.push(route.request().method());
    return json(route, current);
  });
  await page.goto("/settings/account");
  await page.getByRole("button", { name: "Resume setup" }).click();
  const setup = page.getByTestId("first-run-guided-setup");
  await expect(setup.locator('[aria-current="step"]')).toHaveText(
    /Strong sign-in/,
  );
  await page.getByRole("button", { name: "Save and finish later" }).click();
  await expect(
    page.getByRole("heading", { name: "Account and security" }),
  ).toBeFocused();
  current = parseAccessProjectionResponse({
    ...current,
    first_run_steps: current.first_run_steps.map((step) =>
      step.key === "strong_sign_in"
        ? { ...step, state: "verified" }
        : step.key === "recovery"
          ? { ...step, state: "needs_attention" }
          : step,
    ),
  });
  await page.reload();
  await expect(page.getByText("1 active browser session.")).toBeVisible();
  await page.getByRole("button", { name: "Resume setup" }).click();
  await expect(
    setup.getByRole("heading", { name: "Secure your Fasti account" }),
  ).toBeFocused();
  await expect(setup.locator('[aria-current="step"]')).toHaveCount(1);
  await expect(setup.locator('[aria-current="step"]')).toHaveText(/Recovery/);
  await expect(
    setup.getByRole("listitem").filter({ hasText: "Strong sign-in" }),
  ).toContainText("verified");
  expect(mutations).toEqual([]);
  await expectNoHorizontalOverflow(page);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test("all setup exit controls remain disabled while confirmation is pending", async ({
  page,
}, testInfo) => {
  const current = await projection(page);
  const posts: unknown[] = [];
  const mutations: string[] = [];
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/access/v1/**", async (route) => {
    const request = route.request();
    if (request.method() !== "GET")
      mutations.push(`${request.method()} ${new URL(request.url()).pathname}`);
    if (new URL(request.url()).pathname.endsWith("/trailbase/continuation")) {
      if (request.method() === "GET") return json(route, continuation());
      if (request.method() !== "POST") return route.fulfill({ status: 405 });
      posts.push(request.postDataJSON());
      await pending;
      return route.fulfill({ status: 204 });
    }
    return json(route, current);
  });
  try {
    await page.goto("/first-run?auth=continue");
    await page.getByRole("radio", { name: /Workspace 1, profile 1/ }).check();
    await page.getByRole("button", { name: "Confirm access" }).click();
    await expect.poll(() => posts.length).toBe(1);
    await expect(
      page.getByRole("button", { name: "Confirming…" }),
    ).toBeDisabled();
    await page.screenshot({
      path: testInfo.outputPath("confirmation-pending.png"),
    });
    for (const name of [
      "Save and leave",
      "Cancel sign-in",
      "Manage existing access",
    ]) {
      await expect.soft(page.getByRole("button", { name })).toBeDisabled();
    }
    await page.getByRole("button", { name: "Manage existing access" }).focus();
    await page.keyboard.press("Enter");
    await page.screenshot({
      path: testInfo.outputPath("after-header-keyboard.png"),
    });
    await expect.soft(page).toHaveURL(/\/first-run$/);
    await expect.soft(page.getByTestId("first-run-guided-setup")).toBeVisible();
    expect(mutations).toEqual(["POST /api/access/v1/trailbase/continuation"]);
  } finally {
    release();
  }
  await expect(
    page.getByText(
      "Account access confirmed. Review the remaining security tasks.",
    ),
  ).toBeVisible();
  await expect(page).toHaveURL(/\/first-run$/);
  await expect(
    page.getByTestId("first-run-guided-setup").locator('[aria-current="step"]'),
  ).toHaveText(/Strong sign-in/);
  expect(posts).toEqual([
    {
      candidate_revision: continuation().candidate_revision,
      choice_ordinal: 0,
    },
  ]);
  expect(mutations).toEqual(["POST /api/access/v1/trailbase/continuation"]);
});

test("dismissing cancellation preserves choice and returns keyboard focus", async ({
  page,
}) => {
  const current = await projection(page);
  const mutations: string[] = [];
  await page.route("**/api/access/v1/**", (route) => {
    const request = route.request();
    if (request.method() !== "GET") mutations.push(request.method());
    return json(
      route,
      new URL(request.url()).pathname.endsWith("/trailbase/continuation")
        ? continuation()
        : current,
    );
  });
  await page.goto("/first-run?auth=continue");
  const choice = page.getByRole("radio", { name: /Workspace 1, profile 1/ });
  await choice.check();
  let dialogs = 0;
  page.once("dialog", async (dialog) => {
    dialogs += 1;
    await dialog.dismiss();
  });
  const cancel = page.getByRole("button", { name: "Cancel sign-in" });
  await cancel.focus();
  await page.keyboard.press("Enter");
  await expect.poll(() => dialogs).toBe(1);
  await expect(cancel).toBeFocused();
  await expect(choice).toBeChecked();
  await expect(page).toHaveURL(/\/first-run$/);
  expect(mutations).toEqual([]);
});

for (const deadline of ["idle_expires_at", "absolute_expires_at"] as const) {
  test(`${deadline} removes displayed authority without focus or reread`, async ({
    page,
  }) => {
    const now = new Date("2026-09-05T12:00:00Z");
    await page.clock.install({ time: now });
    const base = await projection(page);
    const current = parseAccessProjectionResponse({
      ...base,
      current_session: {
        ...base.current_session,
        [deadline]: new Date(now.getTime() + 60_000).toISOString(),
      },
    });
    let reads = 0;
    const mutations: string[] = [];
    await page.route("**/api/access/v1/**", (route) => {
      if (route.request().method() === "GET") reads += 1;
      else mutations.push(route.request().method());
      return json(route, current);
    });
    await page.goto("/discover");
    await page.getByRole("button", { name: "Open account access" }).click();
    const dialog = page.getByRole("dialog", { name: "Account access" });
    await expect(dialog.getByText("Signed in", { exact: true })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    const initialReads = reads;
    await page.clock.fastForward(61_000);
    await expect(page.getByRole("alert")).toContainText(
      "Sign in to use configured metadata providers",
    );
    expect(reads).toBe(initialReads);
    expect(mutations).toEqual([]);
  });
}

test("retry after committed revocation only rereads and restores focus", async ({
  page,
}) => {
  const base = await projection(page);
  if (!base.current_session)
    throw new Error("Harness requires current session");
  const other = {
    ...base.current_session,
    browser_session_id: "ses_018f0e0e7f7b70008000000000000001",
    is_current: false,
  };
  let reads = 0;
  const mutations: string[] = [];
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
  await page.route("**/api/access/v1/**", (route) => {
    const request = route.request();
    if (request.method() !== "GET") {
      mutations.push(`${request.method()} ${new URL(request.url()).pathname}`);
      return json(route, { revoked_count: 1 });
    }
    reads += 1;
    if (reads >= 2 && reads <= 4)
      return json(
        route,
        problem("storage_unavailable", "access.projection.read"),
        503,
      );
    return json(
      route,
      reads === 1 ? { ...base, sessions: [...base.sessions, other] } : base,
    );
  });
  await page.goto("/settings/account");
  const inventory = page
    .locator("summary")
    .filter({ hasText: "Browser sessions" });
  await inventory.focus();
  await page.keyboard.press("Enter");
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: /^Revoke session last used/ }).click();
  await expect(
    page.getByRole("heading", { name: "Storage unavailable" }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toBeFocused();
  const retry = page.getByRole("button", { name: "Retry" });
  await retry.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByText("1 active browser session.")).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Account and security" }),
  ).toBeFocused();
  await inventory.focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("button", { name: /^Revoke session last used/ }),
  ).toHaveCount(0);
  expect(mutations).toEqual([
    `DELETE /api/access/v1/browser-sessions/${other.browser_session_id}`,
  ]);
  expect(reads).toBe(5);
});

test("keyboard confirmation uses the revised choice and clears the old selection", async ({
  page,
}) => {
  const current = await projection(page);
  let reads = 0;
  const posts: unknown[] = [];
  const mutations: string[] = [];
  await page.route("**/api/access/v1/**", (route) => {
    const request = route.request();
    if (request.method() !== "GET")
      mutations.push(`${request.method()} ${new URL(request.url()).pathname}`);
    if (new URL(request.url()).pathname.endsWith("/trailbase/continuation")) {
      if (request.method() === "GET")
        return json(route, continuation(++reads > 1));
      if (request.method() !== "POST") return route.fulfill({ status: 405 });
      posts.push(request.postDataJSON());
      return posts.length === 1
        ? json(
            route,
            problem("auth_selection_changed", "browser.session.create"),
            409,
          )
        : route.fulfill({ status: 204 });
    }
    return json(route, current);
  });
  await page.goto("/first-run?auth=continue");
  await page.getByRole("radio", { name: /Workspace 1, profile 1/ }).focus();
  await page.keyboard.press("Space");
  await page.keyboard.press("Tab");
  await expect(
    page.getByRole("button", { name: "Confirm access" }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("status")).toContainText(
    "Your available access changed",
  );
  await expect(page.getByRole("radio", { checked: true })).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Confirm access" }),
  ).toBeDisabled();
  await expect
    .soft(page.getByRole("heading", { name: "Secure your Fasti account" }))
    .toBeFocused();
  await page.getByRole("radio", { name: /Workspace 1, profile 1/ }).focus();
  await page.keyboard.press("ArrowDown");
  await expect(
    page.getByRole("radio", { name: /Workspace 2, profile 2/ }),
  ).toBeChecked();
  await page.keyboard.press("Tab");
  await page.keyboard.press("Enter");
  await expect(
    page.getByText(
      "Account access confirmed. Review the remaining security tasks.",
    ),
  ).toBeVisible();
  expect(posts).toEqual([
    {
      candidate_revision: continuation().candidate_revision,
      choice_ordinal: 0,
    },
    {
      candidate_revision: continuation(true).candidate_revision,
      choice_ordinal: 1,
    },
  ]);
  expect(mutations).toEqual([
    "POST /api/access/v1/trailbase/continuation",
    "POST /api/access/v1/trailbase/continuation",
  ]);
});
