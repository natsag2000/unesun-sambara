// Phase P3-03 end-to-end test: Latin-to-Mongolian mapping editor.
//
// Covers the override lifecycle: open the Input tab, edit a key's
// mapping, save, confirm it's actually used for live input conversion,
// reload (override persists via localStorage, independent of the
// document-content/settings persistence covered elsewhere), then reset
// it back to the built-in default.
const { test, expect } = require("@playwright/test");

async function waitForReady(page) {
  await page.waitForFunction(() => window.__unsTestHooks?.isReady() === true, {
    timeout: 30_000,
  });
  await expect(page.locator("#loading")).toBeHidden();
}

test.describe("Latin-to-Mongolian mapping editor (P3-03)", () => {
  test("overriding a key changes live conversion and persists across reload", async ({
    page,
  }) => {
    await page.goto("/");
    await waitForReady(page);

    // Input mode is on by default; leave it on for this test (we're
    // testing the conversion table it reads from).
    await page.locator("#settings-btn").click();
    await page.locator("#input-tab-btn").click();
    await expect(page.locator("#input-tab")).toBeVisible();

    // Override the "z" key to a distinctive placeholder character not
    // used anywhere else in the mapping.
    const zInput = page.locator('#latin-mapping-table-body input[data-latin-key="z"]');
    await expect(zInput).toBeVisible();
    await zInput.fill("Q");
    // Editing a row shouldn't require a modal "Save" - overrides are
    // applied (and persisted) immediately, unlike the color/font
    // fields elsewhere in this modal.
    await expect(zInput).toHaveClass(/border-editor-accent/);
    await expect(page.locator('#latin-mapping-table-body button[data-latin-reset-key="z"]')).toBeVisible();

    await page.locator("#settings-close").click();

    // Clear the document, then typing "z" should now insert "Q".
    await page.locator("#clear-btn").click();
    await page.locator("#editor-canvas").click();
    await page.keyboard.press("z");
    await expect
      .poll(() => page.evaluate(() => window.__unsTestHooks.getText().trim()))
      .toBe("Q");

    // The override survives a reload (separate localStorage key from
    // both settings and tab content).
    await page.reload();
    await waitForReady(page);
    await page.locator("#settings-btn").click();
    await page.locator("#input-tab-btn").click();
    const zInputAfterReload = page.locator(
      '#latin-mapping-table-body input[data-latin-key="z"]',
    );
    await expect(zInputAfterReload).toHaveValue("Q");

    // Reset it back to the built-in default.
    await page.locator('#latin-mapping-table-body button[data-latin-reset-key="z"]').click();
    await expect(zInputAfterReload).not.toHaveClass(/border-editor-accent/);
    const defaultValue = await zInputAfterReload.inputValue();
    expect(defaultValue).not.toBe("Q");
    expect(defaultValue.length).toBeGreaterThan(0);
  });
});
