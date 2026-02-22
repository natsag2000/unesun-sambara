#!/bin/bash
set -e

echo "Building Tailwind CSS..."
npm run build:css

echo ""
echo "Building WASM..."
wasm-pack build --target web --release

echo ""
echo "Build complete!"
echo ""
echo "To run the editor, start an HTTP server:"
echo "  python3 -m http.server 8000"
echo ""
echo "Then open http://localhost:8000 in your browser."
