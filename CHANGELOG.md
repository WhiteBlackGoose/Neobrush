# Changelog

## v0.3.1-alpha

### Changed
- Layer chords: <kbd>L</kbd> <kbd>K</kbd> / <kbd>L</kbd> <kbd>J</kbd> switch to the layer above / below; <kbd>L</kbd> <kbd>⇧K</kbd> / <kbd>L</kbd> <kbd>⇧J</kbd> move the active layer up / down.
- <kbd>L</kbd> <kbd>1</kbd> <kbd>1</kbd>, <kbd>L</kbd> <kbd>2</kbd> <kbd>2</kbd>, … switch to layer 1, 2, …

## v0.3.0-alpha

### New
- **Toolbar sections**: File, Edit, View, Image, Layers, Adjustments, Effects and Tools, each with its most used commands as labelled icon buttons and a **⋯** popup with the rest. On narrow windows the less important buttons hide.
- **Command palette button** with a spinning gradient ring on hover, right after the toolbar icons.
- **Tool options moved into the left sidebar** below a six-column tool grid, so the canvas gets more height.
- Help: link to the source code on GitHub. A small link in the status bar to donate to [Voices of Children](https://voices.org.ua/en/).

### Changed
- The main menu is hidden by default. **Tap Alt** to show it (like most desktop apps); it hides again after you pick something. View › Menu Bar keeps it visible. On macOS it stays in the system menu bar.
- The toolbar no longer shows the “Neobrush” wordmark.

## v0.2.0-alpha

### New
- **Command palette.** Tap <kbd>Space</kbd> (or click the search bar) and type: fuzzy search over every command, tool, layer, open document and recent file, with matches highlighted and recently used commands ranked first. Every row shows its shortcut. Quick commands understand free text: `zoom 200`, `#ff6a00`, `size 40`, `opacity 50`, `rename Sky`, `new 1920x1080`, `layer 3`.
- **Chords.** Key sequences for everything, with a hint panel showing what comes next: `L 2 T` toggles layer 2, `L 2 S` switches to it, `L 2 O` shows only layer 2, `F B G` opens Gaussian Blur, `A C` Curves, `I R` Resize, `V T D` dark theme.
- **Every action has a shortcut**, shown in the menus, the palette and the new keyboard shortcut overview (<kbd>F1</kbd>).
- Layers show their number (used by the chords). <kbd>Alt</kbd>+<kbd>1</kbd>…<kbd>9</kbd> switches documents.
- A Tools menu. <kbd>Esc</kbd> closes dialogs.

### Changed
- Clone Stamp moved to <kbd>C</kbd>. <kbd>G</kbd> cycles Gradient and Paint Bucket. <kbd>F</kbd> starts effect chords.
- Holding <kbd>Space</kbd> still pans; a short tap opens the palette.

### Faster
Measured on an 8K image:
- Zoomed-out rendering 4× faster (mipmaps); moving the mouse no longer re-renders the canvas
- Rectangle selection drag: 17 ms → ~0 per update; ellipse 34 → 8 ms
- Undo/redo 7× faster (only changed tiles are recomposited)
- Paint bucket and magic wand 60× faster
- Effect previews only compute the visible area; gradients and moving pixels render the visible area while dragging
- The web version is built with full optimization and WebAssembly SIMD

### Fixed
- Keyboard shortcuts work again right after closing a dialog or the palette.

## v0.1.1-alpha

### Fixed
- Menu shortcuts (Ctrl+A, Ctrl+Z, Ctrl+C, Ctrl+V…) no longer change the image while you type in a text field.
- Menu actions are ignored while a dialog is open, so e.g. Ctrl+Z can't undo behind an open Curves dialog.
- The History panel now scrolls to keep the current step visible.
- Sliders (e.g. layer opacity) now follow external changes after being dragged, so they no longer show stale values when you switch layers.
- The hex color field selects its contents when clicked, so you can type a new color straight away.

## v0.1.0-alpha

First public preview.
