import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page, type Route } from "@playwright/test";
import {
  PUBLIC_PROBLEM_CATALOG,
  type AccessProjectionResponse,
  type AnimeGroupingPolicyChangeDto,
  type ProblemDetails,
} from "@fasti/sdk";

const csrf = "a".repeat(64);
const profileId = "prf_018f0e0e7f7b70008000000000000000";
const clientId = "cli_018f0e0e7f7b70008000000000000000";
const recordIds = [
  "rec_018f0e0e7f7b70008000000000000001",
  "rec_018f0e0e7f7b70008000000000000002",
] as const;

function projection(): AccessProjectionResponse {
  const currentSession = {
    browser_session_id: "ses_018f0e0e7f7b70008000000000000000",
    workspace_id: "wsp_018f0e0e7f7b70008000000000000000",
    selected_profile_grant_id: "grt_018f0e0e7f7b70008000000000000000",
    is_current: true,
    created_at: "2026-08-31T12:00:00Z",
    last_seen_at: "2026-08-31T12:01:00Z",
    idle_expires_at: "2099-08-31T12:31:00Z",
    absolute_expires_at: "2099-08-31T20:00:00Z",
    rotation_generation: 1,
  } as const;
  return {
    generated_at: "2026-08-31T12:01:00Z",
    subject: {
      auth_subject_id: "sub_018f0e0e7f7b70008000000000000000",
      lifecycle: "active",
      created_at: "2026-08-31T12:00:00Z",
      updated_at: "2026-08-31T12:01:00Z",
    },
    membership: {
      membership_id: "mem_018f0e0e7f7b70008000000000000000",
      workspace_id: currentSession.workspace_id,
      lifecycle: "active",
      role: "administrator",
      created_at: "2026-08-31T12:00:00Z",
      updated_at: "2026-08-31T12:01:00Z",
    },
    current_session: currentSession,
    sessions: [currentSession],
    sessions_truncated: false,
    profile_grants: [
      {
        profile_grant_id: currentSession.selected_profile_grant_id,
        profile_id: profileId,
        owner_client_id: clientId,
        selected: true,
      },
    ],
    profile_grants_truncated: false,
    session_policy: {
      idle_timeout_seconds: 1_800,
      browser_lifetime_seconds: 28_800,
      remembered_browser_lifetime_seconds: 2_592_000,
      last_seen_write_interval_seconds: 60,
    },
    authentication: {
      method: "trail_base_password",
      verified_at: "2026-08-31T12:00:00Z",
      activation_generation: 1,
      recent_authentication: { state: "unavailable", expires_at: null },
    },
    trailbase: {
      state: "active",
      blocker: null,
      trailbase_instance_id: "tbi_018f0e0e7f7b70008000000000000000",
      generation: 1,
      session_generation_current: true,
      updated_at: "2026-08-31T12:00:00Z",
    },
    first_run_steps: [
      { key: "account_confirmed", state: "verified" },
      { key: "strong_sign_in", state: "needs_attention" },
      { key: "recovery", state: "unavailable" },
      { key: "devices_and_clients", state: "unavailable" },
      { key: "external_identity", state: "unavailable" },
    ],
    evidence: [],
    evidence_truncated: false,
  };
}

function problem(
  code: ProblemDetails["code"],
  capabilityId: ProblemDetails["capability_id"],
  status: number,
): ProblemDetails {
  const canonical = PUBLIC_PROBLEM_CATALOG.problems.find(
    (candidate) =>
      candidate.code === code && candidate.capability_id === capabilityId,
  );
  if (!canonical || canonical.status !== status)
    throw new Error(`missing ${capabilityId}.${code} contract`);
  const { param_policy: _paramPolicy, ...contract } = canonical;
  return {
    ...contract,
    actual: null,
    correlation_id: "req_018f0e0e7f7b70008000000000000009",
    violations: [],
  };
}

async function fulfillJson(route: Route, body: unknown, status = 200) {
  await route.fulfill({
    status,
    contentType:
      status >= 400 ? "application/problem+json" : "application/json",
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

function previewRecord(recordId: string, rollback = false) {
  return {
    record_id: recordId,
    previous_preference: rollback ? "group_by_tv_work" : "automatic",
    proposed_preference: rollback ? "automatic" : "group_by_tv_work",
    previous_status: "selected",
    proposed_status: "selected",
    previous_route: {
      identifier: {
        namespace: rollback ? "imdb.title" : "mal.anime",
        grain: "release",
        value: rollback
          ? recordId.endsWith("1")
            ? "tt28254942"
            : "tt12345678"
          : recordId.endsWith("1")
            ? "49894"
            : "51009",
      },
      kind: rollback ? "verified_alias" : "provider_native",
      accepted_assertions: [],
    },
    proposed_route: {
      identifier: {
        namespace: rollback ? "mal.anime" : "imdb.title",
        grain: "release",
        value: rollback
          ? recordId.endsWith("1")
            ? "49894"
            : "51009"
          : recordId.endsWith("1")
            ? "tt28254942"
            : "tt12345678",
      },
      kind: rollback ? "provider_native" : "verified_alias",
      accepted_assertions: [],
    },
    route_changed: true,
    possible_season_regrouping: false,
  } as const;
}

test("browser anime policy review is complete, retry-safe, and recoverable", async ({
  page,
}, testInfo) => {
  await setCsrfCookie(page);
  let revision = 1;
  let preference = "automatic";
  let failedOnce = false;
  let delayedNextPage:
    { started: () => void; response: Promise<void> } | undefined;
  let delayedPreview:
    { started: () => void; response: Promise<void> } | undefined;
  const applyRequests: Array<{
    authorization?: string;
    csrf?: string;
    body: {
      operation_id: string;
      change: AnimeGroupingPolicyChangeDto;
    };
  }> = [];

  await page.route("**/api/access/v1/**", (route) =>
    fulfillJson(route, projection()),
  );
  await page.route(
    "**/api/v1/profile/anime-grouping-policy**",
    async (route) => {
      const request = route.request();
      if (request.method() === "GET") {
        return fulfillJson(route, {
          policy: {
            profile_id: profileId,
            scope: { kind: "profile", client_id: null },
            source: "profile_default",
            preference,
            revision,
          },
        });
      }
      const body = request.postDataJSON() as {
        after_record_id?: string | null;
        operation_id?: string;
        change: AnimeGroupingPolicyChangeDto;
      };
      if (request.method() === "POST") {
        const rollback = body.change.kind === "rollback";
        const secondPage = Boolean(body.after_record_id);
        if (secondPage && delayedNextPage) {
          delayedNextPage.started();
          await delayedNextPage.response;
          delayedNextPage = undefined;
        }
        if (!secondPage && delayedPreview) {
          delayedPreview.started();
          await delayedPreview.response;
          delayedPreview = undefined;
        }
        return fulfillJson(route, {
          policy: {
            profile_id: profileId,
            scope: { kind: "profile", client_id: null },
            source: "profile_default",
            preference,
            revision,
          },
          proposed_preference: rollback ? "automatic" : "group_by_tv_work",
          proposed_source: "profile_default",
          total_records: rollback ? 1 : 2,
          affected_records: rollback ? 1 : 2,
          unresolved_routes: 0,
          possible_season_regroupings: 0,
          records: [
            previewRecord(secondPage ? recordIds[1] : recordIds[0], rollback),
          ],
          next_after_record_id: !rollback && !secondPage ? recordIds[0] : null,
        });
      }
      applyRequests.push({
        authorization: request.headers().authorization,
        csrf: request.headers()["x-csrf-token"],
        body: {
          operation_id: body.operation_id ?? "",
          change: body.change,
        },
      });
      if (!failedOnce) {
        failedOnce = true;
        return fulfillJson(
          route,
          problem(
            "storage_unavailable",
            "profile.anime_grouping_policy.apply",
            503,
          ),
          503,
        );
      }
      revision += 1;
      preference = "group_by_tv_work";
      return fulfillJson(route, {
        operation_id: body.operation_id,
        change: body.change,
        previous_preference: "automatic",
        previous_source: "profile_default",
        policy: {
          profile_id: profileId,
          scope: { kind: "profile", client_id: null },
          source: "profile_default",
          preference,
          revision,
        },
        affected_records: 2,
        unresolved_routes: 0,
        possible_season_regroupings: 0,
        rolled_back_operation_id: null,
      });
    },
  );

  await page.goto("/settings/preferences");
  const card = page.getByTestId("anime-grouping-policy");
  await expect(card.getByText("Revision 1")).toBeVisible();
  await card.getByLabel("Profile default").selectOption("group_by_tv_work");
  await card.getByTestId("preview-anime-grouping-policy").click();

  const heading = card.getByRole("heading", {
    name: "Review anime identifier changes",
  });
  await expect(heading).toBeFocused();
  await expect(card).toContainText("2 affected Records");
  await expect(card).toContainText("Chronicle events, history");
  await expect(card).toContainText("mal.anime:49894");
  await expect(card).toContainText("imdb.title:tt28254942");

  let markPageStarted: () => void = () => undefined;
  let releasePage: () => void = () => undefined;
  const pageStarted = new Promise<void>((resolve) => {
    markPageStarted = resolve;
  });
  delayedNextPage = {
    started: markPageStarted,
    response: new Promise<void>((resolve) => {
      releasePage = resolve;
    }),
  };
  await card.getByTestId("load-more-anime-grouping-policy").click();
  await pageStarted;
  await card.getByTestId("cancel-anime-grouping-policy").click();
  await expect(card.getByTestId("preview-anime-grouping-policy")).toBeFocused();
  let markPreviewStarted: () => void = () => undefined;
  let releasePreview: () => void = () => undefined;
  const previewStarted = new Promise<void>((resolve) => {
    markPreviewStarted = resolve;
  });
  delayedPreview = {
    started: markPreviewStarted,
    response: new Promise<void>((resolve) => {
      releasePreview = resolve;
    }),
  };
  await card.getByTestId("preview-anime-grouping-policy").click();
  await previewStarted;
  releasePage();
  await expect(heading).toHaveCount(0);
  await expect(
    card.getByTestId("preview-anime-grouping-policy"),
  ).toBeDisabled();

  releasePreview();
  await expect(heading).toBeFocused();
  await card.getByTestId("load-more-anime-grouping-policy").click();
  await expect(card.locator("tbody tr")).toHaveCount(2);
  await expect(card.getByTestId("load-more-anime-grouping-policy")).toHaveCount(
    0,
  );

  const targets = card.locator("button:visible, select:visible");
  expect(
    await targets.evaluateAll((elements) =>
      elements
        .map((element) => ({
          label:
            element.getAttribute("aria-label") ?? element.textContent?.trim(),
          ...element.getBoundingClientRect().toJSON(),
        }))
        .filter(({ width, height }) => width < 44 || height < 44),
    ),
  ).toEqual([]);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);

  await card.getByTestId("apply-anime-grouping-policy").click();
  await expect(card.getByRole("status")).toContainText(
    "Saved the profile policy",
  );
  expect(applyRequests).toHaveLength(2);
  expect(applyRequests[0]?.body.operation_id).toMatch(/^op_[0-9a-f]{32}$/);
  expect(applyRequests[1]?.body.operation_id).toBe(
    applyRequests[0]?.body.operation_id,
  );
  expect(applyRequests.map(({ authorization }) => authorization)).toEqual([
    undefined,
    undefined,
  ]);
  expect(applyRequests.map(({ csrf: token }) => token)).toEqual([csrf, csrf]);

  await card.getByTestId("review-anime-grouping-rollback").click();
  await expect(heading).toBeFocused();
  await expect(card.getByTestId("apply-anime-grouping-policy")).toHaveText(
    "Apply rollback",
  );
  const rollbackRow = card.locator("tbody tr").first();
  await expect(rollbackRow).toContainText("imdb.title:tt28254942");
  await expect(rollbackRow).toContainText("mal.anime:49894");
  await card.getByTestId("cancel-anime-grouping-policy").click();
  await expect(card.getByTestId("preview-anime-grouping-policy")).toBeFocused();
  await expect(heading).toHaveCount(0);

  for (const theme of ["light", "dark"] as const) {
    await page.evaluate((mode) => {
      localStorage.setItem("fasti-theme-settings", JSON.stringify({ mode }));
    }, theme);
    for (const width of [320, 768, 1440] as const) {
      await page.setViewportSize({
        width,
        height: width === 320 ? 900 : 1_024,
      });
      await page.goto("/settings/preferences");
      const matrixCard = page.getByTestId("anime-grouping-policy");
      await matrixCard.getByTestId("preview-anime-grouping-policy").click();
      await expect(
        matrixCard.getByRole("heading", {
          name: "Review anime identifier changes",
        }),
      ).toBeFocused();
      await expect(page.locator("html")).toHaveAttribute(
        "data-bs-theme",
        theme,
      );
      expect(
        await page.evaluate(
          () =>
            document.documentElement.scrollWidth -
            document.documentElement.clientWidth,
        ),
      ).toBeLessThanOrEqual(0);
      if (width < 768) {
        await expect(matrixCard.locator("tbody td").first()).toHaveAttribute(
          "data-label",
          "Record",
        );
      }
      expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
      await page.screenshot({
        path: testInfo.outputPath(`anime-policy-${theme}-${width}.png`),
        fullPage: true,
        animations: "disabled",
      });
    }
  }
});
