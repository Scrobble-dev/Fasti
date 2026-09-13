import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import { mockAuthenticatedAccess } from "./test-helpers";

// Regression: ISSUE-001 and ISSUE-002 — M4 Search lost provider context and stable selection.
// Found by /qa on 2026-09-05.
// Report: .gstack/qa-reports/qa-report-fasti-local-2026-09-05.md
async function mockNativeSearch(page: Page) {
  await mockAuthenticatedAccess(page);
  await page.addInitScript(() => {
    const receiptId = "scr_01991f588e0070008000000000000001";
    const duplicateReceiptId = "scr_01991f588e0070008000000000000002";
    const differentYearReceiptId = "scr_01991f588e0070008000000000000003";
    const candidate = {
      provider: "tmdb",
      provider_id: "693134",
      grain: "film",
      kind: "movie",
      title: "Dune: Part Two",
      original_title: "Dune: Part Two",
      release_year: 2024,
      authors: [],
      image_url: null,
      overview: "Paul joins Chani and the Fremen.",
    };
    const differentCandidate = {
      ...candidate,
      provider: "tvdb",
      provider_id: "movies/341030",
      release_year: 1984,
    };
    const field = (value: string | null, source: string | null = null) => ({
      tier: value === null ? "empty" : "preferred_provider_claim",
      value,
      source,
      is_stale: false,
    });
    const localRecord = {
      record_id: "rec_01991f588e0070008000000000000001",
      grain: "film",
      status: "active",
      title: field("Local Dune", "manual"),
      poster: field(null),
      original_title: field(null),
      overview: field(null),
      release_year: field("1984", "manual"),
      identifiers: [],
      latest_activity: null,
      poster_asset_path: null,
    };
    const createdRecord = {
      ...localRecord,
      record_id: "rec_01991f588e0070008000000000000002",
      title: field("Dune: Part Two", "tmdb"),
      release_year: field("2024", "tmdb"),
    };
    const providers = [
      {
        provider: "tmdb",
        capability_id: "metadata.search",
        label: "TMDB",
        purpose: "Search film and television metadata",
        credential_requirement: "bearer_token",
        credential_state: "valid",
        state: "available",
        source: "environment",
        writable: false,
        testable: true,
        docs_url: "https://developer.themoviedb.org/docs",
      },
      {
        provider: "google-books",
        capability_id: "metadata.search",
        label: "Google Books",
        purpose: "Search book metadata",
        credential_requirement: "optional_api_key",
        credential_state: "optional",
        state: "available",
        source: "none",
        writable: true,
        testable: true,
        docs_url: "https://developers.google.com/books/docs/v1/using",
      },
      {
        provider: "tvdb",
        capability_id: "metadata.search",
        label: "TVDB",
        purpose: "Search film and television metadata",
        credential_requirement: "api_key",
        credential_state: "valid",
        state: "available",
        source: "environment",
        writable: false,
        testable: true,
        docs_url: "https://thetvdb.github.io/v4-api/",
      },
    ];
    const browserWindow = window as typeof window & {
      __SEARCH_PAGE_INPUTS__?: unknown[];
      __SEARCH_ACTION_INPUTS__?: unknown[];
      __FAIL_SEARCH_ACTIONS__?: number;
      __SEARCH_OFFLINE__?: boolean;
      __TAURI_INTERNALS__: {
        invoke: (command: string, arguments_?: unknown) => Promise<unknown>;
      };
    };
    browserWindow.__SEARCH_PAGE_INPUTS__ = [];
    browserWindow.__SEARCH_ACTION_INPUTS__ = [];
    browserWindow.__FAIL_SEARCH_ACTIONS__ = 0;
    browserWindow.__SEARCH_OFFLINE__ = false;
    Object.defineProperty(navigator, "onLine", {
      configurable: true,
      get: () => !browserWindow.__SEARCH_OFFLINE__,
    });
    browserWindow.__TAURI_INTERNALS__ = {
      invoke: async (command, arguments_) => {
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
            return providers;
          case "list_records": {
            const recordId = (arguments_ as { query?: { record_id?: string } })
              ?.query?.record_id;
            return {
              records:
                recordId === createdRecord.record_id ? [createdRecord] : [],
              truncated: false,
            };
          }
          case "list_reviews":
            return [];
          case "search_records":
            if (
              (
                arguments_ as {
                  input?: { query?: string; after?: unknown };
                }
              )?.input?.query === "Continuation"
            ) {
              return (
                arguments_ as {
                  input?: { after?: unknown };
                }
              )?.input?.after
                ? { records: [localRecord], next: null }
                : {
                    records: [],
                    next: {
                      last_record_id: "rec_01991f588e0070008000000000000009",
                      context_digest:
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    },
                  };
            }
            return {
              records: [localRecord],
              next: null,
            };
          case "search_provider_page": {
            const input = (
              arguments_ as {
                input?: { provider_id?: string; request?: unknown };
              }
            )?.input;
            browserWindow.__SEARCH_PAGE_INPUTS__?.push(input);
            const request = input?.request as
              { query?: string; page?: number } | undefined;
            if (
              [
                "Repeated coordinate",
                "Mixed evidence",
                "Non-forward continuation",
                "Receipt collision",
              ].includes(request?.query ?? "")
            ) {
              const secondPage = request?.page === 2;
              const repeated = request?.query === "Repeated coordinate";
              const collision = request?.query === "Receipt collision";
              return {
                outcome: "page",
                provider_id: "tmdb",
                page: request?.page,
                candidates: [
                  {
                    candidate_receipt_id:
                      secondPage && !collision ? duplicateReceiptId : receiptId,
                    grain: "film",
                    candidate: secondPage
                      ? {
                          ...candidate,
                          provider_id: repeated
                            ? candidate.provider_id
                            : "693135",
                          title: repeated
                            ? "Repeated coordinate title"
                            : "Second-page candidate",
                        }
                      : candidate,
                  },
                ],
                next_page:
                  !secondPage || request?.query === "Non-forward continuation"
                    ? 2
                    : repeated
                      ? 3
                      : null,
                cache_state: secondPage ? "observed" : "fresh",
                lifetime: {
                  created_at: "2026-09-05T12:00:00Z",
                  fresh_until: "2026-09-05T12:02:00Z",
                  stale_until: "2026-09-05T12:10:00Z",
                  expires_at: "2026-09-06T12:00:00Z",
                },
                upstream_problem: null,
              };
            }
            if (request?.query === "Provider batch collision") {
              const book = {
                ...candidate,
                provider: "google-books",
                grain: "book",
                kind: "volume",
              };
              return {
                outcome: "page",
                provider_id: input?.provider_id,
                page: request.page,
                candidates:
                  input?.provider_id === "google-books"
                    ? [
                        {
                          candidate_receipt_id: differentYearReceiptId,
                          grain: "book",
                          candidate: {
                            ...book,
                            provider_id: "valid-book",
                            title: "Rejected provider first row",
                          },
                        },
                        {
                          candidate_receipt_id: receiptId,
                          grain: "book",
                          candidate: {
                            ...book,
                            provider_id: "collision-book",
                            title: "Colliding provider row",
                          },
                        },
                      ]
                    : [
                        {
                          candidate_receipt_id:
                            input?.provider_id === "tmdb"
                              ? receiptId
                              : duplicateReceiptId,
                          grain: "film",
                          candidate: {
                            ...candidate,
                            provider: input?.provider_id,
                            provider_id:
                              input?.provider_id === "tmdb"
                                ? candidate.provider_id
                                : "movies/341029",
                            title:
                              input?.provider_id === "tmdb"
                                ? "Batch TMDB result"
                                : "Batch TVDB result",
                          },
                        },
                      ],
                next_page: null,
                cache_state: "fresh",
                lifetime: {
                  created_at: "2026-09-05T12:00:00Z",
                  fresh_until: "2026-09-05T12:02:00Z",
                  stale_until: "2026-09-05T12:10:00Z",
                  expires_at: "2026-09-06T12:00:00Z",
                },
                upstream_problem: null,
              };
            }
            if (request?.query === "Continuation") {
              return request.page === 2
                ? {
                    outcome: "page",
                    provider_id: input?.provider_id,
                    page: 2,
                    candidates: [
                      {
                        candidate_receipt_id: receiptId,
                        grain: "film",
                        candidate,
                      },
                    ],
                    next_page: null,
                    cache_state: "observed",
                    lifetime: {
                      created_at: "2026-09-05T12:00:00Z",
                      fresh_until: "2026-09-05T12:02:00Z",
                      stale_until: "2026-09-05T12:10:00Z",
                      expires_at: "2026-09-06T12:00:00Z",
                    },
                    upstream_problem: null,
                  }
                : {
                    outcome: "page",
                    provider_id: input?.provider_id,
                    page: 1,
                    candidates: [],
                    next_page: 2,
                    cache_state: "observed",
                    lifetime: {
                      created_at: "2026-09-05T12:00:00Z",
                      fresh_until: "2026-09-05T12:02:00Z",
                      stale_until: "2026-09-05T12:10:00Z",
                      expires_at: "2026-09-06T12:00:00Z",
                    },
                    upstream_problem: null,
                  };
            }
            if (input?.provider_id === "google-books") {
              throw new Error(
                "Google Books Search is temporarily unavailable.",
              );
            }
            const isDuplicate = input?.provider_id === "tvdb";
            return {
              outcome: "page",
              provider_id: isDuplicate ? "tvdb" : "tmdb",
              page: 1,
              candidates: isDuplicate
                ? [
                    {
                      candidate_receipt_id: duplicateReceiptId,
                      grain: "film",
                      candidate: {
                        ...candidate,
                        provider: "tvdb",
                        provider_id: "movies/341029",
                        kind: "series",
                      },
                    },
                    {
                      candidate_receipt_id: differentYearReceiptId,
                      grain: "film",
                      candidate: differentCandidate,
                    },
                  ]
                : [
                    {
                      candidate_receipt_id: receiptId,
                      grain: "film",
                      candidate,
                    },
                  ],
              next_page: null,
              cache_state: "observed",
              lifetime: {
                created_at: "2026-09-05T12:00:00Z",
                fresh_until: "2026-09-05T12:02:00Z",
                stale_until: "2026-09-05T12:10:00Z",
                expires_at: "2026-09-06T12:00:00Z",
              },
              upstream_problem: null,
            };
          }
          case "read_search_candidate": {
            const selectedReceiptId = (
              arguments_ as {
                input?: { candidate_receipt_id?: string };
              }
            )?.input?.candidate_receipt_id;
            const selectedCandidate =
              selectedReceiptId === differentYearReceiptId
                ? differentCandidate
                : candidate;
            return {
              outcome: "refetched",
              snapshot: {
                receipt: {
                  candidate_receipt_id: selectedReceiptId ?? receiptId,
                  grain: "film",
                  candidate: selectedCandidate,
                },
                lifetime: {
                  created_at: "2026-09-05T12:00:00Z",
                  fresh_until: "2026-09-05T12:02:00Z",
                  stale_until: "2026-09-05T12:10:00Z",
                  expires_at: "2026-09-06T12:00:00Z",
                },
                locale: "en-US",
              },
              details: {
                ...selectedCandidate,
                overview: "Expanded provider details.",
              },
              locale: "en-US",
            };
          }
          case "save_search_candidate": {
            browserWindow.__SEARCH_ACTION_INPUTS__?.push(arguments_);
            if ((browserWindow.__FAIL_SEARCH_ACTIONS__ ?? 0) > 0) {
              browserWindow.__FAIL_SEARCH_ACTIONS__!--;
              throw new Error("The action response was interrupted.");
            }
            const request = (
              arguments_ as {
                input?: {
                  request?: {
                    operation_id?: string;
                    action?: { kind: "create" };
                    evidence_mode?: "cached" | "refetch";
                  };
                };
              }
            )?.input?.request;
            return {
              outcome: "saved",
              receipt: {
                operation_id: request?.operation_id,
                candidate_receipt_id: receiptId,
                provider_id: "tmdb",
                grain: "film",
                action: request?.action,
                evidence_mode: request?.evidence_mode,
                record_id: "rec_01991f588e0070008000000000000002",
                disposition: "created",
                fetched_at: "2026-09-05T12:00:00Z",
                expires_at: "2026-09-06T12:00:00Z",
                initial_status: "fresh",
                committed_at: "2026-09-05T12:00:01Z",
              },
            };
          }
          default:
            throw new Error(`Unexpected trusted-host command: ${command}`);
        }
      },
    };
  });
}

test("local and receipt-backed provider Search survive a partial source failure", async ({
  page,
}) => {
  await mockNativeSearch(page);
  await page.goto("/discover");
  const provider = page.getByLabel("Metadata provider");
  await expect(provider).toHaveValue("tmdb");
  await expect(
    page.getByRole("searchbox", { name: "Search TMDB" }),
  ).toBeVisible();
  await provider.selectOption("all");
  await page
    .getByRole("searchbox", { name: "Search local Records and providers" })
    .fill("Dune");
  await page.getByRole("button", { name: "Search", exact: true }).click();

  await expect(page.getByRole("heading", { name: "Local Dune" })).toBeVisible();
  const providerResult = page
    .getByRole("listitem")
    .filter({
      has: page.getByRole("heading", { name: "Dune: Part Two" }),
    })
    .filter({ has: page.getByText("tmdb", { exact: true }) });
  await expect(providerResult).toBeVisible();
  await expect(
    page.getByText("Possible match across 2 results.", { exact: false }),
  ).toBeVisible();
  await expect(page.getByText(/Possible match across/)).toHaveCount(1);
  await expect(
    page.getByText("Fasti has not merged these candidates."),
  ).toBeVisible();
  await expect(
    page.getByRole("region", { name: "Search results" }).getByRole("listitem"),
  ).toHaveCount(3);
  await expect(page.getByRole("alert")).toContainText(
    "Google Books Search is temporarily unavailable.",
  );
  await expect
    .poll(() => page.evaluate(() => window.__SEARCH_PAGE_INPUTS__?.length))
    .toBe(3);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);

  await providerResult.getByRole("link", { name: "View details" }).click();
  await expect(page).toHaveURL(
    `/explore/tmdb/film/scr_01991f588e0070008000000000000001/dune-part-two`,
  );
  await expect(
    page.getByRole("heading", { name: "Dune: Part Two", level: 1 }),
  ).toBeVisible();
  await expect(page.getByText("Expanded provider details.")).toBeVisible();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);

  await page.getByRole("button", { name: "Back to Search" }).click();
  await expect(page).toHaveURL("/discover");
  await expect(page.getByRole("heading", { name: "Local Dune" })).toBeVisible();
  await providerResult.getByRole("link", { name: "View details" }).click();

  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Dune: Part Two", level: 1 }),
  ).toBeVisible();
  await page.evaluate(() => {
    window.__FAIL_SEARCH_ACTIONS__ = 2;
  });
  await page.getByRole("button", { name: "Create Record" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "The action response was interrupted.",
  );
  await page.evaluate(() => {
    window.history.pushState(
      {},
      "",
      "/explore/tvdb/film/scr_01991f588e0070008000000000000003/dune-part-two",
    );
    window.dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(
    page.getByRole("heading", { name: "Dune: Part Two", level: 1 }),
  ).toBeVisible();
  await expect(
    page
      .getByRole("alert")
      .filter({ hasText: "The action response was interrupted." }),
  ).toHaveCount(0);
  const recovery = page.locator("#unconfirmed-record-actions");
  await expect(recovery.locator("summary")).toHaveText(
    "Unconfirmed Record actions (1)",
  );
  await recovery.locator("summary").click();
  await expect(recovery).toContainText("tmdb");
  await expect(recovery).toContainText("scr_01991f588e0070008000000000000001");
  await expect(recovery.getByRole("status")).toHaveText(
    "The action response was interrupted.",
  );
  await expect(
    recovery.getByRole("button", {
      name: "Retry original action",
      exact: true,
    }),
  ).toBeEnabled();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await recovery.locator("summary").click();
  await page.evaluate(() => {
    window.history.pushState(
      {},
      "",
      "/explore/tmdb/film/scr_01991f588e0070008000000000000001/dune-part-two",
    );
    window.dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(
    page.getByRole("heading", { name: "Dune: Part Two", level: 1 }),
  ).toBeVisible();
  await page.evaluate(() => {
    window.__SEARCH_OFFLINE__ = true;
  });
  await page.getByRole("button", { name: "Create Record" }).click();
  await expect(page.getByRole("alert")).toContainText(
    "The action response was interrupted.",
  );
  await page.getByRole("button", { name: "Create Record" }).click();
  const actionInputs = await page.evaluate(
    () =>
      window.__SEARCH_ACTION_INPUTS__ as Array<{
        input?: {
          request?: { evidence_mode?: string; operation_id?: string };
        };
      }>,
  );
  expect(
    actionInputs.map((input) => input.input?.request?.evidence_mode),
  ).toEqual(["refetch", "cached", "cached"]);
  expect(actionInputs[0].input?.request?.operation_id).not.toBe(
    actionInputs[1].input?.request?.operation_id,
  );
  expect(actionInputs[1].input?.request?.operation_id).toBe(
    actionInputs[2].input?.request?.operation_id,
  );
  await expect(page).toHaveURL(
    `/records/film/rec_01991f588e0070008000000000000002/dune-part-two`,
  );
  await page.evaluate(() => {
    window.__SEARCH_OFFLINE__ = false;
  });
  await page.screenshot({
    path: ".gstack/qa-reports/screenshots/discover-search-verified.png",
    fullPage: true,
  });

  await page.goto(
    "/explore/tmdb/film/scr_01991f588e0070008000000000000001/old-title",
  );
  await expect(page).toHaveURL(
    `/explore/tmdb/film/scr_01991f588e0070008000000000000001/dune-part-two`,
  );
  await page.getByRole("button", { name: "Back to Search" }).click();
  await expect(page).toHaveURL("/discover");

  await provider.selectOption("tmdb");
  await page
    .getByRole("searchbox", { name: "Search TMDB" })
    .fill("Continuation");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Next local Records" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Retry or load more provider results" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Next local Records" }).click();
  await expect(page.getByRole("heading", { name: "Local Dune" })).toBeVisible();
  await page
    .getByRole("button", { name: "Retry or load more provider results" })
    .click();
  await expect(
    page.getByRole("heading", { name: "Dune: Part Two" }),
  ).toBeVisible();

  await page.goto("/explore/tmdb/film/not-a-receipt/title");
  await expect(page.getByRole("alert")).toContainText(
    "This candidate link is invalid.",
  );
});

for (const scenario of [
  "Repeated coordinate",
  "Mixed evidence",
  "Non-forward continuation",
  "Receipt collision",
]) {
  test(`native provider pagination preserves prior evidence: ${scenario}`, async ({
    page,
  }) => {
    await mockNativeSearch(page);
    await page.goto("/discover");
    await page.getByRole("searchbox", { name: "Search TMDB" }).fill(scenario);
    await page.getByRole("button", { name: "Search", exact: true }).click();
    const results = page.getByRole("region", { name: "Search results" });
    const first = results.getByRole("listitem").filter({
      has: page.getByRole("heading", { name: "Dune: Part Two", exact: true }),
    });
    const firstHref =
      "/explore/tmdb/film/scr_01991f588e0070008000000000000001/dune-part-two";
    await expect(first).toBeVisible();
    await expect(
      first.getByRole("link", { name: "View details" }),
    ).toHaveAttribute("href", firstHref);
    await page
      .getByRole("button", { name: "Retry or load more provider results" })
      .click();
    await expect
      .poll(() => page.evaluate(() => window.__SEARCH_PAGE_INPUTS__?.length))
      .toBe(2);
    await expect(
      page.getByRole("button", { name: "Search", exact: true }),
    ).toBeEnabled();

    if (scenario === "Mixed evidence") {
      await expect(results.getByRole("listitem")).toHaveCount(2);
      await expect(
        first.getByText("Fresh cache evidence", { exact: true }),
      ).toBeVisible();
      const second = results.getByRole("listitem").filter({
        has: page.getByRole("heading", {
          name: "Second-page candidate",
          exact: true,
        }),
      });
      await expect(
        second.getByText("Observed from provider", { exact: true }),
      ).toBeVisible();
      await expect(results.getByRole("status")).not.toContainText(
        "These results came from fresh cache evidence.",
      );
      await expect(results.getByRole("status")).not.toContainText(
        "These results were observed from the provider now.",
      );
      for (const width of [1440, 320]) {
        await page.setViewportSize({ width, height: 1000 });
        await expect(
          first.getByText("Fresh cache evidence", { exact: true }),
        ).toBeVisible();
        await expect(
          second.getByText("Observed from provider", { exact: true }),
        ).toBeVisible();
        expect((await new AxeBuilder({ page }).analyze()).violations).toEqual(
          [],
        );
        await page.evaluate(() => window.scrollTo(0, 0));
        await page.screenshot({
          path: `.gstack/qa-reports/screenshots/discover-search-mixed-evidence-${width}.png`,
          fullPage: true,
        });
      }
    } else {
      await expect(results.getByRole("listitem")).toHaveCount(1);
      await expect(
        results.getByRole("heading", {
          name: "Second-page candidate",
          exact: true,
        }),
      ).toHaveCount(0);
      await expect(
        results.getByRole("heading", {
          name: "Repeated coordinate title",
          exact: true,
        }),
      ).toHaveCount(0);
      if (scenario !== "Repeated coordinate") {
        await expect(page.getByRole("alert")).toContainText(
          /provider|page|receipt|Search/i,
        );
      } else {
        await expect(
          page.getByRole("button", {
            name: "Retry or load more provider results",
          }),
        ).toBeEnabled();
        await expect(page.getByRole("alert")).toHaveCount(0);
      }
    }
    await expect(
      first.getByRole("link", { name: "View details" }),
    ).toHaveAttribute("href", firstHref);
  });
}

test("native all-provider Search rejects only the colliding provider page and preserves source order", async ({
  page,
}) => {
  await mockNativeSearch(page);
  await page.goto("/discover");
  await page.getByLabel("Metadata provider").selectOption("all");
  await page
    .getByRole("searchbox", { name: "Search local Records and providers" })
    .fill("Provider batch collision");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  const results = page.getByRole("region", { name: "Search results" });
  await expect(results.getByRole("listitem").getByRole("heading")).toHaveText([
    "Batch TMDB result",
    "Batch TVDB result",
  ]);
  await expect(results.getByRole("listitem")).toHaveCount(2);
  await expect(page.getByRole("heading", { name: "Local Dune" })).toBeVisible();
  await expect(
    page.getByText("Rejected provider first row", { exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByText("Colliding provider row", { exact: true }),
  ).toHaveCount(0);
  await expect(page.getByRole("alert")).toContainText(/receipt/i);
  const retry = page.getByRole("button", {
    name: "Retry or load more provider results",
  });
  await expect(retry).toBeEnabled();
  await retry.click();
  await expect
    .poll(() => page.evaluate(() => window.__SEARCH_PAGE_INPUTS__?.length))
    .toBe(4);
  await expect(retry).toBeEnabled();
  const submitted = await page.evaluate(
    () =>
      window.__SEARCH_PAGE_INPUTS__ as Array<{
        provider_id: string;
        request: { page: number };
      }>,
  );
  expect(
    submitted.map((input) => [input.provider_id, input.request.page]),
  ).toEqual([
    ["tmdb", 1],
    ["google-books", 1],
    ["tvdb", 1],
    ["google-books", 1],
  ]);
  await expect(results.getByRole("listitem").getByRole("heading")).toHaveText([
    "Batch TMDB result",
    "Batch TVDB result",
  ]);
  await expect(results.getByRole("listitem")).toHaveCount(2);
  await expect(page.getByRole("alert")).toContainText(/receipt/i);
});
