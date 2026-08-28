import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

const health = { status: "healthy", version: "0.1.0" };
const healthEndpoint = /\/api\/v1\/health$/;
const viewports = [
  { width: 320, height: 800 },
  { width: 768, height: 900 },
  { width: 1440, height: 1000 },
] as const;

async function mockHealth(page: Page) {
  await page.route(healthEndpoint, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(health),
    }),
  );
}

async function mockTrustedHost(page: Page) {
  await page.addInitScript(() => {
    const networkConfiguration = {
      connection: {
        service_url: {
          value: "http://127.0.0.1:8420",
          source: "default",
          managed: false,
        },
        public_url: { value: null, source: "default", managed: false },
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
    const providerStatus = [
      {
        provider: "google-books",
        label: "Google Books",
        configured: false,
        source: "none",
        writable: true,
        docs_url: "https://developers.google.com/books/docs/v1/using",
      },
    ];
    const browserWindow = window as typeof window & {
      __PROVIDER_SECRET_MATCH__?: boolean;
      __TAURI_INTERNALS__: {
        invoke: (command: string, arguments_: unknown) => Promise<unknown>;
      };
    };
    browserWindow.__TAURI_INTERNALS__ = {
      invoke: async (command, arguments_) => {
        switch (command) {
          case "setup_status":
            return { phase: "ready", proof_cleanup_pending: false };
          case "load_network_configuration":
            return networkConfiguration;
          case "provider_credential_status":
            return providerStatus;
          case "test_endpoint_connection":
            return {
              endpoint: "http://127.0.0.1:8420",
              scheme: "http",
              status: "healthy",
              version: "0.1.0-test",
            };
          case "save_provider_credential": {
            const candidate = arguments_ as {
              input?: { provider?: string; credential?: string };
            };
            browserWindow.__PROVIDER_SECRET_MATCH__ =
              candidate.input?.provider === "google-books" &&
              candidate.input?.credential === "test-secret-not-retained";
            throw {
              code: "secure_storage_unavailable",
              title: "Secure storage is unavailable",
              detail: "The credential store rejected the test value.",
              next_action: "Unlock the credential store, then retry.",
            };
          }
          default:
            throw new Error(`Unexpected trusted-host command: ${command}`);
        }
      },
    };
  });
}

test("the development browser user can sign in, edit, and delete itself", async ({
  page,
}) => {
  const userId = "usr_01991f58-8e00-7000-8000-000000000001";
  let user = {
    active: true,
    created_at: "2026-08-28T00:00:00Z",
    is_admin: true,
    is_test_account: true,
    updated_at: "2026-08-28T00:00:00Z",
    user_id: userId,
    username: "testadmin",
  };
  let deleted = false;

  await page.route(/\/api\/v1\/browser\/session$/, async (route) => {
    const request = route.request();
    if (request.method() === "GET") {
      await route.fulfill({
        status: 401,
        contentType: "application/problem+json",
        body: JSON.stringify({ detail: "Sign in to continue." }),
      });
      return;
    }
    const body = request.postDataJSON();
    expect(body).toMatchObject({
      username: user.username,
      password: user.username,
      session_timeout_minutes: 60,
    });
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      headers: {
        "set-cookie": "fasti_csrf=csrf-test; Path=/; SameSite=Strict",
      },
      body: JSON.stringify({
        expires_at: "2026-08-28T01:00:00Z",
        user,
      }),
    });
  });
  await page.route(/\/api\/v1\/browser\/users(?:\/[^/]+)?$/, async (route) => {
    const request = route.request();
    if (request.method() === "GET") {
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({ users: deleted ? [] : [user] }),
      });
      return;
    }
    expect(request.headers()["x-fasti-csrf"]).toBe("csrf-test");
    const body = request.postDataJSON();
    if (request.method() === "PATCH") {
      expect(body.current_password).toBe(user.username);
      user = {
        ...user,
        username: body.username,
        updated_at: "2026-08-28T00:05:00Z",
      };
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify(user),
      });
      return;
    }
    expect(body).toEqual({ current_password: "editedadmin" });
    deleted = true;
    await route.fulfill({ status: 204 });
  });
  await page.route(/\/api\/v1\/records$/, (route) =>
    route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ records: [] }),
    }),
  );

  await page.goto("/?surface=workbench");
  await page.getByRole("button", { name: "Sign in" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Password").fill("testadmin");
  await dialog.getByRole("button", { name: "Sign in" }).click();
  await expect(
    page.getByRole("button", { name: "Manage account testadmin" }),
  ).toBeVisible();

  await dialog.getByRole("button", { name: "Edit" }).click();
  await dialog.getByLabel("Username").fill("editedadmin");
  await dialog.getByLabel(/New password/).fill("editedadmin");
  await dialog.getByLabel("Your current password").fill("testadmin");
  await dialog.getByRole("button", { name: "Save changes" }).click();
  await expect(dialog.getByRole("status")).toContainText("Sign in again");
  await expect(dialog.getByLabel("Username")).toHaveValue("editedadmin");

  await dialog.getByLabel("Password").fill("editedadmin");
  await dialog.getByRole("button", { name: "Sign in" }).click();
  await dialog.getByRole("button", { name: "Edit" }).click();
  await dialog.getByLabel("Your current password").fill("editedadmin");
  await dialog.getByRole("checkbox", { name: /cannot be undone/ }).check();
  await dialog.getByRole("button", { name: "Delete user" }).click();

  await expect(dialog.getByRole("status")).toHaveText(
    "Account deleted. The development seed will not recreate it.",
  );
  await expect(page.getByLabel("Sign in", { exact: true })).toBeVisible();
  expect(deleted).toBe(true);
  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
});

test("global search and configured record actions use durable tracking state", async ({
  page,
  context,
}) => {
  const recordId = "rec_01991f588e0070008000000000000002";
  const csrf = "a".repeat(64);
  let updatedDisposition: string | null = null;

  await context.addCookies([
    {
      name: "fasti_csrf",
      value: csrf,
      url: "http://127.0.0.1:4173",
      sameSite: "Strict",
    },
  ]);
  await page.addInitScript(() => {
    localStorage.setItem(
      "fasti-workbench-preferences",
      JSON.stringify({
        contextMenuItems: [
          { id: "view", label: "View Details", visible: false, order: 0 },
        ],
      }),
    );
  });
  await page.route(/\/api\/v1\/browser\/session$/, (route) =>
    route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        expires_at: "2026-08-28T12:00:00Z",
        user: {
          active: true,
          created_at: "2026-08-28T00:00:00Z",
          is_admin: true,
          is_test_account: true,
          updated_at: "2026-08-28T00:00:00Z",
          user_id: "usr_01991f58-8e00-7000-8000-000000000001",
          username: "testadmin",
        },
      }),
    }),
  );
  await page.route(/\/api\/v1\/records$/, (route) =>
    route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        records: [
          {
            grain: "film",
            latest_activity: {
              occurred_at: null,
              interpretation_state: "resolved",
            },
            poster: {
              is_stale: false,
              tier: "empty",
              value: null,
              source: null,
            },
            record_id: recordId,
            status: "active",
            title: {
              is_stale: false,
              tier: "user_override",
              value: "Alpha Film",
              source: "local",
            },
          },
        ],
      }),
    }),
  );
  await page.route(
    /\/api\/v1\/profile\/record-tracking-dispositions(?:\/[^/]+)?$/,
    async (route) => {
      const request = route.request();
      if (request.method() === "GET") {
        await route.abort("failed");
        return;
      }
      expect(request.method()).toBe("PUT");
      expect(request.headers()["x-fasti-csrf"]).toBe(csrf);
      updatedDisposition = request.postDataJSON().disposition;
      await route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({
          record_id: recordId,
          disposition: updatedDisposition,
        }),
      });
    },
  );

  await page.goto("/library");
  await expect(page.getByRole("heading", { name: "Library" })).toBeVisible();
  await expect(page.getByText("Alpha Film", { exact: true })).toBeVisible();
  await expect(page.getByRole("alert")).toContainText(
    "Records still use their activity fallback.",
  );

  const search = page.getByRole("combobox", {
    name: "Search records or commands",
  });
  await search.fill("Alpha");
  await expect(page.getByRole("option", { name: /Alpha Film/ })).toBeVisible();
  await search.press("Enter");
  await expect(page.getByRole("heading", { name: "Alpha Film" })).toBeVisible();
  await expect(
    page.getByRole("combobox", { name: "Profile tracking state" }),
  ).toHaveValue("unset");
  await page.getByRole("button", { name: "Back to Library" }).click();

  await page.getByRole("group", { name: "Alpha Film card" }).click({
    button: "right",
  });
  const menu = page.getByRole("menu");
  await expect(
    menu.getByText("Playback & tracking", { exact: true }),
  ).toBeVisible();
  await expect(
    menu.getByText("Library & lists", { exact: true }),
  ).toBeVisible();
  await expect(
    menu.getByText("Identity & metadata", { exact: true }),
  ).toBeVisible();
  await expect(
    menu.getByRole("menuitem", { name: /View media details/ }),
  ).toHaveCount(0);
  await expect(
    menu.getByRole("menuitem", { name: /Update progress and episodes/ }),
  ).toBeDisabled();
  await menu.getByRole("menuitem", { name: "Mark as on hold" }).click();
  await expect(page.getByRole("status")).toHaveText(
    "Tracking state set to on hold.",
  );
  await expect(page.getByText("on hold", { exact: true })).toBeVisible();
  expect(updatedDisposition).toBe("on_hold");

  await page.keyboard.press("Control+K");
  await expect(search).toBeFocused();
  await search.fill("Settings");
  await search.press("Enter");
  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();

  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
});

test("record metadata can refresh or switch through a configured provider", async ({
  page,
}) => {
  const recordId = "rec_01991f588e0070008000000000000003";
  await page.setViewportSize({ width: 320, height: 900 });
  await page.addInitScript((id) => {
    let title = "Dune";
    let overview = "A noble family becomes involved in a war for Arrakis.";
    let identifiers = [{ namespace: "tmdb", grain: "film", value: "438631" }];
    const browserWindow = window as typeof window & {
      __METADATA_CALLS__?: unknown[];
      __TAURI_INTERNALS__: {
        invoke: (command: string, arguments_: unknown) => Promise<unknown>;
      };
    };
    browserWindow.__METADATA_CALLS__ = [];
    browserWindow.__TAURI_INTERNALS__ = {
      invoke: async (command, arguments_) => {
        switch (command) {
          case "setup_status":
            return { phase: "ready", proof_cleanup_pending: false };
          case "provider_credential_status":
            return [
              {
                provider: "tmdb",
                label: "TMDB",
                configured: true,
                source: "credential_store",
                writable: true,
                docs_url: "https://developer.themoviedb.org/",
              },
            ];
          case "list_tracking_dispositions":
            return [];
          case "list_records":
            return [
              {
                grain: "film",
                identifiers,
                latest_activity: null,
                original_title: {
                  is_stale: false,
                  tier: "fallback_provider_claim",
                  value: title,
                  source: "tmdb",
                },
                overview: {
                  is_stale: false,
                  tier: "fallback_provider_claim",
                  value: overview,
                  source: "tmdb",
                },
                poster: {
                  is_stale: false,
                  tier: "empty",
                  value: null,
                  source: null,
                },
                record_id: id,
                release_year: {
                  is_stale: false,
                  tier: "fallback_provider_claim",
                  value: "2021",
                  source: "tmdb",
                },
                status: "active",
                title: {
                  is_stale: false,
                  tier: "fallback_provider_claim",
                  value: title,
                  source: "tmdb",
                },
              },
            ];
          case "search_provider":
            return [
              {
                provider: "tmdb",
                provider_id: "693134",
                title: "Dune: Part Two",
                original_title: "Dune: Part Two",
                kind: "movie",
                release_year: 2024,
                authors: [],
                image_url: null,
                overview: "Paul Atreides unites with Chani and the Fremen.",
              },
            ];
          case "apply_provider_metadata": {
            const call = (arguments_ as { input: unknown }).input as {
              record_id: string;
              selection: {
                provider: string;
                provider_id: string;
                kind: string;
              };
            };
            browserWindow.__METADATA_CALLS__?.push(call);
            if (call.selection.provider_id === "693134") {
              title = "Dune: Part Two";
              overview = "Paul Atreides unites with Chani and the Fremen.";
              identifiers = [
                ...identifiers,
                { namespace: "tmdb", grain: "film", value: "693134" },
              ];
            }
            return undefined;
          }
          default:
            throw new Error(`Unexpected trusted-host command: ${command}`);
        }
      },
    };
  }, recordId);

  await page.goto(`/records/${recordId}`);
  await expect(
    page.getByRole("heading", { level: 1, name: "Dune" }),
  ).toBeVisible();
  await page.getByRole("button", { name: /Sources & Identity/ }).click();

  await page.getByRole("button", { name: "Refresh" }).click();
  await expect(page.getByText("Refreshed metadata from tmdb.")).toBeVisible();
  await page
    .getByRole("searchbox", { name: "Search title, creator, or identifier" })
    .fill("Dune Part Two");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(page.getByText("Dune: Part Two", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Use metadata" }).click();
  await expect(
    page.getByRole("heading", { level: 1, name: "Dune: Part Two" }),
  ).toBeVisible();

  expect(
    await page.evaluate(
      () =>
        (
          window as typeof window & {
            __METADATA_CALLS__?: unknown[];
          }
        ).__METADATA_CALLS__,
    ),
  ).toEqual([
    {
      record_id: recordId,
      selection: { provider: "tmdb", provider_id: "438631", kind: "movie" },
    },
    {
      record_id: recordId,
      selection: { provider: "tmdb", provider_id: "693134", kind: "movie" },
    },
  ]);
  expect(
    await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    ),
  ).toBeLessThanOrEqual(0);
  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
});

for (const theme of ["light", "dark"] as const) {
  for (const viewport of viewports) {
    test(`${theme} theme at ${viewport.width}px is truthful, reflowable, and accessible`, async ({
      page,
    }, testInfo) => {
      await page.setViewportSize(viewport);
      await page.addInitScript(
        (value) => localStorage.setItem("fasti-theme", value),
        theme,
      );
      await mockHealth(page);
      await page.goto("/");

      await expect(page).toHaveTitle("Local service status · Fasti");
      await expect(page.getByRole("heading", { level: 1 })).toHaveText(
        "Local service status",
      );
      await expect(
        page.getByRole("heading", { name: "Local service available" }),
      ).toBeVisible();
      await expect(
        page.getByRole("heading", { name: "Network settings" }),
      ).toBeVisible();
      await expect(
        page
          .locator("#network-settings dd")
          .filter({ hasText: "http://127.0.0.1:8420" }),
      ).toBeVisible();
      await expect(page.getByText("http://localhost:8420")).toBeVisible();
      await expect(page.locator("html")).toHaveAttribute(
        "data-bs-theme",
        theme,
      );
      await expect(page.getByText("Review inbox", { exact: true })).toHaveCount(
        0,
      );
      await expect(page.getByText("Discover", { exact: true })).toHaveCount(0);
      await expect(page.getByText("Connections", { exact: true })).toHaveCount(
        0,
      );

      const overflow = await page.evaluate(
        () =>
          document.documentElement.scrollWidth -
          document.documentElement.clientWidth,
      );
      expect(overflow).toBeLessThanOrEqual(0);

      for (const control of await page.getByRole("button").all()) {
        const box = await control.boundingBox();
        expect(box?.width).toBeGreaterThanOrEqual(44);
        expect(box?.height).toBeGreaterThanOrEqual(44);
      }

      const accessibility = await new AxeBuilder({ page }).analyze();
      expect(accessibility.violations).toEqual([]);
      await page.screenshot({
        path: testInfo.outputPath(`fasti-shell-${theme}-${viewport.width}.png`),
        fullPage: true,
        animations: "disabled",
      });
    });
  }
}

test("keyboard path, theme persistence, and unavailable recovery remain clear", async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await page.route(healthEndpoint, (route) => route.abort("connectionrefused"));
  await page.goto("/");

  await expect(page.getByRole("alert")).toContainText(
    "local service is unavailable",
  );
  await page.keyboard.press("Tab");
  await expect(
    page.getByRole("link", { name: "Skip to main content" }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator("main")).toBeFocused();

  const themeButton = page.getByRole("button", { name: "Use dark theme" });
  await themeButton.click();
  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "dark");
  await expect(
    page.getByRole("button", { name: "Use light theme" }),
  ).toBeVisible();
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "dark");
  await page.getByRole("button", { name: "Use light theme" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "light");
  await expect(
    page.getByRole("button", { name: "Use dark theme" }),
  ).toBeVisible();

  await expect(page.getByRole("button", { name: "Try again" })).toBeVisible();
  await page.getByRole("button", { name: "Try again" }).click();
  await expect(page.getByRole("alert")).toBeVisible();
  await expect(page.getByRole("button", { name: "Try again" })).toBeFocused();
  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
  await page.screenshot({
    path: testInfo.outputPath("fasti-shell-unavailable-320.png"),
    fullPage: true,
    animations: "disabled",
  });
});

test("invalid health responses use the contract recovery state", async ({
  page,
}, testInfo) => {
  await page.route(healthEndpoint, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ status: "healthy" }),
    }),
  );
  await page.goto("/");

  await expect(
    page.getByRole("heading", {
      name: "The local service returned an invalid response",
    }),
  ).toBeVisible();
  await expect(page.getByText("generated health contract")).toBeVisible();
  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
  await page.screenshot({
    path: testInfo.outputPath("fasti-shell-invalid-response.png"),
    fullPage: true,
    animations: "disabled",
  });
});

test("the loading state prevents duplicate concurrent retries", async ({
  page,
}) => {
  let requestCount = 0;
  let releaseCurrentResponse: () => void;
  const currentResponse = new Promise<void>((resolve) => {
    releaseCurrentResponse = resolve;
  });
  await page.route(healthEndpoint, async (route) => {
    requestCount += 1;
    if (requestCount === 1) {
      await route.abort("connectionrefused");
      return;
    }
    if (requestCount === 2) {
      await currentResponse;
    }
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(health),
    });
  });
  await page.goto("/");
  const retry = page.getByRole("button", { name: "Try again" });
  await expect(retry).toBeVisible();

  await retry.evaluate((button) => {
    button.click();
    button.click();
  });

  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      ),
  );
  expect(requestCount).toBe(2);
  await expect(page.getByText("Checking the local service")).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
  releaseCurrentResponse!();
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
});

test("the Vite proxy reaches the bounded health fixture", async ({ page }) => {
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
  await expect(page.getByText("0.1.0-test")).toBeVisible();
});

test("the saved theme is applied before the application module", async ({
  page,
}) => {
  await page.addInitScript(() => localStorage.setItem("fasti-theme", "dark"));
  await page.route(/\/src\/main\.ts$/, (route) => route.abort());
  await page.goto("/");

  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "dark");
  await expect(page.locator("body")).toHaveCSS(
    "background-color",
    "rgb(17, 17, 15)",
  );
  await expect(page.locator("body")).toHaveCSS("color", "rgb(255, 253, 248)");
});

test("system dark mode survives unavailable local storage", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.addInitScript(() => {
    Object.defineProperty(Storage.prototype, "getItem", {
      value: () => {
        throw new DOMException("Storage is unavailable", "SecurityError");
      },
    });
  });
  await mockHealth(page);
  await page.goto("/");

  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "dark");
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
});

test("theme changes remain usable when persistence is unavailable", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(Storage.prototype, "setItem", {
      value: () => {
        throw new DOMException("Storage is unavailable", "SecurityError");
      },
    });
  });
  await mockHealth(page);
  await page.goto("/");

  await page.getByRole("button", { name: "Use dark theme" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-bs-theme", "dark");
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
});

test("text enlargement and WCAG text spacing do not lose content", async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 800 });
  await mockHealth(page);
  await page.goto("/");

  await page.locator("html").evaluate((element) => {
    element.style.fontSize = "200%";
  });
  expect(
    await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    ),
  ).toBeLessThanOrEqual(0);
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();

  await page.locator("html").evaluate((element) => {
    element.style.fontSize = "100%";
    const style = document.createElement("style");
    style.textContent = `
      * { line-height: 1.5 !important; letter-spacing: 0.12em !important; word-spacing: 0.16em !important; }
      p { margin-block-end: 2em !important; }
    `;
    document.head.append(style);
  });
  expect(
    await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    ),
  ).toBeLessThanOrEqual(0);
  await expect(page.getByText("Catalogue, review, playback")).toBeVisible();
});

test("reduced motion stops the loading animation", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  let releaseHealth = () => {};
  const pendingHealth = new Promise<void>((resolve) => {
    releaseHealth = resolve;
  });
  await page.route(healthEndpoint, async (route) => {
    await pendingHealth;
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(health),
    });
  });
  await page.goto("/", { waitUntil: "domcontentloaded" });

  await expect(page.getByText("Checking the local service")).toBeVisible();
  await expect(page.locator(".spinner")).toHaveCSS("animation-name", "none");
  releaseHealth();
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
});

test("forced colors preserves visible status and controls", async ({
  page,
}) => {
  await page.emulateMedia({ forcedColors: "active" });
  await mockHealth(page);
  await page.goto("/");

  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Use dark theme" }),
  ).toBeVisible();
  await expect(
    page.getByRole("link", { name: "Skip to main content" }),
  ).toBeAttached();
  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
});

test("the harness does not contact third-party origins", async ({ page }) => {
  const externalOrigins = new Set<string>();
  page.on("request", (request) => {
    const origin = new URL(request.url()).origin;
    if (origin !== "http://127.0.0.1:4173") externalOrigins.add(origin);
  });
  await mockHealth(page);
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Local service available" }),
  ).toBeVisible();

  expect([...externalOrigins]).toEqual([]);
});

test("trusted-host provider settings clear a rejected secret", async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 320, height: 900 });
  await mockTrustedHost(page);
  await page.goto("/settings");

  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  await page.getByRole("button", { name: "Metadata credentials" }).click();
  await expect(page.getByText("No credential is configured.")).toBeVisible();

  const credential = page.getByLabel("New credential");
  await credential.fill("test-secret-not-retained");
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByRole("alert")).toContainText(
    "The credential store rejected the test value.",
  );
  await expect(credential).toHaveValue("");
  expect(
    await page.evaluate(
      () =>
        (window as typeof window & { __PROVIDER_SECRET_MATCH__?: boolean })
          .__PROVIDER_SECRET_MATCH__,
    ),
  ).toBe(true);
  expect((await page.locator("body").textContent()) ?? "").not.toContain(
    "test-secret-not-retained",
  );

  const undersizedControls = await page
    .locator("button:visible, input:visible")
    .evaluateAll((controls) =>
      controls.flatMap((control) => {
        const box = control.getBoundingClientRect();
        return box.width < 44 || box.height < 44
          ? [
              `${control.getAttribute("aria-label") ?? control.textContent?.trim()}: ${box.width}x${box.height}`,
            ]
          : [];
      }),
    );
  expect(undersizedControls).toEqual([]);

  const accessibility = await new AxeBuilder({ page }).analyze();
  expect(accessibility.violations).toEqual([]);
  await page.screenshot({
    path: testInfo.outputPath("provider-settings-rejected-secret-320.png"),
    fullPage: true,
    animations: "disabled",
  });
});
