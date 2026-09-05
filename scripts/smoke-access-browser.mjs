import { chromium } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

const FASTI_ORIGIN = "http://127.0.0.1:8420";
const FIXTURE_TITLE = "Fasti Fixture Film";
const FIXTURE_OVERVIEW =
  "Deterministic provider detail for the real Search journey.";
const FIXTURE_PROVIDER_IDS = ["842001", "842002"];

let raw = "";
for await (const chunk of process.stdin) raw += chunk;
const input = JSON.parse(raw);
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext();
const page = await context.newPage();

async function trailBaseLogin() {
  await page.locator("#login-form").waitFor();
  await page.locator("#login-form input[name=email]").fill(input.email);
  await page.locator("#login-form input[name=password]").fill(input.password);
  await page.locator("#login-form").evaluate((form) => form.requestSubmit());
}

async function signInToFasti() {
  await page.goto(`${FASTI_ORIGIN}/first-run`);
  await page
    .getByRole("button", { name: "Sign in to an existing account" })
    .click();
  await page.waitForURL((url) => url.origin === "http://127.0.0.1:4000");
  await trailBaseLogin();
  await page.waitForURL(
    (url) =>
      url.origin === FASTI_ORIGIN &&
      ["/first-run", "/settings/account"].includes(url.pathname),
    { timeout: 30_000 },
  );
  await page
    .getByRole("heading", { name: "Choose where to continue" })
    .waitFor();
  await page.getByRole("radio").first().check();
  await page.getByRole("button", { name: "Confirm access" }).click();
  await page.waitForFunction(() =>
    document.cookie.includes("__Host-fasti_csrf="),
  );
  if (new URL(page.url()).pathname === "/first-run") {
    await page.getByText("Account confirmed", { exact: true }).waitFor();
    await page.goto(`${FASTI_ORIGIN}/settings/account`);
  }
  await page.getByRole("heading", { name: "Account and security" }).waitFor();
}

function requireValue(condition, message) {
  if (!condition) throw new Error(message);
}

async function requireCount(locator, expected, label) {
  const actual = await locator.count();
  if (actual !== expected) {
    throw new Error(
      `${label} count differs; expected=${expected} actual=${actual}`,
    );
  }
}

async function requireNoAccessibilityViolations(label) {
  const { violations } = await new AxeBuilder({ page }).analyze();
  if (violations.length > 0) {
    throw new Error(
      `${label} has accessibility violations: ${violations
        .map(({ id }) => id)
        .join(",")}`,
    );
  }
}

function providerResult(providerId) {
  return page
    .getByRole("region", { name: "Search results" })
    .getByRole("listitem")
    .filter({ has: page.getByText(providerId, { exact: true }) });
}

async function searchFixtureFilm({ cachedOnly = false } = {}) {
  const provider = page.getByLabel("Metadata provider");
  await provider.selectOption("tmdb");
  if (cachedOnly) {
    await page
      .getByRole("checkbox", {
        name: "Use cached provider results only",
      })
      .check();
  }
  const search = page.getByRole("searchbox", {
    name: "Search The Movie Database (TMDB)",
  });
  await search.fill(FIXTURE_TITLE);
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page
    .getByRole("region", { name: "Search results" })
    .getByText(`2 results for ${FIXTURE_TITLE}.`, { exact: false })
    .waitFor();
  for (const providerId of FIXTURE_PROVIDER_IDS) {
    await requireCount(
      providerResult(providerId),
      1,
      `TMDB candidate ${providerId}`,
    );
  }
}

async function openFixtureDetails(providerId) {
  const result = providerResult(providerId);
  await result.getByRole("link", { name: "View details" }).click();
  await page.waitForURL(
    (url) =>
      url.origin === FASTI_ORIGIN &&
      /^\/explore\/tmdb\/film\/scr_[0-9a-f]{32}\/fasti-fixture-film$/.test(
        url.pathname,
      ),
  );
  const details = page.locator("section").filter({
    has: page.getByRole("heading", { name: FIXTURE_TITLE, level: 1 }),
  });
  await details.getByText(FIXTURE_OVERVIEW, { exact: true }).waitFor();
  await details.getByText(providerId, { exact: true }).waitFor();
  await details.getByText("2020", { exact: true }).waitFor();
  return details;
}

async function runM4SearchJourney() {
  const observed = [];
  const observeRequest = (request) => {
    const url = new URL(request.url());
    if (
      url.origin === FASTI_ORIGIN &&
      url.pathname.startsWith("/api/v1/search/")
    ) {
      observed.push({ method: request.method(), path: url.pathname });
    }
  };
  page.on("request", observeRequest);
  try {
    await page.goto(`${FASTI_ORIGIN}/discover`);
    await page.getByRole("heading", { name: "Discover", level: 1 }).waitFor();
    await searchFixtureFilm();
    // Fresh describes reuse eligibility, including a just-persisted response.
    // The TLS fixture's exact request count separately proves the upstream call.
    await page
      .getByRole("region", { name: "Search results" })
      .getByRole("status")
      .filter({
        hasText: "These results came from fresh cache evidence.",
      })
      .waitFor();
    await requireNoAccessibilityViolations("live provider Search results");

    const firstDetails = await openFixtureDetails(FIXTURE_PROVIDER_IDS[0]);
    await requireNoAccessibilityViolations("provider candidate details");
    await firstDetails.getByRole("button", { name: "Create Record" }).click();
    await page.waitForURL(
      (url) =>
        url.origin === FASTI_ORIGIN &&
        /^\/records\/film\/rec_[0-9a-f]{32}\/fasti-fixture-film$/.test(
          url.pathname,
        ),
    );
    const recordPath = new URL(page.url()).pathname;
    const recordId = recordPath.split("/")[3];
    requireValue(
      /^rec_[0-9a-f]{32}$/.test(recordId),
      "created Record ID is not canonical",
    );
    await page
      .getByRole("heading", { name: FIXTURE_TITLE, level: 1 })
      .waitFor();
    await page.getByText("Fasti Entity ID:", { exact: true }).waitFor();
    await page.getByText(recordId, { exact: true }).first().waitFor();

    await page.goto(`${FASTI_ORIGIN}/discover`);
    await searchFixtureFilm();
    const secondDetails = await openFixtureDetails(FIXTURE_PROVIDER_IDS[1]);
    await secondDetails
      .getByRole("button", {
        name: "Attach to existing Record",
        exact: true,
      })
      .click();
    const dialog = page.getByRole("dialog", {
      name: "Attach to existing Record",
    });
    const localSearch = dialog.getByRole("searchbox", {
      name: "Search local Records",
    });
    await localSearch.waitFor();
    requireValue(
      await localSearch.evaluate(
        (element) => element === document.activeElement,
      ),
      "Attach Record Search did not receive focus",
    );
    await dialog.getByRole("button", { name: "Find Records" }).click();
    const target = dialog.getByRole("radio", {
      name: new RegExp(recordId),
    });
    await target.waitFor();
    await requireCount(target, 1, "exact Attach target");
    await target.check();
    await requireNoAccessibilityViolations("Attach Record picker");
    await dialog.getByRole("button", { name: "Confirm attachment" }).click();
    await page.waitForURL(
      (url) => url.origin === FASTI_ORIGIN && url.pathname === recordPath,
    );
    await page.getByText(recordId, { exact: true }).first().waitFor();

    await page.goto(`${FASTI_ORIGIN}/discover`);
    await searchFixtureFilm({ cachedOnly: true });
    await page
      .getByRole("region", { name: "Search results" })
      .getByRole("status")
      .filter({
        hasText: "These results came from fresh cache evidence.",
      })
      .waitFor();
    const cachedOnly = await page
      .getByRole("checkbox", {
        name: "Use cached provider results only",
      })
      .isChecked();
    requireValue(cachedOnly, "cached-only Search mode was not retained");

    const counts = {
      providerSearch: observed.filter(
        ({ method, path }) =>
          method === "POST" && path === "/api/v1/search/providers/tmdb",
      ).length,
      candidateDetails: observed.filter(
        ({ method, path }) =>
          method === "GET" &&
          /^\/api\/v1\/search\/candidates\/tmdb\/film\/scr_[0-9a-f]{32}$/.test(
            path,
          ),
      ).length,
      candidateActions: observed.filter(
        ({ method, path }) =>
          method === "POST" &&
          /^\/api\/v1\/search\/candidates\/tmdb\/film\/scr_[0-9a-f]{32}\/actions$/.test(
            path,
          ),
      ).length,
      localRecordSearch: observed.filter(
        ({ method, path }) =>
          method === "POST" && path === "/api/v1/search/records",
      ).length,
    };
    requireValue(
      counts.providerSearch === 3,
      "browser provider Search request count differs",
    );
    requireValue(
      counts.candidateDetails === 2,
      "browser candidate details request count differs",
    );
    requireValue(
      counts.candidateActions === 2,
      "browser candidate action request count differs",
    );
    requireValue(
      counts.localRecordSearch >= 4,
      "browser local Record Search requests are incomplete",
    );

    return {
      recordId,
      recordPath,
      providerIds: [...FIXTURE_PROVIDER_IDS],
      cachedOnly,
      browserRequests: counts,
      liveProviderDetailsObserved: true,
      createCanonicalRecordObserved: true,
      attachCanonicalRecordObserved: true,
    };
  } finally {
    page.off("request", observeRequest);
  }
}

async function verifyRestartedRecord() {
  requireValue(
    typeof input.recordId === "string" &&
      /^rec_[0-9a-f]{32}$/.test(input.recordId),
    "restart Record ID is invalid",
  );
  requireValue(
    typeof input.recordPath === "string" &&
      input.recordPath === `/records/film/${input.recordId}/fasti-fixture-film`,
    "restart Record path does not match its canonical identity",
  );
  await page.goto(`${FASTI_ORIGIN}${input.recordPath}`);
  await page.waitForURL(
    (url) => url.origin === FASTI_ORIGIN && url.pathname === input.recordPath,
  );
  await page.getByRole("heading", { name: FIXTURE_TITLE, level: 1 }).waitFor();
  await page.getByText("Fasti Entity ID:", { exact: true }).waitFor();
  await page.getByText(input.recordId, { exact: true }).first().waitFor();
  await page
    .getByRole("button", { name: "Sources & Identity (2)", exact: true })
    .click();
  const identifiers = page.getByRole("region", {
    name: "External identifiers",
  });
  await identifiers.waitFor();
  for (const providerId of FIXTURE_PROVIDER_IDS) {
    const row = identifiers
      .getByRole("row")
      .filter({ has: identifiers.getByText(providerId, { exact: true }) });
    await requireCount(row, 1, `persisted TMDB identity ${providerId}`);
    await row.getByText("tmdb.movie", { exact: true }).waitFor();
  }
  await requireNoAccessibilityViolations("restarted canonical Record");
  return {
    recordId: input.recordId,
    recordPath: input.recordPath,
    providerIds: [...FIXTURE_PROVIDER_IDS],
    canonicalRecordObservedAfterRestart: true,
    exactExternalIdentifiersObserved: true,
  };
}

try {
  if (input.mode === "bootstrap") {
    await page.goto(input.authorizationUrl);
    await trailBaseLogin();
    await page.waitForURL(
      (url) =>
        url.origin === "http://127.0.0.1:8420" &&
        url.pathname === "/api/access/v1/trailbase/callback" &&
        url.searchParams.has("code"),
      { timeout: 30_000 },
    );
    process.stdout.write(JSON.stringify({ callbackUrl: page.url() }));
  } else if (input.mode === "sign-in") {
    await signInToFasti();

    const m3AnimeGroupingPolicy = await page.evaluate(async () => {
      const csrf = document.cookie
        .split("; ")
        .find((pair) => pair.startsWith("__Host-fasti_csrf="))
        ?.split("=", 2)[1];
      if (!csrf) throw new Error("browser CSRF cookie is unavailable");

      function requestOptions(method, body) {
        const headers = {};
        if (body !== undefined) headers["content-type"] = "application/json";
        if (method !== "GET") headers["X-CSRF-Token"] = csrf;
        return {
          method,
          headers,
          body: body === undefined ? undefined : JSON.stringify(body),
          credentials: "same-origin",
        };
      }

      async function parseResponse(response, method, path) {
        const payload = await response.json();
        if (!response.ok) {
          throw new Error(
            `${method} ${path} failed with ${response.status} ${payload.code ?? "unknown"}`,
          );
        }
        return payload;
      }

      async function readPolicy() {
        return parseResponse(
          await fetch(
            "/api/v1/profile/anime-grouping-policy?scope=profile",
            requestOptions("GET"),
          ),
          "GET",
          "/api/v1/profile/anime-grouping-policy?scope=profile",
        );
      }

      async function mutatePolicy(body) {
        return parseResponse(
          await fetch(
            "/api/v1/profile/anime-grouping-policy",
            requestOptions("PUT", body),
          ),
          "PUT",
          "/api/v1/profile/anime-grouping-policy",
        );
      }

      const created = await parseResponse(
        await fetch(
          "/api/v1/records",
          requestOptions("POST", { grain: "release" }),
        ),
        "POST",
        "/api/v1/records",
      );
      const initial = await readPolicy();
      if (initial.policy.revision !== 0) {
        throw new Error("anime grouping policy did not start at revision zero");
      }
      const preview = await parseResponse(
        await fetch(
          "/api/v1/profile/anime-grouping-policy/preview",
          requestOptions("POST", {
            scope: { kind: "profile", client_id: null },
            change: { kind: "set", preference: "group_by_tv_work" },
            after_record_id: null,
            limit: 10,
          }),
        ),
        "POST",
        "/api/v1/profile/anime-grouping-policy/preview",
      );
      if (preview.total_records !== 1) {
        throw new Error(
          "anime grouping preview did not include the durable record",
        );
      }

      const applyBody = {
        operation_id: "op_01998c1a4e2b70008000000000000001",
        scope: { kind: "profile", client_id: null },
        expected_revision: 0,
        change: { kind: "set", preference: "group_by_tv_work" },
      };
      const applied = await mutatePolicy(applyBody);
      const appliedReplay = await mutatePolicy(applyBody);
      if (
        applied.policy.revision !== 1 ||
        applied.policy.preference !== "group_by_tv_work" ||
        JSON.stringify(appliedReplay) !== JSON.stringify(applied)
      ) {
        throw new Error("anime grouping apply or replay differs");
      }

      const rollbackBody = {
        operation_id: "op_01998c1a4e2b70008000000000000002",
        scope: { kind: "profile", client_id: null },
        expected_revision: 1,
        change: {
          kind: "rollback",
          applied_operation_id: applyBody.operation_id,
        },
      };
      const rolledBack = await mutatePolicy(rollbackBody);
      const rollbackReplay = await mutatePolicy(rollbackBody);
      if (
        rolledBack.policy.revision !== 2 ||
        rolledBack.policy.preference !== "automatic" ||
        JSON.stringify(rollbackReplay) !== JSON.stringify(rolledBack)
      ) {
        throw new Error("anime grouping rollback or replay differs");
      }
      return {
        realFastid: true,
        realSqlite: true,
        browserSession: true,
        csrfMutationBoundary: true,
        durableRecordId: created.record_id,
        previewRecords: preview.total_records,
        finalRevision: rolledBack.policy.revision,
        finalPreference: rolledBack.policy.preference,
        applyReplayExact: true,
        rollbackReplayExact: true,
      };
    });

    const cookies = await context.cookies();
    const sessionMatches = cookies.filter(
      (cookie) => cookie.name === "__Host-fasti_session",
    );
    const csrfMatches = cookies.filter(
      (cookie) => cookie.name === "__Host-fasti_csrf",
    );
    if (sessionMatches.length !== 1 || csrfMatches.length !== 1) {
      throw new Error(
        `opaque Fasti cookie count differs; names=${cookies
          .map(({ name }) => name)
          .sort()
          .join(",")}`,
      );
    }
    const [session] = sessionMatches;
    const [csrf] = csrfMatches;
    for (const [label, cookie] of [
      ["session", session],
      ["csrf", csrf],
    ]) {
      if (!/^[0-9a-f]{64}$/.test(cookie.value)) {
        throw new Error(`${label} cookie is not opaque`);
      }
      if (
        !cookie.secure ||
        cookie.sameSite !== "Strict" ||
        cookie.path !== "/" ||
        cookie.domain !== "127.0.0.1"
      ) {
        throw new Error(`${label} cookie policy differs`);
      }
    }
    if (!session.httpOnly || csrf.httpOnly) {
      throw new Error("Fasti cookie HttpOnly policy differs");
    }
    if (session.value === csrf.value) {
      throw new Error("Fasti session and CSRF cookies are not distinct");
    }

    const storage = await page.evaluate(() => ({
      local: Object.entries(localStorage),
      session: Object.entries(sessionStorage),
    }));
    const credential =
      /trailbase|(?:access|auth|id|refresh)[_:.-]?token|\beyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b/i;
    if (
      credential.test("fasti.workbench.state") ||
      credential.test("1.2.3") ||
      !credential.test("eyJhbGciOiJIUzI1NiJ9.c3ViamVjdA.signature")
    ) {
      throw new Error("vendor credential detector differs");
    }
    if (
      [...storage.local, ...storage.session].some(([key, value]) =>
        credential.test(`${key}\n${value}`),
      )
    ) {
      throw new Error("vendor credentials reached browser storage");
    }
    const m4SearchJourney = input.m4SearchJourney
      ? await runM4SearchJourney()
      : undefined;
    process.stdout.write(
      JSON.stringify({
        chromium: browser.version(),
        accountSecuritySurfaceLoaded: true,
        cookies: {
          session: {
            secure: true,
            httpOnly: true,
            sameSite: "Strict",
            path: "/",
            domain: "127.0.0.1",
            opaqueHex64: true,
          },
          csrf: {
            secure: true,
            httpOnly: false,
            sameSite: "Strict",
            path: "/",
            domain: "127.0.0.1",
            opaqueHex64: true,
          },
          distinct: true,
        },
        m3AnimeGroupingPolicy,
        ...(m4SearchJourney ? { m4SearchJourney } : {}),
        fastiOriginVendorCredentialStorageAbsent: true,
      }),
    );
  } else if (input.mode === "restart-record") {
    await signInToFasti();
    const m4RestartedRecord = await verifyRestartedRecord();
    process.stdout.write(
      JSON.stringify({
        chromium: browser.version(),
        m4RestartedRecord,
      }),
    );
  } else {
    throw new Error("unknown proof mode");
  }
} finally {
  input.password = "";
  await browser.close();
}
