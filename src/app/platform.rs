//! Platform specific services: background work, clipboard, file dialogs and settings storage.
//! Desktop builds use threads and the native OS facilities; the web build uses browser APIs.

use crate::core::surface::Surface;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

/// Runs `work` off the UI thread when possible and hands the result to `done` on the UI thread.
pub fn background<T, W, D>(work: W, done: D)
where
    T: Send + 'static,
    W: FnOnce() -> T + Send + 'static,
    D: FnOnce(T) + Send + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        let r = work();
        let _ = slint::invoke_from_event_loop(move || done(r));
    });
    // No threads on the web: run on the next event loop turn so the UI can repaint first.
    #[cfg(target_arch = "wasm32")]
    slint::Timer::single_shot(std::time::Duration::from_millis(1), move || done(work()));
}

// ---------------------------------------------------------------------------------------------
// Clipboard

#[cfg(not(target_arch = "wasm32"))]
pub fn clipboard_get() -> Option<Surface> {
    let mut cb = arboard::Clipboard::new().ok()?;
    if let Ok(img) = cb.get_image() {
        let data = img.bytes.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
        return Some(Surface { w: img.width as u32, h: img.height as u32, data });
    }
    // A file path or URI on the clipboard.
    let t = cb.get_text().ok()?;
    let path = PathBuf::from(t.trim().trim_start_matches("file://"));
    if path.is_file() {
        return crate::core::io::load_surface(&path).ok();
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn clipboard_set(s: &Surface) {
    let bytes: Vec<u8> = s.data.iter().flatten().copied().collect();
    let img = arboard::ImageData { width: s.w as usize, height: s.h as usize, bytes: bytes.into() };
    if let Ok(mut cb) = arboard::Clipboard::new() {
        let _ = cb.set_image(img);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn clipboard_size() -> (i32, i32) {
    match arboard::Clipboard::new().and_then(|mut c| c.get_image()) {
        Ok(img) => (img.width as i32, img.height as i32),
        Err(_) => (0, 0),
    }
}

// The web build keeps an app-internal clipboard only (see Editor::clipboard).
#[cfg(target_arch = "wasm32")]
pub fn clipboard_get() -> Option<Surface> {
    None
}
#[cfg(target_arch = "wasm32")]
pub fn clipboard_set(_: &Surface) {}
#[cfg(target_arch = "wasm32")]
pub fn clipboard_size() -> (i32, i32) {
    (0, 0)
}

// ---------------------------------------------------------------------------------------------
// Settings storage

#[cfg(not(target_arch = "wasm32"))]
fn settings_path() -> Option<PathBuf> {
    let dir = if cfg!(windows) {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("Neobrush"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/Neobrush"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|p| p.join("neobrush"))
    };
    dir.map(|d| d.join("settings.conf"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_settings() -> Option<String> {
    std::fs::read_to_string(settings_path()?).ok()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write_settings(s: &str) {
    if let Some(path) = settings_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, s);
    }
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
pub fn read_settings() -> Option<String> {
    storage()?.get_item("neobrush.settings").ok()?
}

#[cfg(target_arch = "wasm32")]
pub fn write_settings(s: &str) {
    if let Some(st) = storage() {
        let _ = st.set_item("neobrush.settings", s);
    }
}

// ---------------------------------------------------------------------------------------------
// Web file handling

/// Opens the browser's file picker and returns (file name, contents) for each chosen file.
#[cfg(target_arch = "wasm32")]
pub fn pick_files(multiple: bool, accept: &str, done: impl FnOnce(Vec<(String, Vec<u8>)>) + 'static) {
    use wasm_bindgen::{closure::Closure, JsCast};
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return };
    let Ok(input) = doc.create_element("input") else { return };
    let input: web_sys::HtmlInputElement = input.unchecked_into();
    input.set_type("file");
    input.set_multiple(multiple);
    input.set_accept(accept);
    let inp = input.clone();
    let done = std::cell::RefCell::new(Some(done));
    let on_change = Closure::<dyn FnMut()>::new(move || {
        let Some(done) = done.borrow_mut().take() else { return };
        let files = inp.files();
        wasm_bindgen_futures::spawn_local(async move {
            let mut out = Vec::new();
            if let Some(list) = files {
                for i in 0..list.length() {
                    let Some(f) = list.get(i) else { continue };
                    if let Ok(buf) = wasm_bindgen_futures::JsFuture::from(f.array_buffer()).await {
                        out.push((f.name(), js_sys::Uint8Array::new(&buf).to_vec()));
                    }
                }
            }
            done(out);
        });
    });
    input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
    on_change.forget();
    input.click();
}

/// Offers `bytes` to the user as a download.
#[cfg(target_arch = "wasm32")]
pub fn download(name: &str, bytes: &[u8], mime: &str) {
    use wasm_bindgen::JsCast;
    let arr = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::of1(&arr);
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts) else { return };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else { return };
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        if let Ok(a) = doc.create_element("a") {
            let a: web_sys::HtmlAnchorElement = a.unchecked_into();
            a.set_href(&url);
            a.set_download(name);
            a.click();
        }
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}

#[cfg(target_arch = "wasm32")]
pub fn mime_for(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        "ora" => "image/openraster",
        _ => "application/octet-stream",
    }
}

/// Opens a link in the user's browser.
pub fn open_url(url: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = open::that_detached(url);
    }
    #[cfg(target_arch = "wasm32")]
    if let Some(w) = web_sys::window() {
        let _ = w.open_with_url_and_target(url, "_blank");
    }
}
