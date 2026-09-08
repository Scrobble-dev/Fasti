import { expect, test, type Page } from "@playwright/test";
import { mockAuthenticatedAccess } from "./test-helpers";

declare global {
  interface Window {
    __DISCOVER_WINDOW_BYTES__: {
      calls: number[];
      oversized: boolean;
      measurement?: {
        count: number;
        responseBytes: number;
        normalizedCharacters: number;
        normalizedBytes: number;
      };
    };
  }
}

async function installByteLimitHost(page: Page): Promise<void> {
  await mockAuthenticatedAccess(page);
  await page.addInitScript(() => {
    const fixture: Window["__DISCOVER_WINDOW_BYTES__"] = {
      calls: [],
      oversized: true,
    };
    window.__DISCOVER_WINDOW_BYTES__ = fixture;
    const browserWindow = window as typeof window & {
      __TAURI_INTERNALS__: {
        invoke: (command: string, args?: unknown) => Promise<unknown>;
      };
    };
    browserWindow.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        switch (command) {
          case "setup_status":
            return { phase: "ready", proof_cleanup_pending: false };
          case "load_network_configuration":
            return {
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
          case "provider_credential_status":
            return [
              {
                provider: "tmdb",
                capability_id: "metadata.search",
                label: "TMDB",
                purpose: "Search provider metadata",
                credential_requirement: "bearer_token",
                credential_state: "valid",
                state: "available",
                source: "environment",
                writable: false,
                testable: true,
                docs_url: "https://developer.themoviedb.org/docs",
              },
            ];
          case "list_records":
            return { records: [], truncated: false };
          case "list_reviews":
            return [];
          case "search_records":
            return { records: [], next: null };
          case "search_provider_page": {
            const input = (
              args as {
                input: { request: { page: number } };
              }
            ).input;
            const requestedPage = input.request.page;
            fixture.calls.push(requestedPage);
            const oversized = requestedPage === 3 && fixture.oversized;
            // Native-host boundary fixture, not a valid upstream TMDB payload:
            // the generated DTO bounds author count but not each author string.
            // The 100-row page is shape-valid and bypasses HTTP's body ceiling.
            const author = oversized ? "漢".repeat(40_000) : "Fixture author";
            const candidates = Array.from({ length: 100 }, (_, index) => {
              const id = requestedPage * 100 + index + 1;
              return {
                candidate_receipt_id: `scr_01991f588e0070008000${id.toString(16).padStart(12, "0")}`,
                grain: "film",
                candidate: {
                  provider: "tmdb",
                  provider_id: String(id),
                  grain: "film",
                  kind: "movie",
                  title: `Byte window ${requestedPage}-${index}`,
                  original_title: null,
                  release_year: 2024,
                  authors: [author],
                  image_url: null,
                  overview: null,
                },
              };
            });
            const response = {
              outcome: "page",
              provider_id: "tmdb",
              page: requestedPage,
              candidates,
              next_page: requestedPage < 3 ? requestedPage + 1 : null,
              cache_state: "fresh",
              lifetime: {
                created_at: "2026-09-08T12:00:00Z",
                fresh_until: "2026-09-08T12:02:00Z",
                stale_until: "2026-09-08T12:10:00Z",
                expires_at: "2026-09-09T12:00:00Z",
              },
              upstream_problem: null,
            };
            if (oversized) {
              // Measure the normalized retained shape, including its second
              // candidate representation, rather than just the wire response.
              const normalized = JSON.stringify(
                candidates.map((receipt) => ({
                  candidate: {
                    ...receipt.candidate,
                    authors: [...receipt.candidate.authors],
                    original_title: undefined,
                    overview: undefined,
                  },
                  receipt,
                  cacheState: response.cache_state,
                })),
              );
              const encoder = new TextEncoder();
              fixture.measurement = {
                count: candidates.length,
                responseBytes: encoder.encode(JSON.stringify(response))
                  .byteLength,
                normalizedCharacters: normalized.length,
                normalizedBytes: encoder.encode(normalized).byteLength,
              };
            }
            return response;
          }
          default:
            throw new Error(`Unexpected trusted-host command: ${command}`);
        }
      },
    };
  });
}

test("native multibyte page exceeding retained bytes rejects atomically without empty rollover", async ({
  page,
}) => {
  await installByteLimitHost(page);
  await page.goto("/discover");
  await page.locator("#provider-search").fill("Byte window");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  const results = page.getByRole("region", { name: "Search results" });
  const rows = results.getByRole("listitem");
  await expect(rows).toHaveCount(100);
  const firstRow = rows.filter({
    has: page.getByRole("heading", { name: "Byte window 1-0", exact: true }),
  });
  const firstLink = firstRow.getByRole("link", { name: "View details" });
  const firstHref = await firstLink.getAttribute("href");
  await page
    .getByRole("button", { name: "Retry or load more provider results" })
    .click();
  await expect(rows).toHaveCount(200);
  const continueButton = page.getByRole("button", {
    name: "Continue in a new result set",
  });
  await expect(continueButton).toBeEnabled();
  await continueButton.click();
  await expect(
    page.getByText(
      "Provider Search returned a page that exceeds the result limits.",
      {
        exact: true,
      },
    ),
  ).toBeVisible();
  await expect(continueButton).toBeEnabled();

  const measurement = await page.evaluate(
    () => window.__DISCOVER_WINDOW_BYTES__.measurement,
  );
  expect(measurement).toBeDefined();
  expect(measurement!.count).toBe(100);
  const byteLimit = 16 * 1024 * 1024;
  expect(measurement!.responseBytes).toBeLessThan(byteLimit);
  expect(measurement!.normalizedCharacters).toBeLessThan(byteLimit);
  expect(measurement!.normalizedBytes).toBeGreaterThan(byteLimit);
  await expect(rows).toHaveCount(200);
  await expect(firstLink).toHaveAttribute("href", firstHref!);
  await expect(
    firstRow.getByText("Fresh cache evidence", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Byte window 3-0", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Previous provider result set" }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Next provider result set" }),
  ).toHaveCount(0);
  expect(
    await page.evaluate(() => window.__DISCOVER_WINDOW_BYTES__.calls),
  ).toEqual([1, 2, 3]);

  // A corrected retry must request the same page and create history only after
  // admitting its replacement rows; the rejected page cannot consume a cursor.
  await page.evaluate(() => {
    window.__DISCOVER_WINDOW_BYTES__.oversized = false;
  });
  await continueButton.click();
  await expect(rows).toHaveCount(100);
  await expect(
    page.getByRole("heading", { name: "Byte window 3-0", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() => window.__DISCOVER_WINDOW_BYTES__.calls),
  ).toEqual([1, 2, 3, 3]);
  await page
    .getByRole("button", { name: "Previous provider result set" })
    .click();
  await expect(rows).toHaveCount(200);
  await expect(firstLink).toHaveAttribute("href", firstHref!);
  expect(
    await page.evaluate(() => window.__DISCOVER_WINDOW_BYTES__.calls),
  ).toEqual([1, 2, 3, 3]);
});
