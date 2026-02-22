@echo off
echo Building Tailwind CSS...
call npm run build:css
if %ERRORLEVEL% NEQ 0 exit /b 1

echo.
echo Building WASM...
call wasm-pack build --target web --release
if %ERRORLEVEL% NEQ 0 exit /b 1

echo.
echo Build complete!
echo.
echo To run the editor, start an HTTP server:
echo   python -m http.server 8000
echo.
echo Then open http://localhost:8000 in your browser.
