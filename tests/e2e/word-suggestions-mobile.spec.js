// WS-08 (mobile trigger wiring) regression test: word suggestions must
// also appear when text is entered through the hidden `#mobile-input`
// field's `input` event - the path virtual keyboards actually use -
// not just through the desktop canvas's `keydown` listener.
//
// Emulates a real mobile device (`devices["Pixel 5"]`, matching
// `isMobileDevice`'s user-agent sniff in `index.html`, which checks for
// "Android" among others) via `test.use()` so the app's own
// mobile-vs-desktop branching takes the mobile path, exactly as it
// would on a real phone/tablet - this is not a desktop test pretending
// to be mobile, the app genuinely believes it's running on one.
// Deliberately an Android preset, not an iPhone one - Playwright's iOS
// device presets default to the `webkit` browser engine, which isn't
// installed in this environment (only Chromium is - see
// `PHASE_7_04_IMPL.md`); Android presets default to `chromium`.
const { test, expect, devices } = require("@playwright/test");

test.use({ ...devices["Pixel 5"] });

async function waitForReady(page) {
  await page.waitForFunction(() => window.__unsTestHooks?.isReady() === true, {
    timeout: 30_000,
  });
  await expect(page.locator("#loading")).toBeHidden();
}

// Mirrors `word-suggestions.spec.js`'s `typeCyrillic` helper, but for
// the mobile input path: real virtual keyboards drive text entry
// through the hidden `<input>`'s native `input` event, not `keydown`
// (`#mobile-input`'s own `keydown` listener only handles
// Backspace/Enter/popup-navigation - see `index.html`).
async function typeCyrillicViaMobileInput(page, text) {
  await page.evaluate((value) => {
    const el = document.getElementById("mobile-input");
    el.focus();
    el.value = value;
    el.dispatchEvent(new Event("input", { bubbles: true }));
  }, text);
}

async function prepareBlankCyrillicInput(page) {
  // `#keyboard-btn` (toggles Latin->Mongolian conversion off) is
  // "always visible" regardless of viewport width - unlike most of the
  // desktop toolbar, which collapses into `#mobile-menu` below this
  // width (see the "Always visible" comment above it in `index.html`).
  await page.locator("#keyboard-btn").click();
  await page.locator("#mobile-menu-btn").click();
  await page.locator("#mobile-clear-btn").click();
}

function getSuggestionsState(page) {
  return page.evaluate(() => window.__unsTestHooks.getSuggestionsState());
}

test.describe("Word suggestion popup on mobile (WS-08)", () => {
  test("typing via the virtual-keyboard input path shows a suggestion", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);

    const isMobile = await page.evaluate(() =>
      /iPhone|iPad|iPod|Android/i.test(navigator.userAgent),
    );
    expect(isMobile).toBe(true);

    await prepareBlankCyrillicInput(page);
    await typeCyrillicViaMobileInput(page, "аалз");

    const popup = page.locator("#word-suggestions-popup");
    await expect(popup).toBeVisible();
    const state = await getSuggestionsState(page);
    expect(state.suggestions).toEqual(["\u1820\u182d\u1820\u182f\u1835\u1822"]);
  });

  test("tapping the popup canvas accepts the suggestion", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlankCyrillicInput(page);
    await typeCyrillicViaMobileInput(page, "аалз");

    const popup = page.locator("#word-suggestions-popup");
    await expect(popup).toBeVisible();

    await page.locator("#word-suggestions-canvas").tap();

    await expect(popup).toBeHidden();
    await expect
      .poll(() => page.evaluate(() => window.__unsTestHooks.getText().trim()))
      .toBe("\u1820\u182d\u1820\u182f\u1835\u1822");
  });

  test("accepting a noun suffix clears pending NNBSP before a space", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlankCyrillicInput(page);

    // Input mode is off here, so this writes the Bichig stem literally.
    await typeCyrillicViaMobileInput(page, "\u1828\u1823\u182e"); // ном
    // Turn conversion back on; '-' buffers NNBSP and opens suffixes.
    await page.locator("#keyboard-btn").click();
    await typeCyrillicViaMobileInput(page, "-");

    const popup = page.locator("#word-suggestions-popup");
    await expect(popup).toBeVisible();
    const suffix = (await getSuggestionsState(page)).suggestions[0];
    await page.evaluate(() => {
      document
        .getElementById("mobile-input")
        .dispatchEvent(
          new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }),
        );
    });
    // A selected noun case now advances directly to possessive choices.
    await expect(popup).toBeVisible();

    // A stale mobile NNBSP buffer used to emit a second U+202F before
    // this space, displayed by the editor as an [N] marker.
    await typeCyrillicViaMobileInput(page, " ");
    const text = await page.evaluate(() => window.__unsTestHooks.getText());
    expect(text.trimEnd()).toBe("\u1828\u1823\u182e\u202f" + suffix);
    expect([...text].filter((char) => char === "\u202f")).toHaveLength(1);
  });

  test("the on-screen suffix button opens noun suffixes without typing hyphen", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlankCyrillicInput(page);

    await typeCyrillicViaMobileInput(page, "\u1828\u1823\u182e"); // ном
    await page.locator("#mobile-nnbsp-btn").click();

    const popup = page.locator("#word-suggestions-popup");
    await expect(popup).toBeVisible();
    const state = await getSuggestionsState(page);
    expect(state.suggestions).toContain("\u1824\u1828"); // genitive -un
    await expect
      .poll(() => page.evaluate(() => window.__unsTestHooks.getText().trim()))
      .toBe("\u1828\u1823\u182e");
  });

  test("Backspace after a suggestion is showing refreshes it rather than leaving it stale", async ({
    page,
  }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlankCyrillicInput(page);
    await typeCyrillicViaMobileInput(page, "аалз");

    const popup = page.locator("#word-suggestions-popup");
    await expect(popup).toBeVisible();

    await page.evaluate(() => {
      document
        .getElementById("mobile-input")
        .dispatchEvent(
          new KeyboardEvent("keydown", { key: "Backspace", bubbles: true, cancelable: true }),
        );
    });
    await page.waitForTimeout(200);

    // "аал" (after backspacing the "з") no longer matches "аалз"'s
    // exact single suggestion - the popup should reflect the new,
    // shorter word, not keep showing the old one.
    const state = await getSuggestionsState(page);
    expect(state.suggestions).not.toEqual(["\u1820\u182d\u1820\u182f\u1835\u1822"]);
  });
});
