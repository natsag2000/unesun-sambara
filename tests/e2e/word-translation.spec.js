// Double-click word selection + Cyrillic translation popup
// (prompt/WORD_TRANSLATION_PLAN.md, WT-01..WT-06).
//
// Uses the real dictionary like word-suggestions.spec.js. Words were
// picked by inspecting dist/dictionary/01_kv.tsv:
//   ᠨᠣᠮ      (U+1828 U+1823 U+182E)         -> "ном"            (single)
//   ᠣᠲᠠᠴᠢ    (U+1823 U+1832 U+1820 U+1834 U+1822) -> "оточ", "асклепий" (homographs, shortest first)
//   ᠠᠷᠮᠧᠨ + FVS1 (U+180B)                   -> "армен" (dictionary stores a trailing FVS)
//
// Every test turns Latin->Mongolian input mode off first so the literal
// characters reach the buffer unconverted (same convention as
// word-suggestions.spec.js). Editing is `keydown`-event driven, hence the
// synthetic-KeyboardEvent typing helper.
const { test, expect } = require("@playwright/test");

const NOM = "\u1828\u1823\u182e";
const OTOCH = "\u1823\u1832\u1820\u1834\u1822";
const ARMEN_FVS = "\u1820\u1837\u182e\u1827\u1828\u180b";

async function waitForReady(page) {
  await page.waitForFunction(() => window.__unsTestHooks?.isReady() === true, {
    timeout: 30_000,
  });
  await expect(page.locator("#loading")).toBeHidden();
}

async function prepareBlank(page) {
  await page.locator("#keyboard-btn").click(); // Latin->Mongolian off
  await page.locator("#clear-btn").click();
  await page.locator("#editor-canvas").click();
}

async function type(page, text) {
  for (const ch of text) {
    if (ch === " ") {
      await page.keyboard.press("Space");
      continue;
    }
    await page.evaluate((key) => {
      document
        .getElementById("editor-canvas")
        .dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
    }, ch);
  }
}

// The first word starts at the document origin: in the default vertical
// mode that is the top of the first column. Clicking a little inside it
// lands on the first word without needing glyph metrics.
async function dblclickFirstWord(page) {
  const box = await page.locator("#editor-canvas").boundingBox();
  const settings = await page.evaluate(() => window.__unsTestHooks.getSettings());
  const size = settings.fonts.font_size;
  const x = box.x + 10 + size * 0.5;
  const y = box.y + 10 + size * 0.5;
  await page.mouse.dblclick(x, y);
}

const selected = (page) => page.evaluate(() => window.__unsTestHooks.getSelectedText());
const translation = (page) => page.evaluate(() => window.__unsTestHooks.getTranslationState());

test.describe("Double-click word translation", () => {
  test("selects the whole Bichig word and lists its Cyrillic translation", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlank(page);
    await type(page, NOM);
    await page.keyboard.press("Escape"); // close the completion popup

    await dblclickFirstWord(page);

    expect(await selected(page)).toBe(NOM);
    await expect(page.locator("#word-translation-popup")).toBeVisible();
    const state = await translation(page);
    expect(state.label).toBe("Cyrillic");
    expect(state.candidates).toEqual(["ном"]);
    await expect(page.locator("#sr-translation")).toContainText("ном");
  });

  test("selects only the clicked word, not its neighbours", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlank(page);
    await type(page, `${NOM} ${OTOCH}`);

    await dblclickFirstWord(page);

    expect(await selected(page)).toBe(NOM);
  });

  test("lists every homograph, shortest Cyrillic form first", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlank(page);
    await type(page, OTOCH);

    await dblclickFirstWord(page);

    await expect(page.locator("#word-translation-popup")).toBeVisible();
    expect((await translation(page)).candidates).toEqual(["оточ", "асклепий"]);
  });

  test("keeps a trailing format-control character inside the selection and still matches", async ({
    page,
  }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlank(page);
    await type(page, ARMEN_FVS);

    await dblclickFirstWord(page);

    expect(await selected(page)).toBe(ARMEN_FVS);
    await expect(page.locator("#word-translation-popup")).toBeVisible();
    expect((await translation(page)).candidates).toEqual(["армен"]);
  });

  test("an inflected word (stem + NNBSP + suffix) falls back to the stem", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlank(page);
    const inflected = `${NOM}\u202f\u1824\u1828`;
    await type(page, inflected);

    await dblclickFirstWord(page);

    expect(await selected(page)).toBe(inflected);
    await expect(page.locator("#word-translation-popup")).toBeVisible();
    const state = await translation(page);
    expect(state.label).toBe("Cyrillic (for the word stem)");
    expect(state.candidates).toEqual(["ном"]);
  });

  test("an unknown word shows a not-found message", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlank(page);
    await type(page, "\u1820\u1820\u1820\u1820\u1820");

    await dblclickFirstWord(page);

    await expect(page.locator("#word-translation-popup")).toBeVisible();
    const state = await translation(page);
    expect(state.label).toBe("No Cyrillic translation found");
    expect(state.candidates).toEqual([]);
  });

  test("a Cyrillic word is selected but gets no popup", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlank(page);
    await type(page, "ном");

    await dblclickFirstWord(page);

    expect(await selected(page)).toBe("ном");
    await expect(page.locator("#word-translation-popup")).toBeHidden();
  });

  test("Escape, typing and clicking away dismiss the popup", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlank(page);
    await type(page, NOM);
    const popup = page.locator("#word-translation-popup");

    await dblclickFirstWord(page);
    await expect(popup).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(popup).toBeHidden();

    await dblclickFirstWord(page);
    await expect(popup).toBeVisible();
    await page.keyboard.press("ArrowRight");
    await expect(popup).toBeHidden();

    await dblclickFirstWord(page);
    await expect(popup).toBeVisible();
    await page.locator("#settings-btn").hover(); // not a mousedown
    await page.mouse.down();
    await page.mouse.up();
    await expect(popup).toBeHidden();
  });

  test("popup stays inside the editor area", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlank(page);
    await type(page, NOM);

    await dblclickFirstWord(page);
    await expect(page.locator("#word-translation-popup")).toBeVisible();

    const { rect } = await translation(page);
    const container = await page.locator("#canvas-container").boundingBox();
    expect(rect.left).toBeGreaterThanOrEqual(container.x - 1);
    expect(rect.top).toBeGreaterThanOrEqual(container.y - 1);
    expect(rect.right).toBeLessThanOrEqual(container.x + container.width + 1);
    expect(rect.bottom).toBeLessThanOrEqual(container.y + container.height + 1);
  });

  test("works in horizontal orientation too", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);
    await page.locator("#vertical-btn").click();
    const orientation = await page.evaluate(
      () => window.__unsTestHooks.getSettings().editor.orientation,
    );
    expect(orientation).toBe("horizontal");
    await prepareBlank(page);
    await type(page, NOM);

    await dblclickFirstWord(page);

    expect(await selected(page)).toBe(NOM);
    await expect(page.locator("#word-translation-popup")).toBeVisible();
    expect((await translation(page)).candidates).toEqual(["ном"]);
  });

  test("the settings toggle disables the popup but not the selection", async ({ page }) => {
    await page.goto("/");
    await waitForReady(page);
    await prepareBlank(page);
    await type(page, NOM);

    await page.locator("#settings-btn").click();
    await page.locator("#editor-tab-btn").click();
    await page.locator("#word-translation-enabled").uncheck();
    await page.locator("#settings-save").click();
    await expect(page.locator("#word-translation-popup")).toBeHidden();
    expect(
      await page.evaluate(
        () => window.__unsTestHooks.getSettings().editor.word_translation_enabled,
      ),
    ).toBe(false);

    await page.locator("#editor-canvas").click();
    await dblclickFirstWord(page);

    expect(await selected(page)).toBe(NOM);
    await expect(page.locator("#word-translation-popup")).toBeHidden();
  });
});
