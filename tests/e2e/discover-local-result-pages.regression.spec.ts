import { expect, test, type Page } from "@playwright/test";
import { mockAuthenticatedAccess } from "./test-helpers";

interface LocalPageRequest {
  query: string;
  grains: string[];
  after?: { context_digest: string; last_record_id: string } | null;
}

declare global {
  interface Window {
    __DISCOVER_LOCAL_PAGES__: {
      calls: LocalPageRequest[];
      failNext: "main" | "attach" | "oversize-main" | "oversize-attach" | null;
    };
  }
}

// Trusted-host presentation fixture. Each response is one bounded Store page;
// request counts distinguish cached navigation from a fresh local read.
async function installLocalPageHost(page: Page): Promise<void> {
  await mockAuthenticatedAccess(page);
  await page.addInitScript(() => {
    const fixture: Window["__DISCOVER_LOCAL_PAGES__"] = {
      calls: [],
      failNext: null,
    };
    window.__DISCOVER_LOCAL_PAGES__ = fixture;
    const field = (value: string | null) => ({
      value,
      tier: value === null ? "empty" : "preferred_provider_claim",
      source: value === null ? null : "tmdb",
      is_stale: false,
    });
    const browserWindow = window as typeof window & {
      __TAURI_INTERNALS__: {
        invoke: (command: string, args?: unknown) => Promise<unknown>;
      };
    };
    browserWindow.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        switch (command) {
          case "setup_status":
            return { phase: "ready", proof_cleanup_pending: false };
          case "load_network_configuration":
            return {
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
          case "provider_credential_status":
            return [
              {
                provider: "tmdb",
                capability_id: "metadata.search",
                label: "TMDB",
                purpose: "Search film and television metadata",
                credential_requirement: "bearer_token",
                credential_state: "valid",
                state: "available",
                source: "environment",
                writable: false,
                testable: true,
                docs_url: "https://developer.themoviedb.org/docs",
              },
            ];
          case "list_records":
            return { records: [], truncated: false };
          case "list_reviews":
            return [];
          case "search_provider_page":
            return {
              outcome: "live",
              provider_id: "tmdb",
              page: 1,
              candidates: [
                {
                  provider: "tmdb",
                  provider_id: "693134",
                  grain: "film",
                  kind: "movie",
                  title: "Pages",
                  original_title: null,
                  release_year: 2024,
                  authors: [],
                  image_url: null,
                  overview: null,
                },
              ],
              next_page: null,
            };
          case "search_records": {
            const input = (args as { input: LocalPageRequest }).input;
            // Native IPC serializes JSON; Svelte's reactive cursor is a Proxy.
            fixture.calls.push(JSON.parse(JSON.stringify(input)));
            const source = input.grains.includes("film") ? "attach" : "main";
            if (input.after && fixture.failNext === source) {
              fixture.failNext = null;
              throw new Error(
                "Local page fixture unavailable; retry this page.",
              );
            }
            const offset = source === "attach" ? 100000 : 0;
            const pageNumber = input.after
              ? (parseInt(input.after.last_record_id.slice(-12), 16) - offset) /
                  100 +
                1
              : 1;
            const count =
              input.query === "Reset"
                ? 1
                : input.query === "Empty" && pageNumber === 2
                  ? 0
                  : 100;
            const records = Array.from({ length: count }, (_, index) => ({
              record_id: `rec_01991f588e0070008000${(offset + (pageNumber - 1) * 100 + index + 1).toString(16).padStart(12, "0")}`,
              grain: "film",
              status: "active",
              title: field(`${input.query} ${source} ${pageNumber}-${index}`),
              poster: field(null),
              original_title: field(null),
              overview: field(null),
              release_year: field("2024"),
              identifiers: [],
              latest_activity: null,
              poster_asset_path: null,
            }));
            if (input.after && fixture.failNext === `oversize-${source}`) {
              fixture.failNext = null;
              records[0].title = field("x".repeat(4 * 1024 * 1024));
            }
            return {
              records,
              next:
                count <= 1 || pageNumber === 3
                  ? null
                  : {
                      last_record_id: records[records.length - 1].record_id,
                      context_digest:
                        "sha256:" +
                        (source === "attach" ? "b" : "a").repeat(64),
                    },
            };
          }
          default:
            throw new Error(`Unexpected trusted-host command: ${command}`);
        }
      },
    };
  });
}

async function submitSearch(page: Page, query = "Pages"): Promise<void> {
  await page.locator("#provider-search").fill(query);
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Search", exact: true }),
  ).toBeEnabled();
}

async function callsFor(page: Page, source: "main" | "attach") {
  return page.evaluate(
    (source) =>
      window.__DISCOVER_LOCAL_PAGES__.calls.filter(
        (input) => input.grains.includes("film") === (source === "attach"),
      ),
    source,
  );
}

async function openPicker(page: Page) {
  await submitSearch(page);
  await page
    .getByRole("button", { name: "Attach to existing Record", exact: true })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Attach to existing Record",
  });
  await dialog
    .getByRole("button", { name: "Find Records", exact: true })
    .click();
  await expect(dialog.getByRole("radio")).toHaveCount(100);
  return dialog;
}

test.beforeEach(async ({ page }) => {
  await installLocalPageHost(page);
  await page.goto("/discover");
});

test("empty final Attach page leaves focus on the stable dialog heading", async ({
  page,
}) => {
  const dialog = await openPicker(page);
  await dialog
    .getByRole("searchbox", { name: "Search local Records", exact: true })
    .fill("Empty");
  await dialog
    .getByRole("button", { name: "Find Records", exact: true })
    .click();
  await expect(dialog.getByRole("radio")).toHaveCount(100);
  await dialog
    .getByRole("button", { name: "Next matching Records", exact: true })
    .click();
  await expect(dialog.getByRole("radio")).toHaveCount(0);
  await expect(
    dialog.getByText("No compatible Records found.", { exact: true }),
  ).toBeVisible();
  await expect(
    dialog.getByRole("heading", {
      name: "Attach to existing Record",
      exact: true,
    }),
  ).toBeFocused();
  await dialog
    .getByRole("button", { name: "Previous matching Records", exact: true })
    .click();
  await expect(dialog.getByRole("radio")).toHaveCount(100);
  await dialog
    .getByRole("button", { name: "Next matching Records", exact: true })
    .click();
  await expect(
    dialog.getByRole("heading", {
      name: "Attach to existing Record",
      exact: true,
    }),
  ).toBeFocused();
  expect(await callsFor(page, "attach")).toHaveLength(3);
});

for (const source of ["main", "attach"] as const) {
  test(`oversized ${source} page preserves the current page and retry cursor`, async ({
    page,
  }) => {
    const surface =
      source === "attach"
        ? await openPicker(page)
        : page.locator('[aria-labelledby="local-search-results-title"]');
    if (source === "main") await submitSearch(page);
    await page.evaluate((source) => {
      window.__DISCOVER_LOCAL_PAGES__.failNext =
        source === "main" ? "oversize-main" : "oversize-attach";
    }, source);
    const next = surface.getByRole("button", {
      name: source === "main" ? "Next local Records" : "Next matching Records",
      exact: true,
    });
    await next.click();
    await expect(surface.getByRole("alert")).toContainText(
      "exceeds its size limit",
    );
    await expect(
      surface.getByText(`Pages ${source} 1-0`, { exact: true }),
    ).toBeVisible();
    const failed = (await callsFor(page, source))[1];
    await next.click();
    await expect(
      surface.getByText(`Pages ${source} 2-0`, { exact: true }),
    ).toBeVisible();
    expect((await callsFor(page, source))[2]).toEqual(failed);
    await expect(surface.getByRole("alert")).toHaveCount(0);
  });
}

test("local Records retain one page and one adjacent cached page", async ({
  page,
}) => {
  await submitSearch(page);
  const local = page.getByRole("region", {
    name: "Local Records",
    exact: true,
  });
  await expect(local.getByRole("listitem")).toHaveCount(100);
  for (const number of [2, 3]) {
    await local
      .getByRole("button", { name: "Next local Records", exact: true })
      .click();
    await expect(
      local.getByRole("heading", {
        name: `Pages main ${number}-0`,
        exact: true,
      }),
    ).toBeVisible();
    await expect(local.getByRole("listitem")).toHaveCount(100);
    await expect(
      local.getByText("Pages main 1-0", { exact: true }),
    ).toHaveCount(0);
  }
  expect(await callsFor(page, "main")).toHaveLength(3);
  await local
    .getByRole("button", { name: "Previous local Records", exact: true })
    .click();
  await expect(
    local.getByRole("heading", { name: "Pages main 2-0", exact: true }),
  ).toBeVisible();
  await expect(local.getByRole("listitem")).toHaveCount(100);
  await expect(
    local.getByRole("button", { name: "Previous local Records", exact: true }),
  ).toHaveCount(0);
  await local
    .getByRole("button", { name: "Next local Records", exact: true })
    .click();
  await expect(
    local.getByRole("heading", { name: "Pages main 3-0", exact: true }),
  ).toBeVisible();
  await expect(local.getByRole("listitem")).toHaveCount(100);
  expect(await callsFor(page, "main")).toHaveLength(3);
});

test("failed local next preserves its page and retry cursor; a new query clears history", async ({
  page,
}) => {
  await submitSearch(page);
  const local = page.getByRole("region", {
    name: "Local Records",
    exact: true,
  });
  await expect(local.getByRole("listitem")).toHaveCount(100);
  await page.evaluate(() => {
    window.__DISCOVER_LOCAL_PAGES__.failNext = "main";
  });
  await local
    .getByRole("button", { name: "Next local Records", exact: true })
    .click();
  await expect(local.getByRole("alert")).toContainText(
    "Local page fixture unavailable",
  );
  await expect(
    local.getByRole("heading", { name: "Pages main 1-0", exact: true }),
  ).toBeVisible();
  await expect(local.getByRole("listitem")).toHaveCount(100);
  const failed = (await callsFor(page, "main"))[1];
  expect(failed.after?.last_record_id).toBe(
    "rec_01991f588e0070008000000000000064",
  );
  await local
    .getByRole("button", { name: "Next local Records", exact: true })
    .click();
  await expect(
    local.getByRole("heading", { name: "Pages main 2-0", exact: true }),
  ).toBeVisible();
  expect((await callsFor(page, "main"))[2]).toEqual(failed);
  await expect(local.getByRole("alert")).toHaveCount(0);
  await submitSearch(page, "Reset");
  await expect(local.getByRole("listitem")).toHaveCount(1);
  await expect(
    local.getByRole("heading", { name: "Reset main 1-0", exact: true }),
  ).toBeVisible();
  await expect(
    local.getByRole("button", { name: /^(Previous|Next) local Records$/ }),
  ).toHaveCount(0);
  expect((await callsFor(page, "main")).at(-1)?.after ?? null).toBeNull();
});

test("Attach targets retain one page and one adjacent cached page without carrying selection", async ({
  page,
}) => {
  const dialog = await openPicker(page);
  await dialog.getByRole("radio").first().check();
  await expect(
    dialog.getByRole("button", { name: "Confirm attachment", exact: true }),
  ).toBeEnabled();
  for (const number of [2, 3]) {
    await dialog
      .getByRole("button", { name: "Next matching Records", exact: true })
      .click();
    await expect(
      dialog.getByText(`Pages attach ${number}-0`, { exact: true }),
    ).toBeVisible();
    await expect(dialog.getByRole("radio")).toHaveCount(100);
    await expect(
      dialog.getByText("Pages attach 1-0", { exact: true }),
    ).toHaveCount(0);
    await expect(
      dialog.getByRole("button", { name: "Confirm attachment", exact: true }),
    ).toBeDisabled();
  }
  expect(await callsFor(page, "attach")).toHaveLength(3);
  await dialog
    .getByRole("button", { name: "Previous matching Records", exact: true })
    .click();
  await expect(
    dialog.getByText("Pages attach 2-0", { exact: true }),
  ).toBeVisible();
  await expect(dialog.getByRole("radio")).toHaveCount(100);
  await expect(
    dialog.getByRole("button", {
      name: "Previous matching Records",
      exact: true,
    }),
  ).toHaveCount(0);
  await dialog
    .getByRole("button", { name: "Next matching Records", exact: true })
    .click();
  await expect(
    dialog.getByText("Pages attach 3-0", { exact: true }),
  ).toBeVisible();
  await expect(dialog.getByRole("radio")).toHaveCount(100);
  expect(await callsFor(page, "attach")).toHaveLength(3);
});

test("failed Attach next preserves targets and retry cursor; a new target query clears history", async ({
  page,
}) => {
  const dialog = await openPicker(page);
  await page.evaluate(() => {
    window.__DISCOVER_LOCAL_PAGES__.failNext = "attach";
  });
  await dialog
    .getByRole("button", { name: "Next matching Records", exact: true })
    .click();
  await expect(dialog.getByRole("alert")).toContainText(
    "Local page fixture unavailable",
  );
  await expect(
    dialog.getByText("Pages attach 1-0", { exact: true }),
  ).toBeVisible();
  await expect(dialog.getByRole("radio")).toHaveCount(100);
  const failed = (await callsFor(page, "attach"))[1];
  expect(failed.after?.last_record_id).toBe(
    "rec_01991f588e0070008000000000018704",
  );
  await dialog
    .getByRole("button", { name: "Next matching Records", exact: true })
    .click();
  await expect(
    dialog.getByText("Pages attach 2-0", { exact: true }),
  ).toBeVisible();
  expect((await callsFor(page, "attach"))[2]).toEqual(failed);
  await expect(dialog.getByRole("alert")).toHaveCount(0);
  await dialog
    .getByRole("searchbox", { name: "Search local Records", exact: true })
    .fill("Reset");
  await dialog
    .getByRole("button", { name: "Find Records", exact: true })
    .click();
  await expect(dialog.getByRole("radio")).toHaveCount(1);
  await expect(
    dialog.getByText("Reset attach 1-0", { exact: true }),
  ).toBeVisible();
  await expect(
    dialog.getByRole("button", { name: /^(Previous|Next) matching Records$/ }),
  ).toHaveCount(0);
  expect((await callsFor(page, "attach")).at(-1)?.after ?? null).toBeNull();
});
