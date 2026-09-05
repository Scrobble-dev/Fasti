import { expect, test, type Locator } from "@playwright/test";
import {
  expectNoHorizontalOverflow,
  mockAuthenticatedAccess,
} from "./test-helpers";

async function expectCopyLayout(copies: Locator, count: number) {
  await expect(copies).toHaveCount(count);
  const measurements = await copies.evaluateAll((elements) =>
    elements.map((element) => {
      const style = getComputedStyle(element);
      const last = element.lastElementChild!;
      return {
        text: element.textContent,
        mobile: matchMedia("(max-width: 47.99rem)").matches,
        direction: getComputedStyle(element.parentElement!).flexDirection,
        basis: style.flexBasis,
        desktopBasis: `${18 * parseFloat(getComputedStyle(document.documentElement).fontSize)}px`,
        unusedSpace:
          element.getBoundingClientRect().bottom -
          last.getBoundingClientRect().bottom -
          parseFloat(getComputedStyle(last).marginBottom),
        lineHeight: parseFloat(style.lineHeight),
      };
    }),
  );
  for (const result of measurements) {
    expect(result.direction, result.text ?? "").toBe(
      result.mobile ? "column" : "row",
    );
    expect(result.basis, result.text ?? "").toBe(
      result.mobile ? "auto" : result.desktopBasis,
    );
    if (result.mobile)
      expect(result.unusedSpace, result.text ?? "").toBeLessThanOrEqual(
        result.lineHeight + 1,
      );
  }
}

for (const theme of ["light", "dark"] as const) {
  for (const scenario of [
    { width: 320, textSpacing: false },
    { width: 375, textSpacing: false },
    { width: 768, textSpacing: false },
    { width: 1440, textSpacing: false },
    { width: 320, textSpacing: true },
  ]) {
    test(`Access copy fits its flex axis: ${theme} ${scenario.width}px${scenario.textSpacing ? " enlarged text" : ""}`, async ({
      page,
    }, info) => {
      await page.setViewportSize({ width: scenario.width, height: 1024 });
      await page.addInitScript((mode) => {
        localStorage.setItem("fasti-theme-settings", JSON.stringify({ mode }));
      }, theme);
      await mockAuthenticatedAccess(page);
      const mutations: string[] = [];
      page.on("request", (request) => {
        if (
          new URL(request.url()).pathname.startsWith("/api/access/v1/") &&
          request.method() !== "GET"
        )
          mutations.push(`${request.method()} ${request.url()}`);
      });
      await page.goto("/settings/account");
      await expect(page.locator("html")).toHaveAttribute(
        "data-bs-theme",
        theme,
      );
      if (scenario.textSpacing) {
        await page.addStyleTag({
          content: `
            html { font-size: 200% !important; }
            * { line-height: 1.5 !important; letter-spacing: .12em !important; word-spacing: .16em !important; }
            p { margin-bottom: 2em !important; }
          `,
        });
        await expect(page.locator("html")).toHaveCSS("font-size", "32px");
      }
      const account = page.getByTestId("account-security-task-map");
      await expectCopyLayout(account.locator(".task-copy"), 5);
      await expectNoHorizontalOverflow(page);
      await page.screenshot({
        path: info.outputPath("account.png"),
        fullPage: true,
      });
      await page.getByRole("button", { name: "Resume setup" }).click();
      const setup = page.getByTestId("first-run-guided-setup");
      await expect(
        setup.getByRole("heading", { name: "Secure your Fasti account" }),
      ).toBeFocused();
      await expectCopyLayout(setup.locator(".access-heading > div"), 1);
      await expectNoHorizontalOverflow(page);
      await page.screenshot({
        path: info.outputPath("first-run.png"),
        fullPage: true,
      });
      expect(mutations).toEqual([]);
    });
  }
}
