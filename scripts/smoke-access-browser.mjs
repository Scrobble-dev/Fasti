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
      /trailbase|(?:access|auth|id|refresh)[_:.-]?token|[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/i;
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
