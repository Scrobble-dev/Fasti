import { expect, type Page } from "@playwright/test";

export async function expectNoHorizontalOverflow(page: Page): Promise<void> {
  expect(
    await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    ),
  ).toBeLessThanOrEqual(0);
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
