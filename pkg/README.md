# UNS Editor

Traditional Mongolian Text Editor built with WebAssembly and cosmic-text.

## Features

### Core Editing
- **Vertical Text Support**: Traditional Mongolian script rendering in vertical orientation (top-to-bottom, columns left-to-right)
- **Unicode Support**: Full support for Traditional Mongolian Unicode characters (U+1800-U+18AF, U+11660-U+1167F)
- **Glyph Rotation**: Automatic 90° rotation for Latin characters in vertical mode
- **Mouse Interaction**: Click to position cursor, drag to select text
- **Keyboard Navigation**: Arrow keys, Home/End, PageUp/PageDown
- **Text Selection**: Shift+Arrow keys for selection
- **Clipboard Support**: Ctrl+V for paste
- **Orientation Toggle**: Ctrl+Alt+V to switch between vertical and horizontal modes

### Settings & Customization
- **Settings Modal**: Comprehensive settings UI with live preview
- **Font Customization**: Adjustable font family, size (12-48px), and line height (20-60px)
- **Color Themes**: Customizable text and background colors with color pickers
- **Line Numbers**: Optional line and column position display in status bar
- **Persistence**: Settings automatically saved to localStorage
- **Live Preview**: See changes in real-time before applying

### Architecture
- **Modular Design**: Organized into config, editor_core, and UI modules
- **Settings System**: Type-safe Rust configuration with JSON serialization
- **Tailwind CSS**: Modern, maintainable styling with utility classes
- **Extensible**: Ready for plugins, syntax highlighting, and more

## Building

### Prerequisites

- Rust toolchain
- wasm-pack: `cargo install wasm-pack`
- Node.js and npm (for Tailwind CSS)

### Quick Build

**Linux/Mac:**
```bash
npm install  # First time only
./build.sh
```

**Windows:**
```batch
npm install  # First time only
build.bat
```

This will:
1. Build Tailwind CSS (→ `dist/output.css`)
2. Build WASM module (→ `pkg/`)

### Manual Build

1. Install npm dependencies (first time only):
   ```bash
   npm install
   ```

2. Build Tailwind CSS:
   ```bash
   npm run build:css
   ```

3. Build WASM module:
   ```bash
   wasm-pack build --target web --release
   ```

### Local Development

1. Build the project (see above)

2. Serve with a local HTTP server:
   ```bash
   python3 -m http.server 8000
   ```

3. Open http://localhost:8000 in your browser

### Development Workflow

For CSS development with hot reload:
```bash
npm run watch:css  # In one terminal
python3 -m http.server 8000  # In another terminal
```

## Usage

### Basic Editing

1. Click on the canvas to focus the editor
2. Type text using your keyboard (supports Latin and Mongolian input)
3. Use arrow keys to navigate
4. Hold Shift while using arrow keys to select text
5. Click "Toggle Vertical" button or press Ctrl+Alt+V to switch orientations
6. Click "Clear" button to clear all text

### Settings

Click the "⚙️ Settings" button in the header to open the settings modal:

**Font Settings:**
- **Font Family**: Choose between Noto Sans Mongolian and Noto Sans
- **Font Size**: Adjust from 12px to 48px with live preview
- **Line Height**: Adjust from 20px to 60px

**Color Settings:**
- **Text Color**: Choose text color with color picker or hex input
- **Background Color**: Choose background color with color picker or hex input
- Live preview shows changes before saving

**Editor Behavior:**
- **Show Line Numbers**: Toggle line and column display in status bar

**Controls:**
- **Save Changes**: Apply settings and save to localStorage
- **Cancel**: Discard changes and close modal
- **Reset to Defaults**: Restore original settings
- **× Close**: Close modal without saving

Settings persist across browser sessions via localStorage.

## Fonts

The editor uses Noto Sans Mongolian font for Traditional Mongolian script rendering and Noto Sans for Latin text fallback. Font files are located in the `fonts/` directory:

- `NotoSansMongolian-Regular.ttf` - Traditional Mongolian script
- `NotoSans-Regular.ttf` - Latin script fallback

## Dependencies

### Rust
- **cosmic-text**: Custom version with Traditional Mongolian support (path: `../cosmic-text`)
- **wasm-bindgen**: Rust/WASM bindings
- **web-sys**: Web API bindings
- **serde** & **serde_json**: Settings serialization

### JavaScript
- **tailwindcss**: Utility-first CSS framework (dev dependency)

## Project Structure

```
uns-editor/
├── src/
│   ├── config/
│   │   ├── mod.rs              # Config module
│   │   └── settings.rs         # Settings types & serialization
│   ├── editor_core/
│   │   ├── mod.rs              # Core module
│   │   └── editor_state.rs    # Editor state management
│   ├── lib.rs                  # WASM API & rendering
│   └── styles.css              # Tailwind entry point
├── fonts/
│   ├── NotoSansMongolian-Regular.ttf
│   └── NotoSans-Regular.ttf
├── dist/
│   └── output.css              # Compiled Tailwind CSS (generated)
├── pkg/                        # WASM output (generated)
├── index.html                  # Web interface with settings modal
├── tailwind.config.js          # Tailwind configuration
├── package.json                # npm dependencies
├── Cargo.toml                  # Rust dependencies
├── build.sh / build.bat        # Build scripts
└── README.md
```

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Ctrl+Alt+V | Toggle vertical/horizontal text |
| Ctrl+V | Paste from clipboard |
| Shift+Arrow | Extend selection |
| Home | Move to start of line |
| End | Move to end of line |
| Page Up/Down | Scroll by page |
| Backspace | Delete character before cursor |
| Delete | Delete character after cursor |
| Enter | Insert new line |
| Tab | Insert tab/indent |

## Browser Support

Works in all modern browsers that support:
- WebAssembly
- HTML5 Canvas
- ES6 Modules

Tested on:
- Chrome/Chromium 90+
- Firefox 89+
- Safari 14+
- Edge 90+

## Troubleshooting

### WASM module not found
Make sure you've run `wasm-pack build --target web --release` first.

### Cross-Origin Request Blocked
You need to serve the files through an HTTP server, not open them directly as `file://`.

### Fonts not rendering correctly
The editor loads fonts from the `fonts/` directory via fetch API. Make sure:
- The font files exist in the `fonts/` directory
- You're serving the app through an HTTP server
- Check browser console for any font loading errors

## License

This project depends on cosmic-text, which is dual-licensed under MIT OR Apache-2.0.
