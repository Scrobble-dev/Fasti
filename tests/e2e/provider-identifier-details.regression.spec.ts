import { expect, test, type Page } from "@playwright/test";
import { mockAuthenticatedAccess } from "./test-helpers";

const liveProviderId = "438631";
const delayedProviderId = "438632";
const retainedReceiptId = "scr_01991f588e0070008000000000000e01";
const createdRecordId = "rec_01991f588e0070008000000000000e01";
const existingRecordId = "rec_01991f588e0070008000000000000e02";

interface DetailCall {
  provider_id: string;
  grain: string;
  query: {
    provider_record_id: string;
    offline: boolean;
    locale: string | null;
  };
}

interface ProviderActionCall {
  provider_id: string;
  grain: string;
  request: {
    operation_id: string;
    provider_record_id: string;
    action: { kind: string; record_id?: string };
  };
}

declare global {
  interface Window {
    __PROVIDER_IDENTIFIER_DETAILS_FIXTURE__: {
      detailCalls: DetailCall[];
      completedDetailIds: string[];
      providerActions: ProviderActionCall[];
      retainedReads: number;
      retainedActions: number;
      searchProviderRecordId: string;
      offline: boolean;
      releaseDetails?: () => void;
    };
  }
}

// This is a browser presentation fixture at the existing trusted-host boundary.
// Runtime authorization, provider rechecks, payload bounds, and persistence are
// covered by their native Store, runtime, HTTP, and SDK contract suites.
async function installProviderIdentifierDetailsHost(
  page: Page,
  options: { offline?: boolean; holdProviderId?: string } = {},
) {
  await mockAuthenticatedAccess(page);
  await page.addInitScript(
    ({
      liveProviderId,
      createdRecordId,
      existingRecordId,
      offline,
      holdProviderId,
    }) => {
      const fixture: Window["__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__"] = {
        detailCalls: [],
        completedDetailIds: [],
        providerActions: [],
        retainedReads: 0,
        retainedActions: 0,
        searchProviderRecordId: liveProviderId,
        offline,
      };
      window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__ = fixture;
      Object.defineProperty(navigator, "onLine", {
        configurable: true,
        get: () => !fixture.offline,
      });

      const projectedField = (value: string | null) => ({
        value,
        tier: value === null ? "empty" : "preferred_provider_claim",
        source: value === null ? null : "tmdb",
        is_stale: false,
      });
      const candidate = (providerRecordId: string) => ({
        provider: "tmdb",
        provider_id: providerRecordId,
        grain: "film",
        kind: "movie",
        title:
          providerRecordId === liveProviderId
            ? "Dune: Part Two / Director's Cut"
            : "Delayed live candidate A",
        original_title: "Dune: Part Two",
        release_year: 2024,
        authors: [],
        image_url: null,
        overview:
          providerRecordId === liveProviderId
            ? "Full provider details loaded from the selected identifier."
            : "Late live details that must not replace the current route.",
      });
      const retainedCandidate = {
        provider: "tmdb",
        provider_id: "438633",
        grain: "film",
        kind: "movie",
        title: "Current retained candidate B",
        original_title: null,
        release_year: 2025,
        authors: [],
        image_url: null,
        overview: "Current retained details remain visible.",
      };
      const records = [
        {
          record_id: createdRecordId,
          grain: "film",
          status: "active",
          title: projectedField("Dune: Part Two / Director's Cut"),
          poster: projectedField(null),
          original_title: projectedField(null),
          overview: projectedField(null),
          release_year: projectedField("2024"),
          identifiers: [],
          latest_activity: null,
          poster_asset_path: null,
        },
        {
          record_id: existingRecordId,
          grain: "film",
          status: "active",
          title: projectedField("Existing Dune Record"),
          poster: projectedField(null),
          original_title: projectedField(null),
          overview: projectedField(null),
          release_year: projectedField("2024"),
          identifiers: [],
          latest_activity: null,
          poster_asset_path: null,
        },
      ];
      const browserWindow = window as typeof window & {
        __TAURI_INTERNALS__: {
          invoke: (command: string, args?: unknown) => Promise<unknown>;
        };
      };
      browserWindow.__TAURI_INTERNALS__ = {
        invoke: async (command, args) => {
          const input = (
            args as
              | {
                  input?:
                    DetailCall | ProviderActionCall | Record<string, unknown>;
                  query?: { record_id?: string };
                }
              | undefined
          )?.input;
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
              ];
            case "list_records": {
              const recordId = (
                args as { query?: { record_id?: string } } | undefined
              )?.query?.record_id;
              return {
                records: recordId
                  ? records.filter((record) => record.record_id === recordId)
                  : records,
                truncated: false,
              };
            }
            case "list_reviews":
              return [];
            case "search_records":
              return { records: [records[1]], next: null };
            case "search_provider_page": {
              const providerRecordId = fixture.searchProviderRecordId;
              return {
                outcome: "live",
                provider_id: "tmdb",
                page: 1,
                candidates: [candidate(providerRecordId)],
                next_page: null,
              };
            }
            case "read_provider_identifier_details": {
              const detailInput = input as DetailCall;
              fixture.detailCalls.push(structuredClone(detailInput));
              if (!/^[1-9][0-9]*$/.test(detailInput.query.provider_record_id)) {
                throw {
                  detail: "The provider identifier is invalid.",
                  next_action: "Open details from a current Search result.",
                };
              }
              if (detailInput.query.provider_record_id === holdProviderId) {
                await new Promise<void>((resolve) => {
                  fixture.releaseDetails = resolve;
                });
              }
              fixture.completedDetailIds.push(
                detailInput.query.provider_record_id,
              );
              if (detailInput.query.offline) {
                return {
                  outcome: "unavailable",
                  provider_id: detailInput.provider_id,
                  grain: detailInput.grain,
                  provider_record_id: detailInput.query.provider_record_id,
                  problem_code: "offline",
                };
              }
              return {
                outcome: "details",
                provider_id: detailInput.provider_id,
                grain: detailInput.grain,
                provider_record_id: detailInput.query.provider_record_id,
                details: candidate(detailInput.query.provider_record_id),
                locale: detailInput.query.locale,
              };
            }
            case "read_search_candidate": {
              fixture.retainedReads += 1;
              return {
                outcome: "refetched_without_snapshot",
                details: retainedCandidate,
                locale: null,
              };
            }
            case "save_search_candidate":
              fixture.retainedActions += 1;
              throw new Error(
                "Live details must not use a retained receipt action",
              );
            case "save_provider_identifier": {
              const actionInput = input as ProviderActionCall;
              fixture.providerActions.push(structuredClone(actionInput));
              const recordId =
                actionInput.request.action.kind === "attach"
                  ? actionInput.request.action.record_id
                  : createdRecordId;
              return {
                outcome: "saved",
                receipt: {
                  operation_id: actionInput.request.operation_id,
                  provider_id: actionInput.provider_id,
                  provider_record_id: actionInput.request.provider_record_id,
                  grain: actionInput.grain,
                  action: actionInput.request.action,
                  origin: "user_selected_provider_identifier",
                  record_id: recordId,
                  disposition:
                    actionInput.request.action.kind === "attach"
                      ? "attached"
                      : "created",
                  committed_at: "2026-09-06T12:00:00Z",
                },
              };
            }
            default:
              throw new Error(`Unexpected trusted-host command: ${command}`);
          }
        },
      };
    },
    {
      liveProviderId,
      createdRecordId,
      existingRecordId,
      offline: options.offline ?? false,
      holdProviderId: options.holdProviderId,
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

test("the fixed live coordinate route reloads full details and normalizes locale", async ({
  page,
}, testInfo) => {
  await installProviderIdentifierDetailsHost(page);
  await page.goto(
    "/explore/live/tmdb/film/%34%33%38%36%33%31/candidate?locale=EN-us",
  );
  await page
    .getByText("Full provider details loaded from the selected identifier.")
    .waitFor();
  await page.screenshot({
    path: testInfo.outputPath("live-provider-details-before.png"),
    fullPage: true,
  });

  await expect(
    page.getByRole("heading", {
      level: 1,
      name: "Dune: Part Two / Director's Cut",
    }),
  ).toBeVisible();
  await expect(page).toHaveURL(
    `/explore/live/tmdb/film/${liveProviderId}/candidate?locale=en-us`,
  );
  expect(
    await page.evaluate(
      () => window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__.detailCalls,
    ),
  ).toEqual([
    {
      provider_id: "tmdb",
      grain: "film",
      query: {
        provider_record_id: liveProviderId,
        offline: false,
        locale: "en-us",
      },
    },
  ]);

  await page.reload();
  await expect(
    page.getByText(
      "Full provider details loaded from the selected identifier.",
    ),
  ).toBeVisible();
  await expect(page.getByText("Original title: Dune: Part Two")).toBeVisible();
  await expect(page.getByText("2024", { exact: true })).toBeVisible();
});

test("live Search uses a fixed title-free route bound to the exact provider ID", async ({
  page,
}) => {
  await installProviderIdentifierDetailsHost(page);
  await page.goto("/discover");
  await page.getByRole("searchbox", { name: "Search TMDB" }).fill("Dune");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  const result = page.getByRole("listitem").filter({
    has: page.getByRole("heading", {
      name: "Dune: Part Two / Director's Cut",
    }),
  });
  const details = result.getByRole("link", { name: "View details" });
  await expect(details).toHaveAttribute(
    "href",
    "/explore/live/tmdb/film/438631/candidate",
  );
  await details.click();

  await expect(page).toHaveURL("/explore/live/tmdb/film/438631/candidate");
  await expect(
    page
      .locator(".candidate-detail")
      .getByText("Full provider details loaded from the selected identifier."),
  ).toBeVisible();
  expect(page.url()).not.toContain("dune-part-two");
  expect(
    await page.evaluate(
      () => window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__.detailCalls,
    ),
  ).toEqual([
    {
      provider_id: "tmdb",
      grain: "film",
      query: {
        provider_record_id: liveProviderId,
        offline: false,
        locale: null,
      },
    },
  ]);
});

test("an opaque invalid TMDB identifier is decoded once and rejected by the host", async ({
  page,
}) => {
  await installProviderIdentifierDetailsHost(page);
  await page.goto("/explore/live/tmdb/film/438631%2Falternate/candidate");

  await expect(page.getByRole("alert")).toContainText(
    "The provider identifier is invalid",
  );
  expect(
    await page.evaluate(
      () => window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__.detailCalls,
    ),
  ).toEqual([
    {
      provider_id: "tmdb",
      grain: "film",
      query: {
        provider_record_id: "438631/alternate",
        offline: false,
        locale: null,
      },
    },
  ]);
  await expect(page.getByRole("button", { name: "Create Record" })).toHaveCount(
    0,
  );
});

test("Create from live details saves the selected provider identifier without a receipt", async ({
  page,
}) => {
  await installProviderIdentifierDetailsHost(page);
  await page.goto(`/explore/live/tmdb/film/${liveProviderId}/candidate`);
  await page.getByRole("button", { name: "Create Record" }).click();
  await expect(page).toHaveURL(
    `/records/film/${createdRecordId}/dune-part-two-director-s-cut`,
  );

  const fixture = await page.evaluate(
    () => window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__,
  );
  expect(fixture.providerActions).toHaveLength(1);
  expect(fixture.providerActions[0]).toMatchObject({
    provider_id: "tmdb",
    grain: "film",
    request: {
      provider_record_id: liveProviderId,
      action: { kind: "create" },
    },
  });
  expect(fixture.retainedReads).toBe(0);
  expect(fixture.retainedActions).toBe(0);
});

test("Attach from live details saves the provider identifier against the chosen Record", async ({
  page,
}) => {
  await installProviderIdentifierDetailsHost(page);
  await page.goto(`/explore/live/tmdb/film/${liveProviderId}/candidate`);
  await page
    .getByRole("button", { name: "Attach to existing Record", exact: true })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Attach to existing Record",
  });
  await dialog.getByRole("button", { name: "Find Records" }).click();
  await dialog.getByRole("radio", { name: /Existing Dune Record/ }).check();
  await dialog.getByRole("button", { name: "Confirm attachment" }).click();
  await expect(page).toHaveURL(
    `/records/film/${existingRecordId}/existing-dune-record`,
  );

  const fixture = await page.evaluate(
    () => window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__,
  );
  expect(fixture.providerActions).toHaveLength(1);
  expect(fixture.providerActions[0]).toMatchObject({
    provider_id: "tmdb",
    grain: "film",
    request: {
      provider_record_id: liveProviderId,
      action: { kind: "attach", record_id: existingRecordId },
    },
  });
  expect(fixture.retainedReads).toBe(0);
  expect(fixture.retainedActions).toBe(0);
});

test("malformed and over-budget live coordinates are rejected before the host", async ({
  page,
}) => {
  await installProviderIdentifierDetailsHost(page);
  const invalidRoutes = [
    "/explore/live/tmdb/film//candidate",
    "/explore/live/tmdb/film/438631/not-a-fixed-route",
    `/explore/live/tmdb/film/${encodeURIComponent("é".repeat(129))}/candidate`,
    "/explore/live/tmdb/film/438631/candidate?locale=e",
    "/explore/live/tmdb/film/438631/candidate?locale=en&locale=fr",
    "/explore/live/tmdb/film/438631/candidate?receipt=forbidden",
  ];

  for (const route of invalidRoutes) {
    await page.goto(route);
    await expect(page.getByRole("alert")).toContainText(
      "This candidate link is invalid",
    );
    expect(
      await page.evaluate(
        () => window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__.detailCalls,
      ),
    ).toEqual([]);
  }
});

test("retained details without a snapshot never acquire live identifier actions", async ({
  page,
}) => {
  await installProviderIdentifierDetailsHost(page);
  await page.goto(
    `/explore/tmdb/film/${retainedReceiptId}/current-retained-candidate-b`,
  );

  await expect(
    page.getByText("Current retained details remain visible."),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Create Record" })).toHaveCount(
    0,
  );
  await expect(
    page.getByRole("button", { name: "Attach to existing Record" }),
  ).toHaveCount(0);
  const fixture = await page.evaluate(
    () => window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__,
  );
  expect(fixture.retainedReads).toBe(1);
  expect(fixture.providerActions).toEqual([]);
  expect(fixture.retainedActions).toBe(0);
});

test("delayed live details cannot replace a retained candidate after navigation", async ({
  page,
}) => {
  await installProviderIdentifierDetailsHost(page, {
    holdProviderId: delayedProviderId,
  });
  await page.goto(`/explore/live/tmdb/film/${delayedProviderId}/candidate`);
  await page.waitForFunction(
    (providerRecordId) =>
      window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__.detailCalls.some(
        (call) => call.query.provider_record_id === providerRecordId,
      ),
    delayedProviderId,
  );

  await page.evaluate(
    ({ retainedReceiptId }) => {
      history.pushState(
        {},
        "",
        `/explore/tmdb/film/${retainedReceiptId}/current-retained-candidate-b`,
      );
      dispatchEvent(new PopStateEvent("popstate"));
    },
    { retainedReceiptId },
  );
  await expect(
    page.getByRole("heading", {
      level: 1,
      name: "Current retained candidate B",
    }),
  ).toBeVisible();
  await expect(
    page.getByText("Current retained details remain visible."),
  ).toBeVisible();

  await page.evaluate(() =>
    window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__.releaseDetails?.(),
  );
  await page.waitForFunction(
    (providerRecordId) =>
      window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__.completedDetailIds.includes(
        providerRecordId,
      ),
    delayedProviderId,
  );
  await settleBrowserWork(page);

  await expect(page).toHaveURL(
    `/explore/tmdb/film/${retainedReceiptId}/current-retained-candidate-b`,
  );
  await expect(
    page.getByText("Current retained details remain visible."),
  ).toBeVisible();
  await expect(
    page.getByText(
      "Late live details that must not replace the current route.",
    ),
  ).toHaveCount(0);
});

test("delayed live details cannot repopulate the UI after route exit", async ({
  page,
}) => {
  await installProviderIdentifierDetailsHost(page, {
    holdProviderId: delayedProviderId,
  });
  await page.goto(`/explore/live/tmdb/film/${delayedProviderId}/candidate`);
  await page.waitForFunction(
    (providerRecordId) =>
      window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__.detailCalls.some(
        (call) => call.query.provider_record_id === providerRecordId,
      ),
    delayedProviderId,
  );

  await page.evaluate(() => {
    history.pushState({}, "", "/library");
    dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(
    page.getByRole("heading", { level: 1, name: "Library" }),
  ).toBeVisible();
  await page.evaluate(() =>
    window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__.releaseDetails?.(),
  );
  await page.waitForFunction(
    (providerRecordId) =>
      window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__.completedDetailIds.includes(
        providerRecordId,
      ),
    delayedProviderId,
  );
  await settleBrowserWork(page);

  await expect(page).toHaveURL("/library");
  await expect(
    page.getByText(
      "Late live details that must not replace the current route.",
    ),
  ).toHaveCount(0);
  await page.getByRole("link", { name: "Discover", exact: true }).click();
  await expect(page).toHaveURL("/discover");
  await expect(
    page.getByRole("heading", { name: "Delayed live candidate A" }),
  ).toHaveCount(0);
});

test("offline live details report unavailability without inventing a snapshot", async ({
  page,
}) => {
  await installProviderIdentifierDetailsHost(page, { offline: true });
  await page.goto(`/explore/live/tmdb/film/${liveProviderId}/candidate`);

  await expect(page.getByRole("alert")).toContainText(
    "Provider details are unavailable (offline)",
  );
  await expect(
    page.getByText(
      "Full provider details loaded from the selected identifier.",
    ),
  ).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Create Record" })).toHaveCount(
    0,
  );
  const fixture = await page.evaluate(
    () => window.__PROVIDER_IDENTIFIER_DETAILS_FIXTURE__,
  );
  expect(fixture.detailCalls).toEqual([
    {
      provider_id: "tmdb",
      grain: "film",
      query: {
        provider_record_id: liveProviderId,
        offline: true,
        locale: null,
      },
    },
  ]);
  expect(fixture.providerActions).toEqual([]);
  expect(fixture.retainedReads).toBe(0);
});
