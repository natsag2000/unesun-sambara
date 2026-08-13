#!/bin/bash
set -e

echo "Preparing dictionary..."
npm run prepare:dict

echo ""
echo "Building Tailwind CSS..."
npm run build:css

echo ""
echo "Building WASM..."
wasm-pack build --target web --release

# P7-02: an extra strip pass on top of wasm-opt (already enabled via
# `wasm-opt = ["-Oz"]` in Cargo.toml, which wasm-pack runs
# automatically above). wasm-strip is part of WABT
# (https://github.com/WebAssembly/wabt) and isn't always installed, so
# this degrades gracefully rather than failing the build.
if command -v wasm-strip &> /dev/null; then
  echo ""
  echo "Stripping debug info with wasm-strip..."
  wasm-strip pkg/uns_editor_bg.wasm
else
  echo ""
  echo "wasm-strip not found (part of WABT) - skipping extra strip pass."
  echo "Install it for a slightly smaller binary: https://github.com/WebAssembly/wabt"
fi

echo ""
echo "Final WASM size: $(du -h pkg/uns_editor_bg.wasm | cut -f1)"

echo ""
echo "Build complete!"
echo ""
echo "To run the editor, start an HTTP server:"
echo "  python3 -m http.server 8000"
echo ""
echo "Then open http://localhost:8000 in your browser."
