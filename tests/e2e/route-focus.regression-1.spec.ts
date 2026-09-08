import { expect, test, type Page } from "@playwright/test";

declare global {
  interface Window {
    __ROUTE_FOCUS_FRAME__: {
      release: () => Promise<void>;
      events: string[];
    };
  }
}

async function holdRouteFrames(page: Page): Promise<void> {
  await page.evaluate(() => {
    const requestFrame = window.requestAnimationFrame.bind(window);
    const cancelFrame = window.cancelAnimationFrame.bind(window);
    const pending = new Map<number, FrameRequestCallback>();
    let held = true;
    let nextId = -1;
    window.requestAnimationFrame = (callback) => {
      if (!held) return requestFrame(callback);
      const id = nextId--;
      pending.set(id, callback);
      return id;
    };
    window.cancelAnimationFrame = (id) => {
      if (!pending.delete(id)) cancelFrame(id);
    };
    const events: string[] = [];
    document.addEventListener("focusin", (event) => {
      const target = event.target as HTMLElement;
      events.push(
        target.id || target.getAttribute("aria-label") || target.tagName,
      );
    });
    window.__ROUTE_FOCUS_FRAME__ = {
      events,
      release: () =>
        new Promise<void>((resolve) => {
          held = false;
          requestFrame((time) => {
            const callbacks = [...pending.values()];
            pending.clear();
            for (const callback of callbacks) callback(time);
            resolve();
          });
        }),
    };
  });
}

async function openSettings(page: Page): Promise<void> {
  await page
    .getByRole("navigation", { name: "Main navigation" })
    .getByRole("link", { name: "Settings", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Settings", exact: true }),
  ).toBeVisible();
}

test("deferred route focus cannot replace a newer global Search shortcut", async ({
  page,
}) => {
  await page.goto("/library");
  await expect(
    page.getByRole("heading", { name: "Library", exact: true }),
  ).toBeVisible();
  await holdRouteFrames(page);
  await openSettings(page);

  const search = page.getByRole("combobox", {
    name: "Search records or commands",
  });
  await page.keyboard.press("Control+K");
  await expect(search).toBeFocused();
  await page.evaluate(() => window.__ROUTE_FOCUS_FRAME__.release());
  const focusEvents = await page.evaluate(
    () => window.__ROUTE_FOCUS_FRAME__.events,
  );
  await expect(search, focusEvents.join(" -> ")).toBeFocused();
});

test("normal route focus reaches main content", async ({
  page,
}) => {
  await page.goto("/library");
  await expect(
    page.getByRole("heading", { name: "Library", exact: true }),
  ).toBeVisible();
  await openSettings(page);
  await expect(page.locator("#main-content")).toBeFocused();
});
