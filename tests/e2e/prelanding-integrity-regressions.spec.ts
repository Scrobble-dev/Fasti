import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

type Scenario =
  | "search-race"
  | "status-race"
  | "status-route"
  | "status-setup-error"
  | "status-invalid-response"
  | "credential-delete"
  | "review-resolution"
  | "record-retry"
  | "record-retry-failure";

type Candidate = {
  provider: string;
  provider_id: string;
  title: string;
  kind: string;
  authors: string[];
  image_url: null;
};

declare global {
  interface Window {
    __DELETE_CALLS__?: () => number;
    __ENDPOINT_CALLS__?: () => number;
    __RECORD_CALLS__?: () => number;
    __REVIEW_CALLS__?: () => number;
    __RESOLVE_REVIEW__?: () => void;
    __RESOLVE_SEARCH__?: (index: number, result: Candidate) => void;
    __RESOLVE_STATUS__?: (
      index: number,
      googleConfigured: boolean,
      tmdbConfigured: boolean,
    ) => void;
    __SEARCH_CALLS__?: () => number;
    __STATUS_CALLS__?: () => number;
  }
}

async function installTrustedHost(page: Page, scenario: Scenario) {
  await page.addInitScript((activeScenario) => {
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
    const statuses = (googleConfigured: boolean, tmdbConfigured: boolean) => [
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
    type ProviderStatus = ReturnType<typeof statuses>;
    const searchResolvers: Array<(results: Candidate[]) => void> = [];
    const statusResolvers: Array<{
      resolve: (value: ProviderStatus) => void;
      reject: (reason: unknown) => void;
    }> = [];
    let providerConfigured = true;
    let reviewResolved = false;
    let deleteCalls = 0;
    let reviewCalls = 0;
    let recordCalls = 0;
    let endpointCalls = 0;
    const browserWindow = window as typeof window & {
      __DELETE_CALLS__?: () => number;
      __ENDPOINT_CALLS__?: () => number;
      __RECORD_CALLS__?: () => number;
      __REVIEW_CALLS__?: () => number;
      __RESOLVE_REVIEW__?: () => void;
      __RESOLVE_SEARCH__?: (index: number, result: Candidate) => void;
      __RESOLVE_STATUS__?: (
        index: number,
        googleConfigured: boolean,
        tmdbConfigured: boolean,
      ) => void;
      __SEARCH_CALLS__?: () => number;
      __STATUS_CALLS__?: () => number;
      __TAURI_INTERNALS__: {
        invoke: (command: string, arguments_?: unknown) => Promise<unknown>;
      };
    };
    browserWindow.__DELETE_CALLS__ = () => deleteCalls;
    browserWindow.__ENDPOINT_CALLS__ = () => endpointCalls;
    browserWindow.__RECORD_CALLS__ = () => recordCalls;
    browserWindow.__REVIEW_CALLS__ = () => reviewCalls;
    browserWindow.__SEARCH_CALLS__ = () => searchResolvers.length;
    browserWindow.__STATUS_CALLS__ = () => statusResolvers.length;
    browserWindow.__RESOLVE_SEARCH__ = (index, result) =>
      searchResolvers[index]?.([result]);
    browserWindow.__RESOLVE_STATUS__ = (index, google, tmdb) =>
      statusResolvers[index]?.resolve(statuses(google, tmdb));

    let resolveReview: (() => void) | undefined;
    browserWindow.__RESOLVE_REVIEW__ = () => resolveReview?.();
    browserWindow.__TAURI_INTERNALS__ = {
      invoke: async (command) => {
        switch (command) {
          case "setup_status":
            if (activeScenario === "status-setup-error") {
              throw {
                code: "storage_unavailable",
                title: "Local storage is unavailable",
                detail: "Fasti could not inspect its local data root.",
                next_action: "Check the Fasti data directory, then retry.",
              };
            }
            return {
              phase:
                activeScenario === "status-route" ? "needs_setup" : "ready",
              proof_cleanup_pending: false,
            };
          case "load_network_configuration":
            return networkConfiguration;
          case "test_endpoint_connection":
            endpointCalls += 1;
            if (activeScenario === "status-invalid-response") {
              throw {
                code: "invalid_response",
                title: "Invalid service response",
                detail: "The endpoint returned an invalid health response.",
                next_action:
                  "Stop the local service, rebuild it, and start it again.",
              };
            }
            return {
              endpoint: "http://127.0.0.1:8420",
              scheme: "http",
              status: "healthy",
              version: "0.1.0-test",
            };
          case "provider_credential_status":
            if (activeScenario === "status-race") {
              return new Promise((resolve, reject) => {
                statusResolvers.push({ resolve, reject });
              });
            }
            return activeScenario === "credential-delete"
              ? statuses(providerConfigured, false)
              : statuses(true, true);
          case "search_provider":
            if (activeScenario !== "search-race") return [];
            return new Promise((resolve) => searchResolvers.push(resolve));
          case "delete_provider_credential":
            deleteCalls += 1;
            providerConfigured = false;
            return statuses(false, false);
          case "list_reviews":
            if (activeScenario !== "review-resolution" || reviewResolved) {
              return [];
            }
            return [
              {
                review_item_id: "review-1",
                observation_id: "observation-1",
                current_interpretation_id: "interpretation-1",
                status: "open",
                candidate_record_ids: ["record-existing"],
              },
            ];
          case "resolve_review":
            reviewCalls += 1;
            return new Promise((resolve) => {
              resolveReview = () => {
                reviewResolved = true;
                resolve({
                  review_item_id: "review-1",
                  record_id: "record-existing",
                  interpretation_id: "interpretation-2",
                });
              };
            });
          case "list_records":
            recordCalls += 1;
            if (
              activeScenario === "record-retry-failure" ||
              (activeScenario === "record-retry" && recordCalls === 1)
            ) {
              throw new Error("temporary record read failure");
            }
            return activeScenario === "record-retry"
              ? [
                  {
                    record_id: "record-recovered",
                    grain: "work",
                    status: "active",
                    title: {
                      tier: "user_override",
                      value: "Recovered record",
                      source: "local",
                      is_stale: false,
                    },
                    poster: {
                      tier: "empty",
                      value: null,
                      source: null,
                      is_stale: false,
                    },
                    latest_activity: null,
                  },
                ]
              : [];
          default:
            throw new Error(`Unexpected trusted-host command: ${command}`);
        }
      },
    };
  }, scenario);
}

test("Discover ignores an in-flight result after the provider changes", async ({
  page,
}) => {
  await installTrustedHost(page, "search-race");
  await page.goto("/discover");
  const provider = page.getByLabel("Metadata provider");
  await provider.selectOption("tmdb");
  await page.getByRole("searchbox", { name: "Search TMDB" }).fill("Old query");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect
    .poll(() => page.evaluate(() => window.__SEARCH_CALLS__?.()))
    .toBe(1);

  await provider.selectOption("google-books");
  await page
    .getByRole("searchbox", { name: "Search Google Books" })
    .fill("Current query");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect
    .poll(() => page.evaluate(() => window.__SEARCH_CALLS__?.()))
    .toBe(2);
  await page.evaluate(() =>
    window.__RESOLVE_SEARCH__?.(1, {
      provider: "google-books",
      provider_id: "current",
      title: "Current Google result",
      kind: "book",
      authors: [],
      image_url: null,
    }),
  );
  await expect(page.getByText("Current Google result")).toBeVisible();

  await page.evaluate(() =>
    window.__RESOLVE_SEARCH__?.(0, {
      provider: "tmdb",
      provider_id: "stale",
      title: "Stale TMDB result",
      kind: "show",
      authors: [],
      image_url: null,
    }),
  );
  await expect(page.getByText("Stale TMDB result")).toHaveCount(0);
  await expect(page.getByText("Current Google result")).toBeVisible();
  await expect(provider).toHaveValue("google-books");
});

test("Discover keeps the newest provider status when refreshes resolve out of order", async ({
  page,
}) => {
  await installTrustedHost(page, "status-race");
  await page.goto("/discover");
  await expect
    .poll(() => page.evaluate(() => window.__STATUS_CALLS__?.()))
    .toBe(1);
  await page.getByRole("button", { name: "Library", exact: true }).click();
  await page.getByRole("button", { name: "Discover", exact: true }).click();
  await expect
    .poll(() => page.evaluate(() => window.__STATUS_CALLS__?.()))
    .toBe(2);

  await page.evaluate(() => window.__RESOLVE_STATUS__?.(1, true, false));
  await expect(page.getByLabel("Metadata provider")).toHaveValue(
    "google-books",
  );
  await expect(
    page.getByRole("searchbox", { name: "Search Google Books" }),
  ).toBeVisible();
  await page.evaluate(() => window.__RESOLVE_STATUS__?.(0, false, true));
  await expect(page.getByLabel("Metadata provider")).toHaveValue(
    "google-books",
  );
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("packaged status remains available before setup and uses host health", async ({
  page,
}) => {
  await installTrustedHost(page, "status-route");
  await page.goto("/status");
  await expect(
    page.getByRole("heading", { name: "Local service status" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
  await expect
    .poll(() => page.evaluate(() => window.__ENDPOINT_CALLS__?.()))
    .toBe(1);

  await page.getByRole("button", { name: "Open Media Workbench" }).click();
  await expect(
    page.getByRole("heading", { name: "Keep this record on this device" }),
  ).toBeVisible();
  await page.goBack();
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
});

test("packaged status finishes when setup inspection fails", async ({
  page,
}) => {
  await installTrustedHost(page, "status-setup-error");
  await page.goto("/status");
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
  await expect
    .poll(() => page.evaluate(() => window.__ENDPOINT_CALLS__?.()))
    .toBe(1);

  await page.getByRole("button", { name: "Open Media Workbench" }).click();
  await expect(
    page.getByRole("heading", { name: "Local storage is unavailable" }),
  ).toBeVisible();
});

test("packaged invalid health uses contract recovery", async ({ page }) => {
  await installTrustedHost(page, "status-invalid-response");
  await page.goto("/status");
  await expect(
    page.getByRole("heading", {
      name: "The local service returned an invalid response",
    }),
  ).toBeVisible();
  await expect(page.getByText("generated health contract")).toBeVisible();
});

test("provider credential removal requires confirmation", async ({ page }) => {
  await installTrustedHost(page, "credential-delete");
  await page.goto("/settings");
  await page
    .getByRole("button", { name: "Metadata credentials", exact: true })
    .click();
  const remove = page.getByRole("button", { name: "Remove" }).first();

  page.once("dialog", async (dialog) => {
    expect(dialog.message()).toContain("Remove the Google Books credential?");
    await dialog.dismiss();
  });
  await remove.click();
  expect(await page.evaluate(() => window.__DELETE_CALLS__?.())).toBe(0);

  page.once("dialog", (dialog) => dialog.accept());
  await remove.click();
  await expect(page.getByRole("status")).toContainText("Credential removed");
  expect(await page.evaluate(() => window.__DELETE_CALLS__?.())).toBe(1);
});

test("review resolution accepts one mutation while pending", async ({
  page,
}) => {
  await installTrustedHost(page, "review-resolution");
  await page.goto("/reviews");
  const accept = page.getByRole("button", { name: "Accept as this record" });
  await expect(accept).toBeVisible();
  await accept.evaluate((button) => {
    button.click();
    button.click();
  });
  await expect
    .poll(() => page.evaluate(() => window.__REVIEW_CALLS__?.()))
    .toBe(1);
  await expect(page.locator(".case-card")).toHaveAttribute("aria-busy", "true");
  await expect(
    page.getByRole("button", { name: "Resolving…" }).first(),
  ).toBeDisabled();
  await page.evaluate(() => window.__RESOLVE_REVIEW__?.());
  await expect(
    page.getByRole("heading", { name: "No open reviews" }),
  ).toBeVisible();
});

test("native record failure exposes a working retry", async ({ page }) => {
  await installTrustedHost(page, "record-retry");
  await page.goto("/");
  await expect(page.getByRole("alert")).toContainText(
    "temporary record read failure",
  );
  await page.getByRole("button", { name: "Retry records" }).click();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await page.getByRole("button", { name: "Library", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Recovered record" }),
  ).toBeVisible();
  expect(await page.evaluate(() => window.__RECORD_CALLS__?.())).toBe(2);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test("failed record retry restores focus to recovery", async ({ page }) => {
  await installTrustedHost(page, "record-retry-failure");
  await page.goto("/");
  const retry = page.getByRole("button", { name: "Retry records" });
  await retry.click();
  await expect
    .poll(() => page.evaluate(() => window.__RECORD_CALLS__?.()))
    .toBe(2);
  await expect(retry).toBeFocused();
  await expect(page.getByRole("alert")).toContainText(
    "temporary record read failure",
  );
});
