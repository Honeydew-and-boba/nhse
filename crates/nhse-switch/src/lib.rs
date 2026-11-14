//! NHSE for Nintendo Switch 2: the save editor as a homebrew app (`nhse.nro`).
//!
//! The app is drawn in software into a 1280×720 framebuffer and driven with the buttons. Saves
//! are read and written with `nhse-core`, the same library as the desktop app. The console side
//! (framebuffer, input, software keyboard, accounts, save data) is in `switch/source/glue.c`;
//! `nx.rs` calls it. On other systems `host.rs` runs the same app for previews and tests.
pub mod edits;
pub mod fmt;
pub mod gfx;
pub mod platform;
pub mod session;
pub mod theme;
pub mod ui;
pub mod views;

#[cfg(target_os = "horizon")]
mod nx;

#[cfg(not(target_os = "horizon"))]
pub mod host;

/// Builds the app with the home screen.
pub fn app(platform: Box<dyn platform::Platform>) -> ui::App {
    ui::App::new(platform, Box::new(views::Home))
}
