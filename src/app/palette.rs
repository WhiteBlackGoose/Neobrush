//! Command palette (fuzzy search over commands, tools, layers, documents and smart
//! commands) and the chord engine (key sequences like `L 2 T` or `F B G`).

use super::commands_gen::{Cmd, COMMANDS};
use super::editor::Editor;
use super::i18n::{tr, trf};
use crate::{App, ChordOption, PaletteItem, Seg, ShortcutRow};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

/// What a palette row does when run.
#[derive(Clone)]
pub struct Entry {
    pub action: String,
    pub title: String,
    pub subtitle: String,
    pub icon: &'static str,
    pub keys: Vec<String>,
    pub chord: Vec<String>,
    pub keywords: String,
}

// ---------------------------------------------------------------------------------------------
// Fuzzy matching

fn is_boundary(prev: Option<char>, c: char) -> bool {
    match prev {
        None => true,
        Some(p) => !p.is_alphanumeric() || (p.is_lowercase() && c.is_uppercase()) || (p.is_alphabetic() && c.is_numeric()),
    }
}

/// Scores how well `query` matches `text` as a subsequence (case-insensitive).
/// Rewards consecutive runs, word starts and early matches. Returns the matched char indices.
pub fn fuzzy(query: &str, text: &str) -> Option<(i32, Vec<usize>)> {
    let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).flat_map(|c| c.to_lowercase()).collect();
    if q.is_empty() {
        return Some((0, vec![]));
    }
    let t: Vec<char> = text.chars().collect();
    let tl: Vec<char> = t.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
    let (n, m) = (t.len(), q.len());
    if m > n {
        return None;
    }
    const NEG: i32 = i32::MIN / 4;
    // best[i][j]: best score with q[i] matched at t[j]; from[i][j]: previous position.
    let mut best = vec![vec![NEG; n]; m];
    let mut from = vec![vec![usize::MAX; n]; m];
    for i in 0..m {
        for j in i..n {
            if tl[j] != q[i] {
                continue;
            }
            let mut bonus = 16;
            if is_boundary(if j == 0 { None } else { Some(t[j - 1]) }, t[j]) {
                bonus += 12;
            }
            if i == 0 {
                best[i][j] = bonus + if j == 0 { 10 } else { 0 } - (j as i32).min(10);
                continue;
            }
            for k in (i - 1)..j {
                if best[i - 1][k] == NEG {
                    continue;
                }
                let s = best[i - 1][k] + bonus + if k + 1 == j { 14 } else { -((j - k - 1) as i32).min(6) };
                if s > best[i][j] {
                    best[i][j] = s;
                    from[i][j] = k;
                }
            }
        }
    }
    let (mut j, score) = (0..n).map(|j| (j, best[m - 1][j])).max_by_key(|(_, s)| *s)?;
    if score == NEG {
        return None;
    }
    let mut idx = vec![0; m];
    for i in (0..m).rev() {
        idx[i] = j;
        if i > 0 {
            j = from[i][j];
        }
    }
    // Prefer shorter texts slightly.
    Some((score - (n as i32 / 8), idx))
}

fn segments(text: &str, hl: &[usize]) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::new();
    for (i, c) in text.chars().enumerate() {
        let h = hl.contains(&i);
        match out.last_mut() {
            Some(s) if s.hl == h => s.text = format!("{}{}", s.text, c).into(),
            _ => out.push(Seg { text: c.to_string().into(), hl: h }),
        }
    }
    out
}

fn platform_keys(keys: &[&str]) -> Vec<String> {
    keys.iter()
        .map(|k| if cfg!(target_os = "macos") && *k == "Ctrl" { "⌘".to_string() } else { k.to_string() })
        .collect()
}

/// Display form of a chord key: uppercase letters mean Shift.
fn key_label(k: &str) -> String {
    if k.chars().any(|c| c.is_uppercase()) {
        format!("⇧{k}")
    } else {
        k.to_uppercase()
    }
}

fn chord_keys(chord: &str) -> Vec<String> {
    chord.split_whitespace().map(key_label).collect()
}

impl Entry {
    fn from_cmd(c: &Cmd) -> Entry {
        Entry {
            action: c.id.to_string(),
            title: tr(c.title).trim_end_matches('…').to_string(),
            subtitle: translate_path(c.path),
            icon: c.icon,
            keys: platform_keys(c.keys),
            chord: chord_keys(c.chord),
            // English title and path stay searchable in every language.
            keywords: format!("{} {} {}", c.keywords, c.title, c.path),
        }
    }
}

fn translate_path(p: &str) -> String {
    p.split(" › ").map(tr).collect::<Vec<_>>().join(" › ")
}

// ---------------------------------------------------------------------------------------------
// Chords

/// A chord in progress: the keys typed so far.
#[derive(Default, Clone)]
pub struct ChordState {
    pub keys: Vec<String>,
}

pub enum ChordResult {
    /// Not a chord key.
    None,
    /// Waiting for more keys.
    Pending,
    /// Complete: run this action.
    Run(String),
    /// Wrong key, chord cancelled.
    Cancelled,
}

const LEADERS: &[(&str, &str)] = &[("a", "Adjustments"), ("f", "Effects"), ("i", "Image"), ("l", "Layers"), ("v", "View")];

/// Layer actions available after `L <number>`.
const LAYER_ACTIONS: &[(&str, &str, &str)] = &[
    ("s", "Switch to layer", "select"),
    ("t", "Toggle visibility", "toggle"),
    ("o", "Show only this layer", "solo"),
    ("d", "Duplicate", "duplicate"),
    ("x", "Delete", "delete"),
    ("m", "Merge down", "merge"),
    ("p", "Properties", "properties"),
];

pub fn is_leader(k: &str) -> bool {
    LEADERS.iter().any(|(l, _)| *l == k)
}

/// Group names for chord prefixes, used as HUD headings.
fn group_name(prefix: &[String]) -> String {
    let p = prefix.join(" ");
    let named = [
        ("f a", "Artistic"),
        ("f b", "Blurs"),
        ("f d", "Distort"),
        ("f n", "Noise"),
        ("f p", "Photo"),
        ("f r", "Render"),
        ("f s", "Stylize"),
        ("f o", "Object"),
        ("v t", "Theme"),
    ];
    if let Some((_, n)) = named.iter().find(|(k, _)| *k == p) {
        return tr(n);
    }
    LEADERS.iter().find(|(l, _)| *l == p).map(|(_, n)| tr(n)).unwrap_or_default()
}

/// Options for the next key after `prefix`: (key, label).
pub fn chord_options(prefix: &[String], layer_names: &[String]) -> Vec<(String, String)> {
    // Layer number chords.
    if prefix.len() >= 2 && prefix[0] == "l" && prefix[1..].iter().all(|k| k.chars().all(|c| c.is_ascii_digit())) {
        let mut v: Vec<(String, String)> = LAYER_ACTIONS.iter().map(|(k, label, _)| (k.to_string(), tr(label))).collect();
        if prefix.len() == 2 {
            v.insert(0, (prefix[1].clone(), tr("Switch (repeat digit)")));
        }
        return v;
    }
    let p = prefix.join(" ");
    let mut out: Vec<(String, String)> = Vec::new();
    for c in COMMANDS.iter().filter(|c| !c.chord.is_empty()) {
        let Some(rest) = c.chord.strip_prefix(&(p.clone() + " ")) else { continue };
        let next = rest.split_whitespace().next().unwrap_or("").to_string();
        if out.iter().any(|(k, _)| *k == next) {
            continue;
        }
        let label = if rest.contains(' ') { group_name(&[prefix.to_vec(), vec![next.clone()]].concat()) + " …" } else { tr(c.title).trim_end_matches('…').to_string() };
        out.push((next, label));
    }
    if prefix == ["l"] && !layer_names.is_empty() {
        out.push(("1–9".into(), trf("Layer number ({} layers)", &[&layer_names.len()])));
    }
    out
}

pub fn chord_step(state: &mut ChordState, key: &str, layer_count: usize) -> ChordResult {
    // Letters keep their case after the leader: an uppercase letter means Shift was held.
    if state.keys.is_empty() {
        let key = key.to_lowercase();
        if is_leader(&key) {
            state.keys.push(key);
            return ChordResult::Pending;
        }
        return ChordResult::None;
    }
    let key = key.to_string();
    state.keys.push(key.clone());
    let keys = state.keys.clone();
    // Layer number chords: l <digits> <action>.
    if keys[0] == "l" && keys.len() >= 2 && keys[1].chars().all(|c| c.is_ascii_digit()) {
        let digits: Vec<&String> = keys[1..].iter().take_while(|k| k.chars().all(|c| c.is_ascii_digit())).collect();
        let n: usize = digits.iter().map(|s| s.as_str()).collect::<String>().parse().unwrap_or(0);
        if digits.len() == keys.len() - 1 {
            // The same digit twice switches to that layer: L 2 2.
            if digits.len() == 2 && digits[0] == digits[1] && n % 11 == 0 && n / 11 >= 1 && n / 11 <= layer_count {
                state.keys.clear();
                return ChordResult::Run(format!("layer.select:{}", n / 11));
            }
            // Still typing the number.
            return if n >= 1 && n <= layer_count { ChordResult::Pending } else { state.keys.clear(); ChordResult::Cancelled };
        }
        if let Some((_, _, act)) = LAYER_ACTIONS.iter().find(|(k, _, _)| **k == key.to_lowercase()) {
            if n >= 1 && n <= layer_count {
                state.keys.clear();
                return ChordResult::Run(format!("layer.{act}:{n}"));
            }
        }
        state.keys.clear();
        return ChordResult::Cancelled;
    }
    let seq = keys.join(" ");
    if let Some(c) = COMMANDS.iter().find(|c| c.chord == seq) {
        state.keys.clear();
        return ChordResult::Run(c.id.to_string());
    }
    if COMMANDS.iter().any(|c| c.chord.starts_with(&(seq.clone() + " "))) {
        return ChordResult::Pending;
    }
    state.keys.clear();
    ChordResult::Cancelled
}

// ---------------------------------------------------------------------------------------------
// Editor integration

impl Editor {
    fn layer_names(&self) -> Vec<String> {
        self.doc().map(|d| d.state.layers.iter().map(|l| l.name.clone()).collect()).unwrap_or_default()
    }

    /// Updates the chord HUD from the current chord state.
    pub fn sync_chord_hud(&self) {
        let ui = self.ui();
        let g = ui.global::<App>();
        if self.chord.keys.is_empty() {
            g.set_chord_active(false);
            return;
        }
        let names = self.layer_names();
        let keys = &self.chord.keys;
        let title = if keys[0] == "l" && keys.len() >= 2 && keys[1].chars().all(|c| c.is_ascii_digit()) {
            let n: usize = keys[1..].concat().parse().unwrap_or(0);
            match names.get(n.wrapping_sub(1)) {
                Some(name) => format!("{} · {name}", trf("Layer {}", &[&n])),
                None => trf("Layer {}", &[&n]),
            }
        } else {
            group_name(keys)
        };
        let opts: Vec<ChordOption> = chord_options(keys, &names).into_iter().map(|(k, l)| ChordOption { key: key_label(&k).into(), label: l.into() }).collect();
        g.set_chord_title(title.into());
        g.set_chord_keys(keys.iter().map(|k| key_label(k)).collect::<Vec<_>>().join("  ").into());
        g.set_chord_options(ModelRc::new(VecModel::from(opts)));
        g.set_chord_active(true);
    }

    /// Feeds a key to the chord engine. Returns true if the key was consumed.
    pub fn chord_key(&mut self, key: &str) -> bool {
        let layer_count = self.doc().map(|d| d.state.layers.len()).unwrap_or(0);
        let r = chord_step(&mut self.chord, key, layer_count);
        let consumed = !matches!(r, ChordResult::None);
        match r {
            ChordResult::Run(action) => {
                self.sync_chord_hud();
                self.remember_command(&action);
                self.action(&action);
            }
            ChordResult::Cancelled => {
                self.chord.keys.clear();
                self.sync_chord_hud();
            }
            _ => self.sync_chord_hud(),
        }
        consumed
    }

    pub fn cancel_chord(&mut self) -> bool {
        if self.chord.keys.is_empty() {
            return false;
        }
        self.chord.keys.clear();
        self.sync_chord_hud();
        true
    }

    pub fn remember_command(&mut self, action: &str) {
        self.recent_cmds.retain(|a| a != action);
        self.recent_cmds.insert(0, action.to_string());
        self.recent_cmds.truncate(30);
    }

    // -- Palette --------------------------------------------------------------------------------

    pub fn open_palette(&mut self) {
        self.cancel_chord();
        let ui = self.ui();
        let g = ui.global::<App>();
        g.set_palette_query("".into());
        g.set_dialog("palette".into());
        self.palette_query(String::new());
    }

    /// All static and dynamic entries the palette can show.
    fn palette_entries(&self) -> Vec<Entry> {
        let mut v: Vec<Entry> = COMMANDS.iter().map(Entry::from_cmd).collect();
        if let Some(d) = self.doc() {
            for (i, l) in d.state.layers.iter().enumerate().rev() {
                let n = i + 1;
                let current = if i == d.state.active { format!(" ({})", tr("current")) } else { String::new() };
                v.push(Entry {
                    action: format!("layer.select:{n}"),
                    title: format!("{}{current}", trf("Switch to layer {}: {}", &[&n, &l.name])),
                    subtitle: tr("Layers"),
                    icon: "layers",
                    keys: vec![],
                    chord: chord_keys(&format!("l {n} s")),
                    keywords: "go select active".into(),
                });
                v.push(Entry {
                    action: format!("layer.toggle:{n}"),
                    title: trf(if l.visible { "Hide layer {}: {}" } else { "Show layer {}: {}" }, &[&n, &l.name]),
                    subtitle: tr("Layers"),
                    icon: "eye",
                    keys: vec![],
                    chord: chord_keys(&format!("l {n} t")),
                    keywords: "toggle visibility visible".into(),
                });
                v.push(Entry {
                    action: format!("layer.solo:{n}"),
                    title: trf("Show only layer {}: {}", &[&n, &l.name]),
                    subtitle: tr("Layers"),
                    icon: "eye",
                    keys: vec![],
                    chord: chord_keys(&format!("l {n} o")),
                    keywords: "solo isolate".into(),
                });
            }
        }
        for (i, d) in self.docs.iter().enumerate() {
            v.push(Entry {
                action: format!("doc.switch:{}", i + 1),
                title: trf("Go to document: {}", &[&d.title]),
                subtitle: tr("Documents"),
                icon: "image",
                keys: if i < 9 { vec![if cfg!(target_os = "macos") { "⌥".into() } else { "Alt".into() }, (i + 1).to_string()] } else { vec![] },
                chord: vec![],
                keywords: "tab switch window".into(),
            });
        }
        if cfg!(not(target_arch = "wasm32")) {
            for p in &self.recent_files {
                v.push(Entry {
                    action: format!("file.recent:{}", p.display()),
                    title: trf("Open recent: {}", &[&p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()]),
                    subtitle: p.parent().map(|p| p.display().to_string()).unwrap_or_default(),
                    icon: "folder-open",
                    keys: vec![],
                    chord: vec![],
                    keywords: "file history".into(),
                });
            }
        }
        v
    }

    /// Commands understood from free text, e.g. "zoom 200", "#ff8800", "size 40".
    fn smart_entries(&self, q: &str) -> Vec<Entry> {
        let q = q.trim();
        let lower = q.to_lowercase();
        let mut out = Vec::new();
        let num_after = |words: &[&str]| -> Option<f32> {
            for w in words {
                if let Some(rest) = lower.strip_prefix(w) {
                    let n = rest.trim().trim_end_matches('%').trim_end_matches("px").trim();
                    if let Ok(v) = n.parse::<f32>() {
                        return Some(v);
                    }
                }
            }
            None
        };
        let smart = |action: String, title: String, icon: &'static str| Entry { action, title, subtitle: tr("Quick command"), icon, keys: vec![], chord: vec![], keywords: String::new() };
        let hex = q.trim_start_matches('#');
        if q.starts_with('#') && (hex.len() == 3 || hex.len() == 6 || hex.len() == 8) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            out.push(smart(format!("color.hex:{hex}"), trf("Set color to #{}", &[&hex.to_uppercase()]), "palette"));
        }
        if let Some(z) = num_after(&["zoom", "z "]).or_else(|| if lower.ends_with('%') { lower.trim_end_matches('%').trim().parse().ok() } else { None }) {
            out.push(smart(format!("set.zoom:{z}"), trf("Zoom to {}%", &[&z]), "zoom-in"));
        }
        if let Some(s) = num_after(&["size", "brush", "width"]) {
            out.push(smart(format!("set.size:{}", s.round()), trf("Set brush size to {} px", &[&s.round()]), "brush"));
        }
        if let Some(s) = num_after(&["opacity"]) {
            out.push(smart(format!("set.opacity:{}", s.round()), trf("Set layer opacity to {}%", &[&s.round()]), "sliders"));
        }
        if let Some(s) = num_after(&["hardness"]) {
            out.push(smart(format!("set.hardness:{}", s.round()), trf("Set brush hardness to {}%", &[&s.round()]), "brush"));
        }
        if let Some(s) = num_after(&["tolerance"]) {
            out.push(smart(format!("set.tolerance:{}", s.round()), trf("Set tolerance to {}%", &[&s.round()]), "wand"));
        }
        if let Some(s) = num_after(&["font size", "fontsize", "text size"]) {
            out.push(smart(format!("set.font-size:{}", s.round()), trf("Set font size to {} pt", &[&s.round()]), "type"));
        }
        if let Some(rest) = q.strip_prefix("rename ").or_else(|| q.strip_prefix("Rename ")) {
            if !rest.trim().is_empty() {
                out.push(smart(format!("layer.rename:{}", rest.trim()), trf("Rename current layer to “{}”", &[&rest.trim()]), "sliders"));
            }
        }
        if let Some(rest) = lower.strip_prefix("new ") {
            if let Some((w, h)) = rest.trim().split_once(['x', '×', '*']) {
                if let (Ok(w), Ok(h)) = (w.trim().parse::<u32>(), h.trim().parse::<u32>()) {
                    out.push(smart(format!("image.new:{w}x{h}"), trf("New image {} × {}", &[&w, &h]), "file-plus"));
                }
            }
        }
        if let Some(n) = num_after(&["layer "]) {
            let n = n as usize;
            if let Some(name) = self.layer_names().get(n.wrapping_sub(1)) {
                out.push(smart(format!("layer.select:{n}"), trf("Switch to layer {}: {}", &[&n, name]), "layers"));
            }
        }
        out
    }

    pub fn palette_query(&mut self, q: String) {
        let entries = self.palette_entries();
        let mut results: Vec<(i32, Entry, Vec<usize>)> = Vec::new();
        for e in self.smart_entries(&q) {
            results.push((10_000, e, vec![]));
        }
        let recent_rank = |a: &str| self.recent_cmds.iter().position(|r| r == a);
        if q.trim().is_empty() {
            // Recently used first, then a few useful starting points.
            for a in &self.recent_cmds {
                if let Some(e) = entries.iter().find(|e| &e.action == a) {
                    results.push((0, e.clone(), vec![]));
                }
            }
            for id in ["tool:brush", "file.new", "file.open", "fx:gaussian-blur", "fx:curves", "fx:hue-saturation", "image.resize", "layer.add", "view.theme-dark", "help.shortcuts"] {
                if !results.iter().any(|(_, e, _)| e.action == id) {
                    if let Some(e) = entries.iter().find(|e| e.action == id) {
                        results.push((0, e.clone(), vec![]));
                    }
                }
            }
        } else {
            for e in entries {
                let title_m = fuzzy(&q, &e.title);
                let other = fuzzy(&q, &format!("{} {} {}", e.subtitle, e.keywords, e.title)).map(|(s, _)| (s - 25, vec![]));
                let chord_m = if !e.chord.is_empty() && q.trim().eq_ignore_ascii_case(&e.chord.join(" ")) { Some((500, vec![])) } else { None };
                let best = [title_m, other, chord_m].into_iter().flatten().max_by_key(|(s, _)| *s);
                if let Some((mut s, idx)) = best {
                    if let Some(r) = recent_rank(&e.action) {
                        s += 30 - r as i32;
                    }
                    results.push((s, e, idx));
                }
            }
            results.sort_by(|a, b| b.0.cmp(&a.0));
        }
        results.truncate(60);
        let items: Vec<PaletteItem> = results
            .iter()
            .map(|(_, e, idx)| PaletteItem {
                segs: ModelRc::new(VecModel::from(segments(&e.title, idx))),
                subtitle: e.subtitle.as_str().into(),
                icon: e.icon.into(),
                keys: ModelRc::new(VecModel::from(e.keys.iter().map(|k| SharedString::from(k.as_str())).collect::<Vec<_>>())),
                chord: ModelRc::new(VecModel::from(e.chord.iter().map(|k| SharedString::from(k.as_str())).collect::<Vec<_>>())),
            })
            .collect();
        self.palette_results = results.into_iter().map(|(_, e, _)| e).collect();
        let ui = self.ui();
        let g = ui.global::<App>();
        g.set_palette_items(ModelRc::new(VecModel::from(items)));
        g.set_palette_index(0);
    }

    pub fn palette_run(&mut self, i: i32) {
        let Some(e) = self.palette_results.get(i.max(0) as usize).cloned() else { return };
        let ui = self.ui();
        let g = ui.global::<App>();
        g.set_dialog("".into());
        g.set_text_focus(false);
        self.remember_command(&e.action);
        self.action(&e.action);
        self.refocus();
    }

    /// Rows for the keyboard shortcut overview.
    pub fn sync_shortcuts(&self) {
        let rows: Vec<ShortcutRow> = COMMANDS
            .iter()
            .filter(|c| !c.keys.is_empty() || !c.chord.is_empty())
            .map(|c| ShortcutRow {
                section: tr(c.path.split(" › ").next().unwrap_or("")).into(),
                title: tr(c.title).trim_end_matches('…').into(),
                keys: ModelRc::new(VecModel::from(platform_keys(c.keys).into_iter().map(SharedString::from).collect::<Vec<_>>())),
                chord: ModelRc::new(VecModel::from(chord_keys(c.chord).into_iter().map(SharedString::from).collect::<Vec<_>>())),
            })
            .collect();
        self.ui().global::<App>().set_shortcut_rows(ModelRc::new(VecModel::from(rows)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_ranks_sensibly() {
        let s = |q: &str, t: &str| fuzzy(q, t).map(|(s, _)| s).unwrap_or(i32::MIN);
        assert!(s("gb", "Gaussian Blur") > s("gb", "Merge Layer Down"));
        assert!(s("gauss", "Gaussian Blur") > s("gauss", "Glass"));
        assert!(fuzzy("xyz", "Gaussian Blur").is_none());
        assert_eq!(fuzzy("gb", "Gaussian Blur").unwrap().1, vec![0, 9]);
        assert!(s("hue", "Hue / Saturation") > s("hue", "Show Hue"));
    }

    #[test]
    fn chords() {
        let mut st = ChordState::default();
        assert!(matches!(chord_step(&mut st, "f", 1), ChordResult::Pending));
        assert!(matches!(chord_step(&mut st, "b", 1), ChordResult::Pending));
        assert!(matches!(chord_step(&mut st, "g", 1), ChordResult::Run(a) if a == "fx:gaussian-blur"));
        assert!(matches!(chord_step(&mut st, "l", 3), ChordResult::Pending));
        assert!(matches!(chord_step(&mut st, "2", 3), ChordResult::Pending));
        assert!(matches!(chord_step(&mut st, "t", 3), ChordResult::Run(a) if a == "layer.toggle:2"));
        assert!(matches!(chord_step(&mut st, "l", 3), ChordResult::Pending));
        assert!(matches!(chord_step(&mut st, "n", 3), ChordResult::Run(a) if a == "layer.add"));
        assert!(matches!(chord_step(&mut st, "l", 3), ChordResult::Pending));
        assert!(matches!(chord_step(&mut st, "2", 3), ChordResult::Pending));
        assert!(matches!(chord_step(&mut st, "2", 3), ChordResult::Run(a) if a == "layer.select:2"));
        assert!(matches!(chord_step(&mut st, "l", 3), ChordResult::Pending));
        assert!(matches!(chord_step(&mut st, "k", 3), ChordResult::Run(a) if a == "layer.select-up"));
        assert!(matches!(chord_step(&mut st, "l", 3), ChordResult::Pending));
        assert!(matches!(chord_step(&mut st, "K", 3), ChordResult::Run(a) if a == "layer.up"));
        assert!(matches!(chord_step(&mut st, "L", 3), ChordResult::Pending));
        assert!(matches!(chord_step(&mut st, "J", 3), ChordResult::Run(a) if a == "layer.down"));
        assert!(matches!(chord_step(&mut st, "b", 3), ChordResult::None));
        assert!(matches!(chord_step(&mut st, "a", 3), ChordResult::Pending));
        assert!(matches!(chord_step(&mut st, "q", 3), ChordResult::Cancelled));
        assert!(st.keys.is_empty());
    }
}
