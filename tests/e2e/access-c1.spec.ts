import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page, type Route } from "@playwright/test";
import {
  PUBLIC_PROBLEM_CATALOG,
  type AccessProjectionResponse,
  type ProblemDetails,
  type ReadTrailBaseContinuationResponse,
} from "@fasti/sdk";
import { expectNoHorizontalOverflow } from "./test-helpers";

const browserOrigin = "http://127.0.0.1:4173";
const csrf = "a".repeat(64);

const currentSession = {
  browser_session_id: "ses_018f0e0e7f7b70008000000000000000",
  workspace_id: "wsp_018f0e0e7f7b70008000000000000000",
  selected_profile_grant_id: "grt_018f0e0e7f7b70008000000000000000",
  is_current: true,
  created_at: "2026-08-31T12:00:00Z",
  last_seen_at: "2026-08-31T12:01:00Z",
  idle_expires_at: "2099-08-31T12:31:00Z",
  absolute_expires_at: "2099-08-31T20:00:00Z",
  rotation_generation: 1,
} as const;

const otherSession = {
  ...currentSession,
  browser_session_id: "ses_018f0e0e7f7b70008000000000000001",
  is_current: false,
  last_seen_at: "2026-08-31T11:00:00Z",
} as const;

function projection(
  overrides: Partial<AccessProjectionResponse> = {},
): AccessProjectionResponse {
  return {
    generated_at: "2026-08-31T12:01:00Z",
    subject: {
      auth_subject_id: "sub_018f0e0e7f7b70008000000000000000",
      lifecycle: "active",
      created_at: "2026-08-31T12:00:00Z",
      updated_at: "2026-08-31T12:01:00Z",
    },
    membership: {
      membership_id: "mem_018f0e0e7f7b70008000000000000000",
      workspace_id: currentSession.workspace_id,
      lifecycle: "active",
      role: "administrator",
      created_at: "2026-08-31T12:00:00Z",
      updated_at: "2026-08-31T12:01:00Z",
    },
    current_session: currentSession,
    sessions: [currentSession, otherSession],
    sessions_truncated: false,
    profile_grants: [
      {
        profile_grant_id: currentSession.selected_profile_grant_id,
        profile_id: "prf_018f0e0e7f7b70008000000000000000",
        owner_client_id: "cli_018f0e0e7f7b70008000000000000000",
        selected: true,
      },
    ],
    profile_grants_truncated: false,
    session_policy: {
      idle_timeout_seconds: 1_800,
      browser_lifetime_seconds: 28_800,
      remembered_browser_lifetime_seconds: 2_592_000,
      last_seen_write_interval_seconds: 60,
    },
    authentication: {
      method: "trail_base_password",
      verified_at: "2026-08-31T12:00:00Z",
      activation_generation: 1,
      recent_authentication: { state: "unavailable", expires_at: null },
    },
    trailbase: {
      state: "active",
      blocker: null,
      trailbase_instance_id: "tbi_018f0e0e7f7b70008000000000000000",
      generation: 1,
      session_generation_current: true,
      updated_at: "2026-08-31T12:00:00Z",
    },
    first_run_steps: [
      { key: "account_confirmed", state: "verified" },
      { key: "strong_sign_in", state: "needs_attention" },
      { key: "recovery", state: "unavailable" },
      { key: "devices_and_clients", state: "unavailable" },
      { key: "external_identity", state: "unavailable" },
    ],
    evidence: [
      {
        kind: "current_session_issued",
        state: "verified",
        operation_id: "op_018f0e0e7f7b70008000000000000000",
        correlation_id: "req_018f0e0e7f7b70008000000000000000",
        ceremony_state: "completed",
        failure: null,
        occurred_at: "2026-08-31T12:01:00Z",
      },
    ],
    evidence_truncated: false,
    ...overrides,
  };
}

function trailBaseContinuation(
  overrides: Partial<ReadTrailBaseContinuationResponse> = {},
): ReadTrailBaseContinuationResponse {
  return {
    candidate_revision: `sha256:${"b".repeat(64)}`,
    expires_at: "2026-08-31T12:05:00Z",
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
    ...overrides,
  };
}

function problem(
  code: ProblemDetails["code"],
  capability_id: ProblemDetails["capability_id"],
  status: number,
): ProblemDetails {
  const canonical = PUBLIC_PROBLEM_CATALOG.problems.find(
    (candidate) =>
      candidate.code === code && candidate.capability_id === capability_id,
  );
  if (!canonical) throw new Error(`missing ${capability_id}.${code} contract`);
  if (canonical.status !== status) throw new Error(`wrong ${code} status`);
  const { param_policy: _paramPolicy, ...contract } = canonical;
  return {
    ...contract,
    actual: null,
    correlation_id: "req_018f0e0e7f7b70008000000000000009",
    violations: [],
  };
}

async function fulfillJson(route: Route, body: unknown, status = 200) {
  await route.fulfill({
    status,
    contentType:
      status >= 400 ? "application/problem+json" : "application/json",
    body: JSON.stringify(body),
  });
}

async function setCsrfCookie(page: Page): Promise<void> {
  await page.context().addCookies([
    {
      name: "__Host-fasti_csrf",
      value: csrf,
      url: "https://127.0.0.1:4173",
      secure: true,
      httpOnly: false,
      sameSite: "Strict",
    },
  ]);
}

async function expectAxeClean(page: Page): Promise<void> {
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
}

async function openBrowserSessions(page: Page): Promise<void> {
  const summary = page
    .getByTestId("account-security-task-map")
    .locator("summary")
    .filter({ hasText: "Browser sessions" });
  await summary.focus();
  await page.keyboard.press("Enter");
  await expect(
    page.getByRole("table", { name: "Active Fasti browser sessions" }),
  ).toBeVisible();
}

async function expectAccessTargets(page: Page): Promise<void> {
  const targets = page.locator(
    '[data-testid="account-security-task-map"] button:visible, [data-testid="account-security-task-map"] summary:visible, [data-testid="account-security-task-map"] a:visible, [data-testid="account-security-task-map"] .remember-browser-check:visible, [data-testid="first-run-guided-setup"] button:visible, [data-testid="first-run-guided-setup"] a:visible, [data-testid="first-run-guided-setup"] .remember-browser-check:visible, [data-testid="first-run-guided-setup"] .continuation-choice:visible',
  );
  const measurements = await targets.evaluateAll((elements) =>
    elements.map((element) => ({
      label: element.getAttribute("aria-label") ?? element.textContent?.trim(),
      width: element.getBoundingClientRect().width,
      height: element.getBoundingClientRect().height,
    })),
  );
  expect(measurements.length).toBeGreaterThan(0);
  expect(
    measurements.filter(({ width, height }) => width < 44 || height < 44),
  ).toEqual([]);
}

test("A is permanent, C is separate, and B stays inside the owning surface", async ({
  page,
}) => {
  const accessRequests: string[] = [];
  await page.route("**/api/access/v1/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    accessRequests.push(path);
    if (path.endsWith("/trailbase/continuation")) {
      return fulfillJson(
        route,
        problem("auth_browser_binding_invalid", "browser.session.create", 401),
        401,
      );
    }
    return fulfillJson(route, projection());
  });

  await page.goto("/settings/account");
  const taskMap = page.getByTestId("account-security-task-map");
  await expect(
    taskMap.getByRole("heading", { name: "Account and security" }),
  ).toBeVisible();
  await expect(taskMap.getByText("2 active browser sessions.")).toBeVisible();
  await expect(taskMap.getByText("Generic OpenID Connect")).toBeVisible();
  await expect(taskMap.getByText("managed Authentik support")).toBeVisible();
  await expect(page.getByTestId("first-run-guided-setup")).toHaveCount(0);
  await expect(page.locator("body")).not.toContainText(
    currentSession.browser_session_id,
  );
  await expect(
    taskMap.getByRole("table", { name: "Active Fasti browser sessions" }),
  ).toBeHidden();
  await openBrowserSessions(page);
  await taskMap.getByText("Security evidence", { exact: true }).click();
  await expect(
    taskMap.getByRole("table", { name: "Recent account security evidence" }),
  ).toBeVisible();
  expect(
    accessRequests.filter((path) => path === "/api/access/v1/projection"),
  ).toHaveLength(1);

  await page.getByRole("button", { name: "Resume setup" }).click();
  await expect(page).toHaveURL(`${browserOrigin}/first-run`);
  const guidedSetup = page.getByTestId("first-run-guided-setup");
  await expect(
    guidedSetup.getByRole("heading", { name: "Secure your Fasti account" }),
  ).toBeFocused();
  await expect(guidedSetup.locator('[aria-current="step"]')).toContainText(
    "Strong sign-in",
  );
  await expect(guidedSetup).toContainText("Save and finish later");
  await expect(guidedSetup).toContainText("Back");
  await expect(page.getByTestId("account-security-task-map")).toHaveCount(0);
  await expect(page.locator('a[href="/evidence"]')).toHaveCount(0);
  expect(
    accessRequests.filter((path) => path === "/api/access/v1/projection"),
  ).toHaveLength(1);
  await expectNoHorizontalOverflow(page);
  await expectAxeClean(page);

  for (const exit of [
    "Manage existing access",
    "Save and finish later",
    "Back",
  ]) {
    await guidedSetup.getByRole("button", { name: exit }).click();
    await expect(page).toHaveURL(`${browserOrigin}/settings/account`);
    await expect(
      page.getByRole("heading", { name: "Account and security" }),
    ).toBeFocused();
    if (exit !== "Back") {
      await page.getByRole("button", { name: "Resume setup" }).click();
      await expect(
        guidedSetup.getByRole("heading", { name: "Secure your Fasti account" }),
      ).toBeFocused();
    }
  }
});

for (const mode of ["light", "dark", "night"] as const) {
  test(`${mode} Access surfaces reflow across the approved viewport matrix`, async ({
    page,
  }) => {
    const consoleErrors: string[] = [];
    page.on("console", (message) => {
      if (message.type() === "error") consoleErrors.push(message.text());
    });
    page.on("pageerror", (error) => consoleErrors.push(error.message));
    await page.addInitScript((themeMode) => {
      localStorage.setItem(
        "fasti-theme-settings",
        JSON.stringify({ mode: themeMode }),
      );
    }, mode);
    await page.route("**/api/access/v1/**", (route) =>
      fulfillJson(route, projection()),
    );

    for (const width of [320, 375, 768, 1440] as const) {
      await page.setViewportSize({ width, height: width < 768 ? 900 : 1_024 });
      await page.goto("/settings/account");
      await expect(page.locator("html")).toHaveAttribute(
        "data-bs-theme",
        mode === "light" ? "light" : "dark",
      );
      await expect(page.locator("html")).toHaveAttribute(
        "data-fasti-theme",
        mode,
      );
      await expect(page.locator("h1")).toHaveCount(1);
      await expect(page.getByTestId("first-run-guided-setup")).toHaveCount(0);
      await openBrowserSessions(page);
      await expect(page).toHaveURL(`${browserOrigin}/settings/account`);
      await expectAccessTargets(page);
      await expectNoHorizontalOverflow(page);
      await expectAxeClean(page);

      await page.goto("/first-run");
      await expect(page.locator("h1")).toHaveCount(1);
      await expect(page.getByTestId("account-security-task-map")).toHaveCount(
        0,
      );
      await expect(
        page
          .getByTestId("first-run-guided-setup")
          .locator('[aria-current="step"]'),
      ).toHaveCount(1);
      await expectAccessTargets(page);
      await expectNoHorizontalOverflow(page);
      await expectAxeClean(page);
    }
    expect(consoleErrors).toEqual([]);
  });
}

test("reduced motion removes the Access loading animation", async ({
  page,
}) => {
  let releaseProjection!: () => void;
  const projectionHeld = new Promise<void>((resolve) => {
    releaseProjection = resolve;
  });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.route("**/api/access/v1/projection", async (route) => {
    await projectionHeld;
    await fulfillJson(route, projection());
  });

  await page.goto("/settings/account");
  const spinner = page.locator(".access-spinner");
  await expect(spinner).toBeVisible();
  await expect(spinner).toHaveCSS("animation-name", "none");
  releaseProjection();
});

test("A, B, and C reflow with WCAG text spacing at 200 percent", async ({
  page,
}) => {
  const applyTextSpacing = () =>
    page.addStyleTag({
      content: `
        html { font-size: 200% !important; }
        * { line-height: 1.5 !important; letter-spacing: .12em !important; word-spacing: .16em !important; }
        p { margin-bottom: 2em !important; }
      `,
    });
  await page.setViewportSize({ width: 320, height: 900 });
  await page.route("**/api/access/v1/**", (route) =>
    fulfillJson(route, projection()),
  );
  await page.goto("/settings/account");
  await applyTextSpacing();
  await expect(page.locator("html")).toHaveCSS("font-size", "32px");
  await openBrowserSessions(page);
  await expectNoHorizontalOverflow(page);
  await expectAccessTargets(page);

  await page.goto("/first-run");
  await applyTextSpacing();
  await expect(page.locator("html")).toHaveCSS("font-size", "32px");
  await expectNoHorizontalOverflow(page);
  await expectAccessTargets(page);
});

for (const width of [320, 1440] as const) {
  test(`forced colors preserves A, B, and C at ${width}px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: width === 320 ? 900 : 1_024 });
    await page.emulateMedia({ forcedColors: "active" });
    await page.route("**/api/access/v1/**", (route) =>
      fulfillJson(route, projection()),
    );

    await page.goto("/settings/account");
    await openBrowserSessions(page);
    await expectNoHorizontalOverflow(page);
    await expectAxeClean(page);

    await page.goto("/first-run");
    await expectNoHorizontalOverflow(page);
    await expectAxeClean(page);
  });
}

test("one shared projection read owns navigation into Account and security", async ({
  page,
}) => {
  let projectionReads = 0;
  let releaseProjection!: () => void;
  const projectionHeld = new Promise<void>((resolve) => {
    releaseProjection = resolve;
  });
  await page.route("**/api/access/v1/projection", async (route) => {
    projectionReads += 1;
    await projectionHeld;
    await fulfillJson(route, projection());
  });

  await page.goto("/");
  await expect.poll(() => projectionReads).toBe(1);
  await page.evaluate(() => {
    history.pushState({}, "", "/settings/account");
    dispatchEvent(new PopStateEvent("popstate"));
  });
  releaseProjection();

  await expect(
    page.getByTestId("account-security-task-map").getByRole("heading", {
      name: "Account and security",
    }),
  ).toBeVisible();
  expect(projectionReads).toBe(1);
});

test("the account shortcut reports unknown state after a transient projection failure", async ({
  page,
}) => {
  await page.route("**/api/access/v1/projection", (route) =>
    fulfillJson(
      route,
      problem("storage_unavailable", "access.projection.read", 503),
      503,
    ),
  );

  await page.goto("/");
  await page.getByRole("button", { name: "Open account access" }).click();
  const dialog = page.getByRole("dialog", { name: "Account access" });
  await expect(dialog.getByText("Access state unavailable")).toBeVisible();
  await expect(dialog.getByText("Sign-in required")).toHaveCount(0);
});

test("a scoped web host never sends its bearer credential to cookie-only Access", async ({
  page,
}) => {
  const authorizationHeaders: Array<string | undefined> = [];
  await page.route("**/api/access/v1/projection", async (route) => {
    authorizationHeaders.push(route.request().headers().authorization);
    await fulfillJson(route, projection());
  });
  await page.goto("/");
  await page.evaluate(async () => {
    const { createWebHost } = await import("/src/web-host.ts");
    const host = createWebHost(window.location.origin, "scoped-test-token");
    await host.readAccessProjection?.();
  });

  expect(authorizationHeaders.length).toBeGreaterThanOrEqual(2);
  expect(authorizationHeaders).toEqual(
    authorizationHeaders.map(() => undefined),
  );
});

test("an expired browser-session deadline clears profile authority", async ({
  page,
}) => {
  await page.route("**/api/access/v1/projection", (route) =>
    fulfillJson(
      route,
      projection({
        current_session: {
          ...currentSession,
          idle_expires_at: new Date(Date.now() - 1_000).toISOString(),
        },
      }),
    ),
  );

  await page.goto("/discover");
  await expect(page.getByRole("alert")).toContainText(
    "Sign in to use configured metadata providers",
  );
});

test("window focus revalidates cached browser-session authority", async ({
  page,
}) => {
  let revoked = false;
  let revokedReads = 0;
  await page.route("**/api/access/v1/projection", (route) => {
    if (!revoked) return fulfillJson(route, projection());
    revokedReads += 1;
    return fulfillJson(
      route,
      problem("browser_session_revoked", "access.projection.read", 401),
      401,
    );
  });

  await page.goto("/settings/account");
  await expect(page.getByText("2 active browser sessions.")).toBeVisible();
  revoked = true;
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  await page.getByRole("link", { name: "Discover" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "Sign in to use configured metadata providers",
  );
  expect(revokedReads).toBe(1);
});

test("a failed TrailBase callback preserves exact evidence and can be dismissed", async ({
  page,
}) => {
  let deletes = 0;
  await page.route("**/api/access/v1/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (path.endsWith("/trailbase/continuation")) {
      if (request.method() === "DELETE") {
        deletes += 1;
        return route.fulfill({ status: 204 });
      }
      return fulfillJson(
        route,
        problem("trailbase_trust_unavailable", "browser.session.create", 503),
        503,
      );
    }
    return fulfillJson(
      route,
      problem("browser_session_expired", "access.projection.read", 401),
      401,
    );
  });

  await page.goto("/first-run?auth=failed");
  await expect(
    page.getByRole("heading", { name: "TrailBase trust unavailable" }),
  ).toBeVisible();
  await expect(page.getByText("retry after correction")).toBeVisible();
  await expect(page.getByRole("button", { name: "Retry" })).toHaveCount(0);
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Dismiss saved evidence" }).click();
  await expect(
    page.getByText("saved sign-in evidence was dismissed"),
  ).toBeFocused();
  expect(deletes).toBe(1);
});

test("changed continuation choices are reloaded before a nonzero choice is accepted", async ({
  page,
}) => {
  let reads = 0;
  const posts: unknown[] = [];
  const changedContinuation = trailBaseContinuation({
    candidate_revision: `sha256:${"d".repeat(64)}`,
    choices: [
      ...trailBaseContinuation().choices,
      {
        choice_ordinal: 1,
        membership_state: "active",
        profile_created_at: "2026-08-30T12:00:00Z",
        profile_ordinal: 2,
        role: "member",
        workspace_created_at: "2026-08-30T12:00:00Z",
        workspace_ordinal: 2,
      },
    ],
  });
  await page.route("**/api/access/v1/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (path.endsWith("/trailbase/continuation")) {
      if (request.method() === "GET") {
        reads += 1;
        return fulfillJson(
          route,
          reads === 1 ? trailBaseContinuation() : changedContinuation,
        );
      }
      posts.push(request.postDataJSON());
      if (posts.length === 1) {
        return fulfillJson(
          route,
          problem("auth_selection_changed", "browser.session.create", 409),
          409,
        );
      }
      return route.fulfill({ status: 204 });
    }
    return fulfillJson(route, projection());
  });

  await page.goto("/first-run?auth=continue");
  await page.getByLabel("Workspace 1, profile 1").check();
  await page.getByRole("button", { name: "Confirm access" }).click();
  await expect(page.getByText(/available access changed/i)).toBeVisible();
  await expect(page.getByLabel("Workspace 2, profile 2")).toBeVisible();
  await page.getByLabel("Workspace 2, profile 2").check();
  await page.getByRole("button", { name: "Confirm access" }).click();

  expect(posts).toEqual([
    {
      candidate_revision: trailBaseContinuation().candidate_revision,
      choice_ordinal: 0,
    },
    {
      candidate_revision: changedContinuation.candidate_revision,
      choice_ordinal: 1,
    },
  ]);
});

test("canonical Access states and bounded evidence remain explicit", async ({
  page,
}) => {
  let state: AccessProjectionResponse["first_run_steps"][number]["state"] =
    "empty";
  await page.route("**/api/access/v1/**", (route) =>
    fulfillJson(
      route,
      projection({
        sessions_truncated: true,
        profile_grants_truncated: true,
        evidence_truncated: true,
        first_run_steps: projection().first_run_steps.map((step) =>
          step.key === "strong_sign_in" || step.key === "recovery"
            ? { ...step, state }
            : step,
        ),
      }),
    ),
  );

  for (const expected of [
    "empty",
    "loading",
    "needs_attention",
    "failed_safely",
    "unavailable",
    "verified",
  ] as const) {
    state = expected;
    await page.goto("/settings/account");
    await expect(
      page
        .getByTestId("account-security-task-map")
        .locator(".task-row")
        .filter({ hasText: "Sign-in methods" })
        .locator(".badge"),
    ).toHaveText(expected.replaceAll("_", " "));
  }

  await openBrowserSessions(page);
  await expect(
    page.getByText("does not include every active browser session"),
  ).toBeVisible();
  await expect(
    page.getByText("does not include every available media profile grant"),
  ).toBeVisible();
  await page.getByText("Security evidence", { exact: true }).click();
  await expect(
    page.getByText("does not include every security evidence item"),
  ).toBeVisible();
});

test("ending the current session removes profile authority", async ({
  page,
}) => {
  let ended = 0;
  await setCsrfCookie(page);
  await page.route("**/api/access/v1/**", async (route) => {
    const request = route.request();
    if (request.method() === "DELETE") {
      ended += 1;
      expect(request.headers()["x-csrf-token"]).toBe(csrf);
      return route.fulfill({ status: 204 });
    }
    if (ended > 0) {
      return fulfillJson(
        route,
        problem("browser_session_expired", "access.projection.read", 401),
        401,
      );
    }
    return fulfillJson(route, projection());
  });

  await page.goto("/settings/account");
  await openBrowserSessions(page);
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Sign out this browser" }).click();
  await expect(
    page.getByRole("heading", { name: "Confirm account access" }),
  ).toBeVisible();
  await page.goto("/discover");
  await expect(page.getByRole("alert")).toContainText(
    "Sign in to use configured metadata providers",
  );
  expect(ended).toBe(1);
});

test("first-run keeps deferred packaged bootstrap visible with the Unix recovery path", async ({
  page,
}) => {
  await page.route("**/api/access/v1/**", (route) => {
    const path = new URL(route.request().url()).pathname;
    return path.endsWith("/trailbase/continuation")
      ? fulfillJson(
          route,
          problem(
            "auth_browser_binding_invalid",
            "browser.session.create",
            401,
          ),
          401,
        )
      : fulfillJson(
          route,
          problem("browser_session_expired", "access.projection.read", 401),
          401,
        );
  });

  await page.goto("/first-run");
  const setup = page.getByTestId("first-run-guided-setup");
  await expect(
    setup.getByRole("button", { name: "Confirm first Fasti administrator" }),
  ).toBeDisabled();
  await expect(setup.getByRole("status")).toContainText(
    "The packaged WebView cannot yet retain the required Secure callback cookie.",
  );
  await expect(setup.getByRole("status")).toContainText(
    "fasti access bootstrap-administrator",
  );
  await expect(
    setup.getByRole("button", { name: "Sign in to an existing account" }),
  ).toBeEnabled();
});

test("the callback hint is scrubbed and one explicit continuation choice is posted once", async ({
  page,
}) => {
  const continuation = trailBaseContinuation();
  const posted: unknown[] = [];
  await page.route("**/api/access/v1/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (
      path.endsWith("/trailbase/continuation") &&
      request.method() === "GET"
    ) {
      return fulfillJson(route, continuation);
    }
    if (
      path.endsWith("/trailbase/continuation") &&
      request.method() === "POST"
    ) {
      posted.push(request.postDataJSON());
      return route.fulfill({ status: 204 });
    }
    return fulfillJson(route, projection());
  });

  await page.goto(
    "/settings/account?auth=continue&correlation_id=req_018f0e0e7f7b70008000000000000009&source=trailbase#resume",
  );
  await expect(page).toHaveURL(
    `${browserOrigin}/settings/account?source=trailbase#resume`,
  );
  const confirm = page.getByRole("button", { name: "Confirm access" });
  await expect(confirm).toBeDisabled();
  await page.getByLabel("Workspace 1, profile 1").check();
  await confirm.dblclick();
  await expect(page.getByText("Account access confirmed")).toBeVisible();
  expect(posted).toEqual([
    {
      candidate_revision: continuation.candidate_revision,
      choice_ordinal: 0,
    },
  ]);
});

test("Save and leave preserves continuation while Cancel sign-in deletes it", async ({
  page,
}) => {
  let deletes = 0;
  const continuation = trailBaseContinuation();
  await page.route("**/api/access/v1/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (path.endsWith("/trailbase/continuation")) {
      if (request.method() === "DELETE") {
        deletes += 1;
        return route.fulfill({ status: 204 });
      }
      return fulfillJson(route, continuation);
    }
    return fulfillJson(
      route,
      problem("browser_session_expired", "access.projection.read", 401),
      401,
    );
  });

  await page.goto("/first-run");
  await page.getByRole("button", { name: "Save and leave" }).click();
  await expect(page).toHaveURL(`${browserOrigin}/settings/account`);
  expect(deletes).toBe(0);

  await page.goto("/first-run");
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Cancel sign-in" }).click();
  await expect(
    page.getByText("saved sign-in evidence was dismissed"),
  ).toBeFocused();
  expect(deletes).toBe(1);
});

test("session actions stay on the immutable same-origin Access client", async ({
  page,
}) => {
  let revokeRequests = 0;
  let currentProjection = projection();
  await page.addInitScript(() => {
    localStorage.setItem(
      "fasti-network-config",
      JSON.stringify({ service_url: "https://remote.fasti.test" }),
    );
  });
  await setCsrfCookie(page);
  await page.route("**/api/access/v1/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    expect(new URL(request.url()).origin).toBe(browserOrigin);
    if (request.method() === "DELETE" && path.includes("/browser-sessions/")) {
      revokeRequests += 1;
      expect(request.headers()["x-csrf-token"]).toBe(csrf);
      currentProjection = projection({ sessions: [currentSession] });
      return fulfillJson(route, { revoked_count: 1 });
    }
    return fulfillJson(route, currentProjection);
  });

  await page.goto("/settings/account");
  await openBrowserSessions(page);
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: /^Revoke session last used/ }).click();
  await expect(
    page.getByText("selected browser session was revoked"),
  ).toBeFocused();
  expect(revokeRequests).toBe(1);
});

test("a governed no-mutation rotation failure restores cached authority", async ({
  page,
}) => {
  await setCsrfCookie(page);
  await page.route("**/api/access/v1/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (
      request.method() === "POST" &&
      path === "/api/access/v1/browser-session/rotation"
    ) {
      return fulfillJson(
        route,
        problem("storage_unavailable", "browser.session.rotate", 503),
        503,
      );
    }
    return fulfillJson(route, projection());
  });

  await page.goto("/settings/account");
  await openBrowserSessions(page);
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Rotate this session" }).click();
  await expect(
    page.getByRole("heading", { name: "Storage unavailable" }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toBeFocused();

  await page.getByRole("button", { name: "Open account access" }).click();
  const dialog = page.getByRole("dialog", { name: "Account access" });
  await expect(dialog.getByText("Signed in", { exact: true })).toBeVisible();
});

test("a committed revocation cannot resurrect stale session inventory", async ({
  page,
}) => {
  let projectionReads = 0;
  await setCsrfCookie(page);
  await page.route("**/api/access/v1/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (request.method() === "GET" && path.endsWith("/projection")) {
      projectionReads += 1;
      if (projectionReads >= 2 && projectionReads <= 4) {
        return fulfillJson(
          route,
          problem("storage_unavailable", "access.projection.read", 503),
          503,
        );
      }
      return fulfillJson(
        route,
        projectionReads === 1
          ? projection()
          : projection({ sessions: [currentSession] }),
      );
    }
    if (request.method() === "DELETE") {
      return fulfillJson(route, { revoked_count: 1 });
    }
    return fulfillJson(route, projection({ sessions: [currentSession] }));
  });

  await page.goto("/settings/account");
  await openBrowserSessions(page);
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: /^Revoke session last used/ }).click();
  await expect(
    page.getByRole("heading", { name: "Storage unavailable" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Dismiss saved evidence" }),
  ).toHaveCount(0);

  await page.getByRole("button", { name: "Retry" }).click();
  await openBrowserSessions(page);
  await expect(page.getByText("1 active browser session.")).toBeVisible();
  await expect(
    page.getByRole("button", { name: /^Revoke session last used/ }),
  ).toHaveCount(0);
  expect(projectionReads).toBe(5);
});

test("completed first-run setup returns to A with a focused completion notice", async ({
  page,
}) => {
  const continuation = trailBaseContinuation({
    candidate_revision: `sha256:${"c".repeat(64)}`,
  });
  const completeProjection = projection({
    first_run_steps: projection().first_run_steps.map((step) => ({
      ...step,
      state: "verified",
    })),
  });
  await page.route("**/api/access/v1/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (path.endsWith("/trailbase/continuation")) {
      if (request.method() === "POST") return route.fulfill({ status: 204 });
      return fulfillJson(route, continuation);
    }
    return fulfillJson(route, completeProjection);
  });

  await page.goto("/first-run?auth=continue");
  await page.getByLabel("Workspace 1, profile 1").check();
  await page.getByRole("button", { name: "Confirm access" }).click();

  await expect(page).toHaveURL(`${browserOrigin}/settings/account`);
  await expect(page.getByTestId("first-run-guided-setup")).toHaveCount(0);
  await expect(
    page.getByTestId("account-security-task-map").getByRole("heading", {
      name: "Account and security",
    }),
  ).toBeVisible();
  await expect(page.getByText("Account setup is complete.")).toBeFocused();
});
