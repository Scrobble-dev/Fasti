import { expect, type Page } from "@playwright/test";

export async function expectNoHorizontalOverflow(page: Page): Promise<void> {
  const overflow = await page.evaluate(() => {
    const viewportWidth = document.documentElement.clientWidth;
    return {
      pixels: document.documentElement.scrollWidth - viewportWidth,
      layout: [
        "body",
        ".settings-container",
        ".settings-layout",
        ".settings-panel",
        ".access-surface",
        ".table-responsive",
      ].map((selector) => ({
        selector,
        bounds: document
          .querySelector(selector)
          ?.getBoundingClientRect()
          .toJSON(),
        clientWidth: document.querySelector<HTMLElement>(selector)?.clientWidth,
        scrollWidth: document.querySelector<HTMLElement>(selector)?.scrollWidth,
      })),
      elements: [...document.querySelectorAll<HTMLElement>("body *")]
        .filter(
          (element) =>
            !element.closest(".table-responsive") &&
            element.getBoundingClientRect().right > viewportWidth + 0.5,
        )
        .slice(0, 8)
        .map((element) => ({
          element: `${element.tagName.toLowerCase()}${element.id ? `#${element.id}` : ""}.${[...element.classList].join(".")}`,
          bounds: element.getBoundingClientRect().toJSON(),
        })),
      internallyOverflowing: [
        ...document.querySelectorAll<HTMLElement>("body *"),
      ]
        .filter(
          (element) =>
            !element.closest(".table-responsive") &&
            element.closest(".access-surface") &&
            !element.classList.contains("visually-hidden") &&
            element.scrollWidth > element.clientWidth + 1,
        )
        .slice(0, 8)
        .map((element) => ({
          element: `${element.tagName.toLowerCase()}${element.id ? `#${element.id}` : ""}.${[...element.classList].join(".")}`,
          clientWidth: element.clientWidth,
          scrollWidth: element.scrollWidth,
        })),
    };
  });
  expect(
    overflow.pixels,
    JSON.stringify(
      {
        layout: overflow.layout,
        elements: overflow.elements,
        internallyOverflowing: overflow.internallyOverflowing,
      },
      null,
      2,
    ),
  ).toBeLessThanOrEqual(0);
  expect(overflow.internallyOverflowing).toEqual([]);
}

export async function mockAuthenticatedAccess(page: Page): Promise<void> {
  await page.route("**/api/access/v1/projection", async (route) => {
    const response = await page.request.get(
      "http://127.0.0.1:18422/api/access/v1/projection",
    );
    await route.fulfill({ response });
  });
}

export async function mockMissingTmdbProvider(page: Page): Promise<void> {
  await page.route(/\/api\/v1\/providers$/, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        providers: [
          {
            provider_id: "tmdb",
            display_name: "TMDB",
            provider_kind: "metadata",
            documentation_url: "https://developer.themoviedb.org/docs",
            attribution: "TMDB",
            supported_media_grains: ["film", "series"],
            capabilities: [
              {
                capability_id: "metadata.search",
                purpose: "Search film and television metadata",
                credential_requirement: "bearer_token",
                credential_state: "missing",
                credential_source: "none",
                state: "degraded",
                version: 0,
                writable: false,
                testable: false,
                health: {
                  state: "never_run",
                  checked_at: null,
                  safe_problem_code: null,
                },
                credential_test: {
                  state: "never_run",
                  checked_at: null,
                  safe_problem_code: null,
                },
              },
            ],
            network_hosts: ["api.themoviedb.org"],
            locale_support: true,
            region_support: true,
            identity_namespaces: ["tmdb.movie", "tmdb.tv"],
          },
        ],
      }),
    }),
  );
}
