#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod core;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let files: Vec<std::path::PathBuf> = std::env::args_os().skip(1).map(Into::into).collect();
    app::run(files)
}
