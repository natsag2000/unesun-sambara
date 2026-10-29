// WT-06 mobile coverage: double-tap selects a Bichig word and shows its
// Cyrillic translation. Same Pixel 5 emulation as
// word-suggestions-mobile.spec.js (Android preset because the iOS presets
// need webkit, which isn't installed here).
const { test, expect, devices } = require("@playwright/test");

test.use({ ...devices["Pixel 5"] });

const NOM = "\u1828\u1823\u182e";

async function waitForReady(page) {
  await page.waitForFunction(() => window.__unsTestHooks?.isReady() === true, {
    timeout: 30_000,
  });
  await expect(page.locator("#loading")).toBeHidden();
}

test.describe("Double-tap word translation on mobile", () => {
  test("double-tapping a Bichig word selects it and shows the Cyrillic translation", async ({
    page,
  }) => {
    await page.goto("/");
    await waitForReady(page);

    await page.locator("#keyboard-btn").click(); // Latin->Mongolian off
    await page.locator("#mobile-menu-btn").click();
    await page.locator("#mobile-clear-btn").click();
    await page.evaluate((value) => {
      const el = document.getElementById("mobile-input");
      el.focus();
      el.value = value;
      el.dispatchEvent(new Event("input", { bubbles: true }));
    }, NOM);

    const box = await page.locator("#editor-canvas").boundingBox();
    const size = (await page.evaluate(() => window.__unsTestHooks.getSettings())).fonts.font_size;
    const x = box.x + 10 + size * 0.5;
    const y = box.y + 10 + size * 0.5;

    // A single tap only moves the cursor.
    await page.touchscreen.tap(x, y);
    await expect(page.locator("#word-translation-popup")).toBeHidden();
    expect(await page.evaluate(() => window.__unsTestHooks.getSelectedText())).toBeNull();

    await page.waitForTimeout(500); // let the tap window lapse
    await page.touchscreen.tap(x, y);
    await page.touchscreen.tap(x, y);

    await expect(page.locator("#word-translation-popup")).toBeVisible();
    expect(await page.evaluate(() => window.__unsTestHooks.getSelectedText())).toBe(NOM);
    const state = await page.evaluate(() => window.__unsTestHooks.getTranslationState());
    expect(state.candidates).toEqual(["ном"]);
  });
});
