import { expect, test, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import {
  expectNoHorizontalOverflow,
  mockAuthenticatedAccess,
} from "./test-helpers";

declare global {
  interface Window {
    __RECORD_NOTICE_FIXTURE__?: {
      recordRead: boolean;
      trackingRead: boolean;
      releaseRecords?: () => void;
      trackingWrites: string[];
      releaseTrackingWrite?: () => void;
      trackingWriteCompleted: boolean;
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
  holdGlobalRecords = false,
) {
  await mockAuthenticatedAccess(page);
  await page.addInitScript(
    ({ record, tracking, holdGlobalRecords }) => {
      const fixture: NonNullable<Window["__RECORD_NOTICE_FIXTURE__"]> = {
        recordRead: false,
        trackingRead: false,
        trackingWrites: [],
        trackingWriteCompleted: false,
      };
      const browserWindow = window as typeof window & {
        __RECORD_NOTICE_FIXTURE__: typeof fixture;
        __TAURI_INTERNALS__: {
          invoke: (
            command: string,
            args?: {
              query?: { record_id?: string };
              input?: { record_id: string; disposition: string };
            },
          ) => Promise<unknown>;
        };
      };
      browserWindow.__RECORD_NOTICE_FIXTURE__ = fixture;
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
              return [];
            case "list_records":
              fixture.recordRead = true;
              if (holdGlobalRecords && !args?.query?.record_id) {
                await new Promise<void>((resolve) => {
                  fixture.releaseRecords = resolve;
                });
              }
              return { records: [record], truncated: !args?.query?.record_id };
            case "list_tracking_dispositions":
              fixture.trackingRead = true;
              if (tracking === "failure") {
                throw new Error("Tracking fixture unavailable");
              }
              return { states: [], truncated: true };
            case "set_tracking_disposition":
              if (!args?.input) throw new Error("Missing tracking input");
              fixture.trackingWrites.push(args.input.disposition);
              if (fixture.trackingWrites.length === 2) {
                await new Promise<void>((resolve) => {
                  fixture.releaseTrackingWrite = resolve;
                });
                fixture.trackingWriteCompleted = true;
              }
              return {
                record_id: args.input.record_id,
                disposition:
                  args.input.disposition === "unset"
                    ? null
                    : args.input.disposition,
              };
            case "list_reviews":
              return [];
            default:
              throw new Error(`Unexpected trusted-host command: ${command}`);
          }
        },
      };
    },
    { record, tracking, holdGlobalRecords },
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
// Report: .gstack/qa-reports/qa-report-fasti-m4-review-2026-09-05.md
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

test("simultaneous notices remain separate and accessible on narrow screens", async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 320, height: 812 });
  await installRecordNoticeHost(page, "truncated");
  await page.goto("/library");
  await waitForRecordNoticeFixture(page);
  const notices = page.locator(".alert[role='status'] p");
  await expect(notices).toHaveCount(2);
  const first = await notices.nth(0).boundingBox();
  const second = await notices.nth(1).boundingBox();
  expect(first).not.toBeNull();
  expect(second).not.toBeNull();
  expect(second!.y).toBeGreaterThanOrEqual(first!.y + first!.height);
  await expectNoHorizontalOverflow(page);
  await expect(page.getByRole("radio")).toHaveCount(10);
  for (const radio of await page.getByRole("radio").all()) {
    await expect(radio).toBeVisible();
  }
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await page.screenshot({
    path: testInfo.outputPath("issue-004-narrow-notices.png"),
    fullPage: true,
  });
});

for (const tracking of ["truncated", "failure"] as const) {
  test(`a completed tracking mutation discards an older ${tracking} notice`, async ({
    page,
  }) => {
    await installRecordNoticeHost(page, tracking, true);
    await page.goto(`/records/${record.record_id}`);
    await expect(
      page.getByRole("heading", {
        name: "Notice boundary Record",
        exact: true,
      }),
    ).toBeVisible();
    await page.waitForFunction(
      () =>
        window.__RECORD_NOTICE_FIXTURE__?.trackingRead &&
        typeof window.__RECORD_NOTICE_FIXTURE__?.releaseRecords === "function",
    );
    const selector = page.getByRole("combobox", {
      name: "Profile tracking state",
    });
    await selector.selectOption("on_hold");
    await expect(
      page.getByText("Tracking state set to on hold.", { exact: true }),
    ).toBeVisible();
    await page.evaluate(() =>
      window.__RECORD_NOTICE_FIXTURE__?.releaseRecords?.(),
    );
    await selector.selectOption("dropped");
    await page.waitForFunction(
      () =>
        typeof window.__RECORD_NOTICE_FIXTURE__?.releaseTrackingWrite ===
        "function",
    );
    try {
      await page.getByRole("button", { name: "Back to Library" }).click();
      await expect(
        page.getByText(
          "Only the first 500 records are shown. Additional records remain stored.",
          { exact: true },
        ),
      ).toBeVisible();
      await expect(
        page.getByText(
          "Only the first 500 profile tracking states are shown. Additional states remain stored.",
          { exact: true },
        ),
      ).toHaveCount(0);
      await expect(
        page.getByText("Could not load profile tracking state.", {
          exact: false,
        }),
      ).toHaveCount(0);
    } finally {
      await page.evaluate(() =>
        window.__RECORD_NOTICE_FIXTURE__?.releaseTrackingWrite?.(),
      );
      await page.waitForFunction(
        () => window.__RECORD_NOTICE_FIXTURE__?.trackingWriteCompleted,
      );
    }
  });
}
