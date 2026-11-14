//! Screens that work without an open save: home, the SD card browser, users and backups.
pub mod editor;
pub mod items;

use crate::fmt;
use crate::platform::{btn, UserAccount};
use crate::session::{self, BackupInfo};
use crate::ui::{Cx, Info, Nav, Reply, Row, Style, View};
use std::path::{Path, PathBuf};

/// `"VillagerPersonality_Uchi"`-style enum names → `"Uchi"`, `"BigSister"` → `"Big Sister"`.
pub fn prettify(name: &str) -> String {
    let base = name.rsplit('_').next().unwrap_or(name);
    let mut out = String::new();
    let mut prev_lower = false;
    for ch in base.chars() {
        if ch.is_uppercase() && prev_lower {
            out.push(' ');
        }
        prev_lower = ch.is_lowercase() || ch.is_ascii_digit();
        out.push(ch);
    }
    out
}

/// Opens the editor on a freshly opened session.
fn opened(cx: &mut Cx, s: session::Session) -> Nav {
    let warnings = s.save.warnings.clone();
    let town = s.town();
    let backed_up = s.backup.is_some();
    cx.session = Some(s);
    cx.toast(if backed_up {
        format!("Opened {town}. A backup was saved to the SD card.")
    } else {
        format!("Opened {town}.")
    });
    if !warnings.is_empty() {
        cx.toast_error(warnings.join(" "));
    }
    Nav::Reset(Box::new(editor::Editor))
}

const T_QUIT: u32 = 1;
const T_DISCARD_GAME: u32 = 2;
const T_DISCARD_SD: u32 = 3;

pub struct Home;

impl Home {
    const CONTINUE: usize = 0;
    fn layout(cx: &Cx) -> Vec<usize> {
        // stable row ids, with "continue" only while a save is open
        let mut ids = Vec::new();
        if cx.session.is_some() {
            ids.push(Self::CONTINUE);
        }
        ids.extend([10, 1, 2, 11, 3, 4]);
        ids
    }

    fn open_game(cx: &mut Cx) -> Nav {
        Nav::Push(users_screen(cx, UserPurpose::OpenGame))
    }
}

impl View for Home {
    fn title(&self, _cx: &Cx) -> String {
        "Animal Crossing: New Horizons save editor".into()
    }

    fn rows(&self, cx: &Cx) -> Vec<Row> {
        Self::layout(cx)
            .into_iter()
            .map(|id| match id {
                Self::CONTINUE => {
                    let s = cx.session.as_ref().expect("session");
                    Row::new(format!("Continue editing {}", s.town()))
                        .value(if s.dirty { "Unsaved changes" } else { "" })
                        .style(Style::Action)
                        .help(format!("Go back to the save you have open.\n\n{}", s.location()))
                }
                10 => Row::section("Open a save"),
                1 => Row::new("Animal Crossing on this console").help(
                    "Edit the game's own save. Pick the user who plays, edit, then press + to save.\n\nClose Animal Crossing before you start (press HOME, then X on the game). The save is backed up to the SD card when it is opened, and the game's save only changes when you save.",
                ),
                2 => Row::new("A save on the SD card").help(
                    "Open a save folder (the one with main.dat in it), main.dat itself, or a .zip of the save, for example a dump made with a save manager.",
                ),
                11 => Row::section("More"),
                3 => {
                    let n = session::list_backups(cx.platform.as_ref()).len();
                    Row::new("Backups").value(n.to_string()).help(format!(
                        "Every save you open is backed up first. Restore a backup to the game, or copy the folders to a computer: each one is a save folder that the desktop NHSE opens.\n\nFolder: {}",
                        session::backups_dir(cx.platform.as_ref()).display()
                    ))
                }
                4 => Row::new("About NHSE").help(format!(
                    "NHSE {} for Nintendo Switch 2, built with ounce-devkit.\n\nSupports Animal Crossing: New Horizons {} to {}.",
                    env!("CARGO_PKG_VERSION"),
                    nhse_core::save::REVISIONS.first().map(|r| r.version).unwrap_or(""),
                    nhse_core::save::REVISIONS.last().map(|r| r.version).unwrap_or(""),
                )),
                _ => Row::new("?"),
            })
            .collect()
    }

    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        match Self::layout(cx).get(row).copied() {
            Some(Self::CONTINUE) => Nav::Push(Box::new(editor::Editor)),
            Some(1) if cx.dirty() => Nav::confirm_danger(
                "Discard your changes?",
                "The open save has changes that are not saved. Open another save anyway?",
                T_DISCARD_GAME,
            ),
            Some(1) => Self::open_game(cx),
            Some(2) if cx.dirty() => Nav::confirm_danger(
                "Discard your changes?",
                "The open save has changes that are not saved. Open another save anyway?",
                T_DISCARD_SD,
            ),
            Some(2) => Nav::Push(Box::new(Browser::new(cx.platform.sd_root()))),
            Some(3) => Nav::Push(Box::new(Backups)),
            Some(4) => Nav::message(
                "About NHSE",
                format!(
                    "NHSE {} — save editor for Animal Crossing: New Horizons, rewritten in Rust and running natively on Switch 2.\n\nA rewrite of NHSE by Kaphotics and contributors. GNU GPL v3. Text uses the Inter typeface (SIL Open Font License).\n\nWe do not support or condone cheating at the expense of others. Please be responsible!",
                    env!("CARGO_PKG_VERSION")
                ),
            ),
            _ => Nav::Stay,
        }
    }

    fn button(&mut self, cx: &mut Cx, b: u32, _row: usize) -> Option<Nav> {
        (b == btn::PLUS).then(|| self.back(cx))
    }

    fn back(&mut self, cx: &mut Cx) -> Nav {
        if cx.dirty() {
            Nav::confirm_danger(
                "Exit without saving?",
                "The open save has changes that are not saved. They are lost if you exit.",
                T_QUIT,
            )
        } else {
            Nav::Quit
        }
    }

    fn reply(&mut self, cx: &mut Cx, r: Reply) -> Nav {
        match r {
            Reply::Confirmed(T_QUIT) => Nav::Quit,
            Reply::Confirmed(T_DISCARD_GAME) => {
                cx.session = None;
                Self::open_game(cx)
            }
            Reply::Confirmed(T_DISCARD_SD) => {
                cx.session = None;
                Nav::Push(Box::new(Browser::new(cx.platform.sd_root())))
            }
            Reply::User(user) => {
                Nav::busy(format!("Opening the save of {}…", user.name), move |cx| {
                    match session::open_game(cx.platform.as_mut(), &user) {
                        Ok(s) => opened(cx, s),
                        Err(e) => Nav::message("The save could not be opened", e),
                    }
                })
            }
            _ => Nav::Stay,
        }
    }
}

#[derive(Clone)]
pub enum UserPurpose {
    OpenGame,
    Restore,
}

/// Picks a user of the console.
pub struct Users {
    purpose: UserPurpose,
    users: Option<Result<Vec<UserAccount>, String>>,
}

impl Users {
    fn list(&self) -> &[UserAccount] {
        match &self.users {
            Some(Ok(u)) => u,
            _ => &[],
        }
    }
}

impl View for Users {
    fn title(&self, _cx: &Cx) -> String {
        match self.purpose {
            UserPurpose::OpenGame => "Whose island save?".into(),
            UserPurpose::Restore => "Restore to which user?".into(),
        }
    }

    fn rows(&self, _cx: &Cx) -> Vec<Row> {
        self.list()
            .iter()
            .map(|u| {
                Row::new(&u.name).help(match self.purpose {
                    UserPurpose::OpenGame => "Opens this user's Animal Crossing save. Close the game first.",
                    UserPurpose::Restore => "Replaces this user's Animal Crossing save with the backup. Close the game first.",
                })
            })
            .collect()
    }

    fn activate(&mut self, _cx: &mut Cx, row: usize) -> Nav {
        match self.list().get(row) {
            Some(u) => Nav::PopReply(Reply::User(u.clone())),
            None => Nav::Stay,
        }
    }

    fn empty_text(&self) -> &str {
        match &self.users {
            Some(Err(_)) => "The user accounts could not be read.",
            _ => "No user accounts.",
        }
    }

    fn info(&self, cx: &Cx, rows: &[Row], row: usize) -> Info {
        if let Some(Err(e)) = &self.users {
            return Info {
                title: "No users".into(),
                text: e.clone(),
                facts: Vec::new(),
            };
        }
        let _ = cx;
        rows.get(row)
            .map(|r| Info {
                title: r.label.clone(),
                text: r.help.clone(),
                facts: vec![],
            })
            .unwrap_or_default()
    }

    fn a_label(&self, _cx: &Cx, _row: usize) -> &'static str {
        "Choose"
    }
}

/// Loads the user list the first time the screen is shown.
pub fn users_screen(cx: &mut Cx, purpose: UserPurpose) -> Box<dyn View> {
    Box::new(Users {
        purpose,
        users: Some(cx.platform.users()),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    Folder,
    SaveFolder,
    Zip,
    MainDat,
}

/// The SD card browser.
pub struct Browser {
    dir: PathBuf,
    entries: Vec<(String, EntryKind)>,
    error: Option<String>,
}

impl Browser {
    pub fn new(dir: PathBuf) -> Self {
        let mut b = Self {
            dir,
            entries: Vec::new(),
            error: None,
        };
        b.load();
        b
    }

    fn load(&mut self) {
        self.entries.clear();
        let rd = match std::fs::read_dir(&self.dir) {
            Ok(rd) => rd,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let path = e.path();
            let kind = if path.is_dir() {
                if path.join("main.dat").is_file() {
                    EntryKind::SaveFolder
                } else {
                    EntryKind::Folder
                }
            } else if name.to_ascii_lowercase().ends_with(".zip") {
                EntryKind::Zip
            } else if name.eq_ignore_ascii_case("main.dat") {
                EntryKind::MainDat
            } else {
                continue;
            };
            self.entries.push((name, kind));
        }
        self.entries.sort_by(|a, b| {
            let rank = |k: EntryKind| match k {
                EntryKind::MainDat => 0,
                EntryKind::SaveFolder => 1,
                EntryKind::Folder => 2,
                EntryKind::Zip => 3,
            };
            rank(a.1)
                .cmp(&rank(b.1))
                .then(a.0.to_lowercase().cmp(&b.0.to_lowercase()))
        });
    }

    fn open(path: PathBuf) -> Nav {
        Nav::busy("Opening the save…", move |cx| {
            match session::open_sd(cx.platform.as_ref(), &path) {
                Ok(s) => opened(cx, s),
                Err(e) => Nav::message("The save could not be opened", e),
            }
        })
    }
}

fn display_path(p: &Path) -> String {
    p.display().to_string()
}

impl View for Browser {
    fn title(&self, _cx: &Cx) -> String {
        display_path(&self.dir)
    }

    fn rows(&self, _cx: &Cx) -> Vec<Row> {
        self.entries
            .iter()
            .map(|(name, kind)| match kind {
                EntryKind::Folder => {
                    Row::new(format!("{name}/")).help("A folder. Press A to look inside.")
                }
                EntryKind::SaveFolder => Row::new(format!("{name}/"))
                    .value("Save")
                    .style(Style::Action)
                    .help("This folder holds a save (main.dat). Press A to open it."),
                EntryKind::Zip => Row::new(name.clone()).value("ZIP").help(
                    "A .zip archive. Press A to open the save inside. Saving rewrites the archive.",
                ),
                EntryKind::MainDat => Row::new(name.clone())
                    .value("Save")
                    .style(Style::Action)
                    .help("The save in this folder. Press A to open it."),
            })
            .collect()
    }

    fn activate(&mut self, _cx: &mut Cx, row: usize) -> Nav {
        let Some((name, kind)) = self.entries.get(row) else {
            return Nav::Stay;
        };
        let path = self.dir.join(name);
        match kind {
            EntryKind::Folder => Nav::Push(Box::new(Browser::new(path))),
            _ => Self::open(path),
        }
    }

    fn a_label(&self, _cx: &Cx, row: usize) -> &'static str {
        match self.entries.get(row).map(|e| e.1) {
            Some(EntryKind::Folder) => "Open folder",
            _ => "Open save",
        }
    }

    fn empty_text(&self) -> &str {
        if self.error.is_some() {
            "This folder cannot be read."
        } else {
            "No folders or saves here."
        }
    }

    fn info(&self, _cx: &Cx, rows: &[Row], row: usize) -> Info {
        if let Some(e) = &self.error {
            return Info {
                title: "Cannot read this folder".into(),
                text: e.clone(),
                facts: Vec::new(),
            };
        }
        let mut i = rows
            .get(row)
            .map(|r| Info {
                title: r.label.clone(),
                text: r.help.clone(),
                facts: Vec::new(),
            })
            .unwrap_or_else(|| Info {
                title: "Find your save".into(),
                text: "Look for a folder with main.dat, mainHeader.dat and Villager0 … Villager7 in it.".into(),
                facts: Vec::new(),
            });
        if let Some((name, EntryKind::SaveFolder)) = self.entries.get(row) {
            let p = self.dir.join(name);
            if let Ok(meta) = std::fs::metadata(p.join("main.dat")) {
                i.facts.push(("main.dat".into(), fmt::size(meta.len())));
            }
        }
        i
    }
}

/// The list of backups.
pub struct Backups;

impl View for Backups {
    fn title(&self, _cx: &Cx) -> String {
        "Backups".into()
    }

    fn rows(&self, cx: &Cx) -> Vec<Row> {
        session::list_backups(cx.platform.as_ref())
            .iter()
            .map(|b| Row::new(&b.town).value(fmt::display_stamp(b.time)))
            .collect()
    }

    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        match session::list_backups(cx.platform.as_ref())
            .into_iter()
            .nth(row)
        {
            Some(b) => Nav::Push(Box::new(BackupMenu { b, user: None })),
            None => Nav::Stay,
        }
    }

    fn empty_text(&self) -> &str {
        "No backups yet. Opening a save makes one."
    }

    fn info(&self, cx: &Cx, _rows: &[Row], row: usize) -> Info {
        match session::list_backups(cx.platform.as_ref())
            .into_iter()
            .nth(row)
        {
            Some(b) => backup_info(&b),
            None => Info::default(),
        }
    }
}

fn backup_info(b: &BackupInfo) -> Info {
    Info {
        title: b.town.clone(),
        text: format!("Folder: {}", b.path.display()),
        facts: vec![
            ("Made".into(), fmt::display_stamp(b.time)),
            ("From".into(), b.source.clone()),
            ("Size".into(), fmt::size(b.size)),
        ],
    }
}

const T_RESTORE: u32 = 1;
const T_DELETE: u32 = 2;

/// What to do with one backup.
pub struct BackupMenu {
    b: BackupInfo,
    user: Option<UserAccount>,
}

impl View for BackupMenu {
    fn title(&self, _cx: &Cx) -> String {
        format!("Backup of {}", self.b.town)
    }

    fn rows(&self, _cx: &Cx) -> Vec<Row> {
        vec![
            Row::new("Restore to the game…").style(Style::Action).help(
                "Replace a user's Animal Crossing save on this console with this backup. What is in the game now is backed up first. Close the game before you do this.",
            ),
            Row::new("Open in the editor").help(
                "Open this backup to look at it or edit it. Saving writes into the backup folder, not the game.",
            ),
            Row::new("Delete this backup").style(Style::Danger).help("Removes the backup folder from the SD card."),
        ]
    }

    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        match row {
            0 => Nav::Push(users_screen(cx, UserPurpose::Restore)),
            1 if cx.dirty() => Nav::message(
                "Save or discard first",
                "The open save has unsaved changes. Save them, or discard them in the editor, then open the backup.",
            ),
            1 => Browser::open(self.b.path.clone()),
            2 => Nav::confirm_danger(
                "Delete this backup?",
                format!("{} — {}", self.b.town, fmt::display_stamp(self.b.time)),
                T_DELETE,
            ),
            _ => Nav::Stay,
        }
    }

    fn info(&self, _cx: &Cx, rows: &[Row], row: usize) -> Info {
        let mut i = backup_info(&self.b);
        if let Some(r) = rows.get(row) {
            i.text = format!("{}\n\n{}", r.help, i.text);
        }
        i
    }

    fn reply(&mut self, cx: &mut Cx, r: Reply) -> Nav {
        match r {
            Reply::User(u) => {
                let text = format!(
                    "Replace the Animal Crossing save of {} with the backup of {} from {}?\n\nWhat is in the game now is backed up first. Animal Crossing must be closed.",
                    u.name,
                    self.b.town,
                    fmt::display_stamp(self.b.time)
                );
                self.user = Some(u);
                Nav::confirm_danger("Restore this backup?", text, T_RESTORE)
            }
            Reply::Confirmed(T_RESTORE) => {
                let Some(user) = self.user.take() else {
                    return Nav::Stay;
                };
                let b = self.b.clone();
                Nav::busy("Restoring…", move |cx| {
                    match session::restore_backup(cx.platform.as_mut(), &b, &user) {
                        Ok(msg) => {
                            cx.toast(msg);
                            Nav::Stay
                        }
                        Err(e) => Nav::message("The restore failed", e),
                    }
                })
            }
            Reply::Confirmed(T_DELETE) => match session::delete_backup(&self.b) {
                Ok(()) => {
                    cx.toast("Backup deleted.");
                    Nav::Pop
                }
                Err(e) => Nav::message("The backup could not be deleted", e),
            },
            _ => Nav::Stay,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn prettify_names() {
        assert_eq!(super::prettify("Kind_BigSister"), "Big Sister");
        assert_eq!(super::prettify("Uchi"), "Uchi");
    }
}
