import { chromium } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { readFile, rename, writeFile } from "node:fs/promises";

const FASTI_ORIGIN = "http://127.0.0.1:8420";
const FIXTURE_TITLE = "Fasti Fixture Film";
const FIXTURE_OVERVIEW =
  "Deterministic provider detail for the real Search journey.";
const FIXTURE_PROVIDER_IDS = ["842001", "842002"];
const M4_FIXTURES = {
  tmdb: {
    provider: "tmdb",
    title: FIXTURE_TITLE,
    grain: "film",
    slug: "fasti-fixture-film",
    searchLabel: "Search The Movie Database (TMDB)",
    namespace: "tmdb.movie",
  },
  igdb: {
    provider: "igdb",
    title: "Fasti Fixture Game",
    grain: "game_release",
    slug: "fasti-fixture-game",
    searchLabel: "Search IGDB (Games)",
    namespace: "igdb.game",
  },
};

function m4Fixture(provider = "tmdb") {
  if (provider !== "tmdb" && provider !== "igdb") {
    throw new Error("unknown M4 fixture provider");
  }
  return M4_FIXTURES[provider];
}
const NO_STORE_TITLE = "Fasti Transient Fixture Film";
const NO_STORE_OVERVIEW =
  "Transient provider payload sentinel 94a8bcd73cdd4ef5b350f838d5ec4dd1.";
const NO_STORE_PROVIDER_IDS = ["843001", "843002"];

let raw = "";
for await (const chunk of process.stdin) raw += chunk;
const input = JSON.parse(raw);
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext();
const page = await context.newPage();

async function trailBaseLogin(targetPage = page) {
  await targetPage.locator("#login-form").waitFor();
  await targetPage.locator("#login-form input[name=email]").fill(input.email);
  await targetPage
    .locator("#login-form input[name=password]")
    .fill(input.password);
  await targetPage
    .locator("#login-form")
    .evaluate((form) => form.requestSubmit());
}

async function signInToFasti(targetPage = page) {
  await targetPage.goto(`${FASTI_ORIGIN}/first-run`);
  await targetPage
    .getByRole("button", { name: "Sign in to an existing account" })
    .click();
  await targetPage.waitForURL((url) => url.origin === "http://127.0.0.1:4000");
  await trailBaseLogin(targetPage);
  await targetPage.waitForURL(
    (url) =>
      url.origin === FASTI_ORIGIN &&
      ["/first-run", "/settings/account"].includes(url.pathname),
    { timeout: 30_000 },
  );
  await targetPage
    .getByRole("heading", { name: "Choose where to continue" })
    .waitFor();
  await targetPage.getByRole("radio").first().check();
  await targetPage.getByRole("button", { name: "Confirm access" }).click();
  await targetPage.waitForFunction(() =>
    document.cookie.includes("__Host-fasti_csrf="),
  );
  if (new URL(targetPage.url()).pathname === "/first-run") {
    await targetPage.getByText("Account confirmed", { exact: true }).waitFor();
    await targetPage.goto(`${FASTI_ORIGIN}/settings/account`);
  }
  await targetPage
    .getByRole("heading", { name: "Account and security" })
    .waitFor();
}

function requireValue(condition, message) {
  if (!condition) throw new Error(message);
}

async function runClientInventoryJourney() {
  const expectedClientId = input.clientInventoryClientId;
  requireValue(
    typeof expectedClientId === "string" &&
      /^cli_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$/.test(expectedClientId),
    "client inventory node witness is invalid",
  );

  async function readInventory(targetPage) {
    const pending = targetPage.waitForRequest(
      (request) =>
        request.method() === "GET" &&
        request.url() === `${FASTI_ORIGIN}/api/access/v1/clients?limit=1`,
    );
    const result = await targetPage.evaluate(async () => {
      const response = await fetch("/api/access/v1/clients?limit=1", {
        credentials: "same-origin",
        signal: AbortSignal.timeout(10_000),
      });
      return {
        status: response.status,
        cacheControl: response.headers.get("cache-control"),
        contentType: response.headers.get("content-type"),
        body: await response.json(),
      };
    });
    const headers = await (await pending).allHeaders();
    requireValue(
      headers.authorization === undefined &&
        /(?:^|; )__Host-fasti_session=[0-9a-f]{64}(?:;|$)/.test(
          headers.cookie ?? "",
        ),
      "client inventory request did not use cookie-only authority",
    );
    requireValue(
      result.cacheControl === "private, no-store",
      "client inventory response permits retention",
    );
    return result;
  }

  function requireInventory(result) {
    const body = result.body;
    requireValue(
      result.status === 200 &&
        result.contentType?.startsWith("application/json") &&
        body &&
        Object.keys(body).sort().join(",") === "clients,next" &&
        Array.isArray(body.clients) &&
        body.clients.length === 1 &&
        body.next === null,
      "client inventory bounded response differs",
    );
    const client = body.clients[0];
    requireValue(
      client &&
        Object.keys(client).sort().join(",") ===
          "authentication_type,client_id,created_at,current_credential_epoch,lifecycle,name,owner_subject_id,purpose" &&
        client.client_id === expectedClientId &&
        client.authentication_type === "first_party" &&
        client.purpose === "node" &&
        client.lifecycle === "active" &&
        client.owner_subject_id === null &&
        client.name === null &&
        client.current_credential_epoch === "1" &&
        typeof client.created_at === "string" &&
        /^(?:[0-9]{4}|[+-][0-9]{5,6})-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{6}Z$/.test(
          client.created_at,
        ),
      "client inventory differs from the enrolled node witness",
    );
  }

  requireInventory(await readInventory(page));
  const secondContext = await browser.newContext();
  try {
    const secondPage = await secondContext.newPage();
    await signInToFasti(secondPage);
    requireInventory(await readInventory(secondPage));
    const targetSessionId = await secondPage.evaluate(async () => {
      const response = await fetch("/api/access/v1/browser-session", {
        credentials: "same-origin",
        signal: AbortSignal.timeout(10_000),
      });
      if (response.status !== 200)
        throw new Error("second browser session read failed");
      return (await response.json()).session?.browser_session_id;
    });
    requireValue(
      typeof targetSessionId === "string" &&
        /^ses_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$/.test(
          targetSessionId,
        ),
      "second browser session identifier is invalid",
    );
    const secondCookie = (await secondContext.cookies(FASTI_ORIGIN)).find(
      (cookie) => cookie.name === "__Host-fasti_session",
    );
    const originalCookie = (await context.cookies(FASTI_ORIGIN)).find(
      (cookie) => cookie.name === "__Host-fasti_session",
    );
    requireValue(
      secondCookie &&
        originalCookie &&
        secondCookie.value !== originalCookie.value,
      "browser sessions are not independent",
    );
    await page.evaluate(async (sessionId) => {
      const csrf = document.cookie
        .split("; ")
        .find((pair) => pair.startsWith("__Host-fasti_csrf="))
        ?.split("=", 2)[1];
      if (!csrf) throw new Error("revoking browser CSRF cookie is unavailable");
      const response = await fetch(
        `/api/access/v1/browser-sessions/${sessionId}`,
        {
          method: "DELETE",
          headers: { "X-CSRF-Token": csrf },
          credentials: "same-origin",
          signal: AbortSignal.timeout(10_000),
        },
      );
      if (
        response.status !== 200 ||
        (await response.json()).revoked_count !== 1
      )
        throw new Error("second browser session was not revoked exactly once");
    }, targetSessionId);
    requireValue(
      (await secondContext.cookies(FASTI_ORIGIN)).find(
        (cookie) => cookie.name === "__Host-fasti_session",
      )?.value === secondCookie.value,
      "revoked-session probe lost its original cookie",
    );
    const denied = await readInventory(secondPage);
    requireValue(
      denied.status === 401 &&
        denied.contentType?.startsWith("application/problem+json") &&
        denied.body?.status === 401 &&
        denied.body.code === "browser_session_revoked" &&
        denied.body.capability_id === "access.client.list" &&
        !Object.hasOwn(denied.body, "clients") &&
        !Object.hasOwn(denied.body, "next"),
      "revoked-session client inventory was not denied with its typed problem",
    );
    requireInventory(await readInventory(page));
    return {
      capabilityId: "access.client.list",
      requestedLimit: 1,
      returnedClients: 1,
      nodeClientId: expectedClientId,
      nodeWitnessSource: "stopped_fasti_sqlite_node_state",
      cookieOnly: true,
      noStore: true,
      exactNodeClientObserved: true,
      secondSessionRevoked: true,
      retainedCookieDenied: true,
      denialStatus: 401,
      denialCode: "browser_session_revoked",
      originalSessionStillAuthorized: true,
    };
  } finally {
    await secondContext.close();
  }
}

async function verifyProviderSecretsAbsent() {
  const forbidden = input.forbiddenProviderValues;
  if (forbidden === undefined) return undefined;
  try {
    if (
      !Array.isArray(forbidden) ||
      forbidden.length === 0 ||
      forbidden.length > 64 ||
      forbidden.some(
        (value) => typeof value !== "string" || !value || value.length > 4096,
      )
    ) {
      throw new Error();
    }
    const values = await page.evaluate(() => [
      document.body.innerText,
      ...Object.entries(localStorage).flat(),
      ...Object.entries(sessionStorage).flat(),
    ]);
    values.push(page.url());
    for (const cookie of await context.cookies()) {
      values.push(cookie.name, cookie.value, cookie.domain, cookie.path);
    }
    if (
      forbidden.some((secret) => values.some((value) => value.includes(secret)))
    ) {
      throw new Error();
    }
    return true;
  } catch {
    // Neither a matching value nor inspected browser data belongs in evidence.
    throw new Error("Provider secret absence check failed.");
  }
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

async function searchFixture(fixture, { cachedOnly = false } = {}) {
  const provider = page.getByLabel("Metadata provider");
  await provider.selectOption(fixture.provider);
  if (cachedOnly) {
    await page
      .getByRole("checkbox", {
        name: "Use cached provider results only",
      })
      .check();
  }
  const search = page.getByRole("searchbox", {
    name: fixture.searchLabel,
  });
  await search.fill(fixture.title);
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page
    .getByRole("region", { name: "Search results" })
    .getByText(`2 results for ${fixture.title}.`, { exact: false })
    .waitFor();
  for (const providerId of FIXTURE_PROVIDER_IDS) {
    await requireCount(
      providerResult(providerId),
      1,
      `${fixture.provider} candidate ${providerId}`,
    );
  }
}

async function searchNoStoreFixture({ cachedOnly = false } = {}) {
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
  await search.fill(NO_STORE_TITLE);
  await page.getByRole("button", { name: "Search", exact: true }).click();
  const results = page.getByRole("region", { name: "Search results" });
  if (cachedOnly) {
    await results
      .getByRole("alert")
      .filter({ hasText: "unavailable" })
      .waitFor();
    for (const providerId of NO_STORE_PROVIDER_IDS)
      await requireCount(
        providerResult(providerId),
        0,
        `cached-only transient TMDB candidate ${providerId}`,
      );
    return;
  }
  await results
    .getByText(`2 results for ${NO_STORE_TITLE}.`, { exact: false })
    .waitFor();
  for (const providerId of NO_STORE_PROVIDER_IDS) {
    const result = providerResult(providerId);
    await requireCount(result, 1, `transient TMDB candidate ${providerId}`);
    await result
      .getByRole("link", { name: "View details" })
      .evaluate((link, expected) => {
        if (link.getAttribute("href") !== expected)
          throw new Error("transient candidate route is not canonical");
      }, `/explore/live/tmdb/film/${providerId}/candidate`);
  }
}

async function openNoStoreDetails(providerId) {
  const pendingResponse = page.waitForResponse((response) => {
    const url = new URL(response.url());
    return (
      url.origin === FASTI_ORIGIN &&
      url.pathname === "/api/v1/search/providers/tmdb/film/details" &&
      url.searchParams.get("provider_record_id") === providerId
    );
  });
  await providerResult(providerId)
    .getByRole("link", { name: "View details" })
    .click();
  const response = await pendingResponse;
  requireValue(response.status() === 200, "live detail HTTP read failed");
  requireValue(
    (await response.headerValue("cache-control")) === "private, no-store",
    "live detail HTTP response permits retention",
  );
  await page.waitForURL(
    (url) =>
      url.origin === FASTI_ORIGIN &&
      url.pathname === `/explore/live/tmdb/film/${providerId}/candidate`,
  );
  const details = page.locator("section").filter({
    has: page.getByRole("heading", { name: NO_STORE_TITLE, level: 1 }),
  });
  await details.getByText(NO_STORE_OVERVIEW, { exact: true }).waitFor();
  await details.getByText(providerId, { exact: true }).waitFor();
  await details.getByText("2020", { exact: true }).waitFor();
  return details;
}

async function releaseNoStoreCheckpoint(record) {
  const checkpoint = input.noStoreCheckpoint;
  requireValue(
    checkpoint &&
      typeof checkpoint.readyPath === "string" &&
      typeof checkpoint.continuePath === "string",
    "no-store durability checkpoint is unavailable",
  );
  const temporary = `${checkpoint.readyPath}.tmp`;
  await writeFile(temporary, `${JSON.stringify(record)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  await rename(temporary, checkpoint.readyPath);
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const decision = await readFile(checkpoint.continuePath, "ascii");
      if (decision.trim() !== "continue")
        throw new Error("no-store durability checkpoint was rejected");
      return;
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
  }
  throw new Error("no-store durability checkpoint timed out");
}

async function runM4NoStoreJourney() {
  requireValue(
    typeof input.attachRecordId === "string" &&
      /^rec_[0-9a-f]{32}$/.test(input.attachRecordId) &&
      typeof input.attachRecordPath === "string" &&
      input.attachRecordPath.startsWith(
        `/records/film/${input.attachRecordId}/`,
      ),
    "no-store Attach target is invalid",
  );
  const observed = [];
  const observeRequest = (request) => {
    const url = new URL(request.url());
    if (
      url.origin === FASTI_ORIGIN &&
      url.pathname.startsWith("/api/v1/search/")
    ) {
      observed.push({
        method: request.method(),
        path: url.pathname,
        providerRecordId: url.searchParams.get("provider_record_id"),
      });
    }
  };
  page.on("request", observeRequest);
  try {
    await signInToFasti();
    await page.goto(`${FASTI_ORIGIN}/discover`);
    await page.getByRole("heading", { name: "Discover", level: 1 }).waitFor();
    await searchNoStoreFixture();
    await requireNoAccessibilityViolations("live no-store Search results");

    await openNoStoreDetails(NO_STORE_PROVIDER_IDS[0]);
    await requireNoAccessibilityViolations("live no-store candidate details");
    await page.reload();
    const reloaded = page.locator("section").filter({
      has: page.getByRole("heading", { name: NO_STORE_TITLE, level: 1 }),
    });
    await reloaded.getByText(NO_STORE_OVERVIEW, { exact: true }).waitFor();
    await releaseNoStoreCheckpoint({
      stage: "details_only",
      route: new URL(page.url()).pathname,
      providerRecordId: NO_STORE_PROVIDER_IDS[0],
    });

    await reloaded.getByRole("button", { name: "Create Record" }).click();
    await page.waitForURL(
      (url) =>
        url.origin === FASTI_ORIGIN &&
        /^\/records\/film\/rec_[0-9a-f]{32}\//.test(url.pathname),
    );
    const recordPath = new URL(page.url()).pathname;
    const recordId = recordPath.split("/")[3];
    requireValue(
      /^rec_[0-9a-f]{32}$/.test(recordId),
      "no-store Create did not produce a canonical Record ID",
    );
    await page.getByText(recordId, { exact: true }).first().waitFor();

    await page.goto(
      `${FASTI_ORIGIN}/explore/live/tmdb/film/${NO_STORE_PROVIDER_IDS[1]}/candidate`,
    );
    const secondDetails = page.locator("section").filter({
      has: page.getByRole("heading", { name: NO_STORE_TITLE, level: 1 }),
    });
    await secondDetails.getByText(NO_STORE_OVERVIEW, { exact: true }).waitFor();
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
    await localSearch.fill(FIXTURE_TITLE);
    await dialog.getByRole("button", { name: "Find Records" }).click();
    const target = dialog.getByRole("radio", {
      name: new RegExp(input.attachRecordId),
    });
    await target.waitFor();
    await requireCount(target, 1, "no-store Attach target");
    await target.check();
    await requireNoAccessibilityViolations("no-store Attach Record picker");
    await dialog.getByRole("button", { name: "Confirm attachment" }).click();
    await page.waitForURL(
      (url) =>
        url.origin === FASTI_ORIGIN && url.pathname === input.attachRecordPath,
    );

    await page.goto(`${FASTI_ORIGIN}/discover`);
    await searchNoStoreFixture({ cachedOnly: true });
    requireValue(
      await page
        .getByRole("checkbox", { name: "Use cached provider results only" })
        .isChecked(),
      "cached-only no-store Search mode was not retained",
    );

    const counts = {
      providerSearch: observed.filter(
        ({ method, path }) =>
          method === "POST" && path === "/api/v1/search/providers/tmdb",
      ).length,
      liveDetails: Object.fromEntries(
        NO_STORE_PROVIDER_IDS.map((providerId) => [
          providerId,
          observed.filter(
            ({ method, path, providerRecordId }) =>
              method === "GET" &&
              path === "/api/v1/search/providers/tmdb/film/details" &&
              providerRecordId === providerId,
          ).length,
        ]),
      ),
      providerActions: observed.filter(
        ({ method, path }) =>
          method === "POST" &&
          path === "/api/v1/search/providers/tmdb/film/actions",
      ).length,
      retainedDetails: observed.filter(({ path }) =>
        path.startsWith("/api/v1/search/candidates/"),
      ).length,
    };
    requireValue(counts.providerSearch === 2, "no-store Search count differs");
    requireValue(
      counts.liveDetails[NO_STORE_PROVIDER_IDS[0]] === 2 &&
        counts.liveDetails[NO_STORE_PROVIDER_IDS[1]] === 1,
      "no-store live detail count differs",
    );
    requireValue(
      counts.providerActions === 2,
      "no-store provider action count differs",
    );
    requireValue(
      counts.retainedDetails === 0,
      "no-store journey used a retained candidate route",
    );
    return {
      recordId,
      recordPath,
      attachRecordId: input.attachRecordId,
      attachRecordPath: input.attachRecordPath,
      providerIds: [...NO_STORE_PROVIDER_IDS],
      browserRequests: counts,
      canonicalLiveRouteReloaded: true,
      cachedOnlyRejectedUnretainedResults: true,
      createCanonicalRecordObserved: true,
      attachCanonicalRecordObserved: true,
    };
  } finally {
    page.off("request", observeRequest);
  }
}

async function verifyRestartedNoStoreRecord() {
  requireValue(
    typeof input.recordId === "string" &&
      /^rec_[0-9a-f]{32}$/.test(input.recordId),
    "restart no-store Record ID is invalid",
  );
  await signInToFasti();
  await page.goto(`${FASTI_ORIGIN}${input.recordPath}`);
  await page.getByText(input.recordId, { exact: true }).first().waitFor();
  await page
    .getByRole("button", { name: "Sources & Identity (1)", exact: true })
    .click();
  const identifiers = page.getByRole("region", {
    name: "External identifiers",
  });
  const created = identifiers.getByRole("row").filter({
    has: page.getByRole("cell", {
      name: NO_STORE_PROVIDER_IDS[0],
      exact: true,
    }),
  });
  await requireCount(created, 1, "restarted created no-store TMDB identity");
  await page.goto(`${FASTI_ORIGIN}${input.attachRecordPath}`);
  await page
    .getByRole("button", { name: "Sources & Identity (3)", exact: true })
    .click();
  const attached = page
    .getByRole("region", { name: "External identifiers" })
    .getByRole("row")
    .filter({
      has: page.getByRole("cell", {
        name: NO_STORE_PROVIDER_IDS[1],
        exact: true,
      }),
    });
  await requireCount(attached, 1, "restarted attached no-store TMDB identity");
  await requireNoAccessibilityViolations("restarted no-store Record");
  return {
    recordId: input.recordId,
    recordPath: input.recordPath,
    attachRecordId: input.attachRecordId,
    attachRecordPath: input.attachRecordPath,
    exactExternalIdentifiersObserved: true,
    providerPayloadNotRequiredAfterRestart: true,
  };
}

async function openFixtureDetails(fixture, providerId) {
  const result = providerResult(providerId);
  await result.getByRole("link", { name: "View details" }).click();
  await page.waitForURL(
    (url) =>
      url.origin === FASTI_ORIGIN &&
      new RegExp(
        `^/explore/${fixture.provider}/${fixture.grain}/scr_[0-9a-f]{32}/${fixture.slug}$`,
      ).test(url.pathname),
  );
  const details = page.locator("section").filter({
    has: page.getByRole("heading", { name: fixture.title, level: 1 }),
  });
  await details.getByText(FIXTURE_OVERVIEW, { exact: true }).waitFor();
  await details.getByText(providerId, { exact: true }).waitFor();
  await details.getByText("2020", { exact: true }).waitFor();
  if (fixture.provider === "igdb") {
    await details.getByText("game", { exact: true }).waitFor();
  }
  return details;
}

async function runM4SearchJourney(fixture = m4Fixture()) {
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
    await searchFixture(fixture);
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

    const firstDetails = await openFixtureDetails(
      fixture,
      FIXTURE_PROVIDER_IDS[0],
    );
    await requireNoAccessibilityViolations("provider candidate details");
    await firstDetails.getByRole("button", { name: "Create Record" }).click();
    await page.waitForURL(
      (url) =>
        url.origin === FASTI_ORIGIN &&
        new RegExp(
          `^/records/${fixture.grain}/rec_[0-9a-f]{32}/${fixture.slug}$`,
        ).test(url.pathname),
    );
    const recordPath = new URL(page.url()).pathname;
    const recordId = recordPath.split("/")[3];
    requireValue(
      /^rec_[0-9a-f]{32}$/.test(recordId),
      "created Record ID is not canonical",
    );
    await page
      .getByRole("heading", { name: fixture.title, level: 1 })
      .waitFor();
    await page.getByText("Fasti Entity ID:", { exact: true }).waitFor();
    await page.getByText(recordId, { exact: true }).first().waitFor();

    await page.goto(`${FASTI_ORIGIN}/discover`);
    await searchFixture(fixture);
    const secondDetails = await openFixtureDetails(
      fixture,
      FIXTURE_PROVIDER_IDS[1],
    );
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
    await searchFixture(fixture, { cachedOnly: true });
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
          method === "POST" &&
          path === `/api/v1/search/providers/${fixture.provider}`,
      ).length,
      candidateDetails: observed.filter(
        ({ method, path }) =>
          method === "GET" &&
          new RegExp(
            `^/api/v1/search/candidates/${fixture.provider}/${fixture.grain}/scr_[0-9a-f]{32}$`,
          ).test(path),
      ).length,
      candidateActions: observed.filter(
        ({ method, path }) =>
          method === "POST" &&
          new RegExp(
            `^/api/v1/search/candidates/${fixture.provider}/${fixture.grain}/scr_[0-9a-f]{32}/actions$`,
          ).test(path),
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
      ...(fixture.provider === "igdb" ? { cacheState: "fresh" } : {}),
      browserRequests: counts,
      liveProviderDetailsObserved: true,
      createCanonicalRecordObserved: true,
      attachCanonicalRecordObserved: true,
      providerSecretsAbsent: await verifyProviderSecretsAbsent(),
    };
  } finally {
    page.off("request", observeRequest);
  }
}

async function verifyRestartedRecord() {
  const fixture = m4Fixture(input.fixtureProvider);
  requireValue(
    typeof input.recordId === "string" &&
      /^rec_[0-9a-f]{32}$/.test(input.recordId),
    "restart Record ID is invalid",
  );
  requireValue(
    typeof input.recordPath === "string" &&
      input.recordPath ===
        `/records/${fixture.grain}/${input.recordId}/${fixture.slug}`,
    "restart Record path does not match its canonical identity",
  );
  await page.goto(`${FASTI_ORIGIN}${input.recordPath}`);
  await page.waitForURL(
    (url) => url.origin === FASTI_ORIGIN && url.pathname === input.recordPath,
  );
  await page.getByRole("heading", { name: fixture.title, level: 1 }).waitFor();
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
    const row = identifiers.getByRole("row").filter({
      has: page.getByRole("cell", { name: providerId, exact: true }),
    });
    await row.waitFor();
    await requireCount(
      row,
      1,
      `persisted ${fixture.provider} identity ${providerId}`,
    );
    const cells = row.getByRole("cell");
    await requireCount(cells, 5, "persisted identifier columns");
    requireValue(
      (await cells.nth(0).innerText()).trim() === fixture.namespace,
      `persisted ${fixture.provider} identity ${providerId} has the wrong namespace`,
    );
  }
  await requireNoAccessibilityViolations("restarted canonical Record");
  return {
    recordId: input.recordId,
    recordPath: input.recordPath,
    providerIds: [...FIXTURE_PROVIDER_IDS],
    canonicalRecordObservedAfterRestart: true,
    exactExternalIdentifiersObserved: true,
    providerSecretsAbsent: await verifyProviderSecretsAbsent(),
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
    const clientInventory =
      input.clientInventoryJourney === true
        ? await runClientInventoryJourney()
        : undefined;
    const m4SearchJourney = input.m4SearchJourney
      ? await runM4SearchJourney()
      : undefined;
    const m4IgdbJourney = input.m4IgdbJourney
      ? await runM4SearchJourney(m4Fixture("igdb"))
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
        ...(clientInventory ? { clientInventory } : {}),
        ...(m4SearchJourney ? { m4SearchJourney } : {}),
        ...(m4IgdbJourney ? { m4IgdbJourney } : {}),
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
  } else if (input.mode === "m4-no-store") {
    const m4NoStoreJourney = await runM4NoStoreJourney();
    process.stdout.write(
      JSON.stringify({
        chromium: browser.version(),
        m4NoStoreJourney,
      }),
    );
  } else if (input.mode === "restart-no-store-record") {
    const m4NoStoreRestart = await verifyRestartedNoStoreRecord();
    process.stdout.write(
      JSON.stringify({
        chromium: browser.version(),
        m4NoStoreRestart,
      }),
    );
  } else {
    throw new Error("unknown proof mode");
  }
} finally {
  input.password = "";
  await browser.close();
}
