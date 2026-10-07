<div align="center">

<img src="docs/media/logo.png" width="112" alt="Neobrush logo">

# Neobrush

**A fast, modern raster graphics editor in the spirit of Paint.NET.**<br>
Native on Windows, macOS, Linux and FreeBSD. Light and dark, just like your OS.

[![Release](https://img.shields.io/github/v/release/WhiteBlackGoose/Neobrush?include_prereleases&label=release&color=7c5cff&style=flat-square)](https://github.com/WhiteBlackGoose/Neobrush/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/WhiteBlackGoose/Neobrush/ci.yml?branch=main&label=build&style=flat-square)](https://github.com/WhiteBlackGoose/Neobrush/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-3b82f6?style=flat-square)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20·%20macOS%20·%20Linux%20·%20FreeBSD-444?style=flat-square)
![Rust](https://img.shields.io/badge/Rust-Slint-dea584?style=flat-square&logo=rust&logoColor=white)

[**▶ Try it in your browser**](https://neobrush.wbg.gg) · [**Download**](#download) · [Features](#features) · [Palette & shortcuts](#shortcuts) · [Building](#building)

<br>

<img src="docs/media/screenshot-dark.png" width="49%" alt="Neobrush in dark mode"> <img src="docs/media/screenshot-light.png" width="49%" alt="Neobrush in light mode">

</div>

---

## ✨ See it in action

### ⚡ Drive everything from the keyboard

Tap <kbd>Space</kbd> and type: fuzzy search finds any command, tool or layer, and quick commands like `#ffb347`, `size 60` or `zoom 100` just work. Chords like <kbd>L</kbd> <kbd>2</kbd> <kbd>O</kbd> (show only layer 2) or <kbd>V</kbd> <kbd>T</kbd> <kbd>L</kbd> (light theme) come with a hint panel, so you never have to memorize them.

<p align="center"><img src="docs/media/command-palette.gif" alt="Command palette and chords in Neobrush" width="90%"></p>

<table>
<tr>
<td width="50%" valign="top">

### 🎨 Painting from scratch

A gradient sky, a sun with a glow, lasso-cut mountains, stars on their own layer, a bezier shooting star and a title. Everything is done with Neobrush's own tools.

</td>
<td width="50%" valign="top">

### 📷 Editing a photo

Auto-Level, an S-curve in Curves, Hue/Saturation, Vignette, Sharpen, a crop and a caption layer. Then a jump through the History panel to compare before and after.

</td>
</tr>
<tr>
<td><img src="docs/media/painting.gif" alt="Painting a landscape in Neobrush"></td>
<td><img src="docs/media/photo-editing.gif" alt="Editing a photo in Neobrush"></td>
</tr>
</table>

<div align="center">

| Before | After |
| :---: | :---: |
| <img src="docs/media/before.jpg" width="400" alt="Original photo"> | <img src="docs/media/after.jpg" width="400" alt="Edited photo"> |

<sub>Photo by <a href="https://unsplash.com/photos/Kt5hRENuotI">Andrew Ridley</a> on Unsplash. Both recordings are sped up to fit 30 seconds.</sub>

</div>

---

<a id="download"></a>

## 📦 Download

Grab the latest build from the [**Releases page**](https://github.com/WhiteBlackGoose/Neobrush/releases).

| Platform | Package | Notes |
| --- | --- | --- |
| 🐧 **Linux** (Debian, Ubuntu, Mint…) | `neobrush_*_amd64.deb` | `sudo apt install ./neobrush_*.deb` |
| 🐧 **Linux** (any distro) | `Neobrush-*-x86_64.AppImage` | `chmod +x` it, then run |
| 🌐 **Web** | [neobrush.wbg.gg](https://neobrush.wbg.gg) | runs in any modern browser with WebGL, nothing to install |
| ❄️ **NixOS / Nix** | flake | `nix run github:WhiteBlackGoose/Neobrush` |
| 🪟 **Windows** 10/11 | `Neobrush-*-windows-x86_64.exe` | portable, no installer needed |
| 🍎 **macOS** 11+ (Apple Silicon & Intel) | `Neobrush-*-macos-universal.dmg` | drag to Applications |
| 😈 **FreeBSD** | `neobrush-*-freebsd-x86_64.tar.gz` | |

> [!NOTE]
> Neobrush is in **alpha**. The Windows and macOS builds aren't code-signed yet.
> On macOS, right-click the app and choose **Open** the first time, or run
> `xattr -dr com.apple.quarantine /Applications/Neobrush.app`.
> On Windows, SmartScreen may ask you to confirm: choose **More info → Run anyway**.

The web version is the full editor, compiled to WebAssembly. Open and save go through the browser's file picker and downloads. Text uses the bundled Inter font, and copy / paste only work inside the app.

**Staying up to date on Nix:** `nix profile install github:WhiteBlackGoose/Neobrush` installs it, and `nix profile upgrade Neobrush` updates it.

---

<a id="features"></a>

## 🧰 Features

<table>
<tr>
<td width="50%" valign="top">

#### 🖌️ Tools
- **Selection:** rectangle, ellipse, lasso, magic wand
- **Move selected pixels:** drag, scale with handles, rotate with right-drag
- **Paint:** paintbrush (size, hardness, antialiasing), pencil, eraser
- **Fill:** paint bucket and gradients (linear, reflected, diamond, radial, conical; color or transparency mode)
- **Retouch:** clone stamp, recolor, color picker
- **Text** in any system font, with bold, italic, underline and alignment
- **Line / curve:** bezier, solid, dashed or dotted
- **11 shapes**, from rectangles to stars and hearts
- Shapes, lines and text **stay editable** until you finish them

</td>
<td width="50%" valign="top">

#### 🗂️ Layers & selections
- Unlimited layers with **16 blend modes**
- Opacity, visibility, duplicate, merge down, reorder
- Import from file, flip, rotate / zoom
- Replace, add, subtract, intersect and invert selection modes
- Marching ants. Every tool and effect respects the selection
- Crop to selection, auto crop, resize, canvas size with anchor

</td>
</tr>
<tr>
<td valign="top">

#### 🎛️ Adjustments
Auto-Level · Black & White · Brightness / Contrast · **Curves** (per channel) · Hue / Saturation · Invert · **Levels** · Posterize · Sepia · Vibrance · Temperature / Tint

#### 🌈 Effects with live preview
<details>
<summary>33 effects in 8 categories</summary>

| Category | Effects |
| --- | --- |
| Artistic | Ink Sketch, Oil Painting, Pencil Sketch |
| Blurs | Fragment, Gaussian, Motion, Radial, Surface, Unfocus, Zoom |
| Distort | Bulge, Dents, Frosted Glass, Pixelate, Polar Inversion, Tile Reflection, Twist |
| Noise | Add Noise, Median, Reduce Noise |
| Photo | Glow, Red Eye Removal, Sharpen, Soft Portrait, Vignette |
| Render | Clouds, Julia Fractal, Mandelbrot Fractal |
| Stylize | Edge Detect, Emboss, Outline, Relief |
| Object | Drop Shadow |

</details>

</td>
<td valign="top">

#### 💎 Everything else
- Tabs with live thumbnails for multiple documents
- **Command palette** with fuzzy search and quick commands; **chords** like <kbd>L</kbd> <kbd>2</kbd> <kbd>T</kbd>
- **History panel:** click any step to jump to it
- Clipboard: copy, copy merged, paste, paste into a new layer or image
- Zoom from 1% to 6400%, pixel grid, rulers
- HSV color picker with hex, alpha, palette and recent colors
- Light / dark theme follows the OS, or pick one in *View → Theme*
- Remembers your settings and recent files
- GPU-accelerated UI, multithreaded image processing

#### 💾 File formats
**OpenRaster** (`.ora`, layered, also opens in GIMP, Krita and MyPaint) ·
PNG · JPEG · WebP · BMP · GIF · TIFF · TGA · ICO · QOI

</td>
</tr>
</table>

---

<a id="shortcuts"></a>

## ⌨️ Command palette & shortcuts

Neobrush is built to be driven from the keyboard.

**Tap <kbd>Space</kbd>** to open the command palette and fuzzy-search every command, tool, layer, open document and recent file. Matches are highlighted, the commands you use most rise to the top, and each row shows its shortcut. You can also just type what you want:

| Type | Does |
| --- | --- |
| `gauss`, `hue`, `crop`… | finds the command |
| `zoom 200` / `150%` | zooms |
| `#ff6a00` | sets the color |
| `size 40` · `hardness 80` · `tolerance 30` | sets tool options |
| `opacity 50` · `rename Sky` · `layer 3` | works with layers |
| `new 1920x1080` | creates an image |

**Chords** are short key sequences, and a hint panel shows what you can press next:

| Chord | Action |
| --- | --- |
| <kbd>L</kbd> <kbd>2</kbd> <kbd>2</kbd> | switch to layer 2 (same digit twice) |
| <kbd>L</kbd> <kbd>2</kbd> <kbd>T</kbd> | toggle layer 2 (<kbd>O</kbd> show only it, <kbd>D</kbd> duplicate, <kbd>X</kbd> delete, <kbd>M</kbd> merge down, <kbd>P</kbd> properties) |
| <kbd>L</kbd> <kbd>K</kbd> / <kbd>L</kbd> <kbd>J</kbd> | switch to the layer above / below |
| <kbd>L</kbd> <kbd>⇧K</kbd> / <kbd>L</kbd> <kbd>⇧J</kbd> | move the layer up / down |
| <kbd>L</kbd> <kbd>N</kbd> | new layer |
| <kbd>F</kbd> <kbd>B</kbd> <kbd>G</kbd> | Effects › Blurs › Gaussian Blur (every effect has one) |
| <kbd>A</kbd> <kbd>C</kbd> · <kbd>A</kbd> <kbd>H</kbd> · <kbd>A</kbd> <kbd>L</kbd> | Curves · Hue / Saturation · Levels (every adjustment has one) |
| <kbd>I</kbd> <kbd>R</kbd> · <kbd>I</kbd> <kbd>C</kbd> · <kbd>I</kbd> <kbd>]</kbd> | Resize · Crop to selection · Rotate clockwise |
| <kbd>V</kbd> <kbd>T</kbd> <kbd>D</kbd> · <kbd>V</kbd> <kbd>G</kbd> | dark theme · pixel grid |

**Single keys** pick tools, Paint.NET style:

| Keys | Action |
| --- | --- |
| <kbd>S</kbd> <kbd>M</kbd> <kbd>B</kbd> <kbd>P</kbd> <kbd>E</kbd> <kbd>G</kbd> <kbd>K</kbd> <kbd>C</kbd> <kbd>R</kbd> <kbd>T</kbd> <kbd>O</kbd> <kbd>H</kbd> <kbd>Z</kbd> | tools (<kbd>S</kbd>, <kbd>M</kbd>, <kbd>G</kbd> and <kbd>O</kbd> cycle through related tools) |
| <kbd>[</kbd> / <kbd>]</kbd> · <kbd>X</kbd> / <kbd>D</kbd> | brush size · swap / reset colors |
| hold <kbd>Space</kbd>, middle mouse · <kbd>Ctrl</kbd> + wheel | pan · zoom |
| <kbd>Alt</kbd>+<kbd>1</kbd>…<kbd>9</kbd> | switch document |

Every menu item has a shortcut; press <kbd>F1</kbd> for the full list.

---

<a id="building"></a>

## 🔨 Building from source

You need a recent stable [Rust](https://rustup.rs) toolchain.

```sh
git clone https://github.com/WhiteBlackGoose/Neobrush
cd Neobrush
cargo run --release              # build and run
cargo run --release -- photo.jpg # open files directly
```

<details>
<summary><b>System dependencies</b></summary>

| Platform | Packages |
| --- | --- |
| Debian / Ubuntu | `libfontconfig1-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev` |
| Fedora | `fontconfig-devel libxkbcommon-devel libxkbcommon-x11-devel wayland-devel libX11-devel libXcursor-devel libXrandr-devel libXi-devel mesa-libGL-devel` |
| Arch | `fontconfig libxkbcommon libxkbcommon-x11 wayland libx11 libxcursor libxrandr libxi mesa` |
| FreeBSD | `pkg install rust pkgconf fontconfig libxkbcommon wayland libX11 libXcursor libXrandr libXi mesa-libs` |
| NixOS / Nix | `nix develop` |
| Windows, macOS | nothing extra |

</details>

<details>
<summary><b>Environment variables</b></summary>

- `NEOBRUSH_THEME=light|dark` forces a theme.
- `SLINT_BACKEND=winit-software` uses the CPU renderer when OpenGL isn't available.

</details>

On Linux and FreeBSD the windowing backend (Wayland or X11) is picked automatically,
and file dialogs go through the XDG desktop portal.

---

## 🙏 Credits

Built with [Rust](https://www.rust-lang.org) and [Slint](https://slint.dev).
Icons from [Lucide](https://lucide.dev) (ISC), UI font [Inter](https://rsms.me/inter/) (SIL OFL).
Inspired by the wonderful [Paint.NET](https://www.getpaint.net).

<div align="center">
<br>
<sub>Neobrush is MIT licensed. Slint is used under the Slint Royalty-free Desktop license.</sub>
</div>
