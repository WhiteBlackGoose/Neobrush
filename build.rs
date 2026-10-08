use std::fmt::Write as _;
use std::path::Path;

/// Turns i18n/<lang>.txt (`english ⇒ translation` lines) into gettext catalogs that Slint bundles into the binary.
fn generate_catalogs(out: &Path) {
    println!("cargo:rerun-if-changed=i18n");
    for entry in std::fs::read_dir("i18n").unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().and_then(|n| n.to_str()) == Some("strings.txt") {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let lang = path.file_stem().unwrap().to_str().unwrap().to_string();
        let src = std::fs::read_to_string(&path).unwrap();
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"").replace("\\\\n", "\\n");
        let mut po = String::from("msgid \"\"\nmsgstr \"\"\n\"Content-Type: text/plain; charset=UTF-8\\n\"\n\n");
        for line in src.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let Some((k, v)) = line.split_once(" ⇒ ") else { continue };
            if v.trim().is_empty() {
                continue;
            }
            let _ = write!(po, "msgid \"{}\"\nmsgstr \"{}\"\n\n", esc(k), esc(v));
        }
        let dir = out.join(&lang).join("LC_MESSAGES");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("neobrush.po"), po).unwrap();
    }
}

fn main() {
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("translations");
    generate_catalogs(&out);
    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent".into())
        .with_bundled_translations(&out)
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config).unwrap();

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("packaging/neobrush.ico");
        res.compile().unwrap();
    }
}
