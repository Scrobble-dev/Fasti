import { expect, test } from "@playwright/test";
import { mockAuthenticatedAccess } from "./test-helpers";

// Regression: ISSUE-001 and ISSUE-002 — M4 Search lost provider context and stable selection.
// Found by /qa on 2026-09-05.
// Report: .gstack/qa-reports/qa-report-fasti-local-2026-09-05.md
test("local and receipt-backed provider Search survive a partial source failure", async ({
  page,
}) => {
  await mockAuthenticatedAccess(page);
  await page.addInitScript(() => {
    const receiptId = "scr_01991f588e0070008000000000000001";
    const candidate = {
      provider: "tmdb",
      provider_id: "693134",
      kind: "movie",
      title: "Dune: Part Two",
      original_title: "Dune: Part Two",
      release_year: 2024,
      authors: [],
      image_url: null,
      overview: "Paul joins Chani and the Fremen.",
    };
    const field = (value: string | null, source: string | null = null) => ({
      tier: value === null ? "empty" : "preferred_provider_claim",
      value,
      source,
      is_stale: false,
    });
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
    ];
    const browserWindow = window as typeof window & {
      __SEARCH_PAGE_INPUTS__?: unknown[];
      __TAURI_INTERNALS__: {
        invoke: (command: string, arguments_?: unknown) => Promise<unknown>;
      };
    };
    browserWindow.__SEARCH_PAGE_INPUTS__ = [];
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
          case "list_records":
          case "list_reviews":
            return command === "list_records"
              ? { records: [], truncated: false }
              : [];
          case "search_records":
            return {
              records: [
                {
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
                },
              ],
              next: null,
            };
          case "search_provider_page": {
            const input = (
              arguments_ as {
                input?: { provider_id?: string; request?: unknown };
              }
            )?.input;
            browserWindow.__SEARCH_PAGE_INPUTS__?.push(input);
            if (input?.provider_id === "google-books") {
              throw new Error(
                "Google Books Search is temporarily unavailable.",
              );
            }
            return {
              outcome: "page",
              provider_id: "tmdb",
              page: 1,
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
            };
          }
          case "read_search_candidate":
            return {
              outcome: "refetched_without_snapshot",
              candidate_receipt_id: receiptId,
              provider_id: "tmdb",
              grain: "film",
              details: { ...candidate, overview: "Expanded provider details." },
              locale: "en-US",
            };
          case "save_search_candidate": {
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

  await page.goto("/discover");
  const provider = page.getByLabel("Metadata provider");
  await expect(provider).toHaveValue("tmdb");
  await expect(
    page.getByRole("searchbox", { name: "Search TMDB" }),
  ).toBeVisible();
  await provider.selectOption("all");
  await page
    .getByRole("searchbox", { name: "Search your Library and providers" })
    .fill("Dune");
  await page.getByRole("button", { name: "Search", exact: true }).click();

  await expect(page.getByRole("heading", { name: "Local Dune" })).toBeVisible();
  const providerResult = page.getByRole("listitem").filter({
    has: page.getByRole("heading", { name: "Dune: Part Two" }),
  });
  await expect(providerResult).toBeVisible();
  await expect(page.getByRole("alert")).toContainText(
    "Google Books Search is temporarily unavailable.",
  );
  await expect
    .poll(() => page.evaluate(() => window.__SEARCH_PAGE_INPUTS__?.length))
    .toBe(2);

  await providerResult.getByRole("button", { name: "View details" }).click();
  await expect(
    providerResult.getByText("Expanded provider details."),
  ).toBeVisible();
  await providerResult.getByRole("button", { name: "Create Record" }).click();
  await expect(
    providerResult.getByRole("button", { name: "Record ready" }),
  ).toHaveAttribute("aria-disabled", "true");
  await page.screenshot({
    path: ".gstack/qa-reports/screenshots/discover-search-verified.png",
    fullPage: true,
  });
});
