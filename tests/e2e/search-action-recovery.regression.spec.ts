import { expect, test, type Page } from "@playwright/test";
import { mockAuthenticatedAccess } from "./test-helpers";

const targetId = "rec_01991f588e0070008000000000000a01";
const createdId = "rec_01991f588e0070008000000000000a02";

interface RecoveryAction {
  command: string;
  input: {
    provider_id: string;
    grain: string;
    candidate_receipt_id?: string;
    request: {
      operation_id: string;
      provider_record_id?: string;
      evidence_mode?: string;
      action: { kind: string; record_id?: string };
    };
  };
}

declare global {
  interface Window {
    __ACTION_RECOVERY__: {
      actions: RecoveryAction[];
      fail: boolean;
      holdRecords: boolean;
      failRecords: boolean;
      recordRefreshStarted: number;
      recordRefreshFailed: number;
      releaseRecords?: () => void;
    };
  }
}

// Simulates a lost host response, not a Store transaction. Full request equality
// proves the UI preserves intent; Store tests own committed-action replay.
async function installRecoveryHost(page: Page, retained: boolean) {
  await mockAuthenticatedAccess(page);
  await page.addInitScript(
    ({ retained, targetId, createdId }) => {
      const fixture: Window["__ACTION_RECOVERY__"] = {
        actions: [],
        fail: true,
        holdRecords: false,
        failRecords: false,
        recordRefreshStarted: 0,
        recordRefreshFailed: 0,
      };
      window.__ACTION_RECOVERY__ = fixture;
      const field = (value: string | null) => ({
        value,
        tier: value === null ? "empty" : "preferred_provider_claim",
        source: value === null ? null : "tmdb",
        is_stale: false,
      });
      const record = (record_id: string) => ({
        record_id,
        grain: "film",
        status: "active",
        title: field(
          record_id === targetId
            ? "Exact attachment target"
            : "Confirmed creation",
        ),
        poster: field(null),
        original_title: field(null),
        overview: field(null),
        release_year: field("2024"),
        identifiers: [],
        latest_activity: null,
        poster_asset_path: null,
      });
      const lifetime = {
        created_at: "2026-09-08T12:00:00Z",
        fresh_until: "2026-09-08T12:02:00Z",
        stale_until: "2026-09-08T12:10:00Z",
        expires_at: "2026-09-09T12:00:00Z",
      };
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
              if (fixture.actions.length > 0 && !fixture.fail) {
                fixture.recordRefreshStarted += 1;
                if (fixture.holdRecords) {
                  fixture.holdRecords = false;
                  await new Promise<void>((resolve) => {
                    fixture.releaseRecords = resolve;
                  });
                }
                if (fixture.failRecords) {
                  fixture.recordRefreshFailed += 1;
                  throw new Error(
                    "Record list refresh unavailable after confirmed save.",
                  );
                }
              }
              const id = (args as { query?: { record_id?: string } })?.query
                ?.record_id;
              return { records: id ? [record(id)] : [], truncated: false };
            }
            case "list_reviews":
              return [];
            case "search_records": {
              const input = (args as { input: { grains: string[] } }).input;
              return {
                records: input.grains.includes("film")
                  ? [record(targetId)]
                  : [],
                next: null,
              };
            }
            case "search_provider_page": {
              const { request } = (
                args as { input: { request: { query: string; page: number } } }
              ).input;
              const batch = /^Batch ([0-4])$/.exec(request.query);
              const base = batch
                ? Number(batch[1]) * 100
                : request.query === "Changed"
                  ? 9000
                  : 1000;
              const candidates = Array.from(
                { length: batch && Number(batch[1]) < 4 ? 100 : 1 },
                (_, index) => {
                  const number = base + index + 1;
                  const candidate = {
                    provider: "tmdb",
                    provider_id: String(number),
                    grain: "film",
                    kind: "movie",
                    title: `${request.query} candidate ${number}`,
                    original_title: null,
                    release_year: 2024,
                    authors: [],
                    image_url: null,
                    overview: null,
                  };
                  return retained
                    ? {
                        candidate_receipt_id: `scr_01991f588e0070008000${number.toString(16).padStart(12, "0")}`,
                        grain: "film",
                        candidate,
                      }
                    : candidate;
                },
              );
              return {
                outcome: retained ? "page" : "live",
                provider_id: "tmdb",
                page: request.page,
                candidates,
                next_page: null,
                ...(retained
                  ? {
                      cache_state: "observed",
                      lifetime,
                      upstream_problem: null,
                    }
                  : {}),
              };
            }
            case "save_search_candidate":
            case "save_provider_identifier": {
              const input = (args as { input: RecoveryAction["input"] }).input;
              fixture.actions.push(structuredClone({ command, input }));
              if (fixture.fail)
                throw new Error("Record action response lost after dispatch.");
              return {
                outcome: "saved",
                receipt: {
                  ...input.request,
                  provider_id: input.provider_id,
                  grain: input.grain,
                  candidate_receipt_id: input.candidate_receipt_id,
                  record_id: input.request.action.record_id ?? createdId,
                  disposition:
                    input.request.action.kind === "attach"
                      ? "attached"
                      : "created",
                  fetched_at: lifetime.created_at,
                  expires_at: lifetime.expires_at,
                  initial_status: "fresh",
                  committed_at: "2026-09-08T12:00:01Z",
                },
              };
            }
            default:
              throw new Error(`Unexpected trusted-host command: ${command}`);
          }
        },
      };
    },
    { retained, targetId, createdId },
  );
}

async function search(page: Page, query: string) {
  await page.locator("#provider-search").fill(query);
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Search", exact: true }),
  ).toBeEnabled();
  await expect(
    page
      .getByRole("heading", { name: new RegExp(`^${query} candidate`) })
      .first(),
  ).toBeVisible();
}

async function expandRecovery(page: Page, count: number) {
  const recovery = page.locator("#unconfirmed-record-actions");
  await expect(recovery.locator("summary")).toHaveText(
    `Unconfirmed Record actions (${count})`,
  );
  if (
    !(await recovery.evaluate(
      (element) => (element as HTMLDetailsElement).open,
    ))
  )
    await recovery.locator("summary").click();
  return recovery;
}

for (const retained of [false, true]) {
  test(`${retained ? "retained Attach" : "live Create"} recovers the complete original intent after query replacement`, async ({
    page,
  }) => {
    await installRecoveryHost(page, retained);
    await page.goto("/discover");
    await search(page, "Original");
    if (retained) {
      await page
        .getByRole("button", { name: "Attach to existing Record", exact: true })
        .click();
      const dialog = page.getByRole("dialog", {
        name: "Attach to existing Record",
      });
      await dialog
        .getByRole("button", { name: "Find Records", exact: true })
        .click();
      await dialog
        .getByRole("radio", { name: /Exact attachment target/ })
        .check();
      await dialog
        .getByRole("button", { name: "Confirm attachment", exact: true })
        .click();
      await expect(dialog.getByRole("alert")).toContainText(
        "Record action response lost",
      );
      await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    } else {
      await page
        .getByRole("button", { name: "Create Record", exact: true })
        .click();
      await expect(page.getByRole("alert")).toContainText(
        "Record action response lost",
      );
    }
    const original = await page.evaluate(
      () => window.__ACTION_RECOVERY__.actions[0],
    );
    expect(original.command).toBe(
      retained ? "save_search_candidate" : "save_provider_identifier",
    );
    expect(original.input.request.action).toEqual(
      retained ? { kind: "attach", record_id: targetId } : { kind: "create" },
    );
    expect(original.input.request.operation_id).toBeTruthy();
    if (retained) {
      expect(original.input.request.evidence_mode).toBe("refetch");
      expect(original.input.candidate_receipt_id).toBe(
        "scr_01991f588e00700080000000000003e9",
      );
    } else {
      expect(original.input.request.provider_record_id).toBe("1001");
    }
    await search(page, "Changed");
    await expect(
      page.getByRole("heading", { name: /^Original candidate/ }),
    ).toHaveCount(0);
    const recovery = await expandRecovery(page, 1);
    await expect(recovery).toContainText("tmdb");
    await expect(recovery).toContainText(
      retained ? original.input.candidate_receipt_id! : "1001",
    );
    await expect(recovery).toContainText(retained ? "Attach" : "Create");
    if (retained) await expect(recovery).toContainText(targetId);
    await page.evaluate(() => {
      window.__ACTION_RECOVERY__.fail = false;
    });
    await recovery
      .getByRole("button", { name: "Retry original action", exact: true })
      .click();
    await expect(
      page.getByRole("button", { name: "Open confirmed Record", exact: true }),
    ).toBeVisible();
    const actions = await page.evaluate(
      () => window.__ACTION_RECOVERY__.actions,
    );
    expect(actions).toHaveLength(2);
    expect(actions[1]).toEqual(original);
    await expect(recovery).toContainText(retained ? targetId : createdId);
    await expect(
      recovery.getByRole("button", {
        name: "Retry original action",
        exact: true,
      }),
    ).toHaveCount(0);
    await page
      .getByRole("button", { name: "Open confirmed Record", exact: true })
      .click();
    await expect(page).toHaveURL(new RegExp(retained ? targetId : createdId));
  });
}

test("a recovery awaiting Record refresh keeps the original visible row from redispatching", async ({
  page,
}) => {
  await installRecoveryHost(page, false);
  await page.goto("/discover");
  await search(page, "Original");
  const create = page.getByRole("button", {
    name: "Create Record",
    exact: true,
  });
  await create.click();
  await expect(page.getByRole("alert")).toContainText(
    "Record action response lost",
  );
  const recovery = await expandRecovery(page, 1);
  await page.evaluate(() => {
    window.__ACTION_RECOVERY__.fail = false;
    window.__ACTION_RECOVERY__.holdRecords = true;
  });
  await recovery
    .getByRole("button", { name: "Retry original action", exact: true })
    .click();
  await expect
    .poll(() =>
      page.evaluate(() => window.__ACTION_RECOVERY__.recordRefreshStarted),
    )
    .toBe(1);
  await create.click();
  await expect(page.getByRole("alert")).toContainText(
    "already waiting for confirmation",
  );
  const actions = await page.evaluate(() => window.__ACTION_RECOVERY__.actions);
  expect(actions).toHaveLength(2);
  expect(actions[1]).toEqual(actions[0]);
  await expect(
    recovery.getByRole("button", {
      name: "Waiting for confirmation…",
      exact: true,
    }),
  ).toBeDisabled();
  await page.evaluate(() => window.__ACTION_RECOVERY__.releaseRecords?.());
  await expect(
    recovery.getByRole("button", {
      name: "Open confirmed Record",
      exact: true,
    }),
  ).toBeVisible();
  expect(
    await page.evaluate(() => window.__ACTION_RECOVERY__.actions.length),
  ).toBe(2);
});

test("a confirmed recovery is not re-enqueued when its Record list refresh fails", async ({
  page,
}) => {
  await installRecoveryHost(page, false);
  await page.goto("/discover");
  await search(page, "Original");
  await page
    .getByRole("button", { name: "Create Record", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText(
    "Record action response lost",
  );
  const recovery = await expandRecovery(page, 1);
  await page.evaluate(() => {
    window.__ACTION_RECOVERY__.fail = false;
    window.__ACTION_RECOVERY__.failRecords = true;
  });
  await recovery
    .getByRole("button", { name: "Retry original action", exact: true })
    .click();
  await expect
    .poll(() =>
      page.evaluate(() => window.__ACTION_RECOVERY__.recordRefreshFailed),
    )
    .toBe(1);
  await expect(
    recovery.getByRole("button", {
      name: "Open confirmed Record",
      exact: true,
    }),
  ).toBeVisible();
  await expect(recovery.locator("summary")).toHaveText(
    "Unconfirmed Record actions (0)",
  );
  await expect(
    recovery.getByRole("button", {
      name: "Retry original action",
      exact: true,
    }),
  ).toHaveCount(0);
  const actions = await page.evaluate(() => window.__ACTION_RECOVERY__.actions);
  expect(actions).toHaveLength(2);
  expect(actions[1]).toEqual(actions[0]);
});

test("400 unresolved intents reject only new dispatches; retry confirmation frees a slot", async ({
  page,
}) => {
  test.setTimeout(90_000);
  await installRecoveryHost(page, false);
  await page.goto("/discover");
  for (let batch = 0; batch < 4; batch += 1) {
    await search(page, `Batch ${batch}`);
    // Drive actual enabled DOM controls, not component state or host methods.
    // One browser call per batch avoids 400 remote click round trips.
    await page.evaluate(async (batch) => {
      const buttons = Array.from(
        document.querySelectorAll<HTMLButtonElement>(".results button"),
      ).filter((button) => button.textContent?.trim() === "Create Record");
      if (buttons.length !== 100)
        throw new Error(
          `Expected 100 real action controls, got ${buttons.length}`,
        );
      for (let index = 0; index < buttons.length; index += 1) {
        if (buttons[index].disabled)
          throw new Error("Action control unexpectedly disabled");
        buttons[index].click();
        const expected = batch * 100 + index + 1;
        const deadline = performance.now() + 3000;
        while (
          window.__ACTION_RECOVERY__.actions.length !== expected ||
          buttons[index].disabled ||
          document
            .querySelector("#unconfirmed-record-actions summary")
            ?.textContent?.replace(/\s+/g, " ")
            .trim() !== `Unconfirmed Record actions (${expected})`
        ) {
          if (performance.now() > deadline)
            throw new Error(`Action ${expected} did not settle`);
          await new Promise<void>((resolve) =>
            requestAnimationFrame(() => resolve()),
          );
        }
      }
    }, batch);
  }
  await search(page, "Batch 4");
  await page
    .getByRole("button", { name: "Create Record", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText(/unconfirmed|retry|400/i);
  expect(
    await page.evaluate(() => window.__ACTION_RECOVERY__.actions.length),
  ).toBe(400);
  const recovery = await expandRecovery(page, 400);
  const original = await page.evaluate(
    () => window.__ACTION_RECOVERY__.actions[0],
  );
  await page.evaluate(() => {
    window.__ACTION_RECOVERY__.fail = false;
  });
  await recovery
    .getByRole("button", { name: "Retry original action", exact: true })
    .first()
    .click();
  await expect(
    page.getByRole("button", { name: "Open confirmed Record", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() => window.__ACTION_RECOVERY__.actions[400]),
  ).toEqual(original);
  await expandRecovery(page, 399);
  await page
    .getByRole("button", { name: "Create Record", exact: true })
    .click();
  await expect
    .poll(() => page.evaluate(() => window.__ACTION_RECOVERY__.actions.length))
    .toBe(402);
  const admitted = await page.evaluate(
    () => window.__ACTION_RECOVERY__.actions[401],
  );
  expect(admitted.input.request.provider_record_id).toBe("401");
  expect(admitted.input.request.operation_id).not.toBe(
    original.input.request.operation_id,
  );
});
