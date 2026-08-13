// Phase P7-01 end-to-end test: dirty-flag-driven rendering.
//
// The core claim of P7-01 is "skip the canvas redraw when nothing
// changed" - this test exists to catch a regression where that gating
// silently stops gating (e.g. a future change that marks the frame
// dirty unconditionally somewhere, or removes the check in `animate()`
// entirely), which would be easy to miss since the app would still
// *look* correct, just burn CPU doing full-frame redraws 60 times a
// second again.
const { test, expect } = require("@playwright/test");

async function waitForReady(page) {
  await page.waitForFunction(() => window.__unsTestHooks?.isReady() === true, {
    timeout: 30_000,
  });
  await expect(page.locator("#loading")).toBeHidden();
}

test.describe("Dirty-flag-driven rendering (P7-01)", () => {
  test("needs_render() goes false while idle", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);

    // Poll rather than asserting a single point in time: this races
    // the live rAF loop, which itself legitimately returns `true` once
    // per ~500ms cursor-blink interval. The property under test is
    // "the gate can be closed at all" (i.e. it isn't stuck permanently
    // dirty), not its exact timing - a single idle `false` reading
    // anywhere in this window proves that.
    await expect
      .poll(
        () => page.evaluate(() => window.__unsTestHooks.needsRender(performance.now())),
        { timeout: 5_000 },
      )
      .toBe(false);
  });
});
