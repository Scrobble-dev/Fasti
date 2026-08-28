import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page, type Route } from "@playwright/test";

const credential = "c".repeat(64);

function recordResponse(
  title = "A bounded local record",
  poster: string | null = null,
) {
  return {
    records: [
      {
        record_id: "018f7f2d-8f58-7a0a-8000-000000000002",
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
  };
}

async function fulfillRecords(route: Route, title?: string, poster?: string) {
  expect(route.request().headers().authorization).toBe(`Bearer ${credential}`);
  await route.fulfill({
    status: 200,
    contentType: "application/json",
    body: JSON.stringify(recordResponse(title, poster)),
  });
}

async function submitCredential(page: Page) {
  await page.getByRole("button", { name: "Connect records" }).click();
  await page.getByLabel("API client credential").fill(credential);
  await page.getByRole("button", { name: "Connect", exact: true }).click();
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

test("authentication tabs expose and implement the keyboard tab pattern", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Connect records" }).click();

  await expect(page.locator('[role="tab"][tabindex="0"]')).toHaveCount(1);
  const token = page.getByRole("tab", { name: "API Credential" });
  await expect(token).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("tabpanel")).toHaveAttribute(
    "aria-labelledby",
    "auth-tab-token",
  );

  await token.focus();
  await page.keyboard.press("ArrowRight");
  const passkey = page.getByRole("tab", { name: "Passkey" });
  await expect(passkey).toBeFocused();
  await expect(passkey).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("End");
  await expect(token).toBeFocused();
  await page.keyboard.press("Home");
  await expect(passkey).toBeFocused();
  await page.keyboard.press("ArrowLeft");
  await expect(token).toBeFocused();
});

test("a rejected browser credential stays disconnected and recoverable", async ({
  page,
}) => {
  await page.route(/\/api\/v1\/records$/, (route) =>
    route.fulfill({ status: 401, contentType: "application/json", body: "{}" }),
  );
  await page.goto("/");
  await submitCredential(page);

  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.getByRole("dialog").getByRole("alert")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Connect local credential" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Clear browser credential" }),
  ).toHaveCount(0);
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

test("a saved service URL owns browser record and status requests after reload", async ({
  page,
}) => {
  const savedOrigin = "https://saved.fasti.test";
  const recordUrls: string[] = [];
  const healthUrls: string[] = [];
  await page.addInitScript((serviceUrl) => {
    localStorage.setItem(
      "fasti-network-config",
      JSON.stringify({ service_url: serviceUrl }),
    );
  }, savedOrigin);
  await page.route(`${savedOrigin}/api/v1/records`, async (route) => {
    const request = route.request();
    if (request.method() === "OPTIONS") {
      await route.fulfill({
        status: 204,
        headers: {
          "access-control-allow-origin": "http://127.0.0.1:4173",
          "access-control-allow-headers": "authorization",
          "access-control-allow-methods": "GET",
        },
      });
      return;
    }
    recordUrls.push(request.url());
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      headers: {
        "access-control-allow-origin": "http://127.0.0.1:4173",
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

  await page.goto("/");
  await submitCredential(page);
  await expect(
    page.getByRole("heading", { name: "Saved endpoint record" }),
  ).toBeVisible();
  expect(recordUrls).toEqual([`${savedOrigin}/api/v1/records`]);

  await page.getByRole("link", { name: "Service status" }).click();
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
  expect(healthUrls).toEqual([`${savedOrigin}/api/v1/health`]);
  await expect(page.getByText(savedOrigin, { exact: true })).toBeVisible();
});

test("changing the browser service URL clears the origin-bound credential", async ({
  page,
}) => {
  await page.route(/\/api\/v1\/records$/, (route) => fulfillRecords(route));
  await page.goto("/");
  await submitCredential(page);
  await expect(
    page.getByRole("button", { name: "Clear browser credential" }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByLabel("Service URL").fill("https://new.fasti.test");
  await page.getByRole("button", { name: "Save service URL" }).click();
  await expect(page.getByRole("status")).toHaveText("Settings saved.");
  await expect(
    page.getByRole("button", { name: "Connect local credential" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Clear browser credential" }),
  ).toHaveCount(0);
});

test("record summaries stay truthful, bounded, and free of poster egress", async ({
  page,
}) => {
  const thirdPartyRequests: string[] = [];
  page.on("request", (request) => {
    if (request.url().startsWith("https://tracker.example")) {
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
  await page.goto("/");
  await submitCredential(page);
  await expect(
    page.getByRole("heading", { name: "Summary-only record" }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Library", exact: true }).click();
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
    page.getByText("External identifiers are not included"),
  ).toBeVisible();
  await expect(page.getByText("Custom fields are not included")).toBeVisible();

  const history = page.getByRole("button", { name: "History" });
  await history.click();
  await expect(history).toHaveAttribute("aria-pressed", "true");
  await expect(
    page.getByText("History is unavailable in this view"),
  ).toBeVisible();
  await page.getByRole("button", { name: "Sources & Identity" }).click();
  await expect(page.getByText("Identity claims are unavailable")).toBeVisible();

  await page.getByRole("button", { name: "Calendar", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Active state is unavailable" }),
  ).toBeVisible();
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
  await expect(page.getByRole("button", { name: "Overview" })).toBeVisible();
});

test("Discover selects configured providers and refreshes explicit setup state", async ({
  page,
}) => {
  await page.setViewportSize({ width: 375, height: 812 });
  await page.addInitScript(() => {
    let googleConfigured = false;
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
        configured: true,
        source: "environment",
        writable: false,
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
      __SEARCH_INPUT__?: unknown;
      __TRACK_INPUTS__?: Array<{ command: string; arguments_: unknown }>;
      __TAURI_INTERNALS__: {
        invoke: (command: string, arguments_?: unknown) => Promise<unknown>;
      };
    };
    browserWindow.__TRACK_INPUTS__ = [];
    browserWindow.__TAURI_INTERNALS__ = {
      invoke: async (command, arguments_) => {
        switch (command) {
          case "setup_status":
            return { phase: "ready", proof_cleanup_pending: false };
          case "load_network_configuration":
            return networkConfiguration;
          case "provider_credential_status":
            return providerStatus();
          case "save_provider_credential":
            googleConfigured = true;
            return providerStatus();
          case "search_provider":
            browserWindow.__SEARCH_INPUT__ = arguments_;
            return [
              {
                provider: "tmdb",
                provider_id: "1396",
                title: "Breaking Bad",
                kind: "show",
                authors: [],
                image_url: null,
              },
            ];
          case "register_namespace":
          case "attach_identifier":
            browserWindow.__TRACK_INPUTS__?.push({ command, arguments_ });
            return {};
          case "create_record":
            browserWindow.__TRACK_INPUTS__?.push({ command, arguments_ });
            return { record_id: "record-tmdb-show" };
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
  const providerBox = await provider.boundingBox();
  expect(providerBox?.height).toBeGreaterThanOrEqual(44);
  expect(
    await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    ),
  ).toBeLessThanOrEqual(0);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);

  const search = page.getByRole("searchbox", { name: "Search TMDB" });
  await search.fill("é".repeat(129));
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("256 UTF-8 bytes");
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __SEARCH_INPUT__?: unknown })
          .__SEARCH_INPUT__,
    ),
  ).toBeUndefined();

  await search.fill("Breaking Bad");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Breaking Bad" }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __SEARCH_INPUT__?: unknown })
          .__SEARCH_INPUT__,
    ),
  ).toEqual({
    input: { provider: "tmdb", query: "Breaking Bad" },
  });
  await page.getByRole("button", { name: "Track Now" }).click();
  await expect(
    page.getByRole("button", { name: "Added to library" }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (
          window as typeof window & {
            __TRACK_INPUTS__?: Array<{
              command: string;
              arguments_: unknown;
            }>;
          }
        ).__TRACK_INPUTS__,
    ),
  ).toEqual([
    {
      command: "register_namespace",
      arguments_: {
        input: {
          namespace: "tmdb.tv",
          label: "tmdb.tv",
          grains: ["series"],
          id_pattern: ".+",
          normalization: "identity",
          licence_posture: "identifiers_only",
        },
      },
    },
    {
      command: "create_record",
      arguments_: { grain: "series" },
    },
    {
      command: "attach_identifier",
      arguments_: {
        input: {
          record_id: "record-tmdb-show",
          namespace: "tmdb.tv",
          grain: "series",
          value: "1396",
        },
      },
    },
  ]);

  await provider.selectOption("google-books");
  await expect(
    page.getByRole("heading", { name: "Google Books needs a credential" }),
  ).toBeVisible();
  await expect(page.getByRole("heading", { name: "Breaking Bad" })).toHaveCount(
    0,
  );
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page
    .getByRole("button", { name: "Metadata credentials", exact: true })
    .click();
  await page.getByLabel("New credential").fill("provider-secret");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Credential saved");
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

  await page.getByRole("button", { name: "Discover", exact: true }).click();
  await expect(page.getByLabel("Metadata provider")).toHaveValue(
    "google-books",
  );
  await expect(
    page.getByRole("searchbox", { name: "Search Google Books" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Google Books needs a credential" }),
  ).toHaveCount(0);
});
