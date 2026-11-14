//! Drives the app with scripted buttons, like a player would, and checks what ends up on disk.
use nhse_core::save::player::PlayerList;
use nhse_core::HorizonSave;
use nhse_switch::host::{HostPlatform, Step};
use nhse_switch::platform::btn::*;
use std::path::Path;
use Step::*;

const USER: &str = "Player 1";

fn setup() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let rev = nhse_core::save::REVISIONS.len() - 1;
    nhse_core::testing::write_synthetic_save(&HostPlatform::game_dir(tmp.path(), USER), rev, 2, 3);
    nhse_core::testing::write_synthetic_save(&tmp.path().join("sd/dumps/island"), rev, 1, 9);
    tmp
}

fn run(root: &Path, script: Vec<Step>, keys: &[&str], fail_commit: bool) -> nhse_switch::ui::App {
    let mut p = HostPlatform::new(root, &[USER, "Player 2"], script, &root.join("shots"));
    p.keyboard.extend(keys.iter().map(|k| Some(k.to_string())));
    p.fail_commit = fail_commit;
    let mut app = nhse_switch::app(Box::new(p));
    app.run();
    app
}

fn read_dir_bytes(p: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            } else {
                let rel = e.path().strip_prefix(p).unwrap().display().to_string();
                out.push((rel, std::fs::read(e.path()).unwrap()));
            }
        }
    }
    out.sort();
    out
}

/// Home → game save of USER → first resident → wallet.
fn open_game_and_set_wallet() -> Vec<Step> {
    vec![Press(A), Press(A), Press(A), Press(A)]
}

#[test]
fn game_save_edit_is_written_and_committed() {
    let tmp = setup();
    let root = tmp.path();
    let mut script = open_game_and_set_wallet();
    script.push(Press(PLUS)); // save
    let app = run(root, script, &["54321"], false);

    let s = app.cx.session.as_ref().expect("save still open");
    assert!(!s.dirty, "saved");
    let game = HorizonSave::open(&HostPlatform::game_dir(root, USER)).unwrap();
    assert_eq!(game.player_profile(0).unwrap().wallet, 54_321);
    assert!(game.invalid_hashes().is_empty(), "hashes recomputed");
    // the save was backed up before editing, and the backup holds the old value
    let backups: Vec<_> = std::fs::read_dir(root.join("sd/switch/nhse/backups"))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(backups.len(), 1);
    let old = HorizonSave::open(&backups[0].path()).unwrap();
    assert_ne!(old.player_profile(0).unwrap().wallet, 54_321);
}

#[test]
fn only_changed_files_are_written_to_the_game() {
    let tmp = setup();
    let root = tmp.path();
    let game = HostPlatform::game_dir(root, USER);
    let before = read_dir_bytes(&game);
    let mut script = open_game_and_set_wallet();
    script.push(Press(PLUS));
    run(root, script, &["1000"], false);
    let after = read_dir_bytes(&game);
    let changed: Vec<&str> = before
        .iter()
        .zip(&after)
        .filter(|(a, b)| a.1 != b.1)
        .map(|(a, _)| a.0.as_str())
        .collect();
    assert_eq!(
        changed,
        ["Villager0/personal.dat", "Villager0/personalHeader.dat"]
    );
}

#[test]
fn failed_commit_leaves_the_game_save_untouched() {
    let tmp = setup();
    let root = tmp.path();
    let before = read_dir_bytes(&HostPlatform::game_dir(root, USER));
    let mut script = open_game_and_set_wallet();
    script.push(Press(PLUS));
    let app = run(root, script, &["777"], true);
    assert!(
        app.cx.session.as_ref().unwrap().dirty,
        "still unsaved after the failure"
    );
    assert_eq!(before, read_dir_bytes(&HostPlatform::game_dir(root, USER)));
}

#[test]
fn sd_save_bulk_edits_are_saved() {
    let tmp = setup();
    let root = tmp.path();
    let d = |n| std::iter::repeat_n(Press(DOWN), n);
    // Home → SD card → dumps/ → island/ (opens) → resident → recipes
    let mut script = vec![Press(DOWN), Press(A), Press(A), Press(A), Press(A)];
    script.extend(d(8)); // Wallet … Poki, Pockets, Storage, Recipes
    script.extend([Press(A), Press(A), Press(PLUS)]);
    let app = run(root, script, &[], false);
    assert!(!app.cx.session.as_ref().unwrap().dirty);
    let s = HorizonSave::open(&root.join("sd/dumps/island")).unwrap();
    let flags = s.recipes(0).unwrap();
    let story = nhse_core::editing::itemsets::STORY_RECIPES;
    for (r, _) in &nhse_core::data::tables().recipes {
        if !story.contains(r) {
            assert!(flags[*r as usize][0], "recipe {r:#x} learned");
        }
    }
}

#[test]
fn item_is_placed_in_pockets() {
    let tmp = setup();
    let root = tmp.path();
    let d = |n| std::iter::repeat_n(Press(DOWN), n);
    let mut script = vec![Press(A), Press(A), Press(A)];
    script.extend(d(6)); // → Pockets
    script.extend([
        Press(A),
        Press(DOWN),
        Press(A),
        Press(A),
        Press(A),
        Press(DOWN),
        Press(A),
    ]);
    // item search "wood" → first result; quantity → 30
    script.push(Press(PLUS));
    let _app = run(root, script, &["wood", "30"], false);
    let game = HorizonSave::open(&HostPlatform::game_dir(root, USER)).unwrap();
    let it = game.player_items(0, PlayerList::Pocket).unwrap()[1];
    assert_eq!(it.id, 0x0AD0, "wood");
    assert_eq!(it.count() + 1, 30);
}

#[test]
fn backup_restores_the_game_save() {
    let tmp = setup();
    let root = tmp.path();
    let original = read_dir_bytes(&HostPlatform::game_dir(root, USER));
    // edit and save, then close
    let mut script = open_game_and_set_wallet();
    script.extend([Press(PLUS), Press(B), Press(B), Press(B)]);
    run(root, script, &["4242"], false);
    let edited = HorizonSave::open(&HostPlatform::game_dir(root, USER)).unwrap();
    assert_eq!(edited.player_profile(0).unwrap().wallet, 4242);

    // Home → Backups → the backup of the original → Restore → USER → confirm
    let script = vec![
        Press(DOWN),
        Press(DOWN),
        Press(A),
        Press(A),
        Press(A),
        Press(A),
        Press(A),
    ];
    run(root, script, &[], false);
    let restored = read_dir_bytes(&HostPlatform::game_dir(root, USER));
    assert_eq!(original, restored, "the game save is the original again");
    // and the edited save was backed up before it was replaced
    let n = std::fs::read_dir(root.join("sd/switch/nhse/backups"))
        .unwrap()
        .count();
    assert_eq!(n, 2);
}

#[test]
fn theme_switch_is_remembered() {
    use nhse_switch::theme::ThemeChoice;
    let tmp = setup();
    let root = tmp.path();
    // Home: Appearance is the row above About (the last row)
    let script = vec![Press(ZR), Press(UP), Press(A), Press(A), Press(RIGHT)];
    let app = run(root, script, &[], false);
    // System → Light → Dark → System
    assert_eq!(app.cx.theme_choice, ThemeChoice::System);
    let script = vec![Press(ZR), Press(UP), Press(A), Press(A)];
    let app = run(root, script, &[], false);
    assert_eq!(app.cx.theme_choice, ThemeChoice::Dark);
    assert!(app.cx.theme().dark);
    // a new start reads the setting back
    let app = run(root, vec![], &[], false);
    assert_eq!(app.cx.theme_choice, ThemeChoice::Dark);
    let saved = std::fs::read_to_string(root.join("sd/switch/nhse/settings.txt")).unwrap();
    assert_eq!(saved.trim(), "theme=dark");
}
