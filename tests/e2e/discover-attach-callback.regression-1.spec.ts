import { resolve } from "node:path";
import { expect, test, type Page } from "@playwright/test";

const recordId = "rec_01991f588e0070008000000000000b01";
const componentPath = resolve("packages/ui/src/discover-view.svelte");

declare global {
  interface Window {
    __DISCOVER_ATTACH_CALLBACK__: {
      props: Record<string, unknown>;
      calls: Array<{ kind: string; action: unknown }>;
      opened: string[];
      resolve: () => void;
      reject: () => void;
      dispose: () => Promise<void>;
    };
  }
}

async function mountDiscover(page: Page, retained: boolean): Promise<void> {
  await page.addInitScript(
    ({ retained, recordId }) => {
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
      const field = (value: string | null) => ({
        value,
        tier: value === null ? "empty" : "preferred_provider_claim",
        source: value === null ? null : "tmdb",
        is_stale: false,
      });
      const fixture = (window.__DISCOVER_ATTACH_CALLBACK__ = {
        props: {} as Record<string, unknown>,
        calls: [] as Array<{ kind: string; action: unknown }>,
        opened: [] as string[],
        resolve: () => {},
        reject: () => {},
        dispose: async () => {},
      });
      const attach = (kind: string, action: unknown): Promise<void> => {
        fixture.calls.push({ kind, action });
        return new Promise<void>((resolve, reject) => {
          fixture.resolve = resolve;
          fixture.reject = () =>
            reject(new Error("Attachment rejected by host."));
        });
      };
      fixture.props = {
        providerCredentials: [
          {
            provider: "tmdb",
            capability_id: "metadata.search",
            label: "TMDB",
            purpose: "Search film metadata",
            credential_requirement: "bearer_token",
            credential_state: "valid",
            state: "available",
            source: "environment",
            writable: false,
            testable: true,
            docs_url: "https://developer.themoviedb.org/docs",
          },
        ],
        onSearch: async () => [candidate],
        ...(retained
          ? {
              onSearchProviderPage: async () => ({
                outcome: "page",
                provider_id: "tmdb",
                page: 1,
                cache_state: "observed",
                lifetime: {
                  created_at: "2099-09-05T12:00:00Z",
                  fresh_until: "2099-09-05T12:02:00Z",
                  stale_until: "2099-09-05T12:10:00Z",
                  expires_at: "2099-09-06T12:00:00Z",
                },
                candidates: [
                  {
                    candidate_receipt_id:
                      "scr_01991f588e0070008000000000000b01",
                    grain: "film",
                    candidate,
                  },
                ],
                next_page: null,
              }),
              onCandidateReceiptAction: (
                _receipt: unknown,
                _mode: unknown,
                action: unknown,
              ) => attach("retained", action),
            }
          : {
              onCandidateAction: (_candidate: unknown, action: unknown) =>
                attach("live", action),
            }),
        onSearchAttachTargets: async () => ({
          records: [
            {
              record_id: recordId,
              grain: "film",
              status: "active",
              title: field("Dune local"),
              poster: field(null),
              original_title: field(null),
              overview: field(null),
              release_year: field("2024"),
              identifiers: [],
              latest_activity: null,
            },
          ],
          next: null,
        }),
        onOpenRecord: (id: string) => fixture.opened.push(id),
        onOpenSettings: () => {},
        onRetry: () => {},
      };
    },
    { retained, recordId },
  );

  // Reuse Vite's resolved Svelte runtime and compile the actual exported
  // component. This tests its void callback contract, not an invalid SDK reply.
  await page.route("**/src/main.ts", async (route) => {
    const original = await route.fetch();
    const body = await original.text();
    const runtime = body.match(
      /import\s*\{\s*mount\s*\}\s*from\s*["']([^"']+)["']/,
    )?.[1];
    if (!runtime) throw new Error("Vite's Svelte mount import was not found.");
    await route.fulfill({
      contentType: "text/javascript",
      body: `
        import { mount, unmount } from ${JSON.stringify(runtime)};
        import DiscoverView from ${JSON.stringify(`/@fs/${componentPath}`)};
        import "/src/global.css";
        const fixture = window.__DISCOVER_ATTACH_CALLBACK__;
        const component = mount(DiscoverView, {
          target: document.getElementById("app"), props: fixture.props,
        });
        fixture.dispose = () => unmount(component);
      `,
    });
  });
  await page.goto("/");
  await page.locator("#provider-search").fill("Dune");
  await page.getByRole("button", { name: "Search", exact: true }).click();
  await page
    .getByRole("button", { name: "Attach to existing Record", exact: true })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Attach to existing Record",
  });
  await dialog
    .getByRole("button", { name: "Find Records", exact: true })
    .click();
  await dialog.getByRole("radio", { name: /Dune local/ }).check();
  await dialog.getByRole("button", { name: "Confirm attachment" }).click();
  await expect
    .poll(() => page.evaluate(() => window.__DISCOVER_ATTACH_CALLBACK__.calls))
    .toEqual([
      {
        kind: retained ? "retained" : "live",
        action: { kind: "attach", record_id: recordId },
      },
    ]);
}

for (const retained of [false, true]) {
  const mode = retained ? "retained" : "live";
  test(`${mode} Attach accepts a successful void callback and opens the selected Record once`, async ({
    page,
  }) => {
    await mountDiscover(page, retained);
    const dialog = page.getByRole("dialog", {
      name: "Attach to existing Record",
    });
    await expect(
      dialog.getByRole("button", { name: "Confirm attachment" }),
    ).toBeDisabled();
    await page.evaluate(() => window.__DISCOVER_ATTACH_CALLBACK__.resolve());
    await expect(dialog).not.toBeVisible();
    await expect
      .poll(() =>
        page.evaluate(() => window.__DISCOVER_ATTACH_CALLBACK__.opened),
      )
      .toEqual([recordId]);
    await expect(
      page.getByRole("button", {
        name: "Attach to existing Record",
        exact: true,
      }),
    ).toBeDisabled();
    expect(
      await page.evaluate(
        () => window.__DISCOVER_ATTACH_CALLBACK__.calls.length,
      ),
    ).toBe(1);
  });

  test(`${mode} Attach rejection keeps the picker open without navigating`, async ({
    page,
  }) => {
    await mountDiscover(page, retained);
    await page.evaluate(() => window.__DISCOVER_ATTACH_CALLBACK__.reject());
    const dialog = page.getByRole("dialog", {
      name: "Attach to existing Record",
    });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("alert")).toHaveText(
      "Attachment rejected by host.",
    );
    await expect(
      dialog.getByRole("radio", { name: /Dune local/ }),
    ).toBeChecked();
    await expect(
      dialog.getByRole("button", { name: "Confirm attachment" }),
    ).toBeEnabled();
    expect(
      await page.evaluate(() => window.__DISCOVER_ATTACH_CALLBACK__.opened),
    ).toEqual([]);
  });

  test(`${mode} Attach completion after component teardown cannot navigate`, async ({
    page,
  }) => {
    await mountDiscover(page, retained);
    await page.evaluate(async () => {
      await window.__DISCOVER_ATTACH_CALLBACK__.dispose();
      window.__DISCOVER_ATTACH_CALLBACK__.resolve();
      await Promise.resolve();
    });
    await expect(page.getByRole("dialog")).toHaveCount(0);
    expect(
      await page.evaluate(() => window.__DISCOVER_ATTACH_CALLBACK__.opened),
    ).toEqual([]);
    expect(
      await page.evaluate(
        () => window.__DISCOVER_ATTACH_CALLBACK__.calls.length,
      ),
    ).toBe(1);
  });
}
