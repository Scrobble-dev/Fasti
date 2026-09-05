import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import {
  expectNoHorizontalOverflow,
  mockAuthenticatedAccess,
} from "./test-helpers";

const recordA = "rec_01991f588e0070008000000000000a01";
const recordB = "rec_01991f588e0070008000000000000a02";
const receiptId = "scr_01991f588e0070008000000000000a01";

declare global {
  interface Window {
    __ATTACH_FIXTURE__: {
      searches: Array<{ query: string; grains: string[]; after?: unknown }>;
      actions: Array<{
        command: string;
        input: {
          request: {
            operation_id: string;
            action: { kind: string; record_id?: string };
            evidence_mode?: string;
          };
        };
      }>;
      failures: number;
      holdAction: boolean;
      holdSearch: boolean;
      wrongGrain: boolean;
      offline: boolean;
      release?: () => void;
    };
  }
}

// Workbench presentation through the existing trusted-host fixture boundary.
// Atomicity, provider reauthorization and replay remain covered by Store/HTTP tests.
async function installAttachHost(page: Page, noStore = false) {
  await mockAuthenticatedAccess(page);
  await page.addInitScript(
    ({ recordA, recordB, receiptId, noStore }) => {
      const fixture: Window["__ATTACH_FIXTURE__"] = {
        searches: [],
        actions: [],
        failures: 0,
        holdAction: false,
        holdSearch: false,
        wrongGrain: false,
        offline: false,
      };
      window.__ATTACH_FIXTURE__ = fixture;
      Object.defineProperty(navigator, "onLine", {
        configurable: true,
        get: () => !fixture.offline,
      });
      const field = (value: string | null) => ({
        value,
        tier: value === null ? "empty" : "preferred_provider_claim",
        source: value === null ? null : "tmdb",
        is_stale: false,
      });
      const records = [recordA, recordB].map((record_id, index) => ({
        record_id,
        grain: "film",
        status: "active",
        title: field(index === 0 ? "Dune local A" : "Dune local B"),
        poster: field(null),
        original_title: field(null),
        overview: field(null),
        release_year: field("2024"),
        identifiers: [],
        latest_activity: null,
        poster_asset_path: null,
      }));
      const candidate = {
        provider: "tmdb",
        provider_id: "693134",
        grain: "film",
        kind: "movie",
        title: "Dune: Part Two",
        original_title: null,
        release_year: 2024,
        authors: [],
        image_url: null,
        overview: "Provider candidate.",
      };
      const receipt = {
        candidate_receipt_id: receiptId,
        grain: "film",
        candidate,
      };
      const lifetime = {
        created_at: "2026-09-05T12:00:00Z",
        fresh_until: "2026-09-05T12:02:00Z",
        stale_until: "2026-09-05T12:10:00Z",
        expires_at: "2026-09-06T12:00:00Z",
      };
      const browserWindow = window as typeof window & {
        __TAURI_INTERNALS__: {
          invoke: (command: string, args?: unknown) => Promise<unknown>;
        };
      };
      browserWindow.__TAURI_INTERNALS__ = {
        invoke: async (command, args) => {
          const input = (args as { input?: any })?.input;
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
              const id = (args as { query?: { record_id?: string } })?.query
                ?.record_id;
              return {
                records: id
                  ? records.filter((record) => record.record_id === id)
                  : [],
                truncated: false,
              };
            }
            case "list_reviews":
              return [];
            case "search_records": {
              fixture.searches.push(input);
              if (!input.grains.length) return { records: [], next: null };
              const wrongGrain = fixture.wrongGrain;
              if (fixture.holdSearch)
                await new Promise<void>((resolve) => {
                  fixture.release = resolve;
                });
              if (wrongGrain)
                return {
                  records: [{ ...records[0], grain: "series" }],
                  next: null,
                };
              return input.after
                ? { records: [records[1]], next: null }
                : {
                    records: [records[0]],
                    next: {
                      last_record_id: recordA,
                      context_digest: "sha256:" + "a".repeat(64),
                    },
                  };
            }
            case "search_provider_page":
              return noStore
                ? {
                    outcome: "live",
                    provider_id: "tmdb",
                    page: 1,
                    candidates: [candidate],
                    next_page: null,
                  }
                : {
                    outcome: "page",
                    provider_id: "tmdb",
                    page: 1,
                    candidates: [receipt],
                    next_page: null,
                    cache_state: fixture.offline
                      ? "stale_on_error"
                      : "observed",
                    lifetime,
                    upstream_problem: null,
                  };
            case "read_search_candidate":
              return {
                outcome: "snapshot",
                snapshot: { receipt, lifetime, locale: null },
              };
            case "save_search_candidate":
            case "save_provider_identifier": {
              fixture.actions.push({ command, input });
              if (fixture.holdAction)
                await new Promise<void>((resolve) => {
                  fixture.release = resolve;
                });
              if (fixture.failures-- > 0)
                throw new Error(
                  "Attachment response interrupted. Retry the same selection.",
                );
              return {
                outcome: "saved",
                receipt: {
                  ...input.request,
                  candidate_receipt_id: receiptId,
                  provider_id: "tmdb",
                  grain: "film",
                  record_id: input.request.action.record_id,
                  disposition: "attached",
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
    },
    { recordA, recordB, receiptId, noStore },
  );
}

async function openPicker(page: Page) {
  await page.goto("/discover");
  await page.getByRole("searchbox", { name: "Search TMDB" }).fill("Dune");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page
    .getByRole("button", { name: "Attach to existing Record", exact: true })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Attach to existing Record",
  });
  await expect(dialog.getByRole("searchbox")).toBeFocused();
  await dialog.getByRole("button", { name: "Find Records" }).click();
  return dialog;
}

for (const noStore of [false, true]) {
  test(`${noStore ? "no-store" : "retained"} Search attaches the explicit target with stable retry and canonical navigation`, async ({
    page,
  }) => {
    await installAttachHost(page, noStore);
    const dialog = await openPicker(page);
    await dialog.getByRole("radio", { name: /Dune local A/ }).check();
    await page.evaluate(() => {
      window.__ATTACH_FIXTURE__.failures = 2;
    });
    await dialog.getByRole("button", { name: "Confirm attachment" }).click();
    await expect(dialog.getByRole("alert")).toContainText(
      "Attachment response interrupted",
    );
    await dialog.getByRole("button", { name: "Confirm attachment" }).click();
    await expect
      .poll(() => page.evaluate(() => window.__ATTACH_FIXTURE__.actions.length))
      .toBe(2);
    await expect(dialog.getByRole("alert")).toContainText(
      "Attachment response interrupted",
    );
    await dialog
      .getByRole("button", { name: "Load more matching Records" })
      .click();
    await expect(dialog.getByRole("radio")).toHaveCount(2);
    await dialog.getByRole("radio", { name: /Dune local B/ }).check();
    await dialog.getByRole("button", { name: "Confirm attachment" }).click();
    await expect(page).toHaveURL(`/records/film/${recordB}/dune-local-b`);
    const fixture = await page.evaluate(() => window.__ATTACH_FIXTURE__);
    expect(fixture.actions.map((action) => action.command)).toEqual(
      Array(3).fill(
        noStore ? "save_provider_identifier" : "save_search_candidate",
      ),
    );
    const requests = fixture.actions.map((action) => action.input.request);
    expect(requests.map((request) => request.action)).toEqual([
      { kind: "attach", record_id: recordA },
      { kind: "attach", record_id: recordA },
      { kind: "attach", record_id: recordB },
    ]);
    expect(requests[0].operation_id).toBe(requests[1].operation_id);
    expect(requests[2].operation_id).not.toBe(requests[1].operation_id);
    expect(fixture.searches.slice(1).map((input) => input.grains)).toEqual([
      ["film"],
      ["film"],
    ]);
    expect(fixture.searches[2].after).toEqual({
      last_record_id: recordA,
      context_digest: "sha256:" + "a".repeat(64),
    });
  });
}

test("Attach is keyboard accessible at 320px, cannot change a submitted target, and cancels safely before submission", async ({
  page,
}, testInfo) => {
  await installAttachHost(page);
  await page.setViewportSize({ width: 320, height: 740 });
  const dialog = await openPicker(page);
  await dialog.getByRole("radio", { name: /Dune local A/ }).check();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await expectNoHorizontalOverflow(page);
  await page.screenshot({
    path: testInfo.outputPath("attach-picker-320.png"),
    fullPage: true,
  });
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
  const opener = page.getByRole("button", {
    name: "Attach to existing Record",
    exact: true,
  });
  await expect(opener).toBeFocused();
  expect(await page.evaluate(() => window.__ATTACH_FIXTURE__.actions)).toEqual(
    [],
  );
  await opener.click();
  await dialog.getByRole("button", { name: "Find Records" }).click();
  await dialog.getByRole("radio", { name: /Dune local A/ }).check();
  await page.evaluate(() => {
    window.__ATTACH_FIXTURE__.holdAction = true;
  });
  await dialog.getByRole("button", { name: "Confirm attachment" }).click();
  await expect
    .poll(() => page.evaluate(() => window.__ATTACH_FIXTURE__.actions.length))
    .toBe(1);
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeDisabled();
  await expect(dialog.getByRole("radio")).toBeDisabled();
  await expect(
    dialog.getByRole("button", { name: "Confirm attachment" }),
  ).toBeDisabled();
  await page.evaluate(() => window.__ATTACH_FIXTURE__.release?.());
  await expect(page).toHaveURL(`/records/film/${recordA}/dune-local-a`);
});

test("Attach rejects wrong-grain targets and discards a closed picker's late search", async ({
  page,
}) => {
  await installAttachHost(page);
  const dialog = await openPicker(page);
  await expect(dialog.getByRole("radio")).toHaveCount(1);
  await page.evaluate(() => {
    window.__ATTACH_FIXTURE__.wrongGrain = true;
  });
  await dialog.getByRole("button", { name: "Find Records" }).click();
  await expect(dialog.getByRole("alert")).toContainText(
    "incompatible identity grain",
  );
  await expect(dialog.getByRole("radio")).toHaveCount(0);
  await expect(
    dialog.getByRole("button", { name: "Confirm attachment" }),
  ).toBeDisabled();
  await page.evaluate(() => {
    window.__ATTACH_FIXTURE__.wrongGrain = false;
    window.__ATTACH_FIXTURE__.holdSearch = true;
  });
  await dialog.getByRole("button", { name: "Find Records" }).click();
  await expect(dialog.getByText("Searching local Records…")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
  await page.evaluate(() => window.__ATTACH_FIXTURE__.release?.());
  await page
    .getByRole("button", { name: "Attach to existing Record", exact: true })
    .click();
  await expect(dialog.getByRole("radio")).toHaveCount(0);
  await expect(dialog.getByRole("alert")).toHaveCount(0);
  expect(await page.evaluate(() => window.__ATTACH_FIXTURE__.actions)).toEqual(
    [],
  );
});

test("a routed candidate supports Attach and late confirmation cannot undo navigation", async ({
  page,
}) => {
  await installAttachHost(page);
  await page.goto(`/explore/tmdb/film/${receiptId}/dune-part-two`);
  await page
    .getByRole("button", { name: "Attach to existing Record", exact: true })
    .click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Find Records" }).click();
  await dialog.getByRole("radio", { name: /Dune local A/ }).check();
  await page.evaluate(() => {
    window.__ATTACH_FIXTURE__.holdAction = true;
  });
  await dialog.getByRole("button", { name: "Confirm attachment" }).click();
  await expect
    .poll(() => page.evaluate(() => window.__ATTACH_FIXTURE__.actions.length))
    .toBe(1);
  await page.evaluate(() => {
    window.history.pushState({}, "", "/library");
    window.dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(dialog).not.toBeVisible();
  await page.evaluate(() => window.__ATTACH_FIXTURE__.release?.());
  await expect(
    page.getByRole("heading", { name: "Library", exact: true }),
  ).toBeVisible();
  await expect(page).toHaveURL("/library");
  await page.getByRole("link", { name: "Discover", exact: true }).click();
  await expect(page).toHaveURL("/discover");
  await expect(page.getByRole("dialog")).not.toBeVisible();
  await expect(page.getByText("Dune local A", { exact: true })).toHaveCount(0);
});

for (const theme of ["light", "dark"]) {
  test(`Attach uses a labelled Tabler dialog with focus containment in ${theme} theme`, async ({
    page,
  }, testInfo) => {
    await installAttachHost(page);
    await page.addInitScript((theme) => {
      localStorage.setItem(
        "fasti-theme-settings",
        JSON.stringify({ mode: theme }),
      );
    }, theme);
    await page.setViewportSize({ width: 1440, height: 900 });
    const dialog = await openPicker(page);
    await dialog.getByRole("radio", { name: /Dune local A/ }).check();
    await dialog.getByRole("button", { name: "Confirm attachment" }).focus();
    await page.keyboard.press("Tab");
    await expect(dialog.getByRole("searchbox")).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(
      dialog.getByRole("button", { name: "Confirm attachment" }),
    ).toBeFocused();
    await expect(page.locator("html")).toHaveAttribute("data-bs-theme", theme);
    expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
    await expectNoHorizontalOverflow(page);
    await page.screenshot({
      path: testInfo.outputPath(`attach-picker-${theme}.png`),
    });
  });
}

test("retained offline evidence is sent to server policy and never switches to no-store", async ({
  page,
}) => {
  await installAttachHost(page);
  await page.goto("/discover");
  await page.evaluate(() => {
    window.__ATTACH_FIXTURE__.offline = true;
  });
  await page.getByRole("searchbox", { name: "Search TMDB" }).fill("Dune");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page
    .getByRole("button", { name: "Attach to existing Record", exact: true })
    .click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Find Records" }).click();
  await dialog.getByRole("radio", { name: /Dune local A/ }).check();
  await dialog.getByRole("button", { name: "Confirm attachment" }).click();
  await expect(page).toHaveURL(`/records/film/${recordA}/dune-local-a`);
  const [action] = await page.evaluate(() => window.__ATTACH_FIXTURE__.actions);
  expect(action.command).toBe("save_search_candidate");
  expect(action.input.request.evidence_mode).toBe("cached");
});
