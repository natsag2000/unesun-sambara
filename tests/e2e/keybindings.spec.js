// Phase P6-01 end-to-end test: configurable keybindings.
//
// Covers the override lifecycle for an app-level shortcut: open the
// Keybindings tab, re-record "New Tab" to a combo that isn't used by
// anything else, confirm the new combo actually triggers the action and
// the old default no longer does, then reset it back to default.
const { test, expect } = require("@playwright/test");

async function waitForReady(page) {
  await page.waitForFunction(() => window.__unsTestHooks?.isReady() === true, {
    timeout: 30_000,
  });
  await expect(page.locator("#loading")).toBeHidden();
}

// Playwright's `toHaveClass(/border-editor-accent/)` would also match
// the row's own `hover:border-editor-accent` utility class (a substring
// collision, not a real "is overridden" signal) - `classList.contains`
// via `evaluate` checks the exact class instead.
function hasOverrideStyling(locator) {
  return locator.evaluate((el) => el.classList.contains("border-editor-accent"));
}

test.describe("Configurable keybindings (P6-01)", () => {
  test("re-recording New Tab's shortcut changes which combo triggers it", async ({
    page,
  }) => {
    await page.goto("/");
    await waitForReady(page);

    const tabItems = page.locator("#tab-strip > div[data-tab-id]");
    await expect(tabItems).toHaveCount(1);

    await page.locator("#settings-btn").click();
    await page.locator("#keybindings-tab-btn").click();
    await expect(page.locator("#keybindings-tab")).toBeVisible();

    const newTabRecordBtn = page.locator(
      '#keybindings-table-body button[data-keybinding-record="editor.newTab"]',
    );
    await expect(newTabRecordBtn).toHaveText("Ctrl+N");

    await newTabRecordBtn.click();
    await expect(newTabRecordBtn).toHaveText(/Press keys/);
    await page.keyboard.press("Control+Shift+K");
    await expect(newTabRecordBtn).toHaveText("Ctrl+Shift+K");
    await expect.poll(() => hasOverrideStyling(newTabRecordBtn)).toBe(true);
    await expect(
      page.locator('#keybindings-table-body button[data-keybinding-reset="editor.newTab"]'),
    ).toBeVisible();

    await page.locator("#settings-close").click();

    // The old default no longer does anything for this action...
    await page.locator("#editor-canvas").click();
    await page.keyboard.press("Control+n");
    await expect(tabItems).toHaveCount(1);

    // ...but the newly recorded combo does.
    await page.keyboard.press("Control+Shift+K");
    await expect(tabItems).toHaveCount(2);

    // Reset back to the default.
    await page.locator("#settings-btn").click();
    await page.locator("#keybindings-tab-btn").click();
    await page
      .locator('#keybindings-table-body button[data-keybinding-reset="editor.newTab"]')
      .click();
    await expect(newTabRecordBtn).toHaveText("Ctrl+N");
    await expect.poll(() => hasOverrideStyling(newTabRecordBtn)).toBe(false);
  });
});
