#!/usr/bin/env python3
"""Builds the UI fonts for non-Latin languages: Noto subsets reduced to the
characters used in i18n/<lang>.txt, with Latin coverage so they can serve as
the whole UI font. Also rebuilds Inter-400/500/600 (Latin, Cyrillic, Greek) from InterVariable.
Usage: make_fonts.py <noto-fonts dir> <NotoSansCJK-VF.otf.ttc> <InterVariable.ttf>"""
import sys
from fontTools.ttLib import TTFont, TTCollection
from fontTools.ttLib.scaleUpem import scale_upem
from fontTools.varLib import instancer
from fontTools import subset, merge

LATIN = "".join(chr(c) for c in range(0x20, 0x17F)) + "…·×→←“”„«»‘’–—•"

def native_names():
    """Language names as listed in src/app/i18n.rs, so each font can draw its own."""
    import re
    src = open("src/app/i18n.rs", encoding="utf-8").read()
    return "".join(re.findall(r'\("[a-z]+", "([^"]+)"\)', src))

def chars(langs):
    cs = set(native_names())
    for l in langs:
        for line in open(f"i18n/{l}.txt", encoding="utf-8"):
            if " ⇒ " in line:
                cs |= set(line.split(" ⇒ ", 1)[1].strip())
    return "".join(sorted(cs))

def static(f):
    if "fvar" in f:
        axes = {a.axisTag: (500 if a.axisTag == "wght" else a.defaultValue) for a in f["fvar"].axes}
        f = instancer.instantiateVariableFont(f, axes, downgradeCFF2="CFF2" in f, updateFontNames=False)
    return f

def sub(f, text):
    o = subset.Options(); o.layout_features = ["*"]; o.name_IDs = ["*"]; o.notdef_outline = True
    s = subset.Subsetter(o); s.populate(text=text); s.subset(f)
    return f

def rename(f, family):
    n = f["name"]
    for rec in list(n.names):
        if rec.nameID in (1, 4, 16, 3, 6):
            n.removeNames(nameID=rec.nameID)
    n.setName(family, 1, 3, 1, 0x409); n.setName("Regular", 2, 3, 1, 0x409)
    n.setName(family, 4, 3, 1, 0x409); n.setName(family.replace(" ", ""), 6, 3, 1, 0x409)
    n.setName(family, 3, 3, 1, 0x409)
    f["OS/2"].usWeightClass = 500

noto, cjk, inter_var = sys.argv[1], sys.argv[2], sys.argv[3]

UI_LATIN = (LATIN + "".join(chr(c) for c in range(0x370, 0x3FF)) + "".join(chr(c) for c in range(0x400, 0x530))
            + "".join(chr(c) for c in range(0x2000, 0x2070)) + "₴€£¥₽™©®°±÷≈≠≤≥∞√∑−⇧⌘⌥⏎⌫←↑→↓↔⋯✓✕")
for w, style in [(400, "Regular"), (500, "Medium"), (600, "SemiBold")]:
    f = instancer.instantiateVariableFont(TTFont(inter_var), {"wght": w, "opsz": 14}, updateFontNames=False)
    f = sub(f, UI_LATIN)
    n = f["name"]
    for rec in list(n.names):
        if rec.nameID in (1, 2, 3, 4, 6, 16, 17, 25):
            n.removeNames(nameID=rec.nameID)
    n.setName("Inter", 1, 3, 1, 0x409); n.setName(style, 2, 3, 1, 0x409)
    n.setName(f"Inter {style}", 4, 3, 1, 0x409); n.setName(f"Inter-{style}", 6, 3, 1, 0x409)
    n.setName(f"Inter-{style}", 3, 3, 1, 0x409)
    f["OS/2"].usWeightClass = w
    f.save(f"ui/fonts/Inter-{w}.ttf")

coll = TTCollection(cjk)
f = [x for x in coll.fonts if x["name"].getDebugName(1).endswith(" SC")][0]
f = sub(static(f), chars(["zh", "ja", "ko"]) + LATIN)
rename(f, "Neobrush CJK")
f.save("ui/fonts/NeobrushCJK.otf")

for lang, src, family, out in [("he", "NotoSansHebrew.ttf", "Neobrush Hebrew", "NeobrushHebrew.ttf"),
                               ("ar", "NotoSansArabic.ttf", "Neobrush Arabic", "NeobrushArabic.ttf")]:
    scr = sub(static(TTFont(f"{noto}/{src}")), chars([lang]))
    scale_upem(scr, 2048)
    tmp = "/tmp/_nb_script.ttf"; scr.save(tmp)
    lat = sub(TTFont("ui/fonts/Inter-500.ttf"), LATIN)
    tmpl = "/tmp/_nb_latin.ttf"; lat.save(tmpl)
    m = merge.Merger().merge([tmpl, tmp])
    # Keep Inter's line metrics so layouts match the Latin UI exactly.
    inter = TTFont("ui/fonts/Inter-500.ttf")
    for a in ("ascent", "descent", "lineGap"):
        setattr(m["hhea"], a, getattr(inter["hhea"], a))
    for a in ("sTypoAscender", "sTypoDescender", "sTypoLineGap", "usWinAscent", "usWinDescent"):
        setattr(m["OS/2"], a, getattr(inter["OS/2"], a))
    # Merged locl lookups end up applying to plain Arabic (dotless final yeh).
    for fr in m["GSUB"].table.FeatureList.FeatureRecord:
        if fr.FeatureTag == "locl":
            fr.Feature.LookupListIndex = []
            fr.Feature.LookupCount = 0
    rename(m, family)
    m.save(f"ui/fonts/{out}")
    print(out)
