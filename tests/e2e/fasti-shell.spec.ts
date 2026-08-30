import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";

const health = { status: "healthy", version: "0.1.0" };
const healthEndpoint = /\/api\/v1\/health$/;
const viewports = [
  { width: 320, height: 800 },
  { width: 768, height: 900 },
  { width: 1440, height: 1000 },
] as const;

test("the bounded health fixture fails closed for unknown routes", async ({
  request,
}) => {
  const response = await request.get("http://127.0.0.1:18422/api/v1/unknown");
  expect(response.status()).toBe(404);
  await expect(response.json()).resolves.toMatchObject({
    title: "Not found",
    status: 404,
  });
  expect(
    (
      await request.get("http://127.0.0.1:18422/api/v1/records-not-a-route")
    ).status(),
  ).toBe(404);
});

async function mockHealth(page: Page) {
  await page.route(healthEndpoint, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(health),
    }),
  );
}

async function mockTrustedHost(
  page: Page,
  providerConfigured = false,
  holdProviderTest = false,
  environmentManaged = false,
) {
  const mockOptions =
    (providerConfigured ? 1 : 0) |
    (holdProviderTest ? 2 : 0) |
    (environmentManaged ? 4 : 0);
  await page.addInitScript((options) => {
    const managed = Boolean(options & 4);
    const configured = Boolean(options & 1) || managed;
    const holdTest = Boolean(options & 2);
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
    let providerIsConfigured = configured;
    const providerStatus = () => [
      {
        provider: "google-books",
        capability_id: "metadata.search",
        label: "Google Books",
        purpose: "Search book metadata",
        credential_requirement: "optional_api_key",
        credential_state: providerIsConfigured ? "valid" : "optional",
        state: "available",
        source: managed
          ? "environment"
          : providerIsConfigured
            ? "credential_store"
            : "none",
        writable: !managed,
        testable: true,
        docs_url: "https://developers.google.com/books/docs/v1/using",
      },
    ];
    let nuvioDocument: unknown = null;
    const browserWindow = window as typeof window & {
      __PROVIDER_SECRET_MATCH__?: boolean;
      __PROVIDER_SAVE_CALLS__?: number;
      __PROVIDER_STATUS_CALLS__?: number;
      __PROVIDER_TEST_CALLS__?: number;
      __RESOLVE_PROVIDER_TEST__?: () => void;
      __NUVIO_REPLACE_COUNT__?: number;
      __TAURI_INTERNALS__: {
        invoke: (command: string, arguments_: unknown) => Promise<unknown>;
      };
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
          case "get_nuvio_collections":
            return { document: nuvioDocument };
          case "replace_nuvio_collections": {
            const candidate = arguments_ as { document?: unknown };
            browserWindow.__NUVIO_REPLACE_COUNT__ =
              (browserWindow.__NUVIO_REPLACE_COUNT__ ?? 0) + 1;
            nuvioDocument = candidate.document;
            return { document: nuvioDocument };
          }
          case "clear_nuvio_collections":
            nuvioDocument = null;
            return { document: null };
          case "test_endpoint_connection":
            return {
              endpoint: "http://127.0.0.1:8420",
              scheme: "http",
              status: "healthy",
              version: "0.1.0-test",
            };
          case "save_provider_credential": {
            browserWindow.__PROVIDER_SAVE_CALLS__ =
              (browserWindow.__PROVIDER_SAVE_CALLS__ ?? 0) + 1;
            const candidate = arguments_ as {
              input?: { provider?: string; credential?: string };
            };
            browserWindow.__PROVIDER_SECRET_MATCH__ =
              candidate.input?.provider === "google-books" &&
              candidate.input?.credential === "test-secret-for-correction";
            if (browserWindow.__PROVIDER_SECRET_MATCH__) {
              throw {
                code: "secure_storage_unavailable",
                title: "Secure storage is unavailable",
                detail: "The credential store rejected the test value.",
                next_action: "Unlock the credential store, then retry.",
              };
            }
            providerIsConfigured = true;
            return providerStatus();
          }
          case "test_provider_credential":
            browserWindow.__PROVIDER_TEST_CALLS__ =
              (browserWindow.__PROVIDER_TEST_CALLS__ ?? 0) + 1;
            if (holdTest) {
              await new Promise<void>((resolve) => {
                browserWindow.__RESOLVE_PROVIDER_TEST__ = resolve;
              });
              return providerStatus();
            }
            if (managed) return providerStatus();
            throw new Error("Trusted provider execution is unavailable.");
          default:
            throw new Error(`Unexpected trusted-host command: ${command}`);
        }
      },
    };
  }, mockOptions);
}

test("global search and configured record actions use durable tracking state", async ({
  page,
}) => {
  const recordId = "rec_01991f588e0070008000000000000002";
  let updatedDisposition: string | null = null;

  await page.addInitScript(() => {
    localStorage.setItem(
      "fasti-workbench-preferences",
      JSON.stringify({
        contextMenuItems: [
          { id: "view", label: "View Details", visible: false, order: 0 },
        ],
      }),
    );
  });
  await page.route(/\/api\/v1\/records$/, (route) =>
    route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        records: [
          {
            grain: "film",
            latest_activity: {
              occurred_at: null,
              interpretation_state: "resolved",
            },
            poster: {
              is_stale: false,
              tier: "empty",
              value: null,
              source: null,
            },
            record_id: recordId,
            status: "active",
            title: {
              is_stale: false,
              tier: "user_override",
              value: "Alpha Film",
              source: "local",
            },
          },
        ],
        truncated: false,
      }),
    }),
  );
  await page.route(
    /\/api\/v1\/profile\/record-tracking-dispositions(?:\/[^/]+)?$/,
    async (route) => {
      const request = route.request();
      if (request.method() === "GET") {
        await route.abort("failed");
        return;
      }
      expect(request.method()).toBe("PUT");
      updatedDisposition = request.postDataJSON().disposition;
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({
          record_id: recordId,
          disposition: updatedDisposition,
        }),
      });
    },
  );

  await page.goto("/library");
  await expect(page.getByRole("heading", { name: "Library" })).toBeVisible();
  await expect(page.getByText("Alpha Film", { exact: true })).toBeVisible();
  await expect(page.getByRole("status")).toContainText(
    "Records still use their activity fallback.",
  );

  const search = page.getByRole("combobox", {
    name: "Search records or commands",
  });
  await search.fill("Alpha");
  await expect(page.getByRole("option", { name: /Alpha Film/ })).toBeVisible();
  await search.press("Enter");
  await expect(page.getByRole("heading", { name: "Alpha Film" })).toBeVisible();
  await expect(
    page.getByRole("combobox", { name: "Profile tracking state" }),
  ).toHaveValue("unset");
  await page.getByRole("button", { name: "Back to Library" }).click();

  await page.getByRole("group", { name: "Alpha Film card" }).click({
    button: "right",
  });
  const menu = page.getByRole("menu");
  await expect(
    menu.getByText("Playback & tracking", { exact: true }),
  ).toBeVisible();
  await expect(
    menu.getByText("Library & lists", { exact: true }),
  ).toBeVisible();
  await expect(
    menu.getByText("Identity & metadata", { exact: true }),
  ).toBeVisible();
  await expect(
    menu.getByRole("menuitem", { name: /View media details/ }),
  ).toHaveCount(0);
  await expect(
    menu.getByRole("menuitem", { name: /Update progress and episodes/ }),
  ).toBeDisabled();
  await menu.getByRole("menuitem", { name: "Mark as on hold" }).click();
  await expect(page.getByRole("status")).toHaveText(
    "Tracking state set to on hold.",
  );
  await expect(page.getByText("on hold", { exact: true })).toBeVisible();
  expect(updatedDisposition).toBe("on_hold");

  await page.keyboard.press("Control+K");
  await expect(search).toBeFocused();
  await search.fill("Settings");
  await search.press("Enter");
  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();

  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
});

test("record metadata can refresh or switch through a configured provider", async ({
  page,
}) => {
  const recordId = "rec_01991f588e0070008000000000000003";
  const showRecordId = "rec_01991f588e0070008000000000000004";
  const bookRecordId = "rec_01991f588e0070008000000000000005";
  await page.setViewportSize({ width: 320, height: 900 });
  await page.addInitScript(
    ({ id, showId, bookId }) => {
      let title = "Dune";
      let overview = "A noble family becomes involved in a war for Arrakis.";
      let identifiers = [
        { namespace: "tmdb.tv", grain: "film", value: "438631" },
      ];
      let providerStatusCalls = 0;
      const browserWindow = window as typeof window & {
        __METADATA_CALLS__?: unknown[];
        __TAURI_INTERNALS__: {
          invoke: (command: string, arguments_: unknown) => Promise<unknown>;
        };
      };
      browserWindow.__METADATA_CALLS__ = [];
      browserWindow.__TAURI_INTERNALS__ = {
        invoke: async (command, arguments_) => {
          switch (command) {
            case "setup_status":
              return { phase: "ready", proof_cleanup_pending: false };
            case "provider_credential_status":
              providerStatusCalls += 1;
              if (providerStatusCalls === 1) {
                throw new Error("Provider status is temporarily unavailable.");
              }
              return [
                {
                  provider: "tmdb",
                  capability_id: "metadata.search",
                  label: "TMDB",
                  purpose: "Search film and television metadata",
                  credential_requirement: "bearer_token",
                  credential_state: "valid",
                  state: "available",
                  source: "credential_store",
                  writable: true,
                  testable: true,
                  docs_url: "https://developer.themoviedb.org/",
                },
                {
                  provider: "google-books",
                  capability_id: "metadata.search",
                  label: "Google Books",
                  purpose: "Search book metadata",
                  credential_requirement: "optional_api_key",
                  credential_state: "valid",
                  state: "available",
                  source: "credential_store",
                  writable: true,
                  testable: true,
                  docs_url: "https://developers.google.com/books/",
                },
              ];
            case "list_tracking_dispositions":
              return { states: [], truncated: false };
            case "list_records":
              return {
                records: [
                  {
                    grain: "film",
                    identifiers,
                    latest_activity: null,
                    original_title: {
                      is_stale: false,
                      tier: "fallback_provider_claim",
                      value: title,
                      source: "tmdb.movie",
                    },
                    overview: {
                      is_stale: false,
                      tier: "fallback_provider_claim",
                      value: overview,
                      source: "tmdb.movie",
                    },
                    poster: {
                      is_stale: false,
                      tier: "empty",
                      value: null,
                      source: null,
                    },
                    record_id: id,
                    release_year: {
                      is_stale: false,
                      tier: "fallback_provider_claim",
                      value: "2021",
                      source: "tmdb.movie",
                    },
                    status: "active",
                    title: {
                      is_stale: false,
                      tier: "fallback_provider_claim",
                      value: title,
                      source: "tmdb.movie",
                    },
                  },
                  {
                    grain: "series",
                    identifiers: [
                      { namespace: "tmdb.tv", grain: "series", value: "1396" },
                    ],
                    latest_activity: null,
                    original_title: {
                      is_stale: false,
                      tier: "empty",
                      value: null,
                      source: null,
                    },
                    overview: {
                      is_stale: false,
                      tier: "fallback_provider_claim",
                      value:
                        "A chemistry teacher becomes a methamphetamine producer.",
                      source: "tmdb.tv",
                    },
                    poster: {
                      is_stale: false,
                      tier: "empty",
                      value: null,
                      source: null,
                    },
                    record_id: showId,
                    release_year: {
                      is_stale: false,
                      tier: "fallback_provider_claim",
                      value: "2008",
                      source: "tmdb.tv",
                    },
                    status: "active",
                    title: {
                      is_stale: false,
                      tier: "fallback_provider_claim",
                      value: "Breaking Bad",
                      source: "tmdb.tv",
                    },
                  },
                  {
                    grain: "edition",
                    identifiers: [
                      {
                        namespace: "googlebooks.volume",
                        grain: "edition",
                        value: "dune-volume",
                      },
                    ],
                    latest_activity: null,
                    original_title: {
                      is_stale: false,
                      tier: "empty",
                      value: null,
                      source: null,
                    },
                    overview: {
                      is_stale: false,
                      tier: "fallback_provider_claim",
                      value: "A science fiction novel.",
                      source: "googlebooks.volume",
                    },
                    poster: {
                      is_stale: false,
                      tier: "empty",
                      value: null,
                      source: null,
                    },
                    record_id: bookId,
                    release_year: {
                      is_stale: false,
                      tier: "fallback_provider_claim",
                      value: "1965",
                      source: "googlebooks.volume",
                    },
                    status: "active",
                    title: {
                      is_stale: false,
                      tier: "fallback_provider_claim",
                      value: "Dune edition",
                      source: "googlebooks.volume",
                    },
                  },
                ],
                truncated: false,
              };
            case "search_provider":
              return [
                {
                  provider: "tmdb",
                  provider_id: "693134",
                  title: "Dune: Part Two",
                  original_title: "Dune: Part Two",
                  kind: "movie",
                  release_year: 2024,
                  authors: [],
                  image_url: null,
                  overview: "Paul Atreides unites with Chani and the Fremen.",
                },
              ];
            case "apply_provider_metadata": {
              const call = (arguments_ as { input: unknown }).input as {
                record_id: string;
                selection: {
                  provider: string;
                  provider_id: string;
                  kind: string;
                };
              };
              browserWindow.__METADATA_CALLS__?.push(call);
              if (call.selection.provider_id === "693134") {
                title = "Dune: Part Two";
                overview = "Paul Atreides unites with Chani and the Fremen.";
                identifiers = [
                  ...identifiers,
                  { namespace: "tmdb.movie", grain: "film", value: "693134" },
                ];
              }
              return undefined;
            }
            default:
              throw new Error(`Unexpected trusted-host command: ${command}`);
          }
        },
      };
    },
    { id: recordId, showId: showRecordId, bookId: bookRecordId },
  );
  const metadataCalls = () =>
    page.evaluate(
      () =>
        (
          window as typeof window & {
            __METADATA_CALLS__?: unknown[];
          }
        ).__METADATA_CALLS__,
    );

  await page.goto(`/records/${recordId}`);
  await expect(
    page.getByRole("heading", { level: 1, name: "Dune" }),
  ).toBeVisible();
  await page.getByRole("button", { name: /Sources & Identity/ }).click();

  await expect(
    page.getByRole("heading", { name: "Provider credits" }),
  ).toBeVisible();
  await expect(
    page.getByRole("row").filter({ hasText: "tmdb.tv" }),
  ).toContainText("No live adapter");
  await page.getByRole("button", { name: "Retry host connection" }).click();
  await page
    .getByRole("searchbox", { name: "Search TMDB" })
    .fill("Dune Part Two");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.getByText("Dune: Part Two", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Use metadata" }).click();
  await expect(
    page.getByRole("heading", { level: 1, name: "Dune: Part Two" }),
  ).toBeVisible();
  await page
    .getByRole("row")
    .filter({ hasText: "tmdb.movie" })
    .getByRole("button", { name: "Refresh" })
    .click();
  await expect(page.getByText("Refreshed metadata from TMDB.")).toBeVisible();
  expect(await metadataCalls()).toEqual([
    {
      record_id: recordId,
      selection: { provider: "tmdb", provider_id: "693134", kind: "movie" },
    },
    {
      record_id: recordId,
      selection: { provider: "tmdb", provider_id: "693134", kind: "movie" },
    },
  ]);

  await page.goto(`/records/${showRecordId}`);
  await page.getByRole("button", { name: /Sources & Identity/ }).click();
  await page.getByRole("button", { name: "Refresh" }).click();
  await expect(page.getByText("Refreshed metadata from TMDB.")).toBeVisible();
  expect(await metadataCalls()).toEqual([
    {
      record_id: showRecordId,
      selection: { provider: "tmdb", provider_id: "1396", kind: "show" },
    },
  ]);

  await page.goto(`/records/${bookRecordId}`);
  await page.getByRole("button", { name: /Sources & Identity/ }).click();
  await page.getByRole("button", { name: "Refresh" }).click();
  await expect(
    page.getByText("Refreshed metadata from Google Books."),
  ).toBeVisible();

  expect(await metadataCalls()).toEqual([
    {
      record_id: bookRecordId,
      selection: {
        provider: "google-books",
        provider_id: "dune-volume",
        kind: "book",
      },
    },
  ]);
  expect(
    await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    ),
  ).toBeLessThanOrEqual(0);
  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
});

for (const theme of ["light", "dark"] as const) {
  for (const viewport of viewports) {
    test(`${theme} theme at ${viewport.width}px is truthful, reflowable, and accessible`, async ({
      page,
    }, testInfo) => {
      await page.setViewportSize(viewport);
      await page.addInitScript(
        (value) => localStorage.setItem("fasti-theme", value),
        theme,
      );
      await mockHealth(page);
      await page.goto("/status");

      await expect(page).toHaveTitle("Local service status · Fasti");
      await expect(page.getByRole("heading", { level: 1 })).toHaveText(
        "Local service status",
      );
      await expect(
        page.getByRole("heading", { name: "Local service available" }),
      ).toBeVisible();
      await expect(
        page.getByRole("heading", { name: "Network settings" }),
      ).toBeVisible();
      await expect(
        page
          .locator("#network-settings dd")
          .filter({ hasText: "http://127.0.0.1:4173" }),
      ).toBeVisible();
      await expect(page.getByText("http://localhost:4173")).toBeVisible();
      await expect(page.locator("html")).toHaveAttribute(
        "data-bs-theme",
        theme,
      );
      await expect(page.getByText("Review inbox", { exact: true })).toHaveCount(
        0,
      );
      await expect(page.getByText("Discover", { exact: true })).toHaveCount(0);
      await expect(page.getByText("Connections", { exact: true })).toHaveCount(
        0,
      );

      const overflow = await page.evaluate(
        () =>
          document.documentElement.scrollWidth -
          document.documentElement.clientWidth,
      );
      expect(overflow).toBeLessThanOrEqual(0);

      for (const control of await page.getByRole("button").all()) {
        const box = await control.boundingBox();
        expect(box?.width).toBeGreaterThanOrEqual(44);
        expect(box?.height).toBeGreaterThanOrEqual(44);
      }

      const accessibility = await new AxeBuilder({ page }).analyze();
      expect(accessibility.violations).toEqual([]);
      await page.screenshot({
        path: testInfo.outputPath(`fasti-shell-${theme}-${viewport.width}.png`),
        fullPage: true,
        animations: "disabled",
      });
    });
  }
}

test("keyboard path, theme persistence, and unavailable recovery remain clear", async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await page.route(healthEndpoint, (route) => route.abort("connectionrefused"));
  await page.goto("/status");

  await expect(page.getByRole("alert")).toContainText(
    "local service is unavailable",
  );
  await page.keyboard.press("Tab");
  await expect(
    page.getByRole("link", { name: "Skip to main content" }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator("main")).toBeFocused();

  const themeButton = page.getByRole("button", { name: "Use dark theme" });
  await themeButton.click();
  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "dark");
  await expect(
    page.getByRole("button", { name: "Use light theme" }),
  ).toBeVisible();
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "dark");
  await page.getByRole("button", { name: "Use light theme" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "light");
  await expect(
    page.getByRole("button", { name: "Use dark theme" }),
  ).toBeVisible();

  await expect(page.getByRole("button", { name: "Try again" })).toBeVisible();
  await page.getByRole("button", { name: "Try again" }).click();
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(page.getByRole("button", { name: "Try again" })).toBeFocused();
  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
  await page.screenshot({
    path: testInfo.outputPath("fasti-shell-unavailable-320.png"),
    fullPage: true,
    animations: "disabled",
  });
});

test("invalid health responses use the contract recovery state", async ({
  page,
}, testInfo) => {
  await page.route(healthEndpoint, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ status: "healthy" }),
    }),
  );
  await page.goto("/status");

  await expect(
    page.getByRole("heading", {
      name: "The local service returned an invalid response",
    }),
  ).toBeVisible();
  await expect(page.getByText("generated health contract")).toBeVisible();
  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
  await page.screenshot({
    path: testInfo.outputPath("fasti-shell-invalid-response.png"),
    fullPage: true,
    animations: "disabled",
  });
});

test("the loading state prevents duplicate concurrent retries", async ({
  page,
}) => {
  let requestCount = 0;
  let releaseCurrentResponse: () => void;
  const currentResponse = new Promise<void>((resolve) => {
    releaseCurrentResponse = resolve;
  });
  await page.route(healthEndpoint, async (route) => {
    requestCount += 1;
    if (requestCount === 1) {
      await route.abort("connectionrefused");
      return;
    }
    if (requestCount === 2) {
      await currentResponse;
    }
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(health),
    });
  });
  await page.goto("/status");
  const retry = page.getByRole("button", { name: "Try again" });
  await expect(retry).toBeVisible();

  await retry.evaluate((button) => {
    button.click();
    button.click();
  });

  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      ),
  );
  expect(requestCount).toBe(2);
  await expect(page.getByText("Checking the local service")).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
  releaseCurrentResponse!();
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
});

test("the Vite proxy reaches the bounded health fixture", async ({ page }) => {
  await page.goto("/status");
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
  await expect(page.getByText("0.1.0-test")).toBeVisible();
});

test("the saved theme is applied before the application module", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem("fasti-theme", "light");
    localStorage.setItem(
      "fasti-theme-settings",
      JSON.stringify({ mode: "night" }),
    );
  });
  await page.route(/\/src\/main\.ts$/, (route) => route.abort());
  await page.goto("/status");

  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "dark");
  await expect(page.locator("html")).toHaveAttribute(
    "data-fasti-theme",
    "night",
  );
  const colors = await page.locator("body").evaluate((body) => {
    const style = getComputedStyle(body);
    return {
      background: style.backgroundColor,
      foreground: style.color,
    };
  });
  expect(colors.background).not.toBe("rgba(0, 0, 0, 0)");
  expect(colors.background).not.toBe("rgb(247, 244, 237)");
  expect(colors.foreground).not.toBe("rgb(24, 23, 22)");
});

test("system dark mode survives unavailable local storage", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.addInitScript(() => {
    Object.defineProperty(Storage.prototype, "getItem", {
      value: () => {
        throw new DOMException("Storage is unavailable", "SecurityError");
      },
    });
  });
  await mockHealth(page);
  await page.goto("/status");

  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "dark");
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
});

test("theme changes remain usable when persistence is unavailable", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(Storage.prototype, "setItem", {
      value: () => {
        throw new DOMException("Storage is unavailable", "SecurityError");
      },
    });
  });
  await mockHealth(page);
  await page.goto("/status");

  await page.getByRole("button", { name: "Use dark theme" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "dark");
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
});

test("text enlargement and WCAG text spacing do not lose content", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await mockHealth(page);
  await page.goto("/status");

  await page.locator("html").evaluate((element) => {
    element.style.fontSize = "200%";
  });
  expect(
    await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    ),
  ).toBeLessThanOrEqual(0);
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();

  await page.locator("html").evaluate((element) => {
    element.style.fontSize = "100%";
    const style = document.createElement("style");
    style.textContent = `
      * { line-height: 1.5 !important; letter-spacing: 0.12em !important; word-spacing: 0.16em !important; }
      p { margin-block-end: 2em !important; }
    `;
    document.head.append(style);
  });
  expect(
    await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    ),
  ).toBeLessThanOrEqual(0);
  await expect(
    page.getByText("Records and durable occurrence ingress"),
  ).toBeVisible();
});

test("reduced motion stops the loading animation", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  let releaseHealth = () => {};
  const pendingHealth = new Promise<void>((resolve) => {
    releaseHealth = resolve;
  });
  await page.route(healthEndpoint, async (route) => {
    await pendingHealth;
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(health),
    });
  });
  await page.goto("/status", { waitUntil: "domcontentloaded" });

  await expect(page.getByText("Checking the local service")).toBeVisible();
  await expect(page.locator(".spinner")).toHaveCSS("animation-name", "none");
  releaseHealth();
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
});

test("forced colors preserves visible status and controls", async ({
  page,
}) => {
  await page.emulateMedia({ forcedColors: "active" });
  await mockHealth(page);
  await page.goto("/status");

  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Use dark theme" }),
  ).toBeVisible();
  await expect(
    page.getByRole("link", { name: "Skip to main content" }),
  ).toBeAttached();
  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
});

test("the harness does not contact third-party origins", async ({ page }) => {
  const externalOrigins = new Set<string>();
  page.on("request", (request) => {
    const origin = new URL(request.url()).origin;
    if (origin !== "http://127.0.0.1:4173") externalOrigins.add(origin);
  });
  await mockHealth(page);
  await page.goto("/status");
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();

  expect([...externalOrigins]).toEqual([]);
});

test("trusted-host provider settings retain a rejected secret for correction", async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 320, height: 900 });
  await mockTrustedHost(page);
  await page.goto("/settings");

  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  await page
    .getByLabel("Settings section", { exact: true })
    .selectOption("providers");
  await expect(page.getByText("optional", { exact: true })).toBeVisible();

  const credential = page.getByLabel(
    "Google Books API key for metadata.search",
  );
  await expect(credential).toHaveAttribute("type", "password");
  await credential.fill("test-secret-for-correction");
  await page.getByRole("button", { name: "Show secret" }).click();
  await expect(credential).toHaveAttribute("type", "text");
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByRole("alert")).toContainText(
    "The credential store rejected the test value.",
  );
  await expect(credential).toHaveValue("test-secret-for-correction");
  await expect(credential).toHaveAttribute("type", "password");
  await expect(credential).toBeFocused();
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __PROVIDER_SECRET_MATCH__?: boolean })
          .__PROVIDER_SECRET_MATCH__,
    ),
  ).toBe(true);
  expect((await page.locator("body").textContent()) ?? "").not.toContain(
    "test-secret-for-correction",
  );
  expect(page.url()).not.toContain("test-secret-for-correction");
  expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toContain(
    "test-secret-for-correction",
  );

  await credential.fill("test-secret-stored");
  await page.getByRole("button", { name: "Show secret" }).click();
  await expect(credential).toHaveAttribute("type", "text");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(
    page.getByRole("status").filter({ hasText: "Credential saved" }),
  ).toBeVisible();
  await expect(credential).toHaveValue("");
  await expect(credential).toHaveAttribute("type", "password");

  const undersizedControls = await page
    .locator("button:visible, input:visible")
    .evaluateAll((controls) =>
      controls.flatMap((control) => {
        const box = control.getBoundingClientRect();
        return box.width < 44 || box.height < 44
          ? [
              `${control.getAttribute("aria-label") ?? control.textContent?.trim()}: ${box.width}x${box.height}`,
            ]
          : [];
      }),
    );
  expect(undersizedControls).toEqual([]);

  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
  await page.screenshot({
    path: testInfo.outputPath("provider-settings-rejected-secret-320.png"),
    fullPage: true,
    animations: "disabled",
  });
});

test("provider credential tests fail closed when trusted execution is unavailable", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 900 });
  await mockTrustedHost(page, true);
  await page.goto("/settings/metadata");

  await page.getByRole("button", { name: "Test credential" }).click();
  const result = page.locator(".test-result-alert");
  await expect(result).toHaveText("Trusted provider execution is unavailable.");
  await expect(result).not.toContainText("Credential test passed");
});

test("environment-managed credentials remain testable and read-only", async ({
  page,
}) => {
  await mockTrustedHost(page, false, false, true);
  await page.goto("/settings/metadata");

  await expect(
    page.getByText("Managed by the process environment."),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Save" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Remove" })).toHaveCount(0);
  await page.getByRole("button", { name: "Test credential" }).click();
  await expect(
    page.getByRole("status").filter({ hasText: "Credential test passed" }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __PROVIDER_TEST_CALLS__?: number })
          .__PROVIDER_TEST_CALLS__,
    ),
  ).toBe(1);
});

test("provider credential operations remain single-flight", async ({
  page,
}) => {
  await mockTrustedHost(page, true, true);
  await page.goto("/settings/metadata");

  const credential = page.getByLabel(
    "Google Books API key for metadata.search",
  );
  await credential.fill("queued-save");
  await page.getByRole("button", { name: "Test credential" }).click();
  await expect(page.getByRole("button", { name: "Testing…" })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Refresh" })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Save" })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Remove" })).toBeDisabled();
  await expect(credential).toBeDisabled();
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __PROVIDER_TEST_CALLS__?: number })
          .__PROVIDER_TEST_CALLS__,
    ),
  ).toBe(1);
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __PROVIDER_STATUS_CALLS__?: number })
          .__PROVIDER_STATUS_CALLS__,
    ),
  ).toBe(1);
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __PROVIDER_SAVE_CALLS__?: number })
          .__PROVIDER_SAVE_CALLS__ ?? 0,
    ),
  ).toBe(0);

  await page.evaluate(() =>
    (
      window as typeof window & {
        __RESOLVE_PROVIDER_TEST__?: () => void;
      }
    ).__RESOLVE_PROVIDER_TEST__?.(),
  );
  await expect(
    page.getByRole("status").filter({ hasText: "Credential test passed" }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Save" })).toBeEnabled();
  await page.getByRole("button", { name: "Save" }).click();
  await expect(
    page.getByRole("status").filter({ hasText: "Credential saved" }),
  ).toBeVisible();
  await expect(
    page.getByRole("status").filter({ hasText: "Credential test passed" }),
  ).toHaveCount(0);
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __PROVIDER_SAVE_CALLS__?: number })
          .__PROVIDER_SAVE_CALLS__,
    ),
  ).toBe(1);
});

test("profile Nuvio Collections import, export, and clear stay local", async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 320, height: 900 });
  await mockTrustedHost(page);
  const externalOrigins = new Set<string>();
  page.on("request", (request) => {
    const origin = new URL(request.url()).origin;
    if (origin !== "http://127.0.0.1:4173") externalOrigins.add(origin);
  });
  await page.goto("/settings/collections");

  await expect(page.getByText("Not imported", { exact: true })).toBeVisible();
  const input = [
    {
      id: "collection",
      title: "Collection",
      folders: [
        {
          id: "folder",
          title: "Folder",
          coverImageUrl: "https://example.invalid/never-requested.jpg",
          sources: [
            {
              id: "source",
              provider: "tmdb",
              tmdbSourceType: "discover",
              filters: { vote_count_gte: 10 },
            },
          ],
        },
      ],
    },
  ];
  await page.getByLabel("Nuvio JSON file").setInputFiles({
    name: "nuvio.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(input)),
  });
  await page.getByRole("button", { name: "Import and replace" }).click();
  await expect(page.getByRole("status")).toContainText(
    "Imported 1 collections, 1 folders, and 1 sources",
  );
  await expect(page.getByText("Saved", { exact: true })).toBeVisible();
  await page.screenshot({
    path: testInfo.outputPath("nuvio-collections-imported-320.png"),
    fullPage: true,
    animations: "disabled",
  });

  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export JSON" }).click();
  const download = await downloadPromise;
  const downloadPath = await download.path();
  expect(download.suggestedFilename()).toMatch(
    /^fasti-nuvio-collections-\d{4}-\d{2}-\d{2}\.json$/,
  );
  expect(JSON.parse(await readFile(downloadPath!, "utf8"))).toEqual(input);

  const preset = page.locator(".preset-card").filter({
    hasText: "Kaptain's Mega Collection",
  });
  page.once("dialog", (dialog) => dialog.dismiss());
  await preset.getByRole("button", { name: "Install pack" }).click();
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __NUVIO_REPLACE_COUNT__?: number })
          .__NUVIO_REPLACE_COUNT__,
    ),
  ).toBe(1);
  const dismissedDownloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export JSON" }).click();
  const dismissedDownload = await dismissedDownloadPromise;
  expect(
    JSON.parse(await readFile((await dismissedDownload.path())!, "utf8")),
  ).toEqual(input);

  page.once("dialog", (dialog) => dialog.accept());
  await preset.getByRole("button", { name: "Install pack" }).click();
  await expect(page.getByRole("status")).toContainText(
    "Installed Kaptain's Mega Collection",
  );
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __NUVIO_REPLACE_COUNT__?: number })
          .__NUVIO_REPLACE_COUNT__,
    ),
  ).toBe(2);
  const presetDownloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export JSON" }).click();
  const presetDownload = await presetDownloadPromise;
  const presetDownloadPath = await presetDownload.path();
  expect(await readFile(presetDownloadPath!, "utf8")).not.toContain(
    "coverImageUrl",
  );

  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Clear saved document" }).click();
  await expect(page.getByRole("status")).toHaveText(
    "This profile's Nuvio Collections document was cleared.",
  );
  await expect(page.getByText("Not imported", { exact: true })).toBeVisible();
  expect(externalOrigins).toEqual(new Set());
  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
});
