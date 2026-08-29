import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page, type Route } from "@playwright/test";
import { expectNoHorizontalOverflow } from "./test-helpers";

const browserOrigin = "http://127.0.0.1:4173";

async function openWorkbenchSection(page: Page, name: string): Promise<void> {
  const trigger = page.getByRole("button", { name: "Open navigation" });
  if (await trigger.isVisible()) await trigger.click();
  await page.getByRole("link", { name, exact: true }).click();
}

function recordResponse(
  title = "A bounded local record",
  poster: string | null = null,
) {
  return {
    records: [
      {
        record_id: "rec_01991f588e0070008000000000000002",
        grain: "work",
        status: "active",
        title: {
          tier: "preferred_provider_claim",
          value: title,
          source: "google-books",
          is_stale: false,
        },
        poster: {
          tier: poster ? "preferred_provider_claim" : "empty",
          value: poster,
          source: poster ? "untrusted-test" : null,
          is_stale: false,
        },
        latest_activity: {
          interpretation_state: "resolved",
          occurred_at: {
            original: "2026-08-27T12:00:00Z",
            precision: "second",
            trust: "device_observed",
          },
        },
      },
    ],
    truncated: false,
  };
}

async function fulfillRecords(route: Route, title?: string, poster?: string) {
  expect(route.request().headers().authorization).toBeUndefined();
  await route.fulfill({
    status: 200,
    contentType: "application/json",
    body: JSON.stringify(recordResponse(title, poster)),
  });
}

async function installBrowserSession(
  page: Page,
  origin = browserOrigin,
): Promise<void> {
  await page.route(`${origin}/api/v1/browser/session`, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      headers: {
        "access-control-allow-origin": browserOrigin,
        "access-control-allow-credentials": "true",
      },
      body: JSON.stringify({
        expires_at: "2026-08-28T23:00:00Z",
        user: {
          user_id: "usr_01991f588e0070008000000000000001",
          username: "testadmin",
          is_admin: true,
          is_test_account: true,
          active: true,
          created_at: "2026-08-28T00:00:00Z",
          updated_at: "2026-08-28T00:00:00Z",
        },
      }),
    }),
  );
}

test("browser history keeps the Workbench and status route synchronized", async ({
  page,
}) => {
  let healthRequests = 0;
  await page.route(/\/api\/v1\/health$/, async (route) => {
    healthRequests += 1;
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ status: "healthy", version: "0.1.0" }),
    });
  });

  await page.goto("/status");
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Open Media Workbench" }).click();
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();

  await page.goBack();
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
  expect(healthRequests).toBe(2);

  await page.goForward();
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
});

test("browser account dialog supports keyboard navigation and escape", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open account access" }).click();

  const dialog = page.getByRole("dialog", { name: "Account access" });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("Username").focus();
  await page.keyboard.press("Tab");
  await expect(dialog.getByLabel("Password")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
});

test("a rejected browser sign-in stays disconnected and recoverable", async ({
  page,
}) => {
  await page.route(/\/api\/v1\/browser\/session$/, (route) =>
    route.fulfill({
      status: 401,
      contentType: "application/problem+json",
      body: JSON.stringify({
        type: "https://fasti.scrobble.dev/v1/problems/authentication-failed",
        title: "Authentication failed",
        status: 401,
        detail: "the presented local credential is not active",
        code: "authentication_failed",
        capability_id: "browser.session.create",
        safe_state: "no_mutation",
        retryability: "not_retryable",
        next_actions: [
          {
            id: "use_active_credential",
            label: "Use an active local credential or enroll again",
          },
        ],
        correlation_id: "req_01991f588e0070008000000000000002",
        param: null,
        actual: null,
        violations: [],
      }),
    }),
  );
  await page.goto("/");
  await page
    .locator("#main-content")
    .getByRole("button", { name: "Sign in", exact: true })
    .click();
  const dialog = page.getByRole("dialog", { name: "Account access" });
  const username = dialog.getByLabel("Username");
  const password = dialog.getByLabel("Password");
  await expect(username).toHaveValue("");
  await expect(dialog).not.toContainText("testadmin / testadmin");
  await username.fill("testadmin");
  await password.fill("incorrect-password");
  await dialog.getByRole("button", { name: "Sign in" }).click();

  await expect(dialog.getByRole("alert")).toContainText("username or password");
  await expect(username).toHaveAttribute("aria-invalid", "true");
  await expect(password).toHaveAttribute("aria-invalid", "true");
  await username.fill("editedadmin");
  await expect(username).toHaveAttribute("aria-invalid", "false");
  await expect(password).toHaveAttribute("aria-invalid", "false");
  await expect(username).not.toHaveAttribute(
    "aria-describedby",
    "auth-problem",
  );
  await expect(
    page
      .locator("#main-content")
      .getByRole("button", { name: "Sign in", exact: true }),
  ).toBeVisible();
});

test("profile settings discard the previous browser account state", async ({
  page,
  context,
}) => {
  const csrf = "a".repeat(64);
  const users = {
    a: {
      user_id: "usr_01991f58-8e00-7000-8000-000000000001",
      username: "account-a",
      is_admin: true,
      is_test_account: false,
      active: true,
      created_at: "2026-08-28T00:00:00Z",
      updated_at: "2026-08-28T00:00:00Z",
    },
    b: {
      user_id: "usr_01991f58-8e00-7000-8000-000000000002",
      username: "account-b",
      is_admin: true,
      is_test_account: false,
      active: true,
      created_at: "2026-08-28T00:00:00Z",
      updated_at: "2026-08-28T00:00:00Z",
    },
  } as const;
  let currentUser: (typeof users)[keyof typeof users] | null = users.a;
  let releaseAccountB: (() => void) | undefined;
  let accountBRequests = 0;

  await context.addCookies([
    {
      name: "fasti_csrf",
      value: csrf,
      url: browserOrigin,
      sameSite: "Strict",
    },
  ]);
  await page.route(/\/api\/v1\/browser\/session$/, async (route) => {
    const method = route.request().method();
    if (method === "DELETE") {
      currentUser = null;
      await route.fulfill({ status: 204 });
      return;
    }
    if (method === "POST") currentUser = users.b;
    if (!currentUser) {
      await route.fulfill({ status: 401 });
      return;
    }
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        expires_at: "2026-08-28T23:00:00Z",
        user: currentUser,
      }),
    });
  });
  await page.route(/\/api\/v1\/browser\/users$/, (route) =>
    route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ users: currentUser ? [currentUser] : [] }),
    }),
  );
  await page.route(/\/api\/v1\/profile\/nuvio-collections$/, async (route) => {
    if (currentUser?.user_id === users.a.user_id) {
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({
          document: [
            {
              id: "account-a-private",
              title: "Account A private",
              folders: [],
            },
          ],
        }),
      });
      return;
    }
    accountBRequests += 1;
    await new Promise<void>((resolve) => (releaseAccountB = resolve));
    await route.fulfill({
      status: 403,
      contentType: "application/problem+json",
      body: JSON.stringify({
        type: "https://fasti.scrobble.dev/v1/problems/forbidden",
        title: "Forbidden",
        status: 403,
        detail: "request is not authorized for this capability",
        code: "forbidden",
        capability_id: "profile.nuvio_collections.get",
        safe_state: "no_mutation",
        retryability: "not_retryable",
        next_actions: [
          {
            id: "verify_request_authorization",
            label: "Verify the request context and local grant",
          },
        ],
        correlation_id: "req_01991f588e0070008000000000000002",
        param: null,
        actual: null,
        violations: [],
      }),
    });
  });

  await page.goto("/settings/collections");
  await expect(page.getByText("Account A private")).toBeVisible();
  await page.getByRole("button", { name: "Manage account account-a" }).click();
  const dialog = page.getByRole("dialog", { name: "Account access" });
  await dialog.getByRole("button", { name: "Sign out" }).click();
  await dialog.getByLabel("Username").fill("account-b");
  await dialog.getByLabel("Password").fill("account-b-password");
  await dialog.getByRole("button", { name: "Sign in" }).click();

  await expect.poll(() => accountBRequests).toBe(1);
  await expect(page.getByText("Account A private")).toHaveCount(0);
  releaseAccountB?.();
  await expect(page.getByRole("alert")).toContainText(
    "request is not authorized for this capability",
  );
  await expect(page.getByText("Account A private")).toHaveCount(0);
});

test("endpoint testing rejects a contract-invalid health response", async ({
  page,
}) => {
  await page.route("https://invalid.fasti.test/api/v1/health", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: "{}",
      headers: { "access-control-allow-origin": "*" },
    }),
  );
  await page.goto("/settings");
  await page.getByLabel("Service URL").fill("https://invalid.fasti.test");
  await page.getByRole("button", { name: "Test service URL" }).click();

  await expect(page.getByRole("alert")).toContainText("Health response");
  await expect(page.getByRole("status")).toHaveCount(0);
});

test("an invalid service URL cannot overwrite the last valid endpoint", async ({
  page,
}) => {
  const validEndpoint = "http://localhost:8420";
  await page.addInitScript((endpoint) => {
    localStorage.setItem(
      "fasti-network-config",
      JSON.stringify({ service_url: endpoint }),
    );
  }, validEndpoint);
  await page.goto("/settings");
  await expect(page.getByLabel("Service URL")).toHaveValue(validEndpoint);

  await page
    .getByLabel("Service URL")
    .fill("https://invalid.fasti.test/path-is-not-allowed");
  await page.getByRole("button", { name: "Save service URL" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "must contain only a scheme, host, and optional port",
  );

  await page.reload();
  await expect(page.getByLabel("Service URL")).toHaveValue(validEndpoint);
});

test("a saved service URL owns browser record and status requests after reload", async ({
  page,
}) => {
  const savedOrigin = `https://${"a".repeat(63)}.fasti.test`;
  const recordUrls: string[] = [];
  const healthUrls: string[] = [];
  await page.addInitScript((serviceUrl) => {
    localStorage.setItem(
      "fasti-network-config",
      JSON.stringify({ service_url: serviceUrl }),
    );
  }, savedOrigin);
  await installBrowserSession(page, savedOrigin);
  await page.route(`${savedOrigin}/api/v1/records`, async (route) => {
    const request = route.request();
    if (request.method() === "OPTIONS") {
      await route.fulfill({
        status: 204,
        headers: {
          "access-control-allow-origin": browserOrigin,
          "access-control-allow-methods": "GET",
          "access-control-allow-credentials": "true",
        },
      });
      return;
    }
    recordUrls.push(request.url());
    expect(request.headers().authorization).toBeUndefined();
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      headers: {
        "access-control-allow-origin": browserOrigin,
        "access-control-allow-credentials": "true",
      },
      body: JSON.stringify(recordResponse("Saved endpoint record")),
    });
  });
  await page.route(`${savedOrigin}/api/v1/health`, async (route) => {
    healthUrls.push(route.request().url());
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      headers: {
        "access-control-allow-origin": "http://127.0.0.1:4173",
      },
      body: JSON.stringify({ status: "healthy", version: "0.1.0" }),
    });
  });

  await page.setViewportSize({ width: 320, height: 720 });
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Saved endpoint record" }).first(),
  ).toBeVisible();
  expect(recordUrls).toEqual([`${savedOrigin}/api/v1/records`]);

  await page.getByRole("link", { name: "Service status" }).click();
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
  expect(healthUrls).toEqual([`${savedOrigin}/api/v1/health`]);
  await expect(page.getByText(savedOrigin, { exact: true })).toBeVisible();
});

test("changing the browser service URL resets the origin-bound session state", async ({
  page,
}) => {
  await installBrowserSession(page);
  await page.route(/\/api\/v1\/records$/, (route) => fulfillRecords(route));
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "A bounded local record" }).first(),
  ).toBeVisible();

  await page.route("https://new.fasti.test/api/v1/browser/session", (route) =>
    route.fulfill({
      status: 401,
      contentType: "application/problem+json",
      headers: {
        "access-control-allow-origin": browserOrigin,
        "access-control-allow-credentials": "true",
      },
      body: JSON.stringify({
        type: "about:blank",
        title: "Unauthorized",
        status: 401,
        detail: "Sign in is required.",
      }),
    }),
  );

  await openWorkbenchSection(page, "Settings");
  await page.getByLabel("Service URL").fill("https://new.fasti.test");
  await page.getByRole("button", { name: "Save service URL" }).click();
  await expect(page.getByRole("status")).toHaveText("Settings saved.");
  await expect(
    page.getByRole("button", { name: "Open account access" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: /Manage account/ }),
  ).toHaveCount(0);
});

test("record summaries stay truthful, bounded, and free of poster egress", async ({
  page,
}) => {
  const thirdPartyRequests: string[] = [];
  page.on("request", (request) => {
    if (new URL(request.url()).origin === "https://tracker.example") {
      thirdPartyRequests.push(request.url());
    }
  });
  await page.route(/\/api\/v1\/records$/, (route) =>
    fulfillRecords(
      route,
      "Summary-only record",
      "https://tracker.example/poster.jpg",
    ),
  );
  await installBrowserSession(page);
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Summary-only record" }).first(),
  ).toBeVisible();

  await openWorkbenchSection(page, "Library");
  await expect(page.getByText("Review up to 500 records")).toBeVisible();
  await expect(page.getByRole("button", { name: "Grid view" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page.getByRole("button", { name: "List view" }).click();
  await expect(page.getByRole("button", { name: "List view" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await page
    .getByRole("button", { name: "Summary-only record", exact: true })
    .click();
  await expect(
    page.getByText("No external identifiers are recorded."),
  ).toBeVisible();
  await expect(page.getByText("No custom fields are recorded.")).toBeVisible();

  const history = page.getByRole("button", { name: "History" });
  await history.click();
  await expect(history).toHaveAttribute("aria-pressed", "true");
  await expect(
    page.getByRole("heading", { name: "No occurrences recorded yet" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Sources & Identity" }).click();
  await expect(
    page.getByText("No external identifiers are attached."),
  ).toBeVisible();

  await openWorkbenchSection(page, "Calendar");
  await expect(
    page.getByRole("heading", { name: "Up Next & Calendar" }),
  ).toBeVisible();
  await expect(page.getByText("No episode progress recorded")).toBeVisible();
  expect(thirdPartyRequests).toEqual([]);
});

test("legacy saved navigation cannot revive unsupported destinations", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem(
      "fasti-workbench-preferences",
      JSON.stringify({
        navItems: [
          {
            id: "movies",
            label: "Legacy Movies",
            category: "media",
            visible: true,
            pinned: false,
            order: 0,
          },
        ],
        contextMenuItems: [],
      }),
    );
  });
  await page.goto("/");

  await expect(page.getByRole("button", { name: "Legacy Movies" })).toHaveCount(
    0,
  );
  await expect(page.getByRole("link", { name: "Overview" })).toBeVisible();
});

test("Discover selects configured providers and refreshes explicit setup state", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 900 });
  await page.addInitScript(() => {
    let googleConfigured = false;
    let tmdbConfigured = true;
    const completedSearches = new Set<string>();
    const trackedCandidates = new Set<string>();
    const providerStatus = () => [
      {
        provider: "google-books",
        label: "Google Books",
        configured: googleConfigured,
        source: googleConfigured ? "credential_store" : "none",
        writable: true,
        docs_url: "https://developers.google.com/books/docs/v1/using",
      },
      {
        provider: "tmdb",
        label: "TMDB",
        configured: tmdbConfigured,
        source: tmdbConfigured ? "environment" : "none",
        writable: !tmdbConfigured,
        docs_url: "https://developer.themoviedb.org/docs",
      },
    ];
    const networkConfiguration = {
      connection: {
        service_url: {
          value: "http://127.0.0.1:8420",
          source: "default",
          managed: false,
        },
        public_url: { value: null, source: "default", managed: false },
      },
      outbound_policy: {
        allow_providers: [],
        deny_providers: [],
        allow_capabilities: [],
        deny_capabilities: [],
        allow_hosts: [],
        deny_hosts: [],
        allow_networks: [],
        deny_networks: [],
      },
    };
    const browserWindow = window as typeof window & {
      __PROVIDER_STATUS_CALLS__?: number;
      __SET_TMDB_CONFIGURED__?: (configured: boolean) => void;
      __TAURI_INTERNALS__: {
        invoke: (command: string, arguments_?: unknown) => Promise<unknown>;
      };
    };
    browserWindow.__PROVIDER_STATUS_CALLS__ = 0;
    browserWindow.__SET_TMDB_CONFIGURED__ = (configured) => {
      tmdbConfigured = configured;
    };
    browserWindow.__TAURI_INTERNALS__ = {
      invoke: async (command, arguments_) => {
        switch (command) {
          case "setup_status":
            return { phase: "ready", proof_cleanup_pending: false };
          case "load_network_configuration":
            return networkConfiguration;
          case "provider_credential_status":
            browserWindow.__PROVIDER_STATUS_CALLS__ =
              (browserWindow.__PROVIDER_STATUS_CALLS__ ?? 0) + 1;
            return providerStatus();
          case "save_provider_credential":
            if (
              (arguments_ as { input?: { credential?: string } })?.input
                ?.credential === "rejected-credential"
            ) {
              throw {
                detail: "The provider rejected this credential.",
                next_action: "Check the credential and try again.",
              };
            }
            googleConfigured = true;
            return providerStatus();
          case "search_provider": {
            const input = (
              arguments_ as {
                input?: { provider?: string; query?: string };
              }
            )?.input;
            const searchKey = `${input?.provider}:${input?.query}`;
            if (
              !["tmdb:Breaking Bad", "google-books:Dune"].includes(searchKey) ||
              completedSearches.has(searchKey)
            ) {
              throw new Error(`Unexpected provider search: ${searchKey}`);
            }
            completedSearches.add(searchKey);
            if (input?.provider === "google-books") {
              return [
                {
                  provider: "google-books",
                  provider_id: "dune-volume",
                  title: "Dune",
                  kind: "book",
                  authors: ["Frank Herbert"],
                  image_url: null,
                },
              ];
            }
            return [
              {
                provider: "tmdb",
                provider_id: "1396",
                title: "Breaking Bad",
                kind: "show",
                authors: [],
                image_url: null,
              },
              {
                provider: "tmdb",
                provider_id: "1396",
                title: "Breaking Bad: The Movie",
                kind: "movie",
                authors: [],
                image_url: null,
              },
            ];
          }
          case "track_provider_candidate": {
            const input = (
              arguments_ as {
                input?: {
                  provider?: string;
                  provider_id?: string;
                  kind?: string;
                };
              }
            )?.input;
            const candidateKey = `${input?.provider}:${input?.kind}:${input?.provider_id}`;
            if (
              !["tmdb:show:1396", "tmdb:movie:1396"].includes(candidateKey) ||
              trackedCandidates.has(candidateKey)
            ) {
              throw new Error(`Unexpected Record creation: ${candidateKey}`);
            }
            trackedCandidates.add(candidateKey);
            if (input?.kind === "movie") {
              throw {
                detail: "TMDB could not return the selected movie.",
                next_action: "Try the search again.",
              };
            }
            return {
              record_id: "rec_01991f588e0070008000000000000010",
              grain: "show",
            };
          }
          case "list_records":
          case "list_reviews":
            return [];
          default:
            throw new Error(`Unexpected trusted-host command: ${command}`);
        }
      },
    };
  });

  await page.goto("/discover");
  const provider = page.getByLabel("Metadata provider");
  await expect(provider).toHaveValue("tmdb");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (
            window as typeof window & {
              __PROVIDER_STATUS_CALLS__?: number;
            }
          ).__PROVIDER_STATUS_CALLS__,
      ),
    )
    .toBe(1);
  const providerBox = await provider.boundingBox();
  expect(providerBox?.height).toBeGreaterThanOrEqual(44);
  await expectNoHorizontalOverflow(page);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);

  await openWorkbenchSection(page, "Library");
  await openWorkbenchSection(page, "Discover");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (
            window as typeof window & {
              __PROVIDER_STATUS_CALLS__?: number;
            }
          ).__PROVIDER_STATUS_CALLS__,
      ),
    )
    .toBe(2);
  await expect(provider).toHaveValue("tmdb");

  const search = page.getByRole("searchbox", { name: "Search TMDB" });
  await search.fill("é".repeat(129));
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("256 UTF-8 bytes");

  await search.fill("Breaking Bad");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Breaking Bad", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Create Record" })).toHaveCount(
    2,
  );
  const showResult = page.getByRole("listitem").filter({
    has: page.getByRole("heading", { name: "Breaking Bad", exact: true }),
  });
  await showResult.getByRole("button", { name: "Create Record" }).click();
  const completedAction = showResult.getByRole("button", {
    name: "Record ready",
  });
  await expect(completedAction).toHaveAttribute("aria-disabled", "true");
  await expect(completedAction).toBeFocused();
  await completedAction.press("Enter");
  await expect(showResult.getByRole("alert")).toHaveCount(0);
  await expect(showResult.getByRole("status")).toHaveText(
    "Record ID: rec_01991f588e0070008000000000000010",
  );
  await expectNoHorizontalOverflow(page);
  await expect(
    page
      .getByRole("listitem")
      .filter({
        has: page.getByRole("heading", { name: "Breaking Bad: The Movie" }),
      })
      .getByRole("button", { name: "Create Record" }),
  ).toBeEnabled();
  const movieResult = page.getByRole("listitem").filter({
    has: page.getByRole("heading", { name: "Breaking Bad: The Movie" }),
  });
  await movieResult.getByRole("button", { name: "Create Record" }).click();
  await expect(movieResult.getByRole("alert")).toHaveText(
    "TMDB could not return the selected movie. Try the search again.",
  );
  await expect(
    movieResult.getByRole("button", { name: "Create Record" }),
  ).toBeEnabled();
  await expect(
    movieResult.getByRole("button", { name: "Record ready" }),
  ).toHaveCount(0);

  await provider.selectOption("google-books");
  await expect(
    page.getByRole("heading", { name: /needs a credential$/ }),
  ).toBeVisible();
  await expect(page.getByRole("heading", { name: "Breaking Bad" })).toHaveCount(
    0,
  );
  await page.getByRole("button", { name: "Open provider settings" }).click();
  await expect(
    page.getByLabel("Settings section", { exact: true }),
  ).toHaveValue("providers");
  const googleCredential = page.getByLabel("Google Books API key");
  await googleCredential.fill("rejected-credential");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "The provider rejected this credential. Check the credential and try again.",
  );
  await expect(googleCredential).toHaveValue("rejected-credential");

  await googleCredential.fill("provider-secret");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(
    page.getByRole("status").filter({ hasText: "Credential saved" }),
  ).toBeVisible();
  expect(page.url()).not.toContain("provider-secret");
  expect(await page.locator("body").innerText()).not.toContain(
    "provider-secret",
  );
  expect(
    await page
      .locator("input")
      .evaluateAll((inputs) =>
        inputs.map((input) => (input as HTMLInputElement).value),
      ),
  ).not.toContain("provider-secret");

  await page
    .getByRole("article")
    .filter({ has: page.getByRole("heading", { name: "Google Books" }) })
    .getByRole("button", { name: "Test search" })
    .click();
  await expect(
    page
      .getByRole("status")
      .filter({ hasText: "Search succeeded. 1 candidate returned." }),
  ).toBeVisible();

  await openWorkbenchSection(page, "Discover");
  await expect(page.getByLabel("Metadata provider")).toHaveValue(
    "google-books",
  );
  await expect(
    page.getByRole("searchbox", { name: "Search Google Books" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: /needs a credential$/ }),
  ).toHaveCount(0);

  await provider.selectOption("tmdb");
  await page.evaluate(() =>
    (
      window as typeof window & {
        __SET_TMDB_CONFIGURED__?: (configured: boolean) => void;
      }
    ).__SET_TMDB_CONFIGURED__?.(false),
  );
  await openWorkbenchSection(page, "Library");
  await openWorkbenchSection(page, "Discover");
  await expect(provider).toHaveValue("tmdb");
  await expect(
    page.getByRole("heading", { name: "TMDB needs a credential" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Open provider settings" }).click();
  await expect(page.getByLabel("TMDB API Read Access Token")).toBeVisible();
});

test("browser Discover fails closed when provider credentials are unavailable", async ({
  page,
}) => {
  await page.goto("/discover");
  await expect(
    page.getByRole("heading", { name: /needs a credential$/ }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Create Record" })).toHaveCount(
    0,
  );
  await expect(page.getByRole("searchbox")).toHaveCount(0);
});
