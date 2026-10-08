#!/usr/bin/env python3
"""Collects every translatable English string and reports what each language is missing.

Sources: @tr("...") in ui/*.slint, tr("...")/trf("...") in Rust, command titles and menu
paths, history step names, effect names/parameters, blend modes and shapes.

  python3 tools/i18n_extract.py            # writes i18n/strings.txt (reference list), prints coverage
  python3 tools/i18n_extract.py --missing de   # prints strings missing in de.tsv
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LIT = r'"((?:[^"\\]|\\.)*)"'


def unescape(s):
    return s.replace('\\"', '"').replace("\\\\", "\\")


def collect():
    out = set()
    for f in (ROOT / "ui").glob("*.slint"):
        for m in re.finditer(r"@tr\(" + LIT, f.read_text()):
            out.add(unescape(m.group(1)))
    for f in (ROOT / "src").rglob("*.rs"):
        if f.name == "perf.rs":
            continue
        t = f.read_text()
        # Leave out unit tests.
        t = t.split("#[cfg(test)]")[0]
        for line in t.splitlines():
            if "commit(" in line or "let name = if" in line or "erase_selection(" in line:
                for m in re.finditer(LIT, line):
                    out.add(unescape(m.group(1)))
        for m in re.finditer(r"\btrf?\(\s*" + LIT, t):
            out.add(unescape(m.group(1)))
        for m in re.finditer(r"\b(?:commit|structural)\(\s*" + LIT, t):
            out.add(unescape(m.group(1)))
        # tr(if cond { "a" } else { "b" }) and status hint match arms
        for m in re.finditer(r'trf?\(if [^{]*\{\s*' + LIT + r'\s*\}\s*else\s*\{\s*' + LIT, t):
            out.add(unescape(m.group(1)))
            out.add(unescape(m.group(2)))
    ed = (ROOT / "src/app/editor.rs").read_text()
    for m in re.finditer(r"Tool::\w+ => " + LIT, ed):
        out.add(unescape(m.group(1)))
    gen = (ROOT / "src/app/commands_gen.rs").read_text()
    for m in re.finditer(r"title: " + LIT + r", path: " + LIT, gen):
        out.add(unescape(m.group(1)).rstrip("…"))
        out.add(unescape(m.group(1)))
        for part in unescape(m.group(2)).split(" › "):
            if part:
                out.add(part)
    fl = (ROOT / "src/core/filters.rs").read_text()
    for m in re.finditer(r'f!\(' + LIT + r",\s*" + LIT, fl):
        out.add(unescape(m.group(2)))
    for m in re.finditer(r'\b(?:sl|slf|ck|ch)\(' + LIT, fl):
        out.add(unescape(m.group(1)))
    for m in re.finditer(r'ch\(' + LIT + r',\s*&\[([^\]]*)\]', fl):
        for c in re.findall(LIT, m.group(2)):
            out.add(unescape(c))
    for f in ["src/core/blend.rs", "src/core/paint.rs"]:
        for m in re.finditer(r"=> " + LIT + ",", (ROOT / f).read_text()):
            out.add(unescape(m.group(1)))
    pal = (ROOT / "src/app/palette.rs").read_text().split("#[cfg(test)]")[0]
    for m in re.finditer(r'\("[a-z ]+", ' + LIT + r"\)", pal):
        out.add(unescape(m.group(1)))
    for m in re.finditer(r'\("[a-z]", ' + LIT + r", \"[a-z]+\"\)", pal):
        out.add(unescape(m.group(1)))
    # The history's first entries.
    out |= {"New Image", "Open Image", "Paste into New Image"}
    junk = {"x", "eye", "sliders", "chevron-up", "chevron-down", "plus", "trash", "copy-plus", "merge", "crop", "scissors", "eraser", "bucket", "rotate-cw", "rotate-ccw", "flip-h", "flip-v", "layers-2", "image-plus", "scaling", "frame", "paste", "select", "wand", "lasso", "ellipse-select", "move", "pointer", "gradient", "type", "spline", "shapes", "contrast", "sparkles", "brush"}
    return sorted(s for s in out if re.search(r"[A-Za-z]", s) and s.strip() and ":" not in s.split(" ")[0][:-1] and s not in junk)


def load(lang):
    p = ROOT / "i18n" / f"{lang}.txt"
    if not p.exists():
        return {}
    d = {}
    for line in p.read_text().splitlines():
        if line and not line.startswith("#") and " ⇒ " in line:
            k, v = line.split(" ⇒ ", 1)
            d[k.replace("\\n", "\n")] = v
    return d


def main():
    strings = collect()
    (ROOT / "i18n/strings.txt").write_text("\n".join(s.replace("\n", "\\n") for s in strings) + "\n")
    if len(sys.argv) > 2 and sys.argv[1] == "--missing":
        have = load(sys.argv[2])
        for s in strings:
            if s not in have:
                print(s.replace("\n", "\\n"))
        return
    print(f"{len(strings)} strings")
    for lang in ["de", "es", "fr", "it", "pt", "uk", "ru", "la", "tok", "zh", "ja", "ko", "he", "ar"]:
        have = load(lang)
        missing = sum(1 for s in strings if s not in have)
        print(f"  {lang}: {len(strings) - missing}/{len(strings)}")


if __name__ == "__main__":
    main()
