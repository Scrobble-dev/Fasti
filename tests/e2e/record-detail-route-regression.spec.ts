import { expect, test, type Page } from "@playwright/test";
import type { AccessProjectionResponse } from "@fasti/sdk";
import AxeBuilder from "@axe-core/playwright";
import { expectNoHorizontalOverflow } from "./test-helpers";

// Presentation/transport fixtures only: these do not prove real authorization,
// provider execution, or durable storage. Store/API tests own those gates.
const recordA = "rec_01991f588e0070008000000000000a01";
const recordB = "rec_01991f588e0070008000000000000b01";

interface RouteFixtureWindow extends Window {
  __RECORD_ROUTE_FIXTURE__: {
    calls: Array<string | null>;
    pending: Record<string, () => void>;
    completed: string[];
    hold: string[];
    replacements: string[];
    titles: Record<string, string>;
    finishTracking?: (fail: boolean) => void;
    trackingCompleted: boolean;
    finishTrackingRead?: () => void;
    trackingReadCompleted: boolean;
  };
}

async function trustedRecords(
  page: Page,
  failListing = false,
  hold: string[] = [],
  holdTrackingRead = false,
) {
  await page.addInitScript(
    ({ recordA, recordB, failListing, hold, holdTrackingRead }) => {
      const fixture = {
        calls: [] as Array<string | null>,
        pending: {} as Record<string, () => void>,
        completed: [] as string[],
        hold,
        replacements: [] as string[],
        titles: { [recordA]: "Amélie: The Return", [recordB]: "Second Record" },
        finishTracking: undefined as ((fail: boolean) => void) | undefined,
        trackingCompleted: false,
        finishTrackingRead: undefined as (() => void) | undefined,
        trackingReadCompleted: false,
      };
      const replace = window.history.replaceState.bind(window.history);
      window.history.replaceState = (state, unused, url) => {
        if (url != null) fixture.replacements.push(String(url));
        replace(state, unused, url);
      };
      const makeRecord = (
        record_id: string,
        title: string,
        grain = "film",
      ) => ({
        record_id,
        grain,
        status: "active",
        title: {
          tier: "fallback_provider_claim",
          value: title,
          source: "tmdb",
          is_stale: false,
        },
        poster: { tier: "empty", value: null, source: null, is_stale: false },
        identifiers: [],
        latest_activity: null,
      });
      Object.assign(window, {
        __RECORD_ROUTE_FIXTURE__: fixture,
        __TAURI_INTERNALS__: {
          invoke: async (
            command: string,
            args?: {
              query?: { record_id?: string };
              input?: { record_id: string; disposition: string };
            },
          ) => {
            if (command === "setup_status")
              return { phase: "ready", proof_cleanup_pending: false };
            if (command === "provider_credential_status") return [];
            if (command === "list_tracking_dispositions") {
              if (holdTrackingRead) {
                await new Promise<void>((resolve) => {
                  fixture.finishTrackingRead = resolve;
                });
                fixture.trackingReadCompleted = true;
                return { states: [], truncated: false };
              }
              throw new Error("Tracking fixture unavailable");
            }
            if (command === "set_tracking_disposition") {
              try {
                return await new Promise((resolve, reject) => {
                  fixture.finishTracking = (fail) =>
                    fail
                      ? reject(new Error("Delayed A tracking failure"))
                      : resolve({
                          record_id: args?.input?.record_id,
                          disposition: args?.input?.disposition,
                        });
                });
              } finally {
                fixture.trackingCompleted = true;
              }
            }
            if (command === "list_records") {
              const id = args?.query?.record_id ?? null;
              fixture.calls.push(id);
              const selected =
                id === recordA
                  ? [makeRecord(id, fixture.titles[id])]
                  : id === recordB
                    ? [makeRecord(id, fixture.titles[id], "edition")]
                    : [];
              if (id && fixture.hold.includes(id)) {
                fixture.hold.splice(fixture.hold.indexOf(id), 1);
                await new Promise<void>((resolve) => {
                  fixture.pending[id] = resolve;
                });
                fixture.completed.push(id);
              }
              if (id)
                return {
                  records: selected,
                  truncated: false,
                };
              if (failListing) throw new Error("Library fixture unavailable");
              return {
                records: Array.from({ length: 500 }, (_, index) =>
                  makeRecord(
                    `rec_01991f588e0070008000${index.toString(16).padStart(12, "0")}`,
                    `Library item ${index}`,
                  ),
                ),
                truncated: true,
              };
            }
            throw new Error(`Unavailable fixture command: ${command}`);
          },
        },
      });
    },
    { recordA, recordB, failListing, hold, holdTrackingRead },
  );
}

test("legacy direct detail selects a Record beyond the first 500 and canonicalizes it", async ({
  page,
}) => {
  await trustedRecords(page);
  await page.goto(`/records/${recordA}`);
  try {
    await expect(
      page.getByRole("heading", { level: 1, name: "Amélie: The Return" }),
    ).toBeVisible({ timeout: 5_000 });
    await expect(page).toHaveURL(`/records/film/${recordA}/amelie-the-return`);
    expect(
      await page.evaluate(
        () =>
          (
            window as unknown as {
              __RECORD_ROUTE_FIXTURE__: { calls: Array<string | null> };
            }
          ).__RECORD_ROUTE_FIXTURE__.calls,
      ),
    ).toContain(recordA);
    await page.screenshot({
      path: ".gstack/qa-reports/screenshots/m4-record-detail-after.png",
      fullPage: true,
    });
  } catch (error) {
    await page.screenshot({
      path: ".gstack/qa-reports/screenshots/m4-record-detail-after.png",
      fullPage: true,
    });
    throw error;
  }
});

test("canonical details replace wrong grain and slug with the selected Record identity", async ({
  page,
}) => {
  await trustedRecords(page);
  await page.goto(`/records/edition/${recordA}/old-slug`);
  await expect(
    page.getByRole("heading", { level: 1, name: "Amélie: The Return" }),
  ).toBeVisible();
  const canonical = `/records/film/${recordA}/amelie-the-return`;
  await expect(page).toHaveURL(canonical);
  expect(
    await page.evaluate(
      () =>
        (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.replacements,
    ),
  ).toContain(canonical);
});

test("direct detail does not depend on successful Library or tracking lists", async ({
  page,
}) => {
  await trustedRecords(page, true);
  await page.goto(`/records/edition/${recordB}/second-record`);
  await expect(
    page.getByRole("heading", { level: 1, name: "Second Record" }),
  ).toBeVisible();
  await expect(page).toHaveURL(`/records/edition/${recordB}/second-record`);
  await expect(
    page.getByRole("combobox", { name: "Profile tracking state" }),
  ).toHaveValue("unknown");
});

test("invalid route identifiers never become exact Record queries", async ({
  page,
}) => {
  await trustedRecords(page);
  await page.goto("/records/film/not-a-record/untrusted-title");
  await expect(
    page.getByRole("heading", { level: 1, name: "Amélie: The Return" }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("heading", { level: 1, name: "Second Record" }),
  ).toHaveCount(0);
  // Wait for the real shell to mount before inspecting its transport calls.
  await expect(
    page.getByRole("combobox", { name: "Search records or commands" }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.calls.filter(
        (id) => id !== null,
      ),
    ),
  ).toEqual([]);
});

test("a delayed Record A response cannot replace Record B after navigation", async ({
  page,
}) => {
  await trustedRecords(page, false, [recordA]);
  await page.goto(`/records/film/${recordA}/amelie-the-return`);
  await expect
    .poll(() =>
      page.evaluate(
        (id) =>
          Boolean(
            (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.pending[id],
          ),
        recordA,
      ),
    )
    .toBe(true);
  await page.evaluate((id) => {
    history.pushState({}, "", `/records/edition/${id}/second-record`);
    dispatchEvent(new PopStateEvent("popstate"));
  }, recordB);
  await expect(
    page.getByRole("heading", { level: 1, name: "Second Record" }),
  ).toBeVisible();
  await page.evaluate(
    (id) =>
      (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.pending[id](),
    recordA,
  );
  await expect
    .poll(() =>
      page.evaluate(
        (id) =>
          (
            window as RouteFixtureWindow
          ).__RECORD_ROUTE_FIXTURE__.completed.includes(id),
        recordA,
      ),
    )
    .toBe(true);
  await expect(page).toHaveURL(`/records/edition/${recordB}/second-record`);
  await expect(
    page.getByRole("heading", { level: 1, name: "Second Record" }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { level: 1, name: "Amélie: The Return" }),
  ).toHaveCount(0);
});

function browserProjection(profile: number): AccessProjectionResponse {
  const id = (prefix: string, value = 0) =>
    `${prefix}_018f0e0e7f7b70008000${value.toString(16).padStart(12, "0")}`;
  const at = "2026-08-31T12:00:00Z";
  const current = {
    browser_session_id: id("ses"),
    workspace_id: id("wsp"),
    selected_profile_grant_id: id("grt", profile),
    is_current: true,
    created_at: at,
    last_seen_at: at,
    idle_expires_at: "2099-08-31T12:31:00Z",
    absolute_expires_at: "2099-08-31T20:00:00Z",
    rotation_generation: 1,
  };
  return {
    generated_at: at,
    subject: {
      auth_subject_id: id("sub"),
      lifecycle: "active",
      created_at: at,
      updated_at: at,
    },
    membership: {
      membership_id: id("mem"),
      workspace_id: id("wsp"),
      lifecycle: "active",
      role: "administrator",
      created_at: at,
      updated_at: at,
    },
    current_session: current,
    sessions: [current],
    sessions_truncated: false,
    profile_grants: [
      {
        profile_grant_id: id("grt", profile),
        profile_id: id("prf", profile),
        owner_client_id: id("cli"),
        selected: true,
      },
    ],
    profile_grants_truncated: false,
    session_policy: {
      idle_timeout_seconds: 1800,
      browser_lifetime_seconds: 28800,
      remembered_browser_lifetime_seconds: 2592000,
      last_seen_write_interval_seconds: 60,
    },
    authentication: {
      method: "trail_base_password",
      verified_at: at,
      activation_generation: 1,
      recent_authentication: { state: "unavailable", expires_at: null },
    },
    trailbase: {
      state: "active",
      blocker: null,
      trailbase_instance_id: id("tbi"),
      generation: 1,
      session_generation_current: true,
      updated_at: at,
    },
    first_run_steps: [],
    evidence: [],
    evidence_truncated: false,
  };
}

test("A to B to A keeps the newest A response instead of the delayed first visit", async ({
  page,
}) => {
  await trustedRecords(page, false, [recordA]);
  await page.goto(`/records/${recordA}`);
  await expect
    .poll(() =>
      page.evaluate(
        (id) =>
          Boolean(
            (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.pending[id],
          ),
        recordA,
      ),
    )
    .toBe(true);
  await page.evaluate((id) => {
    history.pushState({}, "", `/records/${id}`);
    dispatchEvent(new PopStateEvent("popstate"));
  }, recordB);
  await expect(
    page.getByRole("heading", { level: 1, name: "Second Record" }),
  ).toBeVisible();
  await page.evaluate((id) => {
    (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.titles[id] =
      "Newest A title";
    history.back();
  }, recordA);
  await expect(
    page.getByRole("heading", { level: 1, name: "Newest A title" }),
  ).toBeVisible();
  await page.evaluate(
    (id) =>
      (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.pending[id](),
    recordA,
  );
  await expect
    .poll(() =>
      page.evaluate(
        (id) =>
          (
            window as RouteFixtureWindow
          ).__RECORD_ROUTE_FIXTURE__.completed.includes(id),
        recordA,
      ),
    )
    .toBe(true);
  await expect(
    page.getByRole("heading", { level: 1, name: "Amélie: The Return" }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("heading", { level: 1, name: "Newest A title" }),
  ).toBeVisible();
  await expect(page).toHaveURL(`/records/film/${recordA}/newest-a-title`);
});

test("an absent exact Record exposes recovery without borrowing a Library row", async ({
  page,
}) => {
  await trustedRecords(page);
  const absent = "rec_01991f588e0070008000000000000c01";
  await page.goto(`/records/film/${absent}/not-present`);
  await expect(
    page.getByRole("heading", { level: 1, name: "Media Detail" }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toContainText(
    "This Record is not available in the current workspace.",
  );
  await expect(
    page.getByRole("button", { name: "Retry Record", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { level: 1, name: /Library item/ }),
  ).toHaveCount(0);
  expect(
    await page.evaluate(
      () => (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.calls,
    ),
  ).toContain(absent);
  await page.getByRole("button", { name: "Retry Record", exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(
        (id) =>
          (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.calls.filter(
            (value) => value === id,
          ).length,
        absent,
      ),
    )
    .toBe(2);
  await expect(
    page.getByRole("button", { name: "Retry Record", exact: true }),
  ).toBeFocused();
});

test("returning from exact detail preserves the bounded Library listing", async ({
  page,
}) => {
  await trustedRecords(page);
  await page.goto(`/records/${recordA}`);
  await expect(
    page.getByRole("heading", { level: 1, name: "Amélie: The Return" }),
  ).toBeVisible();
  await page.getByRole("link", { name: "Library", exact: true }).click();
  await expect(
    page.getByRole("heading", { level: 1, name: "Library" }),
  ).toBeVisible();
  await expect(
    page.getByText(
      "Only the first 500 records are shown. Additional records remain stored.",
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", {
      level: 2,
      name: "Library item 499",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { level: 1, name: "Amélie: The Return" }),
  ).toHaveCount(0);
  expect(
    await page.evaluate(
      () => (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.calls,
    ),
  ).toContain(null);
});

test("extra canonical URL segments are rejected before exact lookup", async ({
  page,
}) => {
  await trustedRecords(page);
  await page.goto(`/records/film/${recordA}/title/extra`);
  await expect(
    page.getByRole("heading", { level: 1, name: "Media Detail" }),
  ).toBeVisible();
  await expect(
    page.getByRole("combobox", { name: "Search records or commands" }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.calls.filter(
        (id) => id !== null,
      ),
    ),
  ).toEqual([]);
  await expect(
    page.getByRole("heading", { level: 1, name: "Amélie: The Return" }),
  ).toHaveCount(0);
});

for (const fail of [false, true]) {
  test(`late tracking ${fail ? "failure" : "success"} on A cannot show feedback on B`, async ({
    page,
  }) => {
    await trustedRecords(page);
    await page.goto(`/records/${recordA}`);
    await expect(
      page.getByRole("heading", { level: 1, name: "Amélie: The Return" }),
    ).toBeVisible();
    await page
      .getByRole("combobox", { name: "Profile tracking state" })
      .selectOption("dropped");
    await expect
      .poll(() =>
        page.evaluate(() =>
          Boolean(
            (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__
              .finishTracking,
          ),
        ),
      )
      .toBe(true);
    await page.evaluate((id) => {
      history.pushState({}, "", `/records/${id}`);
      dispatchEvent(new PopStateEvent("popstate"));
    }, recordB);
    await expect(
      page.getByRole("heading", { level: 1, name: "Second Record" }),
    ).toBeVisible();
    await page.evaluate(
      (fail) =>
        (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.finishTracking!(
          fail,
        ),
      fail,
    );
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__
              .trackingCompleted,
        ),
      )
      .toBe(true);
    await expect(
      page.getByText("Tracking state set to dropped.", { exact: true }),
    ).toHaveCount(0);
    await expect(
      page.getByText("Delayed A tracking failure", { exact: true }),
    ).toHaveCount(0);
    await expect(
      page.getByRole("combobox", { name: "Profile tracking state" }),
    ).toHaveValue("unknown");
    await expect(page).toHaveURL(`/records/edition/${recordB}/second-record`);
  });
}

test("a delayed initial tracking read cannot replace a confirmed tracking choice", async ({
  page,
}) => {
  await trustedRecords(page, false, [], true);
  await page.goto(`/records/${recordA}`);
  await expect(
    page.getByRole("heading", { level: 1, name: "Amélie: The Return" }),
  ).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(() =>
        Boolean(
          (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__
            .finishTrackingRead,
        ),
      ),
    )
    .toBe(true);
  const tracking = page.getByRole("combobox", {
    name: "Profile tracking state",
  });
  await tracking.selectOption("on_hold");
  await expect
    .poll(() =>
      page.evaluate(() =>
        Boolean(
          (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__
            .finishTracking,
        ),
      ),
    )
    .toBe(true);
  await page.evaluate(() =>
    (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.finishTracking!(
      false,
    ),
  );
  await expect(
    page.getByText("Tracking state set to on hold.", { exact: true }),
  ).toBeVisible();
  await expect(tracking).toHaveValue("on_hold");
  await page.evaluate(async () => {
    (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__
      .finishTrackingRead!();
    // Let the old read and its UI update settle before checking the newer choice.
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
  });
  await expect(tracking).toHaveValue("on_hold");
});

test("a title with no ASCII words uses the stable fallback slug", async ({
  page,
}) => {
  await trustedRecords(page);
  await page.addInitScript((id) => {
    (window as RouteFixtureWindow).__RECORD_ROUTE_FIXTURE__.titles[id] = "東京";
  }, recordA);
  await page.goto(`/records/${recordA}`);
  await expect(
    page.getByRole("heading", { level: 1, name: "東京" }),
  ).toBeVisible();
  await expect(page).toHaveURL(`/records/film/${recordA}/record`);
});

test("canonical details remain readable and accessible in a narrow dark viewport", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
  await trustedRecords(page, true);
  await page.addInitScript(() => localStorage.setItem("fasti-theme", "dark"));
  await page.goto(`/records/film/${recordA}/amelie-the-return`);
  await expect(
    page.getByRole("heading", { level: 1, name: "Amélie: The Return" }),
  ).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "dark");
  await expectNoHorizontalOverflow(page);
  const tabs = page.locator(".content-tabs .tab-btn");
  for (const tab of await tabs.all()) {
    expect(
      await tab.evaluate(
        (element) => element.scrollWidth <= element.clientWidth + 1,
      ),
    ).toBe(true);
  }
  await tabs.nth((await tabs.count()) - 2).focus();
  await page.keyboard.press("Tab");
  await expect(tabs.last()).toBeFocused();
  await expect
    .poll(() =>
      tabs.last().evaluate((element) => {
        const tab = element.getBoundingClientRect();
        const owner = element.closest(".content-tabs")!;
        const container = owner.getBoundingClientRect();
        return JSON.stringify({
          visible:
            tab.left >= container.left - 1 && tab.right <= container.right + 1,
          tab: { left: tab.left, right: tab.right, width: tab.width },
          container: { left: container.left, right: container.right },
          clientWidth: owner.clientWidth,
          scrollLeft: owner.scrollLeft,
          scrollWidth: owner.scrollWidth,
        });
      }),
    )
    .toContain('"visible":true');
  await page.getByRole("combobox", { name: "Profile tracking state" }).focus();
  await expect(
    page.getByRole("combobox", { name: "Profile tracking state" }),
  ).toBeFocused();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await page.evaluate(() => {
    (document.activeElement as HTMLElement | null)?.blur();
    document.querySelector(".content-tabs")!.scrollLeft = 0;
    window.scrollTo(0, 0);
  });
  await page.screenshot({
    path: ".gstack/qa-reports/screenshots/m4-record-detail-mobile-dark.png",
    fullPage: false,
  });
});

async function profileFixture(page: Page, heldProfile: number) {
  let profile = 1;
  let release!: () => void;
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  const calls: number[] = [];
  let released = false;
  await page.route("**/api/access/v1/projection", (route) =>
    route.fulfill({ json: browserProjection(profile) }),
  );
  await page.route("**/api/v1/records**", async (route) => {
    const url = new URL(route.request().url());
    if (url.pathname !== "/api/v1/records") return route.continue();
    const selected = url.searchParams.get("record_id");
    if (!selected)
      return route.fulfill({ json: { records: [], truncated: false } });
    const requestedProfile = profile;
    calls.push(requestedProfile);
    if (requestedProfile === heldProfile) await held;
    const result = {
      records: [
        {
          record_id: selected,
          grain: "film",
          status: "active",
          title: {
            tier: "profile_override",
            value: `Profile ${requestedProfile} title`,
            source: null,
            is_stale: false,
          },
          poster: { tier: "empty", value: null, source: null, is_stale: false },
          identifiers: [],
          latest_activity: null,
        },
      ],
      truncated: false,
    };
    await route.fulfill({ json: result }).catch(() => undefined);
    if (requestedProfile === heldProfile) released = true;
  });
  return {
    calls,
    changeProfile: () => {
      profile = 2;
    },
    release,
    released: () => released,
  };
}

test("profile refresh preserves detail URL intent and clears old profile data while reloading", async ({
  page,
}) => {
  const fixture = await profileFixture(page, 2);
  try {
    await page.goto(`/records/film/${recordA}/profile-1-title`);
    await expect(
      page.getByRole("heading", { level: 1, name: "Profile 1 title" }),
    ).toBeVisible();
    fixture.changeProfile();
    await page.evaluate(() => dispatchEvent(new Event("focus")));
    await expect.poll(() => fixture.calls.includes(2)).toBe(true);
    await expect(page).toHaveURL(new RegExp(`/records/film/${recordA}/`));
    await expect(
      page.getByRole("heading", { level: 1, name: "Profile 1 title" }),
    ).toHaveCount(0);
    fixture.release();
    await expect(
      page.getByRole("heading", { level: 1, name: "Profile 2 title" }),
    ).toBeVisible();
    await expect(page).toHaveURL(`/records/film/${recordA}/profile-2-title`);
  } finally {
    fixture.release();
  }
});

test("delayed prior-profile detail cannot overwrite the current profile result", async ({
  page,
}) => {
  const fixture = await profileFixture(page, 1);
  try {
    await page.goto(`/records/film/${recordA}/requested-title`);
    await expect.poll(() => fixture.calls.includes(1)).toBe(true);
    fixture.changeProfile();
    await page.evaluate(() => dispatchEvent(new Event("focus")));
    await expect(
      page.getByRole("heading", { level: 1, name: "Profile 2 title" }),
    ).toBeVisible();
    fixture.release();
    await expect.poll(fixture.released).toBe(true);
    await expect(
      page.getByRole("heading", { level: 1, name: "Profile 2 title" }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { level: 1, name: "Profile 1 title" }),
    ).toHaveCount(0);
    await expect(page).toHaveURL(`/records/film/${recordA}/profile-2-title`);
  } finally {
    fixture.release();
  }
});
