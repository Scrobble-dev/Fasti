import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import type { SearchCandidateActionRequest } from "@fasti/sdk";
import {
  expectNoHorizontalOverflow,
  mockAuthenticatedAccess,
} from "./test-helpers";

interface WindowPageCall {
  provider_id: string;
  request: { query: string; page: number; offline: boolean; grains: string[] };
}

interface WindowActionCall {
  provider_id: string;
  grain: string;
  candidate_receipt_id: string;
  request: SearchCandidateActionRequest;
}

const confirmedRecordId = "rec_01991f588e0070008000000000000001";

declare global {
  interface Window {
    __DISCOVER_WINDOWS__: {
      calls: WindowPageCall[];
      actions: WindowActionCall[];
      failProviders: string[];
      holdPage: number | null;
      released: boolean;
      release?: () => void;
    };
  }
}

// Trusted-host presentation fixture: 100 admitted candidates exercise the
// shared page contract, not TMDB's public 20-result upstream page size.
async function installWindowHost(
  page: Page,
  thirdProvider = false,
): Promise<void> {
  await mockAuthenticatedAccess(page);
  await page.addInitScript((thirdProvider) => {
    const confirmedRecordId = "rec_01991f588e0070008000000000000001";
    const fixture: Window["__DISCOVER_WINDOWS__"] = {
      calls: [],
      actions: [],
      failProviders: [],
      holdPage: null,
      released: false,
    };
    window.__DISCOVER_WINDOWS__ = fixture;
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
                public_url: {
                  value: null,
                  source: "default",
                  managed: false,
                },
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
              "tmdb",
              "google-books",
              ...(thirdProvider ? ["tvdb"] : []),
            ].map((provider) => ({
              provider,
              capability_id: "metadata.search",
              label:
                provider === "tmdb"
                  ? "TMDB"
                  : provider === "tvdb"
                    ? "TVDB"
                    : "Google Books",
              purpose: "Search provider metadata",
              credential_requirement: "bearer_token",
              credential_state: "valid",
              state: "available",
              source: "environment",
              writable: false,
              testable: true,
              docs_url:
                provider === "tmdb"
                  ? "https://developer.themoviedb.org/docs"
                  : "https://developers.google.com/books/docs/v1/using",
            }));
          case "list_records":
            return { records: [], truncated: false };
          case "list_reviews":
            return [];
          case "search_records":
            return { records: [], next: null };
          case "save_search_candidate": {
            const input = (args as { input: WindowActionCall }).input;
            fixture.actions.push(structuredClone(input));
            // Presentation-only confirmation. Store tests own exact-ID reuse;
            // this fixture proves a discarded row may dispatch a new operation.
            return {
              outcome: "saved",
              receipt: {
                operation_id: input.request.operation_id,
                candidate_receipt_id: input.candidate_receipt_id,
                provider_id: input.provider_id,
                grain: input.grain,
                action: input.request.action,
                evidence_mode: input.request.evidence_mode,
                record_id: confirmedRecordId,
                disposition:
                  fixture.actions.length === 1 ? "created" : "reused",
                fetched_at: "2026-09-08T12:00:00Z",
                expires_at: "2026-09-09T12:00:00Z",
                initial_status: "fresh",
                committed_at: "2026-09-08T12:00:01Z",
              },
            };
          }
          case "search_provider_page": {
            const input = (args as { input: WindowPageCall }).input;
            fixture.calls.push(structuredClone(input));
            const { query, page: requestedPage } = input.request;
            if (fixture.holdPage === requestedPage) {
              await new Promise<void>((resolve) => {
                fixture.release = resolve;
              });
              fixture.released = true;
            }
            if (fixture.failProviders.includes(input.provider_id)) {
              throw new Error(`${input.provider_id} fixture page unavailable`);
            }
            const effectivePage =
              query === "Duplicates" && requestedPage === 2 ? 1 : requestedPage;
            const count =
              query === "Oversized" ? 101 : query.startsWith("Fresh") ? 1 : 100;
            const candidates = Array.from({ length: count }, (_, index) => {
              const number = effectivePage * 100 + index + 1;
              const book = input.provider_id === "google-books";
              const grain = book ? "edition" : "film";
              return {
                candidate_receipt_id: `scr_01991f588e0070008000${(
                  number +
                  (book ? 1000000 : input.provider_id === "tvdb" ? 2000000 : 0)
                )
                  .toString(16)
                  .padStart(12, "0")}`,
                grain,
                candidate: {
                  provider: input.provider_id,
                  provider_id: book ? `book-${number}` : String(number),
                  grain,
                  kind: book ? "book" : "movie",
                  title: `${query} ${input.provider_id} ${effectivePage}-${index}`,
                  original_title: null,
                  release_year: 2024,
                  authors: [],
                  image_url: null,
                  overview: null,
                },
              };
            });
            const lastPage = query === "Partial" ? 2 : 6;
            return {
              outcome: "page",
              provider_id: input.provider_id,
              page: requestedPage,
              candidates,
              next_page:
                count === 1 || requestedPage === lastPage
                  ? null
                  : requestedPage + 1,
              cache_state: requestedPage === 1 ? "fresh" : "observed",
              lifetime: {
                created_at: "2026-09-08T12:00:00Z",
                fresh_until: "2026-09-08T12:02:00Z",
                stale_until: "2026-09-08T12:10:00Z",
                expires_at: "2026-09-09T12:00:00Z",
              },
              upstream_problem: null,
            };
          }
          default:
            throw new Error(`Unexpected trusted-host command: ${command}`);
        }
      },
    };
  }, thirdProvider);
}

const moreLabel = "Retry or load more provider results";
const continueLabel = "Continue in a new result set";
const previousLabel = "Previous provider result set";
const nextLabel = "Next provider result set";

async function submitSearch(page: Page, query: string): Promise<void> {
  await page.locator("#provider-search").fill(query);
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Search", exact: true }),
  ).toBeEnabled();
}

async function pageCalls(page: Page): Promise<WindowPageCall[]> {
  return page.evaluate(() => window.__DISCOVER_WINDOWS__.calls);
}

test("replacement restores heading focus after its connected Continue button loses focus while disabled", async ({
  page,
}) => {
  await installWindowHost(page);
  await page.goto("/discover");
  await submitSearch(page, "Windows");
  const results = page.getByRole("region", { name: "Search results" });
  const rows = results.getByRole("listitem");
  await expect(rows).toHaveCount(100);
  await page.getByRole("button", { name: moreLabel }).click();
  await expect(rows).toHaveCount(200);
  await page.evaluate(() => {
    window.__DISCOVER_WINDOWS__.holdPage = 3;
  });
  const continuation = page.getByRole("button", { name: continueLabel });
  await continuation.focus();
  await expect(continuation).toBeFocused();
  await page.keyboard.press("Enter");
  await expect
    .poll(() =>
      page.evaluate(() => Boolean(window.__DISCOVER_WINDOWS__.release)),
    )
    .toBe(true);
  await expect(continuation).toBeDisabled();
  expect(
    await continuation.evaluate((element: HTMLButtonElement) => ({
      connected: element.isConnected,
      disabled: element.disabled,
    })),
  ).toEqual({ connected: true, disabled: true });
  // Model browsers that reset focus when a still-connected control is disabled.
  await continuation.evaluate((element: HTMLButtonElement) => element.blur());
  expect(
    await page.evaluate(() => document.activeElement === document.body),
  ).toBe(true);
  await page.evaluate(() => window.__DISCOVER_WINDOWS__.release?.());
  await expect(rows).toHaveCount(100);
  await expect(
    results.getByRole("heading", { name: "Search results", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(
    rows.first().getByRole("link", { name: "View details" }),
  ).toBeFocused();
});

test("three provider result sets stay bounded and preserve one-step history without refetch", async ({
  page,
}, testInfo) => {
  await installWindowHost(page);
  await page.goto("/discover");
  await submitSearch(page, "Windows");
  const results = page.getByRole("region", { name: "Search results" });
  const rows = results.getByRole("listitem");
  const first = rows.filter({
    has: page.getByRole("heading", { name: "Windows tmdb 1-0", exact: true }),
  });
  const firstHref = await first
    .getByRole("link", { name: "View details" })
    .getAttribute("href");
  await first
    .getByRole("button", { name: "Create Record", exact: true })
    .click();
  await expect(
    first.getByRole("button", { name: "Record ready", exact: true }),
  ).toHaveAttribute("aria-disabled", "true");
  await expect(
    first.getByText(confirmedRecordId, { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: moreLabel }).click();
  await expect(rows).toHaveCount(200);
  await expect(page.getByRole("button", { name: continueLabel })).toBeVisible();
  await expect(rows).toHaveCount(200);
  await expect(
    first.getByText("Fresh cache evidence", { exact: true }),
  ).toBeVisible();
  await expect(
    first.getByRole("button", { name: "Record ready", exact: true }),
  ).toHaveAttribute("aria-disabled", "true");
  await expect(
    first.getByText(confirmedRecordId, { exact: true }),
  ).toBeVisible();
  await expect(
    first.getByRole("link", { name: "View details" }),
  ).toHaveAttribute("href", firstHref!);

  await page.getByRole("button", { name: continueLabel }).focus();
  await page.keyboard.press("Enter");
  await expect(rows).toHaveCount(100);
  await expect(
    results.getByRole("heading", { name: "Search results", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(
    rows.first().getByRole("link", { name: "View details" }),
  ).toBeFocused();
  await expect(
    page.getByRole("heading", { name: "Windows tmdb 3-99", exact: true }),
  ).toBeVisible();
  expect((await pageCalls(page)).map((call) => call.request.page)).toEqual([
    1, 2, 3,
  ]);
  const beforeHistory = await pageCalls(page);
  await page.getByRole("button", { name: previousLabel }).click();
  await expect(rows).toHaveCount(200);
  await expect(
    first.getByRole("link", { name: "View details" }),
  ).toHaveAttribute("href", firstHref!);
  await expect(
    first.getByText("Fresh cache evidence", { exact: true }),
  ).toBeVisible();
  await expect(
    first.getByRole("button", { name: "Record ready", exact: true }),
  ).toHaveAttribute("aria-disabled", "true");
  await expect(
    first.getByText(confirmedRecordId, { exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() => window.__DISCOVER_WINDOWS__.actions),
  ).toHaveLength(1);
  await page.getByRole("button", { name: nextLabel }).click();
  await expect(rows).toHaveCount(100);
  expect(await pageCalls(page)).toEqual(beforeHistory);
  await page.getByRole("button", { name: moreLabel }).click();
  await expect(rows).toHaveCount(200);
  await expect(page.getByRole("button", { name: continueLabel })).toBeVisible();
  await expect(rows).toHaveCount(200);
  await page.getByRole("button", { name: continueLabel }).click();
  await expect(rows).toHaveCount(100);
  await page.setViewportSize({ width: 320, height: 1000 });
  await expectNoHorizontalOverflow(page);
  expect(
    (
      await new AxeBuilder({ page })
        .include('.results[aria-labelledby="search-results-title"]')
        .analyze()
    ).violations,
  ).toEqual([]);
  for (const width of [320, 768, 1440]) {
    await page.setViewportSize({ width, height: 1000 });
    await page
      .getByRole("button", { name: previousLabel })
      .scrollIntoViewIfNeeded();
    await expectNoHorizontalOverflow(page);
    await page.screenshot({
      path: testInfo.outputPath(`provider-window-navigation-${width}.png`),
    });
  }
  await page.getByRole("button", { name: moreLabel }).click();
  await expect(rows).toHaveCount(200);
  await expect(
    page.getByRole("heading", { name: "Windows tmdb 6-99", exact: true }),
  ).toBeVisible();
  const finalCalls = await pageCalls(page);
  await page.getByRole("button", { name: previousLabel }).click();
  await expect(rows).toHaveCount(200);
  await expect(
    page.getByRole("heading", { name: "Windows tmdb 3-0", exact: true }),
  ).toBeVisible();
  await expect(first).toHaveCount(0);
  await page.getByRole("button", { name: nextLabel }).click();
  await expect(
    page.getByRole("heading", { name: "Windows tmdb 6-99", exact: true }),
  ).toBeVisible();
  expect(await pageCalls(page)).toEqual(finalCalls);

  // The first result set has now left both retained windows. Revisiting its
  // identical receipt is a new explicit action, not lifetime-wide UI replay.
  await submitSearch(page, "Windows");
  await expect(first).toBeVisible();
  await expect(
    first.getByRole("button", { name: "Record ready", exact: true }),
  ).toHaveCount(0);
  await expect(first.getByText(confirmedRecordId, { exact: true })).toHaveCount(
    0,
  );
  await first
    .getByRole("button", { name: "Create Record", exact: true })
    .click();
  await expect(
    first.getByRole("button", { name: "Record ready", exact: true }),
  ).toHaveAttribute("aria-disabled", "true");
  await expect(
    first.getByText(confirmedRecordId, { exact: true }),
  ).toBeVisible();
  const actions = await page.evaluate(
    () => window.__DISCOVER_WINDOWS__.actions,
  );
  expect(actions).toHaveLength(2);
  expect(actions[1].candidate_receipt_id).toBe(actions[0].candidate_receipt_id);
  expect(actions.map((action) => action.request.action)).toEqual([
    { kind: "create" },
    { kind: "create" },
  ]);
  expect(actions[1].request.operation_id).not.toBe(
    actions[0].request.operation_id,
  );
});

test("failed replacement retains the full prior result set and partial providers remain retryable", async ({
  page,
}) => {
  await installWindowHost(page, true);
  await page.goto("/discover");
  await page.getByLabel("Metadata provider").selectOption("all");
  await submitSearch(page, "Partial");
  const rows = page
    .getByRole("region", { name: "Search results" })
    .getByRole("listitem");
  await expect(rows).toHaveCount(200);
  await expect(page.getByRole("button", { name: continueLabel })).toBeVisible();
  await page.evaluate(() => {
    window.__DISCOVER_WINDOWS__.failProviders = [
      "tmdb",
      "google-books",
      "tvdb",
    ];
  });
  await page.getByRole("button", { name: continueLabel }).click();
  await expect(page.getByRole("alert")).toContainText(
    "fixture page unavailable",
  );
  await expect(rows).toHaveCount(200);
  await expect(
    page.getByRole("heading", { name: "Partial tmdb 1-0", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", {
      name: "Partial google-books 1-0",
      exact: true,
    }),
  ).toBeVisible();
  await page.evaluate(() => {
    window.__DISCOVER_WINDOWS__.failProviders = ["tmdb", "google-books"];
  });
  await page.getByRole("button", { name: continueLabel }).click();
  await expect(rows).toHaveCount(100);
  await expect(
    page.getByRole("heading", { name: "Partial tvdb 1-99", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toContainText(
    "google-books fixture page unavailable",
  );
  // Capacity-blocked TVDB page 1 must precede peers' next pages on rollover.
  expect(
    (await pageCalls(page))
      .slice(3, 6)
      .map((call) => [call.provider_id, call.request.page]),
  ).toEqual([
    ["tvdb", 1],
    ["tmdb", 2],
    ["google-books", 2],
  ]);
  const beforeHistory = await pageCalls(page);
  await page.getByRole("button", { name: previousLabel }).click();
  await expect(rows).toHaveCount(200);
  await expect(
    page.getByRole("heading", { name: "Partial tmdb 1-0", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: nextLabel }).click();
  await expect(rows).toHaveCount(100);
  expect(await pageCalls(page)).toEqual(beforeHistory);
  await page.evaluate(() => {
    window.__DISCOVER_WINDOWS__.failProviders = [];
  });
  await page.getByRole("button", { name: moreLabel }).click();
  await expect(rows).toHaveCount(200);
  await expect(
    page.getByRole("heading", { name: "Partial tmdb 2-99", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: continueLabel })).toBeVisible();
  await page.getByRole("button", { name: continueLabel }).click();
  await expect(rows).toHaveCount(200);
  await expect(
    page.getByRole("heading", {
      name: "Partial google-books 2-99",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Partial tvdb 2-99", exact: true }),
  ).toBeVisible();
  expect(
    (await pageCalls(page)).map((call) => [
      call.provider_id,
      call.request.page,
    ]),
  ).toEqual([
    ["tmdb", 1],
    ["google-books", 1],
    ["tvdb", 1],
    ["tvdb", 1],
    ["tmdb", 2],
    ["google-books", 2],
    ["tvdb", 1],
    ["tmdb", 2],
    ["google-books", 2],
    ["tmdb", 2],
    ["google-books", 2],
    ["tvdb", 2],
    ["google-books", 2],
    ["tvdb", 2],
  ]);
});

test("duplicate-only pages advance and a held old page cannot restore another query or provider", async ({
  page,
}) => {
  await installWindowHost(page);
  await page.goto("/discover");
  await submitSearch(page, "Duplicates");
  const rows = page
    .getByRole("region", { name: "Search results" })
    .getByRole("listitem");
  await expect(rows).toHaveCount(100);
  const firstHref = await rows
    .first()
    .getByRole("link", { name: "View details" })
    .getAttribute("href");
  await page.getByRole("button", { name: moreLabel }).click();
  await expect.poll(async () => (await pageCalls(page)).length).toBe(2);
  await expect(rows).toHaveCount(100);
  await expect(
    rows.first().getByRole("link", { name: "View details" }),
  ).toHaveAttribute("href", firstHref!);
  await expect(
    rows.first().getByText("Fresh cache evidence", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: moreLabel }).click();
  await expect(rows).toHaveCount(200);
  expect((await pageCalls(page)).map((call) => call.request.page)).toEqual([
    1, 2, 3,
  ]);
  await page.evaluate(() => {
    window.__DISCOVER_WINDOWS__.holdPage = 4;
  });
  await page.getByRole("button", { name: continueLabel }).click();
  await expect
    .poll(() =>
      page.evaluate(() => Boolean(window.__DISCOVER_WINDOWS__.release)),
    )
    .toBe(true);
  const provider = page.getByLabel("Metadata provider");
  await provider.focus();
  await page.evaluate(() => {
    window.__DISCOVER_WINDOWS__.release?.();
  });
  await expect(rows).toHaveCount(100);
  await expect(
    page.getByRole("heading", { name: "Duplicates tmdb 4-0", exact: true }),
  ).toBeVisible();
  await page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
      }),
  );
  await expect(provider).toBeFocused();
  await page.getByRole("button", { name: moreLabel }).click();
  await expect(rows).toHaveCount(200);
  await page.evaluate(() => {
    const fixture = window.__DISCOVER_WINDOWS__;
    fixture.holdPage = 6;
    fixture.release = undefined;
    fixture.released = false;
  });
  await page.getByRole("button", { name: continueLabel }).click();
  await expect
    .poll(() =>
      page.evaluate(() => Boolean(window.__DISCOVER_WINDOWS__.release)),
    )
    .toBe(true);
  await page.getByLabel("Metadata provider").selectOption("google-books");
  await submitSearch(page, "Fresh query");
  await expect(rows).toHaveCount(1);
  await expect(
    page.getByRole("heading", {
      name: "Fresh query google-books 1-0",
      exact: true,
    }),
  ).toBeVisible();
  await page.evaluate(() => {
    window.__DISCOVER_WINDOWS__.release?.();
  });
  await expect
    .poll(() => page.evaluate(() => window.__DISCOVER_WINDOWS__.released))
    .toBe(true);
  await expect(rows).toHaveCount(1);
  await expect(page.getByRole("heading", { name: /^Duplicates / })).toHaveCount(
    0,
  );
  await expect(page.getByRole("button", { name: continueLabel })).toHaveCount(
    0,
  );
  await expect(page.getByRole("button", { name: previousLabel })).toHaveCount(
    0,
  );
  await submitSearch(page, "Fresh replacement");
  await expect(rows).toHaveCount(1);
  await expect(
    page.getByRole("heading", {
      name: "Fresh replacement google-books 1-0",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", {
      name: "Fresh query google-books 1-0",
      exact: true,
    }),
  ).toHaveCount(0);
  await submitSearch(page, "Oversized");
  await expect(rows).toHaveCount(0);
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(page.getByRole("button", { name: continueLabel })).toHaveCount(
    0,
  );
});
