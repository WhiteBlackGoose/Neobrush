//! Translations for strings produced in Rust. The Slint UI uses the same tables
//! (bundled as gettext catalogs by build.rs) through `@tr(...)`.

use std::collections::HashMap;
use std::sync::RwLock;

/// (code, native name). The code is the gettext language of the bundled catalogs.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("de", "Deutsch"),
    ("es", "Español"),
    ("fr", "Français"),
    ("it", "Italiano"),
    ("pt", "Português"),
    ("uk", "Українська"),
    ("ru", "Русский"),
    ("la", "Latina"),
    ("tok", "toki pona"),
    ("zh", "中文"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("he", "עברית"),
    ("ar", "العربية"),
];

fn table_source(code: &str) -> &'static str {
    match code {
        "de" => include_str!("../../i18n/de.txt"),
        "es" => include_str!("../../i18n/es.txt"),
        "fr" => include_str!("../../i18n/fr.txt"),
        "it" => include_str!("../../i18n/it.txt"),
        "pt" => include_str!("../../i18n/pt.txt"),
        "uk" => include_str!("../../i18n/uk.txt"),
        "ru" => include_str!("../../i18n/ru.txt"),
        "la" => include_str!("../../i18n/la.txt"),
        "tok" => include_str!("../../i18n/tok.txt"),
        "zh" => include_str!("../../i18n/zh.txt"),
        "ja" => include_str!("../../i18n/ja.txt"),
        "ko" => include_str!("../../i18n/ko.txt"),
        "he" => include_str!("../../i18n/he.txt"),
        "ar" => include_str!("../../i18n/ar.txt"),
        _ => "",
    }
}

/// Parses `english ⇒ translation` lines (`\n` escapes a newline, `#` starts a comment).
pub fn parse_table(src: &str) -> HashMap<String, String> {
    src.lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once(" ⇒ "))
        .filter(|(_, t)| !t.trim().is_empty())
        .map(|(k, v)| (k.replace("\\n", "\n"), v.replace("\\n", "\n")))
        .collect()
}

static TABLE: RwLock<Option<HashMap<String, String>>> = RwLock::new(None);
static CURRENT: RwLock<String> = RwLock::new(String::new());

pub fn set_language(code: &str) {
    let table = if code == "en" { None } else { Some(parse_table(table_source(code))) };
    *TABLE.write().unwrap() = table;
    *CURRENT.write().unwrap() = code.to_string();
    let _ = slint::select_bundled_translation(code);
}

/// UI font family for a language: Inter covers Latin and Cyrillic, the bundled
/// Neobrush fonts (Noto subsets with Inter's Latin) cover the rest.
pub fn ui_font(code: &str) -> &'static str {
    match code {
        "zh" | "ja" | "ko" => "Neobrush CJK",
        "he" => "Neobrush Hebrew",
        "ar" => "Neobrush Arabic",
        _ => "Inter",
    }
}

pub fn current() -> String {
    CURRENT.read().unwrap().clone()
}

/// Translates an English UI string (falls back to English).
pub fn tr(s: &str) -> String {
    match TABLE.read().unwrap().as_ref().and_then(|t| t.get(s)) {
        Some(t) => t.clone(),
        None => s.to_string(),
    }
}

/// Translates a string with `{}` placeholders and fills them in order.
pub fn trf(s: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut it = args.iter();
    let t = tr(s);
    let mut rest = t.as_str();
    while let Some(i) = rest.find("{}") {
        out.push_str(&rest[..i]);
        if let Some(a) = it.next() {
            out.push_str(&a.to_string());
        }
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    out
}

/// Picks the best supported language for the OS locale.
pub fn detect() -> &'static str {
    let loc = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    let prim = loc.split(['-', '_']).next().unwrap_or("");
    LANGUAGES.iter().map(|(c, _)| *c).find(|c| *c == prim).unwrap_or("en")
}
