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

async function mockRecords(page: Page, credential: string) {
  const authorizations: Array<string | undefined> = [];
  await page.route(/\/api\/v1\/records$/, (route) => {
    authorizations.push(route.request().headers().authorization);
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        records: [
          {
            record_id: "018f7f2d-8f58-7a0a-8000-000000000001",
            grain: "work",
            status: "active",
            title: {
              tier: "preferred_provider_claim",
              value: "A real local record",
              source: "google-books",
              is_stale: false,
            },
            poster: {
              tier: "empty",
              value: null,
              source: null,
              is_stale: false,
            },
            latest_activity: {
              interpretation_state: "resolved",
              occurred_at: {
                original: "2026-08-27T12:00:00Z",
                precision: "second",
                trust: "device_observed",
              },
            },
          },
        ],
      }),
    });
  });
  return authorizations;
}

async function connectBrowserRecords(page: Page, credential: string) {
  const authorizations = await mockRecords(page, credential);
  await page.goto("/");
  await page.getByRole("button", { name: "Connect records" }).click();
  await page.getByLabel("API client credential").fill(credential);
  await page.getByRole("button", { name: "Connect", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "A real local record" }),
  ).toBeVisible();
  expect(authorizations).toEqual([`Bearer ${credential}`]);
}

test("the product root restores the Workbench and ignores the retired surface preference", async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem("fasti-surface", "status"),
  );
  await page.goto("/");

  await expect(page).toHaveTitle("Fasti · Living Chronicle");
  await expect(
    page.getByRole("complementary", { name: "Main Navigation" }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Overview" })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Connect local credential" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Open Workbench" }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "Local service status" }),
  ).toHaveCount(0);
});

test("browser record access uses a real memory-only bearer and exposes unsupported actions honestly", async ({
  page,
}) => {
  const credential = "a".repeat(64);
  await page.addInitScript((retiredCredential) => {
    localStorage.setItem("fasti-bearer-credential", retiredCredential);
  }, credential);
  const authorizations = await mockRecords(page, credential);
  await page.goto("/");

  await expect(page.getByRole("alert")).toContainText(
    "Records need an active local bearer credential",
  );
  await page.getByRole("button", { name: "Connect local credential" }).click();
  await page.getByRole("tab", { name: "Passkey" }).click();
  await expect(
    page.getByRole("heading", { name: "Passkey sign-in is not active" }),
  ).toBeVisible();
  await page.getByRole("tab", { name: "OIDC / SSO" }).click();
  await expect(
    page.getByRole("heading", { name: "OIDC and SSO are not active" }),
  ).toBeVisible();
  await page.getByRole("tab", { name: "NuvioTV Device" }).click();
  await expect(
    page.getByRole("heading", {
      name: "NuvioTV device pairing is not active",
    }),
  ).toBeVisible();
  await page.getByRole("tab", { name: "Master Password" }).click();
  await expect(
    page.getByRole("heading", {
      name: "Master-password sign-in is not active",
    }),
  ).toBeVisible();
  await page.getByRole("tab", { name: "API Credential" }).click();
  const input = page.getByLabel("API client credential");
  await input.fill("not-a-credential");
  await expect(
    page.getByText("Enter exactly 64 hexadecimal characters."),
  ).toHaveClass(/problem/);
  await expect(input).toHaveAttribute("aria-invalid", "true");
  await expect(
    page.getByRole("button", { name: "Connect", exact: true }),
  ).toBeDisabled();

  await input.fill(credential);
  await expect(input).toHaveAttribute("aria-invalid", "false");
  await page.getByRole("button", { name: "Connect", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "A real local record" }),
  ).toBeVisible();
  expect(authorizations).toEqual([`Bearer ${credential}`]);
  await expect(
    page.getByRole("button", { name: "Clear browser credential" }),
  ).toBeVisible();
  await expect(
    page
      .getByRole("button", { name: "Watch-state changes unavailable" })
      .first(),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Watchlists unavailable" }).first(),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Collections unavailable" }).first(),
  ).toBeDisabled();
  await expect(
    page
      .getByRole("button", { name: "Ratings and reviews unavailable" })
      .first(),
  ).toBeDisabled();

  await page.getByRole("button", { name: "Library", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Library" })).toBeVisible();
  await expect(page.getByText("tracking state unavailable")).toBeVisible();
  await page
    .getByRole("button", { name: "A real local record", exact: true })
    .click();
  await expect(
    page.getByRole("combobox", { name: "User rating unavailable" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("combobox", { name: "Watch status unavailable" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Actions & Progress" }).click();
  const iconActions = page.locator(".icon-action-btn");
  expect(await iconActions.count()).toBeGreaterThan(0);
  for (const control of await iconActions.all()) {
    const box = await control.boundingBox();
    expect(box?.width).toBeGreaterThanOrEqual(44);
    expect(box?.height).toBeGreaterThanOrEqual(44);
  }
  await expect(
    page.getByText("This host exposes this record as read-only."),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: /Update Progress/ }),
  ).toBeDisabled();

  expect(
    await page.evaluate(() =>
      Object.keys(localStorage).filter((key) => key.includes("credential")),
    ),
  ).toEqual([]);
  await page.reload();
  await expect(page.getByRole("alert")).toContainText(
    "Records need an active local bearer credential",
  );
});

for (const theme of ["light", "dark"] as const) {
  for (const viewport of viewports) {
    test(`Workbench ${theme} theme at ${viewport.width}px is reflowable and accessible`, async ({
      page,
    }, testInfo) => {
      const credential = "b".repeat(64);
      await page.setViewportSize(viewport);
      await page.addInitScript((mode) => {
        localStorage.setItem(
          "fasti-theme-settings",
          JSON.stringify({
            mode,
            accentColor: "#a22f2b",
            density: "normal",
          }),
        );
      }, theme);
      await connectBrowserRecords(page, credential);

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
      const undersizedControls = await page
        .locator("button:visible, input:visible, select:visible")
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
        path: testInfo.outputPath(
          `fasti-workbench-${theme}-${viewport.width}.png`,
        ),
        fullPage: true,
        animations: "disabled",
      });
    });
  }
}

test("semantic badge contrast remains AA in both themes", async ({ page }) => {
  await page.goto("/");
  for (const theme of ["light", "dark"] as const) {
    const ratios = await page.evaluate((mode) => {
      document.documentElement.setAttribute("data-bs-theme", mode);
      const parseColor = (value: string): number[] => {
        const channels =
          value
            .match(/[\d.]+/g)
            ?.slice(0, 3)
            .map(Number) ?? [];
        if (channels.length !== 3) return [];
        return value.startsWith("color(")
          ? channels.map((channel) => channel * 255)
          : channels;
      };
      const luminance = (channels: number[]): number =>
        channels
          .map((channel) => channel / 255)
          .map((channel) =>
            channel <= 0.04045
              ? channel / 12.92
              : ((channel + 0.055) / 1.055) ** 2.4,
          )
          .reduce(
            (sum, channel, index) =>
              sum + channel * [0.2126, 0.7152, 0.0722][index],
            0,
          );
      const contrast = (background: string, foreground: string): number => {
        const left = luminance(parseColor(background));
        const right = luminance(parseColor(foreground));
        return (Math.max(left, right) + 0.05) / (Math.min(left, right) + 0.05);
      };
      return [
        ["--fasti-brand-mark", "--fasti-brand-contrast"],
        ["--fasti-state-verified", "--fasti-verified-contrast"],
      ].map(([background, foreground]) => {
        const sample = document.createElement("span");
        sample.style.background = `var(${background})`;
        sample.style.color = `var(${foreground})`;
        sample.textContent = "Status";
        document.body.append(sample);
        const style = getComputedStyle(sample);
        const ratio = contrast(style.backgroundColor, style.color);
        sample.remove();
        return ratio;
      });
    }, theme);
    for (const ratio of ratios) expect(ratio).toBeGreaterThanOrEqual(4.5);
  }
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
      await page.goto("/status");

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
          .filter({ hasText: "http://127.0.0.1:4173" }),
      ).toBeVisible();
      await expect(page.getByText("http://localhost:4173")).toBeVisible();
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
  await page.goto("/status");

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
  await page.goto("/status");

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
  await page.goto("/status");
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
  await page.goto("/status");
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
  await page.goto("/status");

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
  await page.goto("/status");

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
  await page.goto("/status");

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
  await page.goto("/status");

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
  await expect(
    page.getByText("Records and durable occurrence ingress"),
  ).toBeVisible();
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
  await page.goto("/status", { waitUntil: "domcontentloaded" });

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
  await page.goto("/status");

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
  await page.goto("/status");
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
