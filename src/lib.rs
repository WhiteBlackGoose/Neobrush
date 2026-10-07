//! Neobrush: a modern, cross-platform raster graphics editor.

pub mod app;
pub mod core;

slint::include_modules!();

pub use app::run;

/// Entry point of the web build.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn wasm_main() {
    console_error_panic_hook::set_once();
    if let Err(e) = app::run(vec![]) {
        web_sys::console::log_1(&e.to_string().into());
    }
}
