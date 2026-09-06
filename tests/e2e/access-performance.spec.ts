import { expect, test, type TestInfo } from "@playwright/test";
import {
  parseAccessProjectionResponse,
  parseReadTrailBaseContinuationResponse,
} from "@fasti/sdk";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { writeFile } from "node:fs/promises";
import { setTimeout as delay } from "node:timers/promises";

async function attachJson(info: TestInfo, name: string, value: unknown) {
  const path = info.outputPath(`${name}.json`);
  await writeFile(path, JSON.stringify(value));
  await info.attach(name, { path, contentType: "application/json" });
}

// Lighthouse 13.4.1: docs/user-flows.md and core/audits/metrics/
// interaction-to-next-paint.js. Timespan INP requires unsimulated interactions.
// This is a short, unthrottled, fixture-only lab gate, not field performance,
// initial-load coverage, mobile hardware emulation, or real authentication.
for (const width of [320, 1440]) {
  test(`Access delayed confirmation at ${width}px @performance`, async ({
    request,
    baseURL,
  }, info) => {
    test.setTimeout(90_000);
    const { startFlow, desktopConfig } = await import("lighthouse");
    const { default: puppeteer } = await import("puppeteer-core");
    if (!baseURL)
      throw new Error("The canonical browser harness needs baseURL");
    const origin = new URL(baseURL).origin;
    const response = await request.get(
      "http://127.0.0.1:18422/api/access/v1/projection",
    );
    expect(response.ok()).toBe(true);
    const projection = parseAccessProjectionResponse(await response.json());
    const continuation = parseReadTrailBaseContinuationResponse({
      candidate_revision: `sha256:${"b".repeat(64)}`,
      expires_at: "2099-08-31T12:05:00Z",
      remembered: false,
      choices: [
        {
          choice_ordinal: 0,
          membership_state: "active",
          role: "administrator",
          profile_created_at: "2026-08-31T12:00:00Z",
          profile_ordinal: 1,
          workspace_created_at: "2026-08-31T12:00:00Z",
          workspace_ordinal: 1,
        },
      ],
    });
    const mutations: { method: string; path: string; body: unknown }[] = [];
    const errors: string[] = [];
    let release!: () => void;
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    // Chrome 152 restores the interaction trace event needed by Lighthouse 13.
    // Playwright's Chromium 151 cannot supply that audit; missing metrics fail.
    // This owned browser never connects to a user's browser or MCP service.
    const browser = await puppeteer.launch({
      channel: "chrome",
      headless: true,
      args: ["--no-sandbox"],
    });
    try {
      const page = await browser.newPage();
      page.setDefaultTimeout(15_000);
      await page.setViewport({ width, height: 1024, deviceScaleFactor: 1 });
      await page.setBypassServiceWorker(true);
      page.on("pageerror", (error) => errors.push(String(error)));
      await page.evaluateOnNewDocument(() => {
        localStorage.setItem(
          "fasti-theme-settings",
          JSON.stringify({ mode: "light" }),
        );
      });
      await page.setRequestInterception(true);
      page.on("request", async (intercepted) => {
        try {
          const url = new URL(intercepted.url());
          if (intercepted.method() !== "GET") {
            mutations.push({
              method: intercepted.method(),
              path: url.pathname,
              body: JSON.parse(intercepted.postData() ?? "null"),
            });
            if (
              url.origin !== origin ||
              intercepted.method() !== "POST" ||
              url.pathname !== "/api/access/v1/trailbase/continuation"
            ) {
              await intercepted.respond({ status: 405 });
              return;
            }
            await held;
            await intercepted.respond({ status: 204 });
          } else if (
            url.origin === origin &&
            url.pathname.startsWith("/api/access/v1/")
          ) {
            await intercepted.respond({
              status: 200,
              contentType: "application/json",
              body: JSON.stringify(
                url.pathname.endsWith("/trailbase/continuation")
                  ? continuation
                  : projection,
              ),
            });
          } else if (url.origin === origin) {
            await intercepted.continue();
          } else {
            await intercepted.abort("blockedbyclient");
          }
        } catch (error) {
          errors.push(String(error));
          if (!intercepted.isInterceptResolutionHandled())
            await intercepted.abort().catch(() => undefined);
        }
      });
      await page.goto(`${origin}/first-run?auth=continue`);
      await page.waitForSelector('input[name="access-choice"][value="0"]');
      await page.evaluate(() => document.fonts.ready);
      const flow = await startFlow(page, {
        name: `Access delayed confirmation, ${width}px`,
        config: desktopConfig,
        flags: {
          screenEmulation: { disabled: true },
          emulatedUserAgent: false,
          throttlingMethod: "provided",
          onlyAudits: ["cumulative-layout-shift", "interaction-to-next-paint"],
        },
      });
      await flow.startTimespan();
      // Lighthouse's CLS emulation correction treats the first 500ms specially.
      // Settle that instrumentation interval before any of the measured inputs.
      await delay(600);
      await page.click('input[name="access-choice"][value="0"]');
      await page.locator('::-p-aria(Confirm access[role="button"])').click();
      await expect.poll(() => mutations.length).toBe(1);
      await page.waitForFunction(() => {
        const names = [
          "Confirming…",
          "Save and leave",
          "Cancel sign-in",
          "Manage existing access",
        ];
        return names.every((name) =>
          [...document.querySelectorAll("button")].some(
            (button) => button.textContent?.trim() === name && button.disabled,
          ),
        );
      });
      // Keep the async confirmation outside the 500ms recent-input exclusion.
      const heldAt = performance.now();
      await delay(1000);
      const heldMs = performance.now() - heldAt;
      expect(heldMs).toBeGreaterThan(500);
      release();
      await page.waitForFunction(
        () =>
          document.querySelector("#access-notice")?.textContent?.trim() ===
          "Account access confirmed. Review the remaining security tasks.",
      );
      await delay(700);
      await page.focus('::-p-aria(Manage existing access[role="button"])');
      await page.keyboard.press("Enter");
      await page.waitForFunction(
        () =>
          location.pathname === "/settings/account" &&
          document.activeElement?.textContent?.trim() ===
            "Account and security",
      );
      await delay(700);
      await flow.endTimespan();
      const artifacts = flow.createArtifactsJson();
      await attachJson(info, "lighthouse-flow-artifacts", artifacts);
      const result = await flow.createFlowResult();
      await attachJson(info, "lighthouse-flow-result", result);
      expect(result.steps).toHaveLength(1);
      const lhr = result.steps[0].lhr;
      const trace = artifacts.gatherSteps[0].artifacts.Trace;
      // Diagnostic count only; the metric itself comes from Lighthouse's audit.
      const legacyInteractionEvents = trace.traceEvents.filter(
        (event) => event.name === "Responsiveness.Renderer.UserInteraction",
      ).length;
      const interactions = new Set(
        trace.traceEvents.flatMap((event) => {
          const data = event.args?.data;
          return event.name === "EventTiming" &&
            data &&
            "interactionId" in data &&
            typeof data.interactionId === "number" &&
            data.interactionId > 0
            ? [data.interactionId]
            : [];
        }),
      ).size;
      const cls = lhr.audits["cumulative-layout-shift"];
      const inp = lhr.audits["interaction-to-next-paint"];
      await attachJson(info, "performance-provenance", {
        head: execFileSync("git", ["rev-parse", "HEAD"], {
          encoding: "utf8",
        }).trim(),
        workingTreeStatus: execFileSync("git", ["status", "--porcelain"], {
          encoding: "utf8",
        }),
        testSha256: createHash("sha256")
          .update(readFileSync(info.file))
          .digest("hex"),
        lighthouse: lhr.lighthouseVersion,
        browser: await browser.version(),
        width,
        height: 1024,
        heldMs,
        interactions,
        legacyInteractionEvents,
        cls: cls?.numericValue ?? null,
        inpMs: inp?.numericValue ?? null,
        limits:
          "Fixture-only, light-theme, unthrottled local Vite timespan. No initial-load, field, mobile-device or production authentication claim.",
      });
      expect(lhr.runtimeError).toBeUndefined();
      expect(interactions).toBeGreaterThan(0);
      expect(cls?.scoreDisplayMode).toBe("numeric");
      expect(inp?.scoreDisplayMode).toBe("numeric");
      expect(Number.isFinite(cls?.numericValue)).toBe(true);
      expect(Number.isFinite(inp?.numericValue)).toBe(true);
      expect(cls.numericValue).toBe(0);
      expect(inp.numericValue).toBeGreaterThanOrEqual(0);
      expect(inp.numericValue).toBeLessThan(100);
      expect(mutations).toEqual([
        {
          method: "POST",
          path: "/api/access/v1/trailbase/continuation",
          body: {
            candidate_revision: continuation.candidate_revision,
            choice_ordinal: 0,
          },
        },
      ]);
      expect(errors).toEqual([]);
    } finally {
      release();
      await browser.close();
      await attachJson(info, "fixture-requests-and-errors", {
        mutations,
        errors,
      });
    }
  });
}
