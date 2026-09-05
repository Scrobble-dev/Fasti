import {
  PUBLIC_PROBLEM_CATALOG,
  type AccessProjectionResponse,
  type ProblemDetails,
} from "@fasti/sdk";
import { expect, test, type Page, type Route } from "@playwright/test";
import { mockAuthenticatedAccess } from "./test-helpers";

const csrf = "a".repeat(64);
const recordA = "rec_01991f588e0070008000000000000b01";
const recordB = "rec_01991f588e0070008000000000000b02";
const receiptId = "scr_01991f588e0070008000000000000b01";
const stubProjectionUrl = "http://127.0.0.1:18422/api/access/v1/projection";

const field = (value: string | null) => ({
  value,
  tier: value === null ? "empty" : "preferred_provider_claim",
  source: value === null ? null : "tmdb",
  is_stale: false,
});

const localRecord = {
  record_id: recordA,
  grain: "film",
  status: "active",
  title: field("Dune local"),
  poster: field(null),
  original_title: field(null),
  overview: field(null),
  release_year: field("2024"),
  identifiers: [],
  latest_activity: null,
};

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

const lifetime = {
  created_at: "2099-09-05T12:00:00Z",
  fresh_until: "2099-09-05T12:02:00Z",
  stale_until: "2099-09-05T12:10:00Z",
  expires_at: "2099-09-06T12:00:00Z",
};

const receipt = {
  candidate_receipt_id: receiptId,
  grain: "film",
  candidate,
};

interface BrowserAttachFixture {
  actionRequests: Array<{
    authorization?: string;
    csrf?: string;
    request: {
      operation_id: string;
      action: { kind: string; record_id?: string };
      evidence_mode: string;
    };
  }>;
  holdTargetSearch: boolean;
  holdAction: boolean;
  holdPostSaveRecords: boolean;
  malformedTarget: boolean;
  targetSearchStarted: number;
  targetSearchResponded: number;
  actionStarted: number;
  actionResponded: number;
  postSaveRecordsStarted: number;
  postSaveRecordsResponded: number;
  releaseTargetSearch?: () => void;
  releaseAction?: () => void;
  releasePostSaveRecords?: () => void;
}

async function fulfillJson(route: Route, body: unknown, status = 200) {
  await route.fulfill({
    status,
    contentType:
      status >= 400 ? "application/problem+json" : "application/json",
    headers: { "cache-control": "no-store" },
    body: JSON.stringify(body),
  });
}

async function setCsrfCookie(page: Page): Promise<void> {
  await page.context().addCookies([
    {
      name: "__Host-fasti_csrf",
      value: csrf,
      url: "https://127.0.0.1:4173",
      secure: true,
      httpOnly: false,
      sameSite: "Strict",
    },
  ]);
}

async function installBrowserAttachHost(
  page: Page,
): Promise<BrowserAttachFixture> {
  await mockAuthenticatedAccess(page);
  await setCsrfCookie(page);
  const fixture: BrowserAttachFixture = {
    actionRequests: [],
    holdTargetSearch: false,
    holdAction: false,
    holdPostSaveRecords: false,
    malformedTarget: false,
    targetSearchStarted: 0,
    targetSearchResponded: 0,
    actionStarted: 0,
    actionResponded: 0,
    postSaveRecordsStarted: 0,
    postSaveRecordsResponded: 0,
  };

  await page.route("**/api/v1/providers", (route) =>
    fulfillJson(route, {
      providers: [
        {
          provider_id: "tmdb",
          display_name: "TMDB",
          provider_kind: "metadata",
          documentation_url: "https://developer.themoviedb.org/docs",
          attribution: "TMDB",
          supported_media_grains: ["film", "series"],
          capabilities: [
            {
              capability_id: "metadata.search",
              purpose: "Search film and television metadata",
              credential_requirement: "bearer_token",
              credential_state: "valid",
              credential_source: "environment",
              state: "available",
              version: 1,
              writable: false,
              testable: true,
              health: {
                state: "passed",
                checked_at: "2099-09-05T12:00:00Z",
                safe_problem_code: null,
              },
              credential_test: {
                state: "passed",
                checked_at: "2099-09-05T12:00:00Z",
                safe_problem_code: null,
              },
            },
          ],
          network_hosts: ["api.themoviedb.org"],
          locale_support: true,
          region_support: true,
          identity_namespaces: ["tmdb.movie", "tmdb.tv"],
        },
      ],
    }),
  );
  await page.route("**/api/v1/search/providers/tmdb", (route) =>
    fulfillJson(route, {
      outcome: "page",
      provider_id: "tmdb",
      page: 1,
      candidates: [receipt],
      next_page: null,
      cache_state: "observed",
      lifetime,
      upstream_problem: null,
    }),
  );
  await page.route("**/api/v1/search/records", async (route) => {
    const request = route.request().postDataJSON() as {
      grains?: string[];
    };
    if (!request.grains?.includes("film")) {
      return fulfillJson(route, { records: [], next: null });
    }
    fixture.targetSearchStarted += 1;
    if (fixture.holdTargetSearch) {
      await new Promise<void>((resolve) => {
        fixture.releaseTargetSearch = resolve;
      });
    }
    await fulfillJson(route, { records: [localRecord], next: null });
    fixture.targetSearchResponded += 1;
  });
  await page.route(
    `**/api/v1/search/candidates/tmdb/film/${receiptId}/actions`,
    async (route) => {
      const request = route
        .request()
        .postDataJSON() as BrowserAttachFixture["actionRequests"][number]["request"];
      fixture.actionStarted += 1;
      fixture.actionRequests.push({
        authorization: route.request().headers().authorization,
        csrf: route.request().headers()["x-csrf-token"],
        request,
      });
      if (fixture.holdAction) {
        await new Promise<void>((resolve) => {
          fixture.releaseAction = resolve;
        });
      }
      const responseTarget = fixture.malformedTarget
        ? recordB
        : request.action.record_id;
      await fulfillJson(route, {
        outcome: "saved",
        receipt: {
          operation_id: request.operation_id,
          candidate_receipt_id: receiptId,
          provider_id: "tmdb",
          grain: "film",
          action: { kind: "attach", record_id: responseTarget },
          evidence_mode: request.evidence_mode,
          record_id: responseTarget,
          disposition: "attached",
          fetched_at: "2099-09-05T12:00:00Z",
          expires_at: "2099-09-06T12:00:00Z",
          initial_status: "fresh",
          committed_at: "2099-09-05T12:00:01Z",
        },
      });
      fixture.actionResponded += 1;
    },
  );
  await page.route(/\/api\/v1\/records(?:\?.*)?$/, async (route) => {
    const holdThisRead =
      fixture.holdPostSaveRecords && fixture.actionStarted > 0;
    if (holdThisRead) {
      fixture.holdPostSaveRecords = false;
      fixture.postSaveRecordsStarted += 1;
      await new Promise<void>((resolve) => {
        fixture.releasePostSaveRecords = resolve;
      });
    }
    const requestedId = new URL(route.request().url()).searchParams.get(
      "record_id",
    );
    await fulfillJson(route, {
      records:
        requestedId === null || requestedId === recordA ? [localRecord] : [],
      truncated: false,
    });
    if (holdThisRead) fixture.postSaveRecordsResponded += 1;
  });

  return fixture;
}

function signedOutProblem(): ProblemDetails {
  const canonical = PUBLIC_PROBLEM_CATALOG.problems.find(
    (problem) =>
      problem.code === "browser_session_expired" &&
      problem.capability_id === "access.projection.read",
  );
  if (!canonical) throw new Error("Missing signed-out Access problem contract");
  const { param_policy: _paramPolicy, ...contract } = canonical;
  return {
    ...contract,
    actual: null,
    correlation_id: "req_018f0e0e7f7b70008000000000000009",
    violations: [],
  };
}

async function changedProjection(
  page: Page,
  kind: "profile" | "subject",
): Promise<AccessProjectionResponse> {
  const response = await page.request.get(stubProjectionUrl);
  expect(response.ok()).toBe(true);
  const current = (await response.json()) as AccessProjectionResponse;
  const suffix = "01";
  const selectedGrant = `grt_018f0e0e7f7b700080000000000000${suffix}`;
  const nextSession = {
    ...current.current_session,
    browser_session_id:
      kind === "subject"
        ? `ses_018f0e0e7f7b700080000000000000${suffix}`
        : current.current_session.browser_session_id,
    workspace_id:
      kind === "subject"
        ? `wsp_018f0e0e7f7b700080000000000000${suffix}`
        : current.current_session.workspace_id,
    selected_profile_grant_id: selectedGrant,
  };
  return {
    ...current,
    evidence: kind === "subject" ? [] : current.evidence,
    subject:
      kind === "subject"
        ? {
            ...current.subject,
            auth_subject_id: `sub_018f0e0e7f7b700080000000000000${suffix}`,
          }
        : current.subject,
    membership: {
      ...current.membership,
      membership_id:
        kind === "subject"
          ? `mem_018f0e0e7f7b700080000000000000${suffix}`
          : current.membership.membership_id,
      workspace_id: nextSession.workspace_id,
    },
    current_session: nextSession,
    sessions: [nextSession],
    profile_grants: [
      {
        ...current.profile_grants[0],
        owner_client_id:
          kind === "subject"
            ? `cli_018f0e0e7f7b700080000000000000${suffix}`
            : current.profile_grants[0].owner_client_id,
        profile_grant_id: selectedGrant,
        profile_id: `prf_018f0e0e7f7b700080000000000000${suffix}`,
      },
    ],
  };
}

async function revalidateAuthority(
  page: Page,
  change: "profile" | "subject" | "signed_out",
): Promise<void> {
  const response =
    change === "signed_out"
      ? { body: signedOutProblem(), status: 401 }
      : { body: await changedProjection(page, change), status: 200 };
  // Registered after mockAuthenticatedAccess so this material state change is
  // the next projection returned by the browser-session client.
  await page.route("**/api/access/v1/projection", (route) =>
    fulfillJson(route, response.body, response.status),
  );
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
}

async function openAttachPicker(page: Page) {
  await page.goto("/discover");
  await page.getByRole("searchbox", { name: "Search TMDB" }).fill("Dune");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page
    .getByRole("button", { name: "Attach to existing Record", exact: true })
    .click();
  return page.getByRole("dialog", { name: "Attach to existing Record" });
}

async function settleBrowserWork(page: Page): Promise<void> {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      ),
  );
}

test("a browser session attaches one explicit target and opens its canonical Record", async ({
  page,
}) => {
  const fixture = await installBrowserAttachHost(page);
  const dialog = await openAttachPicker(page);
  await dialog.getByRole("button", { name: "Find Records" }).click();
  await dialog.getByRole("radio", { name: /Dune local/ }).check();
  await dialog.getByRole("button", { name: "Confirm attachment" }).click();

  await expect(page).toHaveURL(`/records/film/${recordA}/dune-local`);
  expect(fixture.actionRequests).toHaveLength(1);
  expect(fixture.actionRequests[0]).toMatchObject({
    authorization: undefined,
    csrf,
    request: { action: { kind: "attach", record_id: recordA } },
  });
});

test("a profile change closes a picker and rejects its late Record search", async ({
  page,
}) => {
  const fixture = await installBrowserAttachHost(page);
  const dialog = await openAttachPicker(page);
  fixture.holdTargetSearch = true;
  await dialog.getByRole("button", { name: "Find Records" }).click();
  await expect.poll(() => fixture.targetSearchStarted).toBe(1);

  await revalidateAuthority(page, "profile");
  await expect(dialog).not.toBeVisible();
  fixture.releaseTargetSearch?.();
  await expect.poll(() => fixture.targetSearchResponded).toBe(1);
  await settleBrowserWork(page);

  await expect(page).toHaveURL("/discover");
  await expect(page.getByRole("radio", { name: /Dune local/ })).toHaveCount(0);
  expect(fixture.actionRequests).toEqual([]);
});

test("sign-out during Attach closes private UI and prevents late navigation", async ({
  page,
}) => {
  const fixture = await installBrowserAttachHost(page);
  const dialog = await openAttachPicker(page);
  await dialog.getByRole("button", { name: "Find Records" }).click();
  await dialog.getByRole("radio", { name: /Dune local/ }).check();
  fixture.holdAction = true;
  await dialog.getByRole("button", { name: "Confirm attachment" }).click();
  await expect.poll(() => fixture.actionStarted).toBe(1);

  await revalidateAuthority(page, "signed_out");
  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole("alert")).toContainText(
    "Sign in to use configured metadata providers",
  );
  fixture.releaseAction?.();

  await expect.poll(() => fixture.actionResponded).toBe(1);
  await settleBrowserWork(page);
  await expect(page).toHaveURL("/discover");
  await expect(
    page.getByRole("heading", { level: 1, name: "Dune local" }),
  ).toHaveCount(0);
  expect(fixture.actionRequests[0]).toMatchObject({
    authorization: undefined,
    csrf,
    request: { action: { kind: "attach", record_id: recordA } },
  });
});

test("a new subject during post-save refresh cannot navigate with the old receipt", async ({
  page,
}) => {
  const fixture = await installBrowserAttachHost(page);
  const dialog = await openAttachPicker(page);
  await dialog.getByRole("button", { name: "Find Records" }).click();
  await dialog.getByRole("radio", { name: /Dune local/ }).check();
  fixture.holdPostSaveRecords = true;
  await dialog.getByRole("button", { name: "Confirm attachment" }).click();
  await expect.poll(() => fixture.postSaveRecordsStarted).toBe(1);

  await revalidateAuthority(page, "subject");
  await expect(dialog).not.toBeVisible();
  fixture.releasePostSaveRecords?.();
  await expect.poll(() => fixture.postSaveRecordsResponded).toBe(1);
  await settleBrowserWork(page);

  await expect(page).toHaveURL("/discover");
  await expect(
    page.getByRole("heading", { level: 1, name: "Dune local" }),
  ).toHaveCount(0);
  expect(fixture.actionRequests).toHaveLength(1);
});

test("the browser SDK rejects an Attach receipt for a different target", async ({
  page,
}) => {
  const fixture = await installBrowserAttachHost(page);
  fixture.malformedTarget = true;
  const dialog = await openAttachPicker(page);
  await dialog.getByRole("button", { name: "Find Records" }).click();
  await dialog.getByRole("radio", { name: /Dune local/ }).check();
  await dialog.getByRole("button", { name: "Confirm attachment" }).click();

  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("alert")).toContainText(
    "Candidate action response violates the generated contract",
  );
  await expect(page).toHaveURL("/discover");
  await expect(dialog.getByRole("radio", { name: /Dune local/ })).toBeChecked();
  expect(fixture.actionRequests).toHaveLength(1);
  expect(fixture.postSaveRecordsStarted).toBe(0);
  fixture.malformedTarget = false;
  await dialog.getByRole("button", { name: "Confirm attachment" }).click();
  await expect(page).toHaveURL(`/records/film/${recordA}/dune-local`);
  expect(fixture.actionRequests).toHaveLength(2);
  expect(fixture.actionRequests[1].request).toEqual(
    fixture.actionRequests[0].request,
  );
});
