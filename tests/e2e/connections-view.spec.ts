import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";
import { expectNoHorizontalOverflow } from "./test-helpers";

// Runtime-report fixtures exercise presentation, not configured service health.
const states = [
  ["available", "Available"],
  ["setup_required", "Setup required"],
  ["active", "Active"],
  ["degraded", "Needs attention"],
  ["disabled", "Disabled"],
  ["unsupported", "Not supported here"],
  ["error", "Error"],
] as const;
const integrations = states.map(([state], index) => ({
  id: index === 0 ? "nuvio" : index === 1 ? "mpris" : state,
  label: index === 0 ? "Nuvio" : index === 1 ? "MPRIS" : `${state} adapter`,
  state,
  available: index % 2 === 0,
  endpoint_ready: index % 3 === 0,
  detail: `Runtime detail for ${state}.`,
  setup_action: `Setup instructions for ${state}.`,
}));
const integrationRoute = "**/api/v1/integrations";

test("readiness table preserves every reported state and fact", async ({
  page,
}) => {
  await page.route(integrationRoute, (route) =>
    route.fulfill({ json: { integrations } }),
  );
  await page.goto("/connections");

  const table = page.getByRole("table", {
    name: "Adapter readiness",
    exact: true,
  });
  await expect(table.getByRole("columnheader")).toHaveText([
    "Adapter",
    "Readiness",
    "Endpoint",
    "Platform",
    "Setup action",
  ]);
  await expect(table.getByRole("row")).toHaveCount(integrations.length + 1);
  for (const [index, integration] of integrations.entries()) {
    const row = table.locator(`[data-integration="${integration.id}"]`);
    await expect(row.getByRole("rowheader")).toContainText(integration.label);
    await expect(row.getByRole("rowheader")).toContainText(integration.detail);
    await expect(row.getByRole("cell")).toHaveText([
      states[index]![1],
      integration.endpoint_ready ? "Ready" : "Not exposed",
      integration.available ? "Supported" : "Unavailable",
      integration.setup_action,
    ]);
  }
  await expect(
    page.getByText("not a health check", { exact: false }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "API clients", exact: true }),
  ).toBeVisible();
});

test("refresh retains rows and the client panel while pending, then recovers from an error", async ({
  page,
}) => {
  let releaseRefresh: () => void = () => {};
  const pendingRefresh = new Promise<void>((resolve) => {
    releaseRefresh = resolve;
  });
  let requests = 0;
  await page.route(integrationRoute, async (route) => {
    requests += 1;
    if (requests === 2) {
      await pendingRefresh;
      // A malformed report must produce an error, never stale success.
      await route.fulfill({ json: { integrations: [{ state: "unknown" }] } });
    } else {
      await route.fulfill({
        json: {
          integrations: requests === 1 ? integrations : [integrations[0]],
        },
      });
    }
  });
  await page.goto("/connections");
  const refresh = page.getByRole("button", {
    name: "Refresh status",
    exact: true,
  });
  const table = page.getByRole("table", {
    name: "Adapter readiness",
    exact: true,
  });
  await expect(table.getByRole("row")).toHaveCount(8);
  const clientPanel = await page
    .getByRole("region", { name: "API clients", exact: true })
    .elementHandle();
  expect(clientPanel).not.toBeNull();
  await refresh.click();
  try {
    await expect(refresh).toBeDisabled();
    await expect(
      page.getByText("Loading integration status…", { exact: true }),
    ).toBeVisible();
    await expect(table.getByRole("row")).toHaveCount(8);
    expect(await clientPanel!.evaluate((element) => element.isConnected)).toBe(
      true,
    );
  } finally {
    releaseRefresh();
  }
  await expect(
    page.getByText("Could not read runtime status.", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText(
      "Fasti integration status did not match the supported contract.",
      { exact: true },
    ),
  ).toBeVisible();
  await expect(table).toHaveCount(0);
  await expect(refresh).toBeEnabled();
  await refresh.click();
  await expect(table.getByRole("row")).toHaveCount(2);
  await expect(
    page.getByText("1 integration reported.", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Could not read runtime status.", { exact: true }),
  ).toHaveCount(0);
  expect(await clientPanel!.evaluate((element) => element.isConnected)).toBe(
    true,
  );
});

test("empty readiness report keeps recovery and API clients available", async ({
  page,
}) => {
  await page.route(integrationRoute, (route) =>
    route.fulfill({ json: { integrations: [] } }),
  );
  await page.goto("/connections");
  await expect(
    page.getByRole("heading", {
      name: "No integration capability was reported",
    }),
  ).toBeVisible();
  await expect(
    page.getByText(
      "Check that this interface points to the intended Fasti node, then refresh.",
      { exact: true },
    ),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Refresh status", exact: true }),
  ).toBeEnabled();
  await expect(
    page.getByRole("heading", { name: "API clients", exact: true }),
  ).toBeVisible();
  expect(
    (await new AxeBuilder({ page }).include(".connections-container").analyze())
      .violations,
  ).toEqual([]);
});

for (const mode of ["light", "dark"] as const) {
  for (const width of [320, 768, 1440]) {
    test(`${mode} ${width}px readiness preserves keyboard access and reflow`, async ({
      page,
    }, testInfo) => {
      await page.setViewportSize({ width, height: 1000 });
      await page.addInitScript((themeMode) => {
        localStorage.setItem(
          "fasti-theme-settings",
          JSON.stringify({ mode: themeMode }),
        );
      }, mode);
      await page.route(integrationRoute, (route) =>
        route.fulfill({ json: { integrations } }),
      );
      await page.goto("/connections");
      await expect(page.locator("html")).toHaveAttribute("data-bs-theme", mode);
      const table = page.getByRole("table", {
        name: "Adapter readiness",
        exact: true,
      });
      await expect(table.getByRole("row")).toHaveCount(8);
      await expect(table.locator(".conn-status-pill").first()).toHaveCSS(
        "white-space",
        "nowrap",
      );
      await expectNoHorizontalOverflow(page);
      const refresh = page.getByRole("button", {
        name: "Refresh status",
        exact: true,
      });
      const bounds = await refresh.boundingBox();
      expect(bounds!.width).toBeGreaterThanOrEqual(44);
      expect(bounds!.height).toBeGreaterThanOrEqual(44);
      await refresh.focus();
      await page.keyboard.press("Tab");
      const scrollRegion = page.getByRole("region", {
        name: "Adapter readiness table",
        exact: true,
      });
      await expect(scrollRegion).toBeFocused();
      await expect(scrollRegion).toHaveCSS("outline-style", "solid");
      await expect(scrollRegion).toHaveCSS("outline-width", "3px");
      if (width < 768) {
        await page.keyboard.press("ArrowRight");
        await expect
          .poll(() => scrollRegion.evaluate((element) => element.scrollLeft))
          .toBeGreaterThan(0);
        await scrollRegion.evaluate((element) => {
          element.scrollLeft = 0;
        });
      }
      expect(
        (
          await new AxeBuilder({ page })
            .include(".connections-container")
            .analyze()
        ).violations,
      ).toEqual([]);
      await refresh.focus();
      expect(
        (await new AxeBuilder({ page }).include(".refresh-button").analyze())
          .violations,
      ).toEqual([]);
      await page.evaluate(() => window.scrollTo(0, 0));
      await page.screenshot({
        path: testInfo.outputPath(`connections-${mode}-${width}.png`),
        fullPage: true,
      });
    });
  }
}
