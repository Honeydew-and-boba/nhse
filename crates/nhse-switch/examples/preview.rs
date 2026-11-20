//! Runs the Switch app on the desktop against synthetic saves and writes screenshots.
//!
//!   cargo run -p nhse-switch --example preview -- <output folder> [--dark]
use nhse_switch::host::{HostPlatform, Step};
use nhse_switch::platform::btn::*;

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "preview".into()));
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path();
    let rev = nhse_core::save::REVISIONS.len() - 1;
    nhse_core::testing::write_synthetic_save(&HostPlatform::game_dir(root, "Player 1"), rev, 2, 11);
    nhse_core::testing::write_synthetic_save(&root.join("sd/saves/My island"), rev, 1, 5);
    std::fs::create_dir_all(root.join("sd/switch")).unwrap();

    use Step::*;
    let shot = |n: &str| Shot(n.to_string());
    let d = |n: usize| std::iter::repeat_n(Press(DOWN), n);
    let mut script = vec![
        shot("01-home"),
        Press(A),
        shot("02-users"),
        Press(A),
        shot("03-editor"),
    ];
    script.extend(d(11)); // Map (new editors follow the existing save actions)
    script.extend([
        Press(A),
        shot("19-map"),
        Press(B),
        Press(DOWN),
        Press(A),
        shot("20-designs"),
        Press(A),
        Press(A),
        shot("21-pixels"),
        Press(B),
        Press(B),
        Press(B),
        Press(DOWN),
        Press(A),
        shot("22-houses"),
        Press(A),
        shot("23-house"),
    ]);
    script.extend(d(4));
    script.extend([
        Press(A),
        shot("24-room"),
        Press(B),
        Press(B),
        Press(B),
        Press(ZL),
    ]);
    script.extend([Press(A), shot("04-player"), Press(A)]); // wallet → 54321
    script.extend(d(6)); // → Pockets
    script.extend([Press(A)]);
    script.extend(d(2));
    script.extend([
        shot("05-pockets"),
        Press(A),
        Press(A),
        shot("06-search"),
        Press(A),
        shot("07-item"),
    ]);
    script.extend([Press(B), Press(B)]);
    script.extend(d(2)); // → recipes
    script.extend([
        Press(A),
        shot("08-confirm"),
        Press(A),
        shot("09-learned"),
        Press(B),
    ]);
    script.extend(d(3)); // Player1, Turnips, Villagers
    script.extend([
        Press(A),
        shot("10-villagers"),
        Press(B),
        Press(UP),
        Press(A),
        shot("11-turnips"),
        Press(B),
    ]);
    script.extend(d(3)); // Villagers, Museum, Tools
    script.extend([
        Press(A),
        shot("12-tools"),
        Press(B),
        Press(PLUS),
        Wait(2),
        shot("13-saved"),
    ]);
    script.extend([
        Press(B),
        Press(DOWN),
        Press(A),
        shot("14-browser"),
        Press(A),
        Press(A),
        shot("15-sd-editor"),
    ]);
    script.extend([
        Press(B),
        Press(DOWN),
        Press(A),
        shot("16-backups"),
        Press(A),
        shot("17-backup"),
    ]);
    script.extend([Press(B), Press(B), Press(PLUS), shot("18-quit")]);
    let mut p = HostPlatform::new(root, &["Player 1", "Player 2"], script, &out);
    p.system_dark = std::env::args().any(|arg| arg == "--dark");
    p.keyboard.push_back(Some("54321".into()));
    p.keyboard.push_back(Some("wood".into()));
    let mut app = nhse_switch::app(Box::new(p));
    app.run();
    println!("screenshots in {}", out.display());
}
