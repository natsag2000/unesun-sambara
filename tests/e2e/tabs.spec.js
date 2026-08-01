// Phase P8-03 end-to-end test: multiple documents / tab strip.
//
// Covers the core interactions `TabManager` (index.html) is responsible
// for: creating a tab, each tab keeping independent content while
// backgrounded, and closing a dirty tab requiring confirmation. Session
// persistence across reload is covered by the "reload" assertions in
// smoke.spec.js instead (it's really the same mechanism, just exercised
// there as a side effect of the P7-04 checklist).
const { test, expect } = require("@playwright/test");

async function waitForReady(page) {
  await page.waitForFunction(() => window.__unsTestHooks?.isReady() === true, {
    timeout: 30_000,
  });
  await expect(page.locator("#loading")).toBeHidden();
}

test.describe("Multi-document tabs (P8-03)", () => {
  test("create, switch, and close tabs", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);

    // Turn off Latin input mode so typed ASCII lands literally.
    await page.locator("#keyboard-btn").click();

    // First tab: clear the random greeting, type distinct content.
    await page.locator("#clear-btn").click();
    await page.locator("#editor-canvas").click();
    await page.keyboard.type("first tab content");
    await expect
      .poll(() => page.evaluate(() => window.__unsTestHooks.getText().trim()))
      .toBe("first tab content");

    // A brand new tab starts blank, independent of the first.
    await page.locator("#tab-strip-new-btn").click();
    await expect
      .poll(() => page.evaluate(() => window.__unsTestHooks.getText().trim()))
      .toBe("");
    await page.locator("#editor-canvas").click();
    await page.keyboard.type("second tab content");

    const tabItems = page.locator("#tab-strip > div[data-tab-id]");
    await expect(tabItems).toHaveCount(2);

    // Switching back to the first tab restores exactly what was typed
    // into it, even though it's been backgrounded while the second tab
    // was edited.
    await tabItems.nth(0).click();
    await expect
      .poll(() => page.evaluate(() => window.__unsTestHooks.getText().trim()))
      .toBe("first tab content");

    // Closing a dirty tab must ask for confirmation. Accept it here to
    // verify the *dialog appeared* (via the "dialog" event) rather than
    // the close silently succeeding without one.
    let sawDialog = false;
    page.once("dialog", (dialog) => {
      sawDialog = true;
      dialog.accept();
    });
    await tabItems.nth(0).locator("button").click();
    await expect(tabItems).toHaveCount(1);
    expect(sawDialog).toBe(true);

    // The remaining tab is the second one, untouched by closing the
    // first.
    await expect
      .poll(() => page.evaluate(() => window.__unsTestHooks.getText().trim()))
      .toBe("second tab content");
  });

  test("Ctrl+N opens a new tab", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);

    const tabItems = page.locator("#tab-strip > div[data-tab-id]");
    await expect(tabItems).toHaveCount(1);

    await page.locator("#editor-canvas").click();
    await page.keyboard.press("Control+n");

    await expect(tabItems).toHaveCount(2);
  });
});
