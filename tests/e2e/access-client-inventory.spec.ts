import AxeBuilder from "@axe-core/playwright";
import {
  expect,
  test,
  type Locator,
  type Page,
  type Route,
} from "@playwright/test";
import {
  parseAccessProjectionResponse,
  PUBLIC_PROBLEM_CATALOG,
  type AccessProjectionResponse,
  type ListAccessClientsResponse,
  type ProblemDetails,
} from "@fasti/sdk";
import { expectNoHorizontalOverflow } from "./test-helpers";

const inventoryRoute = "**/api/access/v1/clients*";
const projectionRoute = "**/api/access/v1/projection";
const paths = ["/settings/account", "/first-run"] as const;

function clientPage(count = 32, more = true): ListAccessClientsResponse {
  const clients = Array.from({ length: count }, (_, index) => ({
    client_id: `cli_018f0e0e7f7b700080000000${(index + 1).toString(16).padStart(8, "0")}`,
    owner_subject_id: null,
    name: `Client ${index + 1}`,
    authentication_type: "confidential" as const,
    purpose: "integration" as const,
    lifecycle: "revoked" as const,
    current_credential_epoch: index === 0 ? "9223372036854775807" : "0",
    created_at: `+10000-01-01T00:00:00.${String(999999 - index).padStart(6, "0")}Z`,
  }));
  const last = clients.at(-1);
  return {
    clients,
    next:
      more && last
        ? { client_id: last.client_id, created_at: last.created_at }
        : null,
  };
}

function olderPage(): ListAccessClientsResponse {
  const page = clientPage(1, false);
  return {
    ...page,
    clients: [
      {
        ...page.clients[0]!,
        client_id: "cli_018f0e0e7f7b700080000000000000ff",
        name: "Older client",
        current_credential_epoch: "0",
        created_at: "2016-12-31T23:59:60.999999Z",
      },
    ],
  };
}

function problem(
  code: ProblemDetails["code"],
  capability: ProblemDetails["capability_id"],
): ProblemDetails {
  const canonical = PUBLIC_PROBLEM_CATALOG.problems.find(
    (entry) => entry.code === code && entry.capability_id === capability,
  );
  if (!canonical)
    throw new Error(`Missing canonical problem: ${capability}.${code}`);
  const { param_policy: _paramPolicy, ...contract } = canonical;
  return {
    ...contract,
    actual: null,
    correlation_id: "req_018f0e0e7f7b70008000000000000009",
    violations: [],
  };
}

async function json(route: Route, value: unknown, status = 200): Promise<void> {
  await route.fulfill({
    status,
    contentType:
      status >= 400 ? "application/problem+json" : "application/json",
    body: JSON.stringify(value),
  });
}

async function authenticatedProjection(page: Page): Promise<{
  set(value: AccessProjectionResponse | ProblemDetails): void;
}> {
  // Reuse the canonical QA server fixture; importing its module would start a server.
  const response = await page.request.get(
    "http://127.0.0.1:18422/api/access/v1/projection",
  );
  expect(response.ok()).toBeTruthy();
  let current: AccessProjectionResponse | ProblemDetails =
    parseAccessProjectionResponse(await response.json());
  await page.route(projectionRoute, (route) =>
    json(route, current, "status" in current ? current.status : 200),
  );
  return {
    set(value) {
      current = structuredClone(value);
    },
  };
}

function inventory(page: Page): Locator {
  return page.getByTestId("access-client-inventory");
}

async function openInventory(
  page: Page,
  path: (typeof paths)[number],
): Promise<Locator> {
  await page.goto(path);
  const summary = page
    .locator("summary")
    .filter({ hasText: "Registered clients" });
  await expect(summary).toBeVisible();
  await summary.focus();
  await page.keyboard.press("Enter");
  const region = inventory(page);
  await expect(
    region.getByRole("button", { name: "Load clients", exact: true }),
  ).toBeVisible();
  return region;
}

async function loadWithKeyboard(page: Page, region: Locator): Promise<void> {
  const load = region.getByRole("button", {
    name: "Load clients",
    exact: true,
  });
  await load.focus();
  await expect(load).toBeFocused();
  await page.keyboard.press("Enter");
}

function rows(region: Locator): Locator {
  return region
    .getByRole("table", { name: "Registered application clients" })
    .locator("tbody tr");
}

function deferred(): { promise: Promise<void>; resolve: () => void } {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

async function settleRender(page: Page): Promise<void> {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      ),
  );
}

async function ignoreInventoryAbort(page: Page): Promise<void> {
  // Deliberate fault injection: a late network result must not restore stale UI
  // even when a transport fails to honor the application's AbortSignal.
  await page.addInitScript(() => {
    const originalFetch = window.fetch.bind(window);
    window.fetch = (input, init) => {
      const url = input instanceof Request ? input.url : String(input);
      return originalFetch(
        input,
        new URL(url, location.href).pathname === "/api/access/v1/clients"
          ? { ...init, signal: undefined }
          : init,
      );
    };
  });
}

test("A and C inspect bounded client inventory through the cookie-only host", async ({
  page,
}) => {
  await authenticatedProjection(page);
  const first = clientPage();
  const second = olderPage();
  const requested: URL[] = [];
  await page.route(inventoryRoute, async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    requested.push(url);
    expect(request.method()).toBe("GET");
    expect(request.postData()).toBeNull();
    expect(request.headers()["authorization"]).toBeUndefined();
    expect(request.headers()["x-csrf-token"]).toBeUndefined();
    expect(url.searchParams.get("limit")).toBe("32");
    if (url.searchParams.has("after_created_at")) {
      expect(url.search).toContain("%2B10000");
      expect(url.searchParams.get("after_created_at")).toBe(
        first.next!.created_at,
      );
      expect(url.searchParams.get("after_client_id")).toBe(
        first.next!.client_id,
      );
      return json(route, second);
    }
    return json(route, first);
  });
  for (const path of paths) {
    const before = requested.length;
    const region = await openInventory(page, path);
    expect(requested).toHaveLength(before);
    await loadWithKeyboard(page, region);
    await expect(rows(region)).toHaveCount(32);
    const details = region.locator('summary[aria-label="Inspect Client 1"]');
    await details.focus();
    await page.keyboard.press("Enter");
    await expect(
      region.getByText(first.clients[0]!.created_at, { exact: true }),
    ).toBeVisible();
    await expect(
      region.getByText("9223372036854775807", { exact: true }),
    ).toBeVisible();
    await expect(region).toContainText(
      "Registration does not prove that a device is connected",
    );
    await region.getByRole("button", { name: "Next client page" }).click();
    await expect(rows(region)).toHaveCount(1);
    await expect(
      region.getByRole("rowheader", { name: "Older client", exact: true }),
    ).toBeVisible();
    await expect(
      region.getByRole("rowheader", { name: "Client 1", exact: true }),
    ).toHaveCount(0);
    await region.locator('summary[aria-label="Inspect Older client"]').click();
    await expect(
      region.getByText("2016-12-31T23:59:60.999999Z", { exact: true }),
    ).toBeVisible();
    await expect(
      region.getByText("0 (no issued generation)", { exact: true }),
    ).toBeVisible();
    await expect(
      region.getByRole("button", { name: "Next client page" }),
    ).toHaveCount(0);
    await region.getByRole("button", { name: "Newest clients" }).click();
    await expect(rows(region)).toHaveCount(32);
    expect(requested.at(-1)!.searchParams.has("after_created_at")).toBeFalsy();
    await expect(region).toContainText(
      "This inspection does not complete device setup.",
    );
    if (path === "/settings/account")
      await expect(
        page.getByRole("button", { name: "Open connections", exact: true }),
      ).toBeVisible();
    else
      await expect(
        page.getByRole("button", { name: "Save and finish later" }),
      ).toBeVisible();
  }
});

test("client inventory retries the failed cursor page", async ({ page }) => {
  await authenticatedProjection(page);
  const first = clientPage();
  const failure = problem("storage_unavailable", "access.client.list");
  const failedQueries: string[] = [];
  let recovering = false;
  await page.route(inventoryRoute, async (route) => {
    const url = new URL(route.request().url());
    if (!url.searchParams.has("after_created_at")) return json(route, first);
    failedQueries.push(url.search);
    return recovering
      ? json(route, olderPage())
      : json(route, failure, failure.status);
  });
  const region = await openInventory(page, "/settings/account");
  await loadWithKeyboard(page, region);
  await expect(rows(region)).toHaveCount(32);
  await region.getByRole("button", { name: "Next client page" }).click();
  const retry = region.getByRole("button", { name: "Retry client page" });
  await expect(retry).toBeVisible();
  await expect(region).toContainText(
    "The previous page remains visible. It has not been refreshed.",
  );
  await expect(rows(region)).toHaveCount(32);
  expect(failedQueries.length).toBeGreaterThan(0);
  recovering = true;
  await retry.click();
  await expect(rows(region)).toHaveCount(1);
  expect(new Set(failedQueries).size).toBe(1);
  expect(new URLSearchParams(failedQueries[0]!).get("after_client_id")).toBe(
    first.next!.client_id,
  );
});

test("client inventory cancellation rejects a late result and can load empty state", async ({
  page,
}) => {
  await ignoreInventoryAbort(page);
  await authenticatedProjection(page);
  const held = deferred();
  const started = deferred();
  const completed = deferred();
  let calls = 0;
  await page.route(inventoryRoute, async (route) => {
    if (++calls === 1) {
      started.resolve();
      await held.promise;
      await json(route, clientPage(1, false));
      completed.resolve();
      return;
    }
    return json(route, { clients: [], next: null });
  });
  const region = await openInventory(page, "/settings/account");
  try {
    await loadWithKeyboard(page, region);
    await started.promise;
    await region.getByRole("button", { name: "Cancel loading" }).click();
    await loadWithKeyboard(page, region);
    await expect(region).toContainText(
      "No registered clients are visible to this account on this page.",
    );
    held.resolve();
    await completed.promise;
    await settleRender(page);
    await expect(rows(region)).toHaveCount(0);
    await expect(region).toContainText(
      "No registered clients are visible to this account on this page.",
    );
  } finally {
    held.resolve();
  }
});

test("client inventory discards late responses after authority loss", async ({
  page,
}) => {
  await ignoreInventoryAbort(page);
  const projection = await authenticatedProjection(page);
  const held = deferred();
  const started = deferred();
  const completed = deferred();
  await page.route(inventoryRoute, async (route) => {
    started.resolve();
    await held.promise;
    await json(route, clientPage(1, false));
    completed.resolve();
  });
  const region = await openInventory(page, "/settings/account");
  try {
    await loadWithKeyboard(page, region);
    await started.promise;
    projection.set(
      problem("browser_session_revoked", "access.projection.read"),
    );
    const refreshed = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === "/api/access/v1/projection" &&
        response.status() === 401,
    );
    await page.evaluate(() => window.dispatchEvent(new Event("focus")));
    await refreshed;
    await expect(
      region.getByRole("button", { name: "Cancel loading" }),
    ).toHaveCount(0);
    held.resolve();
    await completed.promise;
    await settleRender(page);
    await expect(rows(region)).toHaveCount(0);
    await expect(
      region.getByRole("button", { name: "Load clients", exact: true }),
    ).toHaveCount(0);
    await expect(
      page.getByRole("rowheader", { name: "Client 1", exact: true }),
    ).toHaveCount(0);
  } finally {
    held.resolve();
  }
});

test("client inventory signout error clears retained page and parent authority", async ({
  page,
}) => {
  const projection = await authenticatedProjection(page);
  const failure = problem("browser_session_revoked", "access.client.list");
  let calls = 0;
  await page.route(inventoryRoute, (route) =>
    ++calls === 1
      ? json(route, clientPage())
      : json(route, failure, failure.status),
  );
  const region = await openInventory(page, "/settings/account");
  await loadWithKeyboard(page, region);
  await expect(rows(region)).toHaveCount(32);
  projection.set(problem("browser_session_revoked", "access.projection.read"));
  await region.getByRole("button", { name: "Next client page" }).click();
  await expect(
    page.getByRole("heading", { name: "Confirm account access" }),
  ).toBeVisible();
  await expect(
    page.getByRole("table", { name: "Registered application clients" }),
  ).toHaveCount(0);
});

test("client inventory ignores an old 401 after a newer parent identity", async ({
  page,
}) => {
  // Isolated fault injection: suppress abort itself, not just fetch's signal.
  // Otherwise the SDK converts the old 401 to FastiAbortError before the
  // component's generation/identity guard can see the original problem.
  await page.addInitScript(() => {
    AbortController.prototype.abort = function (): void {};
  });
  const projection = await authenticatedProjection(page);
  const fixture = await page.request.get(
    "http://127.0.0.1:18422/api/access/v1/projection",
  );
  expect(fixture.ok()).toBeTruthy();
  const baseline = parseAccessProjectionResponse(await fixture.json());
  const nextSession = {
    ...baseline.current_session,
    browser_session_id: "ses_018f0e0e7f7b70008000000000000002",
    rotation_generation: 2,
  };
  const nextProjection: AccessProjectionResponse = {
    ...baseline,
    subject: {
      ...baseline.subject,
      auth_subject_id: "sub_018f0e0e7f7b70008000000000000002",
    },
    membership: {
      ...baseline.membership,
      membership_id: "mem_018f0e0e7f7b70008000000000000002",
      updated_at: "2026-08-31T12:02:00Z",
    },
    current_session: nextSession,
    sessions: [nextSession],
  };
  const currentPage: ListAccessClientsResponse = {
    clients: [
      {
        ...clientPage(1, false).clients[0]!,
        client_id: "cli_018f0e0e7f7b700080000000000000fe",
        owner_subject_id: nextProjection.subject.auth_subject_id,
        name: "New account client",
      },
    ],
    next: null,
  };
  const held = deferred();
  const started = deferred();
  const completed = deferred();
  const staleProblem = problem("browser_session_revoked", "access.client.list");
  let calls = 0;
  await page.route(inventoryRoute, async (route) => {
    if (++calls === 1) {
      started.resolve();
      await held.promise;
      await json(route, staleProblem, staleProblem.status);
      completed.resolve();
      return;
    }
    return json(route, currentPage);
  });
  const region = await openInventory(page, "/settings/account");
  try {
    await loadWithKeyboard(page, region);
    await started.promise;
    projection.set(nextProjection);
    const refreshed = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === "/api/access/v1/projection" &&
        response.status() === 200,
    );
    await page.evaluate(() => window.dispatchEvent(new Event("focus")));
    await refreshed;
    await expect(
      region.getByRole("button", { name: "Cancel loading" }),
    ).toHaveCount(0);
    await loadWithKeyboard(page, region);
    const currentRow = region.getByRole("rowheader", {
      name: "New account client",
      exact: true,
    });
    await expect(currentRow).toBeVisible();
    held.resolve();
    await completed.promise;
    await settleRender(page);
    await expect(currentRow).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Confirm account access" }),
    ).toHaveCount(0);
    await region.getByRole("button", { name: "Newest clients" }).click();
    await expect(currentRow).toBeVisible();
    await expect(
      region.getByRole("button", { name: "Newest clients" }),
    ).toBeEnabled();
    expect(calls).toBe(3);
  } finally {
    held.resolve();
  }
});

for (const theme of ["light", "dark", "night", "forced-colors"] as const) {
  test(`client inventory A+C keyboard, reflow and Axe: ${theme}`, async ({
    page,
  }, testInfo) => {
    test.setTimeout(120_000);
    const mode = theme === "forced-colors" ? "light" : theme;
    await page.addInitScript(
      (value) =>
        localStorage.setItem(
          "fasti-theme-settings",
          JSON.stringify({ mode: value }),
        ),
      mode,
    );
    if (theme === "forced-colors")
      await page.emulateMedia({ forcedColors: "active" });
    await authenticatedProjection(page);
    const longName = "é".repeat(64); // Exactly the domain's 128-byte UTF-8 maximum.
    const longNamePage = clientPage(2, false);
    const boundedNamePage: ListAccessClientsResponse = {
      ...longNamePage,
      clients: [
        longNamePage.clients[0]!,
        { ...longNamePage.clients[1]!, name: longName },
      ],
    };
    await page.route(inventoryRoute, (route) =>
      json(
        route,
        theme === "light" && page.viewportSize()?.width === 320
          ? boundedNamePage
          : clientPage(1, false),
      ),
    );
    for (const width of [320, 375, 768, 1440]) {
      await page.setViewportSize({ width, height: 1024 });
      for (const path of paths) {
        const region = await openInventory(page, path);
        await expect(page.locator("html")).toHaveAttribute(
          "data-fasti-theme",
          mode,
        );
        await loadWithKeyboard(page, region);
        const checksLongName = theme === "light" && width === 320;
        await expect(rows(region)).toHaveCount(checksLongName ? 2 : 1);
        if (checksLongName)
          await expect(
            region.getByRole("rowheader", { name: longName, exact: true }),
          ).toBeVisible();
        const inspect = region.locator(
          'summary[aria-label="Inspect Client 1"]',
        );
        await inspect.focus();
        await expect(inspect).toBeFocused();
        await page.keyboard.press("Enter");
        await expect(
          region.getByText("9223372036854775807", { exact: true }),
        ).toBeVisible();
        await expectNoHorizontalOverflow(page);
        const targets = await region
          .locator("button:visible, summary:visible")
          .evaluateAll((elements) =>
            elements.map((element) => ({
              text: element.textContent,
              width: element.getBoundingClientRect().width,
              height: element.getBoundingClientRect().height,
            })),
          );
        expect(targets.length).toBeGreaterThan(0);
        expect(
          targets.filter(({ width, height }) => width < 44 || height < 44),
        ).toEqual([]);
        expect((await new AxeBuilder({ page }).analyze()).violations).toEqual(
          [],
        );
        if (theme === "light" && (width === 320 || width === 1440)) {
          await page.evaluate(() => window.scrollTo(0, 0));
          await page.screenshot({
            path: testInfo.outputPath(
              `inventory-${path === "/first-run" ? "C" : "A"}-${width}.png`,
            ),
            fullPage: true,
          });
        }
      }
    }
  });
}
