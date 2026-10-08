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


def item(id, title, icon="sparkles", keys=None, key=None, chord=None, kw="", checked=None, enabled=None, visible=None):
    return dict(kind="item", id=id, title=title, icon=icon, keys=keys, key=key, chord=chord, kw=kw, checked=checked, enabled=enabled, visible=visible)


def menu(title, *children, chord_prefix=None):
    return dict(kind="menu", title=title, children=list(children), chord_prefix=chord_prefix)


SEP = dict(kind="sep")


def raw(text):
    return dict(kind="raw", text=text)


def fx(id, title, chord, kw="", icon="sparkles"):
    return item(f"fx:{id}", title, icon, chord=chord, kw=kw)


def adj(id, title, keys, chord, kw="", icon="contrast"):
    return item(f"fx:{id}", title, icon, keys=keys, chord=chord, kw=kw)


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
        item("app.settings", "Settings…", "settings", keys='Control + ","', chord="v s", kw="preferences options language accent color theme"),
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
        item("image.crop", "Crop to Selection", "crop", keys="Control + Shift + X", chord="i c", kw="trim", enabled="App.has-selection"),
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
        item("layer.delete", "Delete Layer", "trash", chord="l x", kw="remove", enabled="App.layer-count > 1"),
        item("layer.duplicate", "Duplicate Layer", "copy-plus", keys="Control + Shift + D", chord="l d", kw="copy clone"),
        item("layer.merge-down", "Merge Layer Down", "merge", keys="Control + M", chord="l m", kw="combine", enabled="App.layer-count > 1"),
        item("layer.import", "Import from File…", "image-plus", chord="l i", kw="open add picture"),
        SEP,
        item("layer.flip-h", "Flip Layer Horizontal", "flip-h", chord="l h", kw="mirror"),
        item("layer.flip-v", "Flip Layer Vertical", "flip-v", chord="l v", kw="mirror"),
        item("fx:rotate-zoom", "Rotate / Zoom…", "rotate-cw", keys="Control + Shift + Z", chord="l r", kw="layer angle scale pan"),
        SEP,
        item("layer.select-up", "Select Layer Above", "chevron-up", chord="l k", kw="switch next go up"),
        item("layer.select-down", "Select Layer Below", "chevron-down", chord="l j", kw="switch previous go down"),
        item("layer.up", "Move Layer Up", "chevron-up", keys="Control + PageUp", chord="l K", kw="raise order"),
        item("layer.down", "Move Layer Down", "chevron-down", keys="Control + PageDown", chord="l J", kw="lower order"),
        SEP,
        item("layer.properties", "Layer Properties…", "sliders", keys="F4", chord="l p", kw="name opacity blend mode rename"),
    ),
    menu(
        "Adjustments",
        adj("auto-level", "Auto-Level", "Control + Shift + L", "a a", "levels automatic", icon="wand"),
        adj("black-and-white", "Black and White", "Control + Shift + G", "a w", "grayscale greyscale desaturate monochrome"),
        adj("brightness-contrast", "Brightness / Contrast…", None, "a b", "lighter darker", icon="sun"),
        adj("curves", "Curves…", "Control + Shift + M", "a c", "tone", icon="chart-spline"),
        adj("hue-saturation", "Hue / Saturation…", "Control + Shift + U", "a h", "color lightness", icon="palette"),
        adj("invert-colors", "Invert Colors", "Control + Shift + I", "a i", "negative"),
        adj("levels", "Levels…", "Control + L", "a l", "gamma input output", icon="chart-column"),
        adj("posterize", "Posterize…", "Control + Shift + P", "a p", "reduce colors"),
        adj("sepia", "Sepia", "Control + Shift + E", "a s", "vintage old photo brown"),
        SEP,
        adj("vibrance", "Vibrance…", None, "a v", "saturation"),
        adj("temperature", "Temperature / Tint…", None, "a t", "warm cool white balance"),
    ),
    menu(
        "Effects",
        item("fx.repeat", "Repeat Last Effect", "repeat", keys="Control + F", chord="f f", kw="again"),
        SEP,
        menu("Artistic", fx("ink-sketch", "Ink Sketch…", "f a i"), fx("oil-painting", "Oil Painting…", "f a o"), fx("pencil-sketch", "Pencil Sketch…", "f a p", "drawing")),
        menu(
            "Blurs",
            fx("fragment", "Fragment…", "f b f"),
            fx("gaussian-blur", "Gaussian Blur…", "f b g", "soft smooth", icon="droplets"),
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
            fx("sharpen", "Sharpen…", "f p s", "unsharp detail", icon="focus"),
            fx("soft-portrait", "Soft Portrait…", "f p p", "skin"),
            fx("vignette", "Vignette…", "f p v", "dark corners", icon="aperture"),
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
        item("help.donate", "Donate to Voices of Children", "heart", kw="support ukraine charity donate", visible="App.show-donate"),
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
    t = f'@tr("{it["title"]}")'
    if it.get("chord") and not it.get("keys"):
        return t + f' + "      {chord_display(it["chord"])}"'
    if it.get("key") and not it.get("keys"):
        return t + f' + "      {it["key"]}"'
    return t


def chord_display(chord):
    """Chord keys for display; uppercase letters in the table mean Shift."""
    return " ".join(("⇧" + c) if c.isalpha() and c.isupper() else c.upper() for c in chord.split())


def shortcut_text(it):
    if it.get("keys"):
        return "+".join(display_keys(it["keys"]))
    if it.get("chord"):
        return chord_display(it["chord"])
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
            out.append(f'{pad}Menu {{\n{pad}    title: @tr("{e["title"]}");')
            out.append(gen_context(e["children"], indent + 4))
            out.append(f"{pad}}}")
        else:
            sc = shortcut_text(e)
            title = f'@tr("{e["title"]}")' + (f' + "      {sc}"' if sc else "")
            props = [f"title: {title};"]
            if e.get("checked"):
                props.append("checkable: true;")
                props.append(f"checked: {e['checked']};")
            if e.get("enabled"):
                props.append(f"enabled: {e['enabled']};")
            props.append(f'activated => {{ App.action("{e["id"]}"); }}')
            cond = f"if {e['visible']}: " if e.get("visible") else ""
            out.append(f"{pad}{cond}MenuItem {{ {' '.join(props)} }}")
    return "\n".join(out)


# Toolbar sections: menu title -> commands shown as buttons ("@zoom" is the zoom percentage).
# Everything else of that menu goes into the section's "more" popup.
BAR = [
    ("View", ["view.zoom-out", "@zoom", "view.zoom-in", "view.fit", "view.grid", "view.rulers"]),
    ("Image", ["image.crop", "image.resize", "image.canvas", "image.rot-cw", "image.flip-h"]),
    ("Layers", ["layer.add", "layer.duplicate", "layer.merge-down", "layer.properties"]),
    ("Adjustments", ["fx:auto-level", "fx:curves", "fx:hue-saturation", "fx:brightness-contrast"]),
    ("Effects", ["fx.repeat", "fx:gaussian-blur", "fx:sharpen", "fx:vignette"]),
    ("Tools", ["color.swap", "color.reset", "brush.smaller", "brush.bigger"]),
]


def strip_bar(entries, ids):
    """Menu entries without the ones shown as buttons (and without dangling separators)."""
    out = []
    for e in entries:
        if e["kind"] == "item" and e["id"] in ids:
            continue
        if e["kind"] == "menu":
            e = dict(e, children=strip_bar(e["children"], ids))
        if e["kind"] == "sep" and (not out or out[-1]["kind"] == "sep"):
            continue
        out.append(e)
    while out and out[-1]["kind"] == "sep":
        out.pop()
    return out


def gen_toolbar_sections():
    items = {e["id"]: e for e, _ in walk(MENUS, [])}
    groups = []
    for title, ids in BAR:
        m = next(m for m in MENUS if m["title"] == title)
        buttons = []
        for n, i in enumerate(ids):
            # The first two buttons of each section always show; the rest only on wide windows.
            cond = "" if n < 2 else "if root.wide: "
            if i == "@zoom":
                buttons.append(f"""            {cond}ZoomLabel {{ }}""")
                continue
            e = items[i]
            sc = shortcut_text(e)
            tip = f'@tr("{e["title"].rstrip("…")}")' + (f' + "  ·  {sc}"' if sc else "")
            enabled = e.get("enabled") or "App.has-doc"
            if e.get("enabled"):
                enabled = f"App.has-doc && ({e['enabled']})"
            active = f" active: {e['checked']};" if e.get("checked") else ""
            buttons.append(f'            {cond}IconButton {{ icon: Icons.named("{e["icon"]}"); tip: {tip}; enabled: {enabled};{active} clicked => {{ App.action("{e["id"]}"); }} }}')
        rest = strip_bar(m["children"], set(ids))
        more = ""
        if rest:
            more = f"""            MoreButton {{
                tip: @tr("More {title.lower()} commands");
                clicked(x, y) => {{ cm-{title.lower()}.show({{ x: x, y: y }}); }}
                cm-{title.lower()} := ContextMenuArea {{
                    Menu {{
{gen_context(rest, 24)}
                    }}
                }}
            }}"""
        if groups:
            groups.append("    VDivider { height: 30px; y: 4px; }")
        groups.append(f"""    ToolbarSection {{
        title: @tr("{title}");
        HorizontalLayout {{
            spacing: 1px;
{chr(10).join(buttons)}
{more}
        }}
    }}""")
    return f"""// @generated by tools/gen_commands.py - do not edit by hand.
import {{ App }} from "state.slint";
import {{ Icons }} from "icons.slint";
import {{ IconButton, MoreButton, ToolbarSection, ZoomLabel, VDivider }} from "widgets.slint";

/// Toolbar sections (View, Image, Layers, ...) with their most used commands as buttons.
export component ToolbarSections inherits HorizontalLayout {{
    in property <bool> wide: true;
    spacing: 6px;
{chr(10).join(groups)}
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
            out.append(f'{pad}Menu {{\n{pad}    title: @tr("{e["title"]}");')
            out.append(gen_slint(e["children"], indent + 4))
            out.append(f"{pad}}}")
        else:
            props = [f"title: {chord_title(e)};"]
            if e.get("keys"):
                props.append(f"shortcut: @keys({e['keys']});")
            if e.get("checked"):
                props.append("checkable: true;")
                props.append(f"checked: {e['checked']};")
            if e.get("enabled"):
                props.append(f"enabled: {e['enabled']};")
            props.append(f'activated => {{ App.action("{e["id"]}"); }}')
            cond = f"if {e['visible']}: " if e.get("visible") else ""
            out.append(f"{pad}{cond}MenuItem {{ {' '.join(props)} }}")
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
    (ROOT / "ui/menus.slint").write_text(gen_toolbar_sections())
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
