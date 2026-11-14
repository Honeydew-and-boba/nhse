//! Renders the editor with each logo style, in light and dark, for choosing one.
//!
//!   cargo run -p nhse-switch --example logos -- <output folder>
use nhse_switch::host::{save_png, HostPlatform, Step};
use nhse_switch::platform::btn::*;
use nhse_switch::theme::ThemeChoice;
use nhse_switch::ui::LogoStyle;

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "logos".into()));
    std::fs::create_dir_all(&out).unwrap();
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();
    let rev = nhse_core::save::REVISIONS.len() - 1;
    nhse_core::testing::write_synthetic_save(&HostPlatform::game_dir(root, "Player 1"), rev, 2, 11);
    // open the game save, so the header shows the island and the save state
    let script = vec![Step::Press(A), Step::Press(A)];
    let mut app = nhse_switch::app(Box::new(HostPlatform::new(
        root,
        &["Player 1"],
        script,
        &out,
    )));
    app.run();
    for (theme, tname) in [(ThemeChoice::Light, "light"), (ThemeChoice::Dark, "dark")] {
        app.cx.theme_choice = theme;
        for (style, sname) in [
            (LogoStyle::Palm, "a-palm"),
            (LogoStyle::Icon, "b-icon"),
            (LogoStyle::Badge, "c-badge"),
            (LogoStyle::Text, "d-text"),
        ] {
            save_png(
                app.render_with_logo(style),
                &out.join(format!("{sname}-{tname}.png")),
            );
        }
    }
    println!("logo previews in {}", out.display());
}
