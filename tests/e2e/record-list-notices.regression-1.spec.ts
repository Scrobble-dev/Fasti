import { expect, test, type Page } from "@playwright/test";
import { mockAuthenticatedAccess } from "./test-helpers";

declare global {
  interface Window {
    __RECORD_NOTICE_FIXTURE__?: {
      recordRead: boolean;
      trackingRead: boolean;
    };
  }
}

const record = {
  record_id: "rec_01991f588e0070008000000000000d01",
  grain: "film",
  status: "active",
  title: {
    tier: "fallback_provider_claim",
    value: "Notice boundary Record",
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

async function installRecordNoticeHost(
  page: Page,
  tracking: "truncated" | "failure",
) {
  await mockAuthenticatedAccess(page);
  await page.addInitScript(
    ({ record, tracking }) => {
      const fixture = { recordRead: false, trackingRead: false };
      const browserWindow = window as typeof window & {
        __RECORD_NOTICE_FIXTURE__: typeof fixture;
        __TAURI_INTERNALS__: {
          invoke: (command: string) => Promise<unknown>;
        };
      };
      browserWindow.__RECORD_NOTICE_FIXTURE__ = fixture;
      browserWindow.__TAURI_INTERNALS__ = {
        invoke: async (command) => {
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
              return [];
            case "list_records":
              fixture.recordRead = true;
              return { records: [record], truncated: true };
            case "list_tracking_dispositions":
              fixture.trackingRead = true;
              if (tracking === "failure") {
                throw new Error("Tracking fixture unavailable");
              }
              return { states: [], truncated: true };
            case "list_reviews":
              return [];
            default:
              throw new Error(`Unexpected trusted-host command: ${command}`);
          }
        },
      };
    },
    { record, tracking },
  );
}

async function waitForRecordNoticeFixture(page: Page) {
  await page.waitForFunction(
    () =>
      window.__RECORD_NOTICE_FIXTURE__?.recordRead === true &&
      window.__RECORD_NOTICE_FIXTURE__?.trackingRead === true,
  );
  await page.getByRole("heading", { name: "Notice boundary Record" }).waitFor();
}

// Regression: ISSUE-004 — Record-list truncation overwrote a simultaneous tracking notice.
// Found by /qa on 2026-09-05.
// Report: .gstack/qa-reports/qa-report-fasti-local-2026-09-05.md
test("Library reports simultaneous Record and tracking truncation", async ({
  page,
}, testInfo) => {
  await installRecordNoticeHost(page, "truncated");
  await page.goto("/library");
  await waitForRecordNoticeFixture(page);
  await page.screenshot({
    path: testInfo.outputPath("issue-004-dual-truncation-before.png"),
    fullPage: true,
  });

  await expect(
    page.getByRole("heading", { name: "Notice boundary Record" }),
  ).toBeVisible();
  await expect(
    page.getByText(
      "Only the first 500 records are shown. Additional records remain stored.",
      { exact: false },
    ),
  ).toBeVisible();
  await expect(
    page.getByText(
      "Only the first 500 profile tracking states are shown. Additional states remain stored.",
      { exact: false },
    ),
  ).toBeVisible();
});

test("Library retains tracking-read failure when the Record list is truncated", async ({
  page,
}, testInfo) => {
  await installRecordNoticeHost(page, "failure");
  await page.goto("/library");
  await waitForRecordNoticeFixture(page);
  await page.screenshot({
    path: testInfo.outputPath("issue-004-tracking-failure-before.png"),
    fullPage: true,
  });

  await expect(
    page.getByRole("heading", { name: "Notice boundary Record" }),
  ).toBeVisible();
  await expect(
    page.getByText(
      "Only the first 500 records are shown. Additional records remain stored.",
      { exact: false },
    ),
  ).toBeVisible();
  await expect(
    page.getByText(
      "Could not load profile tracking state. Records still use their activity fallback.",
      { exact: false },
    ),
  ).toBeVisible();
});
