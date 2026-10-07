mod actions;
mod commands_gen;
mod palette;
mod editor;
mod platform;
mod render;
mod session;
mod settings;
mod tools;

pub use editor::run;

#[cfg(test)]
mod perf;
