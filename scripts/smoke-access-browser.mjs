import { chromium } from "@playwright/test";

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
    await page.goto("http://127.0.0.1:8420/first-run");
    await page
      .getByRole("button", { name: "Sign in to an existing account" })
      .click();
    await page.waitForURL((url) => url.origin === "http://127.0.0.1:4000");
    await trailBaseLogin();
    await page.waitForURL(
      (url) =>
        url.origin === "http://127.0.0.1:8420" &&
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
      await page.goto("http://127.0.0.1:8420/settings/account");
    }
    await page.getByRole("heading", { name: "Account and security" }).waitFor();

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
        fastiOriginVendorCredentialStorageAbsent: true,
      }),
    );
  } else {
    throw new Error("unknown proof mode");
  }
} finally {
  input.password = "";
  await browser.close();
}
