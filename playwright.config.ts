import { defineConfig } from "@playwright/test";

process.env.FASTI_QA_PROXY_TARGET = "http://127.0.0.1:18422";

export default defineConfig({
  testDir: "./tests/e2e",
  outputDir: "test-results",
  fullyParallel: false,
  workers: process.env.CI ? 2 : 1,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["github"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: "http://127.0.0.1:4173",
    channel: process.env.CI ? undefined : "chrome",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  webServer: [
    {
      command: "node tests/e2e/health-stub.mjs",
      url: "http://127.0.0.1:18422/api/v1/health",
      reuseExistingServer: false,
    },
    {
      command:
        "pnpm --filter @fasti/tokens build && pnpm --filter @fasti/sdk build && pnpm --filter @fasti/ui build && pnpm --filter @fasti/web exec vite --port 4173",
      url: "http://127.0.0.1:4173",
      reuseExistingServer: false,
      timeout: 120_000,
      env: {
        FASTI_QA_PROXY_TARGET: "http://127.0.0.1:18422",
      },
    },
    {
      command:
        "pnpm --filter @fasti/tokens build && pnpm --filter @fasti/sdk build && pnpm --filter @fasti/ui build && FASTI_QA_HTTPS=true pnpm --filter @fasti/web exec vite --port 4174",
      url: "https://127.0.0.1:4174",
      ignoreHTTPSErrors: true,
      reuseExistingServer: false,
      timeout: 120_000,
      env: {
        FASTI_QA_PROXY_TARGET: "http://127.0.0.1:18422",
      },
    },
  ],
  projects: [
    {
      name: "chrome",
      testIgnore: "**/search-attach-browser.regression-1.spec.ts",
      use: { browserName: "chromium" },
    },
    {
      name: "https-chrome",
      testMatch: "**/search-attach-browser.regression-1.spec.ts",
      use: {
        browserName: "chromium",
        baseURL: "https://127.0.0.1:4174",
        ignoreHTTPSErrors: true,
      },
    },
  ],
});
