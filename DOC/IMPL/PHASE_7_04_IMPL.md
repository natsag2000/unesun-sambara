# P7-04 — Playwright smoke test (implementation log)

Date: 2026-07-31
Reference: `prompt/FUTURE_PLAN.md`, P7-04
Patch: `patch/Phase_7_04.patch`

## Scope

| ID    | Effort | Status | Summary                                                                          |
|-------|--------|--------|-----------------------------------------------------------------------------------|
| P7-04 | S      | done   | Playwright end-to-end smoke test, run locally against a real headless Chromium and wired into a new CI workflow |

Done ahead of P8-03 (multiple documents / tab strip) at the user's request,
specifically so P8-03 could be built with real automated browser
verification from the start rather than static code review only (the
sandbox this project is developed in has no display).

## Design decisions

- **A read-only test seam (`window.__unsTestHooks`) was added to
  `index.html`.** The editor renders to a single `<canvas>`; there is no
  DOM text node to assert against for "does the document contain X".
  The hook exposes `getText()`, `getSettings()`, `getCursor()`, and
  `isReady()` - all of which already existed as public `WasmEditor`
  methods the app itself calls (`get_text`, `get_settings_json`,
  `get_cursor_position`). This is not a new capability an attacker could
  exploit; it is always present (not gated behind a query param or
  build flag) because the app has no authentication or server-side state
  for such a thing to put at risk. If a future phase makes that
  assumption stop holding (e.g. some kind of account system), this
  should be revisited.
- **The test targets a real, installed headless Chromium, not a
  described-but-unverified test.** Checked feasibility first (network
  + disk available in this environment) before writing anything;
  `npx playwright install chromium` (without `--with-deps`, since `sudo`
  is unavailable here) downloaded a Chromium that launches successfully
  headless with no missing system libraries. The test was run for real
  and passed, then deliberately broken (one assertion's expected value
  changed) and re-run to confirm it actually fails when something is
  wrong, not just when nothing is - a real regression-catching test, not
  a tautology.
- **Browser binaries are not committed**, matching how `pkg/` (wasm-pack
  output) and `Cargo.lock`/`target/` are already handled in this repo:
  large, machine-specific, and trivially regenerated
  (`npx playwright install chromium`). `.gitignore` gained entries for
  Playwright's own run-artifact directories (`test-results/`,
  `playwright-report/`, etc.) for the same reason.
- **The dev server for tests reuses `serve.sh`/`serve.bat`'s exact
  command** (`python3 -m http.server`), via Playwright's `webServer`
  config, so CI and a developer's machine exercise the identical serving
  setup rather than a test-only server implementation that could behave
  differently.
- **A GitHub Actions workflow (`.github/workflows/e2e.yml`) was added**
  even though the plan only explicitly asked for "run headless in CI" as
  a capability, not a specific CI config - there is a real GitHub remote
  for this repo (and a `CNAME`, suggesting GitHub Pages), so wiring it up
  is a small, contained addition that makes the capability real rather
  than aspirational. It rebuilds `pkg/` and `dist/output.css` from
  scratch (both gitignored) before running the same
  `npm run test:e2e` a developer would run locally.
- **The test covers the plan's checklist against *current* (pre-P8-03)
  behavior**, specifically: typing lands via the P2-05 auto-save draft +
  recovery banner for "content survived a reload," since nothing is
  explicitly saved to a file in the test. This is a known, deliberate
  target for near-term change: P8-03 is expected to supersede that
  banner with silent multi-tab session restoration, at which point this
  test's reload/recovery assertions need to be updated to match (tracked
  as a P8-03 follow-up, not deferred indefinitely).

## Files changed

- **New:** `playwright.config.js` - single Chromium project, `webServer`
  auto-starts the same static server used for local dev.
- **New:** `tests/e2e/smoke.spec.js` - one test: toggle off Latin input
  mode, clear the document, type text, toggle orientation, open
  settings and change font size, save, wait for one auto-save poll,
  reload, assert settings persisted (including that orientation -
  toggled via the toolbar, not the settings modal - survived the modal's
  merge-on-save from P0-02), assert the recovery banner appears and
  restoring it reproduces the typed text.
- **New:** `.github/workflows/e2e.yml` - installs Rust wasm32 target +
  wasm-pack + Playwright's Chromium, rebuilds `pkg/`/`dist/output.css`,
  runs the smoke test on push to `main`/`dev` and on pull requests.
- Modified: `index.html` - `window.__unsTestHooks` read-only accessors
  (see design decisions).
- Modified: `package.json` / `package-lock.json` - `@playwright/test`
  dev dependency; new `test:e2e` script.
- Modified: `.gitignore` - Playwright run-artifact directories.

## Verification

- Ran the test for real against a headless Chromium installed in this
  environment: **passed** (`1 passed (6.1s)`).
- Deliberately broke one assertion's expected value and re-ran: **failed
  with a clear diff** (`Expected: "Hello Playwright BROKEN" / Received:
  "Hello Playwright"`), confirming the test actually exercises the app
  rather than trivially passing. Reverted before committing.
- Validated `.github/workflows/e2e.yml`'s YAML syntax by parsing it
  (`js-yaml`) - not executed (no GitHub Actions runner available in this
  environment), so the workflow itself is unverified beyond syntax and
  step-by-step reasoning against commands already used successfully
  locally (`wasm-pack build`, `npm run build:css`,
  `npx playwright install chromium`, `npm run test:e2e`).
- `node --check` against the extracted `<script type="module">` body
  after adding the test hook - no syntax errors.

## Follow-ups / known limits

- **The CI workflow itself has not run in real GitHub Actions** (only
  syntax-validated and reasoned through locally-proven commands) -
  first push should be watched to confirm it actually goes green, in
  particular the `wasm-pack` installer step (`curl | sh`), which is the
  one command in the workflow not already exercised elsewhere in this
  session.
- **Single test, single browser project.** This is deliberately a smoke
  test (per the plan's own framing), not a comprehensive suite. Chromium
  only - no Firefox/WebKit projects were added, since the plan didn't
  ask for cross-browser coverage and this app's WASM+Canvas rendering
  path is unlikely to differ meaningfully across engines for a smoke-level
  check.
- **P8-03 must update this test.** Once tabs supersede the P2-05
  auto-save banner, the reload/content-recovery assertions here
  (`#autosave-banner`, `#autosave-restore-btn`) will reference elements
  that no longer exist. This is called out explicitly so it isn't
  discovered as a surprise CI failure during or after P8-03.
- **No coverage yet for Phase P2/P4 features added in earlier sessions**
  beyond what this smoke test incidentally touches (settings save,
  orientation). Undo/redo, find/replace, line numbers, and themes are
  all unexercised by automation - worth a second, more targeted test
  file if regressions in those areas become a recurring problem, but out
  of scope for a single "smoke test" item.

## Next phase

P8-03 (multiple documents / tab strip), per the user's explicit request
to do these two items ahead of the plan's suggested order. This
infrastructure means P8-03 can be verified with `npm run test:e2e`
during development rather than only after the fact.
