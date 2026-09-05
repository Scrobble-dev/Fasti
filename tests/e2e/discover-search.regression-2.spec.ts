import { expect, test, type Page } from "@playwright/test";
import { mockAuthenticatedAccess } from "./test-helpers";

declare global {
  interface Window {
    __SEARCH_BOUNDARY_FIXTURE__?: {
      localQueries: string[];
      providerQueries: Array<{ provider: string; query: string }>;
      completedProviderQueries: string[];
      candidateQueries: string[];
      candidateDetailsCompleted: string[];
      releaseFirstProviderSearch?: () => void;
      releaseCandidateDetails?: () => void;
    };
  }
}

const candidateReceiptA = "scr_01991f588e0070008000000000000d01";
const candidateReceiptB = "scr_01991f588e0070008000000000000d02";

const candidateA = {
  provider: "tmdb",
  provider_id: "842101",
  grain: "film",
  kind: "movie",
  title: "Delayed Candidate A",
  original_title: "Delayed Candidate A",
  release_year: 2020,
  authors: [],
  image_url: null,
  overview: "Delayed details from candidate A.",
};

const candidateB = {
  ...candidateA,
  provider_id: "842102",
  title: "Current Candidate B",
  original_title: "Current Candidate B",
  overview: "Current details from candidate B.",
};

const localRecord = {
  record_id: "rec_01991f588e0070008000000000000c01",
  grain: "film",
  status: "active",
  title: {
    tier: "fallback_provider_claim",
    value: "Local-only result",
    source: "manual",
    is_stale: false,
  },
  poster: { tier: "empty", value: null, source: null, is_stale: false },
  original_title: {
    tier: "empty",
    value: null,
    source: null,
    is_stale: false,
  },
  overview: { tier: "empty", value: null, source: null, is_stale: false },
  release_year: {
    tier: "fallback_provider_claim",
    value: "2026",
    source: "manual",
    is_stale: false,
  },
  identifiers: [],
  latest_activity: null,
  poster_asset_path: null,
};

function providerStatus(provider: string, label: string, available: boolean) {
  return {
    provider,
    capability_id: "metadata.search",
    label,
    purpose: `Search ${label} metadata`,
    credential_requirement: "api_key",
    credential_state: available ? "valid" : "missing",
    state: available ? "available" : "unavailable",
    source: available ? "environment" : "none",
    writable: false,
    testable: true,
    docs_url: "https://example.invalid/provider-docs",
  };
}

async function installSearchHost(
  page: Page,
  providers: ReturnType<typeof providerStatus>[],
  holdFirstProviderSearch = false,
  holdCandidateReceiptId?: string,
) {
  await mockAuthenticatedAccess(page);
  await page.addInitScript(
    ({
      localRecord,
      providers,
      holdFirstProviderSearch,
      holdCandidateReceiptId,
      candidateReceiptA,
      candidateReceiptB,
      candidateA,
      candidateB,
    }) => {
      const fixture = {
        localQueries: [] as string[],
        providerQueries: [] as Array<{ provider: string; query: string }>,
        completedProviderQueries: [] as string[],
        candidateQueries: [] as string[],
        candidateDetailsCompleted: [] as string[],
        releaseFirstProviderSearch: undefined as (() => void) | undefined,
        releaseCandidateDetails: undefined as (() => void) | undefined,
      };
      const browserWindow = window as typeof window & {
        __SEARCH_BOUNDARY_FIXTURE__: typeof fixture;
        __TAURI_INTERNALS__: {
          invoke: (command: string, arguments_?: unknown) => Promise<unknown>;
        };
      };
      browserWindow.__SEARCH_BOUNDARY_FIXTURE__ = fixture;
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
              return providers;
            case "list_records":
              return { records: [], truncated: false };
            case "list_reviews":
              return [];
            case "search_records": {
              const query = (
                arguments_ as { input?: { query?: string } } | undefined
              )?.input?.query;
              fixture.localQueries.push(query ?? "");
              return { records: [localRecord], next: null };
            }
            case "search_provider_page": {
              const input = (
                arguments_ as {
                  input?: {
                    provider_id?: string;
                    request?: { query?: string; page?: number };
                  };
                }
              )?.input;
              fixture.providerQueries.push({
                provider: input?.provider_id ?? "",
                query: input?.request?.query ?? "",
              });
              if (
                holdFirstProviderSearch &&
                fixture.providerQueries.length === 1
              ) {
                await new Promise<void>((resolve) => {
                  fixture.releaseFirstProviderSearch = resolve;
                });
              }
              fixture.completedProviderQueries.push(
                input?.request?.query ?? "",
              );
              return {
                outcome: "page",
                provider_id: input?.provider_id,
                page: input?.request?.page ?? 1,
                candidates: [],
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
              fixture.candidateQueries.push(selectedReceiptId ?? "");
              if (selectedReceiptId === holdCandidateReceiptId) {
                await new Promise<void>((resolve) => {
                  fixture.releaseCandidateDetails = resolve;
                });
              }
              const candidate =
                selectedReceiptId === candidateReceiptB
                  ? candidateB
                  : candidateA;
              fixture.candidateDetailsCompleted.push(selectedReceiptId ?? "");
              return {
                outcome: "refetched",
                snapshot: {
                  receipt: {
                    candidate_receipt_id:
                      selectedReceiptId ?? candidateReceiptA,
                    grain: "film",
                    candidate,
                  },
                  lifetime: {
                    created_at: "2026-09-05T12:00:00Z",
                    fresh_until: "2026-09-05T12:02:00Z",
                    stale_until: "2026-09-05T12:10:00Z",
                    expires_at: "2026-09-06T12:00:00Z",
                  },
                  locale: "en-US",
                },
                details: candidate,
                locale: "en-US",
              };
            }
            default:
              throw new Error(`Unexpected trusted-host command: ${command}`);
          }
        },
      };
    },
    {
      localRecord,
      providers,
      holdFirstProviderSearch,
      holdCandidateReceiptId,
      candidateReceiptA,
      candidateReceiptB,
      candidateA,
      candidateB,
    },
  );
}

async function settleBrowserWork(page: Page): Promise<void> {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      ),
  );
}

// Regression: ISSUE-003 — Local-only Search claimed an empty provider result without querying a provider.
// Found by /qa on 2026-09-05.
// Report: .gstack/qa-reports/qa-report-fasti-m4-review-2026-09-05.md
test("local-only Search does not claim an empty provider query", async ({
  page,
}, testInfo) => {
  await installSearchHost(page, [providerStatus("tmdb", "TMDB", false)]);
  await page.goto("/discover");
  await page.getByLabel("Metadata provider").selectOption("all");
  await page
    .getByRole("searchbox", { name: "Search local Records and providers" })
    .fill("Local-only");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(
    page.getByRole("region", { name: "Local Records" }),
  ).toBeVisible();
  await expect(page.getByRole("region", { name: "Your Library" })).toHaveCount(
    0,
  );
  await page.waitForFunction(
    () => window.__SEARCH_BOUNDARY_FIXTURE__?.localQueries.length === 1,
  );
  await page.getByRole("heading", { name: "Local-only result" }).waitFor();
  await page.screenshot({
    path: testInfo.outputPath("issue-003-local-only-before.png"),
    fullPage: true,
  });

  await expect(
    page.getByRole("heading", { name: "Local-only result" }),
  ).toBeVisible();
  await expect(
    page.getByText("No compatible titles found", { exact: false }),
  ).toHaveCount(0);
  await expect(
    page.getByText("No provider was queried. Local results are shown above.", {
      exact: true,
    }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => window.__SEARCH_BOUNDARY_FIXTURE__?.providerQueries,
    ),
  ).toEqual([]);
});

test("a queried provider keeps its empty result after a stale source request", async ({
  page,
}, testInfo) => {
  await installSearchHost(
    page,
    [
      providerStatus("tmdb", "TMDB", true),
      providerStatus("tvdb", "TVDB", true),
    ],
    true,
  );
  await page.goto("/discover");
  const provider = page.getByLabel("Metadata provider");
  await provider.waitFor();
  await page.getByRole("searchbox", { name: "Search TMDB" }).fill("Stale");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.waitForFunction(
    () => window.__SEARCH_BOUNDARY_FIXTURE__?.providerQueries.length === 1,
  );

  await provider.selectOption("tvdb");
  await page.evaluate(() =>
    window.__SEARCH_BOUNDARY_FIXTURE__?.releaseFirstProviderSearch?.(),
  );
  await page
    .getByRole("searchbox", { name: "Search TVDB" })
    .fill("Provider zero");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.waitForFunction(
    () => window.__SEARCH_BOUNDARY_FIXTURE__?.providerQueries.length === 2,
  );
  await page
    .getByText("No compatible titles found for Provider zero.", { exact: true })
    .waitFor();
  await page.screenshot({
    path: testInfo.outputPath("issue-003-provider-empty-before.png"),
    fullPage: true,
  });

  await expect(
    page.getByRole("heading", { name: "Local-only result" }),
  ).toBeVisible();
  await expect(
    page.getByText("No compatible titles found for Provider zero.", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByText("No compatible titles found for Stale.", { exact: true }),
  ).toHaveCount(0);
  expect(
    await page.evaluate(
      () => window.__SEARCH_BOUNDARY_FIXTURE__?.providerQueries,
    ),
  ).toEqual([
    { provider: "tmdb", query: "Stale" },
    { provider: "tvdb", query: "Provider zero" },
  ]);
});

test("delayed candidate A details cannot replace candidate B after navigation", async ({
  page,
}) => {
  await installSearchHost(
    page,
    [providerStatus("tmdb", "TMDB", true)],
    false,
    candidateReceiptA,
  );
  await page.goto(
    `/explore/tmdb/film/${candidateReceiptA}/delayed-candidate-a`,
  );
  await page.waitForFunction(
    (receiptId) =>
      window.__SEARCH_BOUNDARY_FIXTURE__?.candidateQueries.includes(receiptId),
    candidateReceiptA,
  );

  await page.evaluate(
    ({ receiptId }) => {
      history.pushState(
        {},
        "",
        `/explore/tmdb/film/${receiptId}/requested-candidate-b`,
      );
      dispatchEvent(new PopStateEvent("popstate"));
    },
    { receiptId: candidateReceiptB },
  );
  await expect(
    page.getByRole("heading", { level: 1, name: candidateB.title }),
  ).toBeVisible();
  await expect(page.getByText(candidateB.overview)).toBeVisible();
  await expect(page).toHaveURL(
    `/explore/tmdb/film/${candidateReceiptB}/current-candidate-b`,
  );

  await page.evaluate(() =>
    window.__SEARCH_BOUNDARY_FIXTURE__?.releaseCandidateDetails?.(),
  );
  await page.waitForFunction(
    (receiptId) =>
      window.__SEARCH_BOUNDARY_FIXTURE__?.candidateDetailsCompleted.includes(
        receiptId,
      ),
    candidateReceiptA,
  );
  await settleBrowserWork(page);

  await expect(page).toHaveURL(
    `/explore/tmdb/film/${candidateReceiptB}/current-candidate-b`,
  );
  await expect(
    page.getByRole("heading", { level: 1, name: candidateB.title }),
  ).toBeVisible();
  await expect(page.getByText(candidateB.overview)).toBeVisible();
  await expect(page.getByText(candidateA.overview)).toHaveCount(0);
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(
    page.getByText("Loading candidate…", { exact: true }),
  ).toHaveCount(0);
});

test("delayed candidate details cannot repopulate Discover after route exit", async ({
  page,
}) => {
  await installSearchHost(
    page,
    [providerStatus("tmdb", "TMDB", true)],
    false,
    candidateReceiptA,
  );
  await page.goto(
    `/explore/tmdb/film/${candidateReceiptA}/delayed-candidate-a`,
  );
  await page.waitForFunction(
    (receiptId) =>
      window.__SEARCH_BOUNDARY_FIXTURE__?.candidateQueries.includes(receiptId),
    candidateReceiptA,
  );

  await page.evaluate(() => {
    history.pushState({}, "", "/library");
    dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(
    page.getByRole("heading", { level: 1, name: "Library" }),
  ).toBeVisible();
  await page.evaluate(() =>
    window.__SEARCH_BOUNDARY_FIXTURE__?.releaseCandidateDetails?.(),
  );
  await page.waitForFunction(
    (receiptId) =>
      window.__SEARCH_BOUNDARY_FIXTURE__?.candidateDetailsCompleted.includes(
        receiptId,
      ),
    candidateReceiptA,
  );
  await settleBrowserWork(page);

  await expect(page).toHaveURL("/library");
  await expect(page.getByText(candidateA.overview)).toHaveCount(0);
  await page.getByRole("link", { name: "Discover", exact: true }).click();
  await expect(page).toHaveURL("/discover");
  await expect(page.getByText(candidateA.overview)).toHaveCount(0);
  await expect(
    page.getByRole("heading", { level: 1, name: candidateA.title }),
  ).toHaveCount(0);
});

test("a pending Search cannot repopulate Discover after route exit", async ({
  page,
}) => {
  await installSearchHost(page, [providerStatus("tmdb", "TMDB", true)], true);
  await page.goto("/discover");
  await page
    .getByRole("searchbox", { name: "Search TMDB" })
    .fill("Abandoned Search");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page.waitForFunction(
    () => window.__SEARCH_BOUNDARY_FIXTURE__?.providerQueries.length === 1,
  );

  await page.evaluate(() => {
    history.pushState({}, "", "/library");
    dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(
    page.getByRole("heading", { level: 1, name: "Library" }),
  ).toBeVisible();
  await page.evaluate(() =>
    window.__SEARCH_BOUNDARY_FIXTURE__?.releaseFirstProviderSearch?.(),
  );
  await page.waitForFunction(() =>
    window.__SEARCH_BOUNDARY_FIXTURE__?.completedProviderQueries.includes(
      "Abandoned Search",
    ),
  );
  await settleBrowserWork(page);

  await expect(page).toHaveURL("/library");
  await page.getByRole("link", { name: "Discover", exact: true }).click();
  await expect(page).toHaveURL("/discover");
  await expect(
    page.getByRole("searchbox", { name: "Search TMDB" }),
  ).toHaveValue("");
  await expect(
    page.getByRole("heading", { name: localRecord.title.value }),
  ).toHaveCount(0);
  await expect(page.getByText(/Abandoned Search/)).toHaveCount(0);
  await expect(page.getByText(/Searching/)).toHaveCount(0);
});
