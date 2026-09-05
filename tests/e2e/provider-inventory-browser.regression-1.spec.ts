import { PUBLIC_PROBLEM_CATALOG, type ProblemDetails } from "@fasti/sdk";
import { expect, test, type Page, type Route } from "@playwright/test";
import { mockAuthenticatedAccess } from "./test-helpers";

const browserOrigin = "http://127.0.0.1:4173";
const savedServiceOrigin = "https://saved-service.fasti.test";

const field = (value: string | null) => ({
  value,
  tier: value === null ? "empty" : "preferred_provider_claim",
  source: value === null ? null : "tmdb",
  is_stale: false,
});

const localRecord = {
  record_id: "rec_01991f588e0070008000000000000d01",
  grain: "film",
  status: "active",
  title: field("Local retained result"),
  poster: field(null),
  original_title: field(null),
  overview: field(null),
  release_year: field("2026"),
  identifiers: [],
  latest_activity: null,
};

const candidate = {
  provider: "tmdb",
  provider_id: "550",
  grain: "film",
  kind: "movie",
  title: "Provider result",
  original_title: null,
  release_year: 1999,
  authors: [],
  image_url: null,
  overview: "A governed provider candidate.",
};

function providerInventory(credentialState: "valid" | "missing") {
  return {
    providers: [
      {
        provider_id: "tmdb",
        display_name: "TMDB",
        provider_kind: "metadata",
        documentation_url: "https://developer.themoviedb.org/docs",
        attribution: "TMDB",
        supported_media_grains: ["film", "series"],
        capabilities: [
          {
            capability_id: "metadata.search",
            purpose: "Search film and television metadata",
            credential_requirement: "bearer_token",
            credential_state: credentialState,
            credential_source:
              credentialState === "valid" ? "environment" : "none",
            state: credentialState === "valid" ? "available" : "degraded",
            version: credentialState === "valid" ? 1 : 0,
            writable: false,
            testable: false,
            health: {
              state: "never_run",
              checked_at: null,
              safe_problem_code: null,
            },
            credential_test: {
              state: "never_run",
              checked_at: null,
              safe_problem_code: null,
            },
          },
        ],
        network_hosts: ["api.themoviedb.org"],
        locale_support: true,
        region_support: true,
        identity_namespaces: ["tmdb.movie", "tmdb.tv"],
      },
    ],
  };
}

async function fulfillJson(route: Route, body: unknown, status = 200) {
  await route.fulfill({
    status,
    contentType:
      status >= 400 ? "application/problem+json" : "application/json",
    headers: { "cache-control": "private, no-store" },
    body: JSON.stringify(body),
  });
}

async function installSavedServiceReads(
  page: Page,
  remoteRequests: string[],
): Promise<void> {
  await page.addInitScript((serviceUrl) => {
    localStorage.setItem(
      "fasti-network-config",
      JSON.stringify({ service_url: serviceUrl }),
    );
  }, savedServiceOrigin);
  await page.route(`${savedServiceOrigin}/**`, async (route) => {
    const request = route.request();
    remoteRequests.push(request.url());
    expect(request.headers().authorization).toBeUndefined();
    const path = new URL(request.url()).pathname;
    if (path === "/api/v1/records") {
      return fulfillJson(route, { records: [], truncated: false });
    }
    if (path === "/api/v1/profile/record-tracking-dispositions") {
      return fulfillJson(route, { states: [], truncated: false });
    }
    return fulfillJson(route, { title: "Unexpected remote request" }, 404);
  });
}

async function installLocalSearch(
  page: Page,
  localSearchRequests: string[],
): Promise<void> {
  await page.route(`${browserOrigin}/api/v1/search/records`, async (route) => {
    const request = route.request();
    localSearchRequests.push(request.url());
    expect(request.headers().authorization).toBeUndefined();
    await fulfillJson(route, { records: [localRecord], next: null });
  });
}

function expiredInventoryProblem(): ProblemDetails {
  const canonical = PUBLIC_PROBLEM_CATALOG.problems.find(
    (problem) =>
      problem.code === "browser_session_expired" &&
      problem.capability_id === "provider.list",
  );
  if (!canonical) throw new Error("Missing provider inventory expiry contract");
  const { param_policy: _paramPolicy, ...contract } = canonical;
  return {
    ...contract,
    actual: null,
    correlation_id: "req_018f0e0e7f7b7000800000000000000d",
    violations: [],
  };
}

test("browser provider inventory and Search stay on the browser-session origin", async ({
  page,
}) => {
  const inventoryRequests: string[] = [];
  const providerSearchRequests: string[] = [];
  const localSearchRequests: string[] = [];
  const remoteRequests: string[] = [];
  await mockAuthenticatedAccess(page);
  const csrf = "a".repeat(64);
  await page.context().addCookies([
    {
      name: "__Host-fasti_csrf",
      value: csrf,
      url: "https://127.0.0.1:4173",
      secure: true,
      sameSite: "Strict",
    },
  ]);
  await installSavedServiceReads(page, remoteRequests);
  await installLocalSearch(page, localSearchRequests);
  await page.route(`${browserOrigin}/api/v1/providers`, async (route) => {
    inventoryRequests.push(route.request().url());
    expect(route.request().headers().authorization).toBeUndefined();
    await fulfillJson(route, providerInventory("valid"));
  });
  await page.route(
    `${browserOrigin}/api/v1/search/providers/tmdb`,
    async (route) => {
      providerSearchRequests.push(route.request().url());
      expect(route.request().headers().authorization).toBeUndefined();
      expect(route.request().headers()["x-csrf-token"]).toBe(csrf);
      expect(route.request().postDataJSON()).toMatchObject({
        query: "Same origin",
        page: 1,
      });
      await fulfillJson(route, {
        outcome: "page",
        provider_id: "tmdb",
        page: 1,
        candidates: [
          {
            candidate_receipt_id: "scr_01991f588e0070008000000000000d01",
            grain: "film",
            candidate,
          },
        ],
        next_page: null,
        cache_state: "observed",
        lifetime: {
          created_at: "2099-09-05T12:00:00Z",
          fresh_until: "2099-09-05T12:00:00Z",
          stale_until: "2099-09-05T12:00:00Z",
          expires_at: "2099-09-06T12:00:00Z",
        },
        upstream_problem: null,
      });
    },
  );

  await page.goto("/discover");
  await expect(page.getByLabel("Metadata provider")).toHaveValue("tmdb");
  await page
    .getByRole("searchbox", { name: "Search TMDB" })
    .fill("Same origin");
  await page.getByRole("button", { name: "Search", exact: true }).click();

  await expect(
    page.getByRole("heading", { name: "Local retained result" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Provider result" }),
  ).toBeVisible();
  expect(inventoryRequests).toEqual([`${browserOrigin}/api/v1/providers`]);
  expect(localSearchRequests).toEqual([
    `${browserOrigin}/api/v1/search/records`,
  ]);
  expect(providerSearchRequests).toEqual([
    `${browserOrigin}/api/v1/search/providers/tmdb`,
  ]);
  expect(
    remoteRequests.filter((url) =>
      /\/api\/v1\/(?:providers|search\/)/u.test(new URL(url).pathname),
    ),
  ).toEqual([]);
  expect(await page.evaluate(() => "__TAURI_INTERNALS__" in window)).toBe(
    false,
  );

  await page.goto("/settings/providers");
  const providerRow = page.getByRole("row", { name: /TMDB/u });
  await expect(providerRow).toContainText(
    "Managed by the process environment.",
  );
  await expect(
    providerRow.getByLabel(/TMDB API Read Access Token/u),
  ).toHaveCount(0);
  await expect(
    providerRow.getByRole("button", {
      name: /Save|Remove|Test credential|Check provider health/u,
    }),
  ).toHaveCount(0);
});

test("a missing browser provider credential preserves truthful local Search", async ({
  page,
}) => {
  const localSearchRequests: string[] = [];
  const providerSearchRequests: string[] = [];
  const remoteRequests: string[] = [];
  await mockAuthenticatedAccess(page);
  await installSavedServiceReads(page, remoteRequests);
  await installLocalSearch(page, localSearchRequests);
  await page.route(`${browserOrigin}/api/v1/providers`, (route) =>
    fulfillJson(route, providerInventory("missing")),
  );
  await page.route(`${browserOrigin}/api/v1/search/providers/**`, (route) => {
    providerSearchRequests.push(route.request().url());
    return fulfillJson(
      route,
      { title: "Provider Search was not available" },
      500,
    );
  });

  await page.goto("/discover");
  await expect(
    page.getByRole("heading", { name: "TMDB needs a credential" }),
  ).toBeVisible();
  await page.getByRole("searchbox", { name: "Search TMDB" }).fill("Local only");
  await page.getByRole("button", { name: "Search", exact: true }).click();

  await expect(
    page.getByRole("heading", { name: "Local retained result" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "TMDB needs a credential" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Open provider settings" }),
  ).toBeVisible();
  expect(localSearchRequests).toHaveLength(1);
  expect(providerSearchRequests).toEqual([]);
});

test("an expired browser inventory does not erase local Search", async ({
  page,
}) => {
  const localSearchRequests: string[] = [];
  const providerSearchRequests: string[] = [];
  const remoteRequests: string[] = [];
  await mockAuthenticatedAccess(page);
  await installSavedServiceReads(page, remoteRequests);
  await installLocalSearch(page, localSearchRequests);
  await page.route(`${browserOrigin}/api/v1/providers`, (route) =>
    fulfillJson(route, expiredInventoryProblem(), 401),
  );
  await page.route(`${browserOrigin}/api/v1/search/providers/**`, (route) => {
    providerSearchRequests.push(route.request().url());
    return fulfillJson(
      route,
      { title: "Provider Search was not available" },
      500,
    );
  });

  await page.goto("/discover");
  await expect(page.getByRole("alert")).toContainText(
    "the Fasti browser session reached its idle or absolute expiry",
  );
  await page
    .getByRole("searchbox", { name: "Search local Records and providers" })
    .fill("Local after expiry");
  await page.getByRole("button", { name: "Search", exact: true }).click();

  await expect(
    page.getByRole("heading", { name: "Local retained result" }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toContainText(
    "the Fasti browser session reached its idle or absolute expiry",
  );
  expect(localSearchRequests).toHaveLength(1);
  expect(providerSearchRequests).toEqual([]);
});
