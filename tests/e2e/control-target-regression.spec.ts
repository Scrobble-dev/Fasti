import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

async function undersizedControls(page: Page) {
  return page
    .locator(
      'button, input:not([type="checkbox"]):not([type="radio"]), select, textarea',
    )
    .evaluateAll((controls) =>
      controls
        .filter((control) => control.getClientRects().length > 0)
        .map((control) => {
          const bounds = control.getBoundingClientRect();
          return {
            tag: control.tagName,
            label:
              control.getAttribute("aria-label") ??
              control.textContent?.trim() ??
              control.tagName,
            width: Math.round(bounds.width),
            height: Math.round(bounds.height),
          };
        })
        .filter(
          (control) =>
            control.height < 44 ||
            (control.tag === "BUTTON" && control.width < 44),
        ),
    );
}

test("Workbench controls inherit the shared 44 pixel target", async ({
  page,
}) => {
  await page.setViewportSize({ width: 375, height: 812 });
  await page.goto("/library");

  expect(await undersizedControls(page)).toEqual([]);

  await page.getByRole("button", { name: "Theme settings" }).click();
  await expect(
    page.getByRole("dialog", { name: "Theme settings" }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Light" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  expect(await undersizedControls(page)).toEqual([]);

  for (const accent of [
    "Tabler Blue",
    "Red",
    "Green",
    "Orange",
    "Purple",
    "Teal",
    "Cyan",
    "Fasti Oxblood",
    "Horological Gold",
  ]) {
    const choice = page.getByRole("button", { name: accent, exact: true });
    await choice.click();
    await expect(choice).toHaveAttribute("aria-pressed", "true");
    expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  }

  for (const mode of ["Dark", "Night"]) {
    await page.getByRole("button", { name: mode }).click();
    expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  }

  await page.getByRole("button", { name: "Done" }).click();
  await page.goto("/records");
  await expect(
    page.getByRole("button", { name: "Choose one from Library" }),
  ).toBeVisible();
  expect(await undersizedControls(page)).toEqual([]);
});
