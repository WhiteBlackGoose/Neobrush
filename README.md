# Neobrush

A modern, fast, cross-platform raster graphics editor in the spirit of Paint.NET,
with a cleaner UI that follows your OS's light/dark theme.

Runs on **Windows**, **macOS**, **Linux** (Wayland and X11) and **FreeBSD**.
It's written in Rust with a [Slint](https://slint.dev) UI and ships as a single native binary.

## Features

**Tools**: Rectangle / Ellipse / Lasso select, Magic Wand, Move Selected Pixels (move, scale, rotate),
Move Selection, Zoom, Pan, Paint Bucket, Gradient (linear, reflected, diamond, radial, conical,
color or transparency mode), Paintbrush (size, hardness, antialiasing), Eraser, Pencil,
Color Picker, Clone Stamp, Recolor, Text (any system font, bold/italic/underline, alignment),
Line / Curve (bezier, dashes) and Shapes (rectangle, rounded rectangle, ellipse, triangle,
diamond, pentagon, hexagon, star, arrow, heart). Shapes, lines and text stay editable
with handles until you finish them.

**Selections**: replace, add, subtract, intersect and invert modes (Ctrl adds, Alt subtracts),
marching ants, select all / deselect / invert, crop to selection. Every paint operation
and effect is clipped to the selection.

**Layers**: unlimited layers with 16 blend modes (Normal, Multiply, Color Burn, Color Dodge,
Reflect, Glow, Overlay, Difference, Negation, Lighten, Darken, Screen, Xor, Hard Light,
Soft Light, Additive), opacity, visibility, add / delete / duplicate / merge down /
reorder, import from file, flip, and rotate / zoom.

**Image**: resize (supersampling, bicubic, bilinear, nearest), canvas size with anchor,
crop to selection, auto crop, flip, rotate 90° / 180°, flatten.

**Adjustments**: Auto-Level, Black and White, Brightness/Contrast, Curves (per channel),
Hue/Saturation, Invert Colors, Levels, Posterize, Sepia, Vibrance, Temperature/Tint.

**Effects**, all with a live preview:
- Artistic: Ink Sketch, Oil Painting, Pencil Sketch
- Blurs: Fragment, Gaussian, Motion, Radial, Surface, Unfocus, Zoom
- Distort: Bulge, Dents, Frosted Glass, Pixelate, Polar Inversion, Tile Reflection, Twist
- Noise: Add Noise, Median, Reduce Noise
- Photo: Glow, Red Eye Removal, Sharpen, Soft Portrait, Vignette
- Render: Clouds, Julia and Mandelbrot fractals
- Stylize: Edge Detect, Emboss, Outline, Relief
- Object: Drop Shadow

**Everything else**: multiple open documents as tabs with thumbnails, a history panel
(click any step to jump to it), clipboard integration (copy, copy merged, paste,
paste into new layer or new image), pixel grid, rulers, zoom from 1% to 6400%,
a color panel with HSV picker, hex input, alpha, palette and recent colors,
unsaved-changes prompts, settings and recent files that persist between sessions,
and light/dark theme following the OS (or forced from *View → Theme*).

**File formats**: OpenRaster `.ora`, Neobrush's native layered format, which is also
readable by GIMP, Krita and MyPaint. It also reads and writes PNG, JPEG (with a quality
setting), WebP, BMP, GIF, TIFF, TGA, ICO and QOI.

## Building

You need a recent stable Rust toolchain (`rustup` is easiest).

```sh
cargo run --release              # build and run
cargo run --release -- image.png # open files directly
```

### Platform dependencies

| Platform | Packages |
| --- | --- |
| Debian / Ubuntu | `libfontconfig1-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev` |
| Fedora | `fontconfig-devel libxkbcommon-devel libxkbcommon-x11-devel wayland-devel libX11-devel libXcursor-devel libXrandr-devel libXi-devel mesa-libGL-devel` |
| Arch | `fontconfig libxkbcommon libxkbcommon-x11 wayland libx11 libxcursor libxrandr libxi mesa` |
| FreeBSD | `pkg install rust pkgconf fontconfig libxkbcommon wayland libX11 libXcursor libXrandr libXi mesa-libs` |
| NixOS / Nix | `nix develop` (dev shell) or `nix run` |
| Windows, macOS | nothing extra |

On Linux and FreeBSD the native windowing backend (Wayland or X11) is picked automatically.
File dialogs go through the XDG desktop portal.

### Environment variables

- `NEOBRUSH_THEME=light|dark` forces a theme (overrides the OS setting).
- `SLINT_BACKEND=winit-software` uses the CPU renderer if OpenGL isn't available.

## Keyboard shortcuts

Press **F1** in the app for the full list. The basics:

| | |
| --- | --- |
| `S` `M` `B` `P` `E` `F` `G` `K` `L` `R` `T` `O` `H` `Z` | tools (pressing `S`, `M` or `O` again cycles through the related tools) |
| `[` `]` | brush size |
| `X` / `D` | swap / reset colors |
| Space + drag, middle mouse | pan |
| Ctrl + wheel | zoom |
| Enter / Esc | finish text, shape or line |
| Delete / Backspace | erase / fill selection |
| Ctrl+Z / Ctrl+Y | undo / redo |

## License

Neobrush is MIT licensed. It uses [Slint](https://slint.dev) under the Slint Royalty-free
Desktop license, [Lucide](https://lucide.dev) icons (ISC) and the
[Inter](https://rsms.me/inter/) typeface (SIL OFL).
