// Phase P7-04 end-to-end smoke test.
//
// Covers exactly the checklist from prompt/FUTURE_PLAN.md P7-04: load
// the page, type text, toggle orientation, open settings and save, then
// reload and assert both content and settings survived.
//
// "Content surviving a reload" means Phase P8-03's multi-tab session
// persistence (`TabManager`): every open tab's content is continuously
// saved to localStorage and the whole tab strip reappears silently on
// reload - no "restore this draft?" banner to click through, unlike
// the P2-05 single-document auto-save mechanism this test originally
// exercised (see DOC/IMPL/PHASE_7_04_IMPL.md's "Follow-ups" and
// DOC/IMPL/PHASE_8_03_IMPL.md for why that changed).
//
// Assertions on document text/settings go through
// `window.__unsTestHooks` (added in `index.html` for this test) rather
// than scraping canvas pixels - the editor renders to a `<canvas>`, so
// there's no DOM text node to read otherwise.
const { test, expect } = require("@playwright/test");

async function waitForReady(page) {
  await page.waitForFunction(() => window.__unsTestHooks?.isReady() === true, {
    timeout: 30_000,
  });
  await expect(page.locator("#loading")).toBeHidden();
}

test.describe("UNS editor smoke test", () => {
  test("type, toggle orientation, save settings, reload, and recover", async ({
    page,
  }) => {
    await page.goto("/");
    await waitForReady(page);

    // The Latin->Mongolian input mode is on by default; turn it off so
    // typing plain ASCII lands in the document literally instead of
    // being transliterated.
    const keyboardBtn = page.locator("#keyboard-btn");
    await expect(keyboardBtn).toHaveClass(/active/);
    await keyboardBtn.click();
    await expect(keyboardBtn).not.toHaveClass(/active/);

    // Start from a blank document so the typed text can be asserted
    // exactly, rather than inserted at an arbitrary point in the
    // random greeting.
    await page.locator("#clear-btn").click();
    await expect
      .poll(() => page.evaluate(() => window.__unsTestHooks.getText().trim()))
      .toBe("");

    await page.locator("#editor-canvas").click();
    await page.keyboard.type("Hello Playwright");

    await expect
      .poll(() => page.evaluate(() => window.__unsTestHooks.getText().trim()))
      .toBe("Hello Playwright");

    // Toggle orientation (default is "vertical" - see
    // EditorBehaviorSettings::default() in src/config/settings.rs).
    let settings = await page.evaluate(() => window.__unsTestHooks.getSettings());
    expect(settings.editor.orientation).toBe("vertical");

    await page.locator("#vertical-btn").click();

    settings = await page.evaluate(() => window.__unsTestHooks.getSettings());
    expect(settings.editor.orientation).toBe("horizontal");

    // Open settings, change font size, save.
    await page.locator("#settings-btn").click();
    await expect(page.locator("#settings-modal")).toBeVisible();

    await page.locator("#font-size-input").fill("60");
    await page.locator("#settings-save").click();
    await expect(page.locator("#settings-modal")).toBeHidden();

    settings = await page.evaluate(() => window.__unsTestHooks.getSettings());
    expect(settings.fonts.font_size).toBe(60);
    // Orientation is preserved through the settings-modal save (P0-02
    // regression coverage): the modal doesn't expose an orientation
    // control, so saving must not silently revert the toolbar toggle
    // above.
    expect(settings.editor.orientation).toBe("horizontal");

    // Give the P8-03 tab poll (every 2s) at least one chance to persist
    // the active tab before reloading.
    await page.waitForTimeout(2500);

    await page.reload();
    await waitForReady(page);

    // Settings: persisted via localStorage, applied on load - no user
    // action needed.
    settings = await page.evaluate(() => window.__unsTestHooks.getSettings());
    expect(settings.fonts.font_size).toBe(60);
    expect(settings.editor.orientation).toBe("horizontal");

    // Content: the tab's text is restored silently - no banner, no
    // click required (P8-03 superseded the P2-05 recovery banner this
    // test used to exercise here).
    await expect
      .poll(() => page.evaluate(() => window.__unsTestHooks.getText().trim()))
      .toBe("Hello Playwright");
  });
});
