#!/usr/bin/env python3
"""Single source of truth for Neobrush's commands.

Generates
  * src/app/commands_gen.rs  - the command registry used by the command palette, the chord
                               engine and the keyboard shortcut overview
  * the MenuBar section of ui/app.slint (between the GENERATED MENU markers)

Run `python3 tools/gen_commands.py` after editing the table below.

Item fields:
  id      action id passed to Editor::action
  title   menu / palette title
  icon    name understood by Icons.named in ui/icons.slint
  keys    Slint key combination (menu shortcut), e.g. "Control + Shift + L"
  key     a plain key shown as the shortcut but not bound by the menu (tools, Delete, ...)
  chord   key sequence handled by the chord engine, e.g. "f b g"
  kw      extra search keywords
  checked / enabled   Slint expressions for the menu item
"""

import pathlib
import re
import textwrap

ROOT = pathlib.Path(__file__).resolve().parent.parent


def item(id, title, icon="sparkles", keys=None, key=None, chord=None, kw="", checked=None, enabled=None):
    return dict(kind="item", id=id, title=title, icon=icon, keys=keys, key=key, chord=chord, kw=kw, checked=checked, enabled=enabled)


def menu(title, *children, chord_prefix=None):
    return dict(kind="menu", title=title, children=list(children), chord_prefix=chord_prefix)


SEP = dict(kind="sep")


def raw(text):
    return dict(kind="raw", text=text)


def fx(id, title, chord, kw=""):
    return item(f"fx:{id}", title, "sparkles", chord=chord, kw=kw)


def adj(id, title, keys, chord, kw=""):
    return item(f"fx:{id}", title, "contrast", keys=keys, chord=chord, kw=kw)


def tool(name, title, icon, key, kw=""):
    return item(f"tool:{name}", title, icon, key=key, kw=kw + " tool")


MENUS = [
    menu(
        "File",
        item("file.new", "New…", "file-plus", keys="Control + N", kw="create image canvas blank"),
        item("file.open", "Open…", "folder-open", keys="Control + O", kw="load image file"),
        raw(
            """Menu {
                title: "Open Recent";
                enabled: App.recent-files.length > 0;
                for f in App.recent-files: MenuItem {
                    title: f;
                    activated => { App.action("file.recent:" + f); }
                }
                MenuSeparator { }
                MenuItem { title: "Clear Recent"; activated => { App.action("file.clear-recent"); } }
            }"""
        ),
        SEP,
        item("file.save", "Save", "save", keys="Control + S", kw="write export"),
        item("file.save-as", "Save As…", "save", keys="Control + Shift + S", kw="export download format"),
        SEP,
        item("file.close", "Close", "x", keys="Control + W", kw="close document tab"),
        item("file.exit", "Exit", "x", keys="Control + Q", kw="quit", enabled="!App.is-web"),
    ),
    menu(
        "Edit",
        item("edit.undo", "Undo", "undo", keys="Control + Z", kw="back revert"),
        item("edit.redo", "Redo", "redo", keys="Control + Y", kw="forward"),
        SEP,
        item("edit.cut", "Cut", "scissors", keys="Control + X", kw="clipboard"),
        item("edit.copy", "Copy", "copy", keys="Control + C", kw="clipboard"),
        item("edit.copy-merged", "Copy Merged", "copy", keys="Control + Shift + C", kw="clipboard all layers"),
        item("edit.paste", "Paste", "paste", keys="Control + V", kw="clipboard"),
        item("edit.paste-new-layer", "Paste into New Layer", "paste", keys="Control + Shift + V", kw="clipboard"),
        item("edit.paste-new-image", "Paste into New Image", "paste", keys="Control + Alt + V", kw="clipboard"),
        SEP,
        item("edit.select-all", "Select All", "select", keys="Control + A", kw="selection everything"),
        item("edit.deselect", "Deselect", "x", keys="Control + D", kw="selection none clear"),
        item("edit.invert-selection", "Invert Selection", "select", keys="Control + I", kw="selection reverse"),
        SEP,
        item("edit.erase", "Erase Selection", "eraser", key="Delete", kw="clear delete transparent"),
        item("edit.fill", "Fill Selection", "bucket", key="Backspace", kw="primary color paint"),
    ),
    menu(
        "View",
        item("palette.open", "Command Palette…", "command", key="Space", kw="search find fuzzy commands"),
        item("view.menubar", "Menu Bar", "menu", chord="v m", kw="show hide main menu", checked="App.show-menubar"),
        SEP,
        item("view.zoom-in", "Zoom In", "zoom-in", keys="Control + Plus", kw="magnify bigger"),
        item("view.zoom-out", "Zoom Out", "zoom-out", keys="Control + HyphenMinus", kw="smaller"),
        item("view.fit", "Zoom to Window", "maximize", keys="Control + B", kw="fit best"),
        item("view.actual", "Actual Size", "zoom-in", keys='Control + "0"', kw="100% one to one"),
        SEP,
        menu(
            "Theme",
            item("view.theme-system", "Follow System", "sun", chord="v t s", kw="theme auto os appearance", checked="App.theme-mode == 0"),
            item("view.theme-light", "Light", "sun", chord="v t l", kw="theme appearance bright", checked="App.theme-mode == 1"),
            item("view.theme-dark", "Dark", "sun", chord="v t d", kw="theme appearance night", checked="App.theme-mode == 2"),
        ),
        SEP,
        item("view.grid", "Pixel Grid", "grid", chord="v g", kw="show hide", checked="App.pixel-grid"),
        item("view.rulers", "Rulers", "ruler", keys="Control + Alt + R", chord="v r", kw="show hide", checked="App.show-rulers"),
        SEP,
        item("view.panel-tools", "Tools Panel", "frame", chord="v 1", kw="show hide window", checked="App.show-tools"),
        item("view.panel-colors", "Colors Panel", "palette", chord="v 2", kw="show hide window", checked="App.show-colors"),
        item("view.panel-layers", "Layers Panel", "layers", chord="v 3", kw="show hide window", checked="App.show-layers"),
        item("view.panel-history", "History Panel", "history", chord="v 4", kw="show hide window", checked="App.show-history"),
    ),
    menu(
        "Image",
        item("image.crop", "Crop to Selection", "crop", keys="Control + Shift + X", chord="i c", kw="trim"),
        item("image.autocrop", "Auto Crop", "crop", keys="Control + Alt + X", chord="i a", kw="trim borders"),
        item("image.resize", "Resize…", "scaling", keys="Control + R", chord="i r", kw="scale size dimensions"),
        item("image.canvas", "Canvas Size…", "frame", keys="Control + Shift + R", chord="i s", kw="expand border"),
        SEP,
        item("image.flip-h", "Flip Horizontal", "flip-h", chord="i h", kw="mirror"),
        item("image.flip-v", "Flip Vertical", "flip-v", chord="i v", kw="mirror upside down"),
        SEP,
        item("image.rot-cw", "Rotate 90° Clockwise", "rotate-cw", keys="Control + H", chord="i ]", kw="turn right"),
        item("image.rot-ccw", "Rotate 90° Counter-clockwise", "rotate-ccw", keys="Control + G", chord="i [", kw="turn left"),
        item("image.rot-180", "Rotate 180°", "rotate-cw", keys="Control + J", chord="i u", kw="turn upside down"),
        SEP,
        item("image.flatten", "Flatten", "layers-2", keys="Control + Shift + F", chord="i f", kw="merge all layers"),
    ),
    menu(
        "Layers",
        item("layer.add", "Add New Layer", "plus", keys="Control + Shift + N", chord="l n", kw="create"),
        item("layer.delete", "Delete Layer", "trash", chord="l x", kw="remove"),
        item("layer.duplicate", "Duplicate Layer", "copy-plus", keys="Control + Shift + D", chord="l d", kw="copy clone"),
        item("layer.merge-down", "Merge Layer Down", "merge", keys="Control + M", chord="l m", kw="combine"),
        item("layer.import", "Import from File…", "image-plus", chord="l i", kw="open add picture"),
        SEP,
        item("layer.flip-h", "Flip Layer Horizontal", "flip-h", chord="l h", kw="mirror"),
        item("layer.flip-v", "Flip Layer Vertical", "flip-v", chord="l v", kw="mirror"),
        item("fx:rotate-zoom", "Rotate / Zoom…", "rotate-cw", keys="Control + Shift + Z", chord="l r", kw="layer angle scale pan"),
        SEP,
        item("layer.up", "Move Layer Up", "chevron-up", keys="Control + PageUp", chord="l k", kw="raise order"),
        item("layer.down", "Move Layer Down", "chevron-down", keys="Control + PageDown", chord="l j", kw="lower order"),
        SEP,
        item("layer.properties", "Layer Properties…", "sliders", keys="F4", chord="l p", kw="name opacity blend mode rename"),
    ),
    menu(
        "Adjustments",
        adj("auto-level", "Auto-Level", "Control + Shift + L", "a a", "levels automatic"),
        adj("black-and-white", "Black and White", "Control + Shift + G", "a w", "grayscale greyscale desaturate monochrome"),
        adj("brightness-contrast", "Brightness / Contrast…", None, "a b", "lighter darker"),
        adj("curves", "Curves…", "Control + Shift + M", "a c", "tone"),
        adj("hue-saturation", "Hue / Saturation…", "Control + Shift + U", "a h", "color lightness"),
        adj("invert-colors", "Invert Colors", "Control + Shift + I", "a i", "negative"),
        adj("levels", "Levels…", "Control + L", "a l", "gamma input output"),
        adj("posterize", "Posterize…", "Control + Shift + P", "a p", "reduce colors"),
        adj("sepia", "Sepia", "Control + Shift + E", "a s", "vintage old photo brown"),
        SEP,
        adj("vibrance", "Vibrance…", None, "a v", "saturation"),
        adj("temperature", "Temperature / Tint…", None, "a t", "warm cool white balance"),
    ),
    menu(
        "Effects",
        item("fx.repeat", "Repeat Last Effect", "sparkles", keys="Control + F", chord="f f", kw="again"),
        SEP,
        menu("Artistic", fx("ink-sketch", "Ink Sketch…", "f a i"), fx("oil-painting", "Oil Painting…", "f a o"), fx("pencil-sketch", "Pencil Sketch…", "f a p", "drawing")),
        menu(
            "Blurs",
            fx("fragment", "Fragment…", "f b f"),
            fx("gaussian-blur", "Gaussian Blur…", "f b g", "soft smooth"),
            fx("motion-blur", "Motion Blur…", "f b m", "speed"),
            fx("radial-blur", "Radial Blur…", "f b r", "spin"),
            fx("surface-blur", "Surface Blur…", "f b s", "bilateral smooth skin"),
            fx("unfocus", "Unfocus…", "f b u", "bokeh lens"),
            fx("zoom-blur", "Zoom Blur…", "f b z"),
        ),
        menu(
            "Distort",
            fx("bulge", "Bulge…", "f d b", "pinch"),
            fx("dents", "Dents…", "f d d", "ripple warp"),
            fx("frosted-glass", "Frosted Glass…", "f d f", "scatter"),
            fx("pixelate", "Pixelate…", "f d p", "mosaic censor"),
            fx("polar-inversion", "Polar Inversion…", "f d i"),
            fx("tile-reflection", "Tile Reflection…", "f d t", "glass tiles"),
            fx("twist", "Twist…", "f d w", "swirl"),
        ),
        menu("Noise", fx("add-noise", "Add Noise…", "f n a", "grain film"), fx("median", "Median…", "f n m", "despeckle"), fx("reduce-noise", "Reduce Noise…", "f n r", "denoise")),
        menu(
            "Photo",
            fx("glow", "Glow…", "f p g", "bloom"),
            fx("red-eye", "Red Eye Removal…", "f p r"),
            fx("sharpen", "Sharpen…", "f p s", "unsharp detail"),
            fx("soft-portrait", "Soft Portrait…", "f p p", "skin"),
            fx("vignette", "Vignette…", "f p v", "dark corners"),
        ),
        menu("Render", fx("clouds", "Clouds…", "f r c", "perlin noise"), fx("julia", "Julia Fractal…", "f r j"), fx("mandelbrot", "Mandelbrot Fractal…", "f r m")),
        menu("Stylize", fx("edge-detect", "Edge Detect…", "f s e"), fx("emboss", "Emboss…", "f s m"), fx("outline", "Outline…", "f s o"), fx("relief", "Relief…", "f s r")),
        menu("Object", fx("drop-shadow", "Drop Shadow…", "f o d")),
    ),
    menu(
        "Tools",
        tool("rect-select", "Rectangle Select", "select", "S", "selection marquee"),
        tool("ellipse-select", "Ellipse Select", "ellipse-select", "S", "selection circle oval"),
        tool("lasso-select", "Lasso Select", "lasso", "S", "selection freehand"),
        tool("magic-wand", "Magic Wand", "wand", "S", "selection similar color"),
        tool("move-pixels", "Move Selected Pixels", "move", "M", "transform scale rotate"),
        tool("move-selection", "Move Selection", "pointer", "M", "transform outline"),
        SEP,
        tool("brush", "Paintbrush", "brush", "B", "paint draw"),
        tool("pencil", "Pencil", "pencil", "P", "pixel draw"),
        tool("eraser", "Eraser", "eraser", "E", "erase rubber"),
        tool("gradient", "Gradient", "gradient", "G", "blend fade"),
        tool("bucket", "Paint Bucket", "bucket", "G", "fill flood"),
        tool("picker", "Color Picker", "pipette", "K", "eyedropper sample"),
        tool("clone", "Clone Stamp", "stamp", "C", "copy heal"),
        tool("recolor", "Recolor", "replace", "R", "replace color"),
        SEP,
        tool("text", "Text", "type", "T", "type font write"),
        tool("line", "Line / Curve", "spline", "O", "bezier arrow"),
        tool("shape", "Shapes", "shapes", "O", "rectangle ellipse star"),
        SEP,
        tool("pan", "Pan", "hand", "H", "hand scroll move view"),
        tool("zoom", "Zoom", "zoom-in", "Z", "magnify"),
        SEP,
        item("color.swap", "Swap Colors", "swap", key="X", kw="primary secondary exchange"),
        item("color.reset", "Reset Colors", "reset", key="D", kw="black white default"),
        item("brush.smaller", "Smaller Brush", "minus", key="[", kw="size decrease"),
        item("brush.bigger", "Bigger Brush", "plus", key="]", kw="size increase"),
    ),
    menu(
        "Help",
        item("help.shortcuts", "Keyboard Shortcuts", "keyboard", keys="F1", kw="keys hotkeys help"),
        item("help.about", "About Neobrush", "info", keys="Shift + F1", kw="version credits"),
        SEP,
        item("help.github", "Source Code on GitHub", "github", kw="repository source issues bug report"),
        item("help.donate", "Donate to Voices of Children", "heart", kw="support ukraine charity donate"),
    ),
]

KEY_NAMES = {"Control": "Ctrl", "HyphenMinus": "-", "Plus": "+", "PageUp": "PgUp", "PageDown": "PgDn"}


def display_keys(keys):
    parts = [p.strip() for p in keys.split("+")]
    out = []
    for p in parts:
        p = p.strip('"')
        out.append(KEY_NAMES.get(p, p))
    return out


def chord_title(it):
    t = it["title"]
    if it.get("chord") and not it.get("keys"):
        return t + "      " + " ".join(c.upper() for c in it["chord"].split())
    if it.get("key") and not it.get("keys"):
        return t + "      " + it["key"]
    return t


def shortcut_text(it):
    if it.get("keys"):
        return "+".join(display_keys(it["keys"]))
    if it.get("chord"):
        return " ".join(c.upper() for c in it["chord"].split())
    return it.get("key") or ""


def gen_context(entries, indent):
    """Menus for the toolbar dropdowns (ContextMenuArea can't bind shortcuts)."""
    pad = " " * indent
    out = []
    for e in entries:
        if e["kind"] == "sep":
            out.append(f"{pad}MenuSeparator {{ }}")
        elif e["kind"] == "raw":
            text = e["text"]
            first, rest = text.split("\n", 1)
            out.append(pad + first + "\n" + textwrap.indent(textwrap.dedent(rest), pad))
        elif e["kind"] == "menu":
            out.append(f'{pad}Menu {{\n{pad}    title: "{e["title"]}";')
            out.append(gen_context(e["children"], indent + 4))
            out.append(f"{pad}}}")
        else:
            sc = shortcut_text(e)
            title = e["title"] + ("      " + sc if sc else "")
            props = [f'title: "{title}";']
            if e.get("checked"):
                props.append("checkable: true;")
                props.append(f"checked: {e['checked']};")
            if e.get("enabled"):
                props.append(f"enabled: {e['enabled']};")
            props.append(f'activated => {{ App.action("{e["id"]}"); }}')
            out.append(f"{pad}MenuItem {{ {' '.join(props)} }}")
    return "\n".join(out)


MENU_ICONS = {"File": "folder-open", "Edit": "scissors", "View": "eye", "Image": "image", "Layers": "layers", "Adjustments": "contrast", "Effects": "sparkles", "Tools": "brush", "Help": "info"}


def gen_toolbar_menus():
    chips = []
    for m in MENUS:
        chips.append(f"""    MenuChip {{
        title: "{m['title']}";
        clicked(x, y) => {{ cm-{m['title'].lower()}.show({{ x: x, y: y }}); }}
        cm-{m['title'].lower()} := ContextMenuArea {{
            Menu {{
{gen_context(m['children'], 16)}
            }}
        }}
    }}""")
    return f"""// @generated by tools/gen_commands.py - do not edit by hand.
import {{ App }} from "state.slint";
import {{ MenuChip }} from "widgets.slint";

/// The main menu as dropdown buttons for the toolbar.
export component ToolbarMenus inherits HorizontalLayout {{
    spacing: 2px;
{chr(10).join(chips)}
}}
"""


def gen_slint(entries, indent):
    pad = " " * indent
    out = []
    for e in entries:
        if e["kind"] == "sep":
            out.append(f"{pad}MenuSeparator {{ }}")
        elif e["kind"] == "raw":
            text = e["text"]
            first, rest = text.split("\n", 1)
            out.append(pad + first + "\n" + textwrap.indent(textwrap.dedent(rest), pad))
        elif e["kind"] == "menu":
            out.append(f'{pad}Menu {{\n{pad}    title: "{e["title"]}";')
            out.append(gen_slint(e["children"], indent + 4))
            out.append(f"{pad}}}")
        else:
            props = [f'title: "{chord_title(e)}";']
            if e.get("keys"):
                props.append(f"shortcut: @keys({e['keys']});")
            if e.get("checked"):
                props.append("checkable: true;")
                props.append(f"checked: {e['checked']};")
            if e.get("enabled"):
                props.append(f"enabled: {e['enabled']};")
            props.append(f'activated => {{ App.action("{e["id"]}"); }}')
            out.append(f"{pad}MenuItem {{ {' '.join(props)} }}")
    return "\n".join(out)


def walk(entries, path):
    for e in entries:
        if e["kind"] == "menu":
            yield from walk(e["children"], path + [e["title"]])
        elif e["kind"] == "item":
            yield e, path


def rs_str(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def gen_rust():
    lines = [
        "// @generated by tools/gen_commands.py - do not edit by hand.",
        "",
        "/// A command shown in menus and the command palette.",
        "pub struct Cmd {",
        "    pub id: &'static str,",
        "    pub title: &'static str,",
        "    /// Menu path, e.g. \"Effects › Blurs\".",
        "    pub path: &'static str,",
        "    pub icon: &'static str,",
        "    /// Key combination, e.g. [\"Ctrl\", \"Shift\", \"L\"] (empty if none).",
        "    pub keys: &'static [&'static str],",
        "    /// Chord key sequence, e.g. \"f b g\" (empty if none).",
        "    pub chord: &'static str,",
        "    pub keywords: &'static str,",
        "}",
        "",
        "pub const COMMANDS: &[Cmd] = &[",
    ]
    seen = set()
    chords = {}
    for e, path in walk(MENUS, []):
        if e["id"] in seen:
            raise SystemExit(f"duplicate id {e['id']}")
        seen.add(e["id"])
        if e.get("chord"):
            if e["chord"] in chords:
                raise SystemExit(f"duplicate chord {e['chord']}: {e['id']} / {chords[e['chord']]}")
            chords[e["chord"]] = e["id"]
        keys = display_keys(e["keys"]) if e.get("keys") else ([e["key"]] if e.get("key") else [])
        keys_rs = "&[" + ", ".join(rs_str(k) for k in keys) + "]"
        lines.append(
            f"    Cmd {{ id: {rs_str(e['id'])}, title: {rs_str(e['title'])}, path: {rs_str(' › '.join(path))}, icon: {rs_str(e['icon'])}, "
            f"keys: {keys_rs}, chord: {rs_str(e.get('chord') or '')}, keywords: {rs_str(e.get('kw') or '')} }},"
        )
    lines.append("];")
    # Prefix conflicts: a full chord must not be the prefix of another.
    for a in chords:
        for b in chords:
            if a != b and b.startswith(a + " "):
                raise SystemExit(f"chord {a} is a prefix of {b}")
    return "\n".join(lines) + "\n"


def main():
    (ROOT / "src/app/commands_gen.rs").write_text(gen_rust())
    app = (ROOT / "ui/app.slint").read_text()
    menubar = "    MenuBar {\n        visible: App.show-menubar;\n" + gen_slint(MENUS, 8) + "\n    }"
    (ROOT / "ui/menus.slint").write_text(gen_toolbar_menus())
    new, n = re.subn(
        r"    // BEGIN GENERATED MENU.*?    // END GENERATED MENU",
        "    // BEGIN GENERATED MENU (tools/gen_commands.py)\n" + menubar + "\n    // END GENERATED MENU",
        app,
        flags=re.S,
    )
    if n != 1:
        raise SystemExit("markers not found in ui/app.slint")
    (ROOT / "ui/app.slint").write_text(new)
    print("generated", sum(1 for _ in walk(MENUS, [])), "commands")


if __name__ == "__main__":
    main()
