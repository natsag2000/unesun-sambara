@echo off
echo Preparing dictionary...
call npm run prepare:dict
if %ERRORLEVEL% NEQ 0 exit /b 1

echo.
echo Building Tailwind CSS...
call npm run build:css
if %ERRORLEVEL% NEQ 0 exit /b 1

echo.
echo Building WASM...
call wasm-pack build --target web --release
if %ERRORLEVEL% NEQ 0 exit /b 1

REM P7-02: an extra strip pass on top of wasm-opt (already enabled via
REM `wasm-opt = ["-Oz"]` in Cargo.toml, which wasm-pack runs
REM automatically above). wasm-strip is part of WABT
REM (https://github.com/WebAssembly/wabt) and isn't always installed,
REM so this degrades gracefully rather than failing the build.
where wasm-strip >nul 2>nul
if %ERRORLEVEL% EQU 0 (
  echo.
  echo Stripping debug info with wasm-strip...
  wasm-strip pkg\uns_editor_bg.wasm
) else (
  echo.
  echo wasm-strip not found (part of WABT^) - skipping extra strip pass.
  echo Install it for a slightly smaller binary: https://github.com/WebAssembly/wabt
)

echo.
echo Build complete!
echo.
echo To run the editor, start an HTTP server:
echo   python -m http.server 8000
echo.
echo Then open http://localhost:8000 in your browser.
