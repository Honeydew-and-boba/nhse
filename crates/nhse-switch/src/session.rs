//! Opening, saving, backing up and restoring saves, from the SD card or the game itself.
//!
//! A game save is never edited in place. When it is opened, its files are copied to a working
//! folder on the SD card (and backed up), and the save data is unmounted again. Saving writes the
//! working folder, then copies the edited files into the game's save data and commits them in one
//! transaction: if anything fails, nothing is committed and the game's save stays as it was.
use crate::fmt;
use crate::platform::{Platform, UserAccount};
use nhse_core::save::provider::{create_dir_all, path_exists, path_len, read_file, read_text};
use nhse_core::save::SaveSource;
use nhse_core::HorizonSave;
use std::fs;
use std::path::{Path, PathBuf};

/// Animal Crossing: New Horizons.
pub const GAME_TITLE_ID: u64 = 0x0100_6F80_0232_6000;

/// Backups kept before the oldest are removed.
pub const KEEP_BACKUPS: usize = 30;

#[derive(Debug, Clone)]
pub enum Origin {
    /// A folder or .zip on the SD card.
    Sd,
    /// The game's own save data of a user.
    Game(UserAccount),
}

pub struct Session {
    pub save: HorizonSave,
    pub origin: Origin,
    pub dirty: bool,
    /// The backup made when the save was opened (None when an identical one existed).
    pub backup: Option<PathBuf>,
    /// Content hash of each decrypted file as the game has it, to write back only what changed.
    written: Vec<(String, u64)>,
}

/// FNV-1a of a file's decrypted data, keyed by its path.
fn content_hashes(s: &HorizonSave) -> Vec<(String, u64)> {
    let hash = |d: &[u8]| {
        d.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3)
        })
    };
    std::iter::once(&s.main)
        .chain(s.players.iter().flat_map(|p| p.files()))
        .map(|f| (f.data_path(), hash(&f.data)))
        .collect()
}

/// The files (data and header) whose content differs from `before`.
fn changed_files(s: &HorizonSave, before: &[(String, u64)]) -> Vec<String> {
    let now = content_hashes(s);
    let mut out = Vec::new();
    for f in std::iter::once(&s.main).chain(s.players.iter().flat_map(|p| p.files())) {
        let path = f.data_path();
        let h = now.iter().find(|(p, _)| *p == path).map(|x| x.1);
        let old = before.iter().find(|(p, _)| *p == path).map(|x| x.1);
        if h != old {
            out.push(f.header_path());
            out.push(path);
        }
    }
    out
}

impl Session {
    pub fn town(&self) -> String {
        self.save.town_name()
    }

    /// Where the save comes from, for the user.
    pub fn location(&self) -> String {
        match &self.origin {
            Origin::Sd => self.save.source.location().display().to_string(),
            Origin::Game(u) => format!("Game save of {}", u.name),
        }
    }
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub fn backups_dir(p: &dyn Platform) -> PathBuf {
    p.data_dir().join("backups")
}

fn work_dir(p: &dyn Platform, user: &UserAccount) -> PathBuf {
    p.data_dir().join("game").join(user.uid_hex())
}

/// FNV-1a over every save file (names and contents): identical saves get identical
/// fingerprints, and a change to any file, a player's included, changes it.
fn fingerprint(src: &SaveSource) -> Result<String, String> {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for f in src.save_files().map_err(err)? {
        for b in f.bytes().chain(src.read(&f).map_err(err)?) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    Ok(format!("{h:016x}"))
}

/// A backup folder (`backups/<Town> <date>`): the save files, openable by the desktop NHSE.
#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub path: PathBuf,
    pub name: String,
    pub town: String,
    pub source: String,
    pub time: i64,
    pub fingerprint: String,
    pub size: u64,
}

const BACKUP_INFO: &str = "nhse-backup.txt";

/// Backups, newest first.
pub fn list_backups(p: &dyn Platform) -> Vec<BackupInfo> {
    let mut out = Vec::new();
    let root = backups_dir(p);
    let Ok(entries) = p.game_entries(&root) else {
        return out;
    };
    for (name, directory) in entries {
        if !directory || name.ends_with(".part") {
            continue;
        }
        let path = root.join(&name);
        let Ok(info) = read_text(&path.join(BACKUP_INFO)) else {
            continue;
        };
        let get = |k: &str| {
            info.lines()
                .find_map(|l| l.strip_prefix(&format!("{k}=")))
                .unwrap_or("")
                .to_string()
        };
        out.push(BackupInfo {
            name,
            town: get("town"),
            source: get("source"),
            time: get("time").parse().unwrap_or(0),
            fingerprint: get("fingerprint"),
            size: dir_size(p, &path),
            path,
        });
    }
    out.sort_by(|a, b| b.time.cmp(&a.time).then(b.name.cmp(&a.name)));
    out
}

fn dir_size(platform: &dyn Platform, path: &Path) -> u64 {
    let Ok(entries) = platform.game_entries(path) else {
        return 0;
    };
    entries
        .into_iter()
        .map(|(name, directory)| {
            let p = path.join(name);
            if directory {
                dir_size(platform, &p)
            } else {
                path_len(&p).unwrap_or(0)
            }
        })
        .sum()
}

/// Backs up the save files of `src`. Returns `None` when an independent backup has identical contents.
pub fn create_backup(
    p: &dyn Platform,
    src: &SaveSource,
    town: &str,
    source_label: &str,
    protected: &Path,
) -> Result<Option<PathBuf>, String> {
    let fp = fingerprint(src)?;
    let existing = list_backups(p);
    for b in &existing {
        if b.path != src.location()
            && b.fingerprint == fp
            && fingerprint(&SaveSource::Folder(b.path.clone()))
                .ok()
                .as_ref()
                == Some(&fp)
        {
            return Ok(None);
        }
    }
    let now = p.local_time();
    let safe = nhse_core::util::strings::clean_file_name(town);
    let safe = if safe.trim().is_empty() {
        "Island".into()
    } else {
        safe
    };
    let mut dir = backups_dir(p).join(format!("{safe} {}", fmt::file_stamp(now)));
    let mut k = 2;
    while path_exists(&dir) {
        dir = backups_dir(p).join(format!("{safe} {} ({k})", fmt::file_stamp(now)));
        k += 1;
    }
    let tmp = dir.with_file_name(format!(
        "{}.part",
        dir.file_name().unwrap().to_string_lossy()
    ));
    p.clear_backup_dir(&tmp)?;
    let res = (|| -> Result<(), String> {
        for rel in src.save_files().map_err(err)? {
            let dst = tmp.join(&rel);
            create_dir_all(dst.parent().unwrap_or(&tmp)).map_err(err)?;
            fs::write(&dst, src.read(&rel).map_err(err)?).map_err(err)?;
        }
        fs::write(
            tmp.join(BACKUP_INFO),
            format!("town={town}\nsource={source_label}\ntime={now}\nfingerprint={fp}\n"),
        )
        .map_err(err)?;
        fs::rename(&tmp, &dir).map_err(err)
    })();
    if let Err(e) = res {
        let _ = p.clear_backup_dir(&tmp);
        return Err(format!("The backup failed: {e}"));
    }
    // keep the newest backups
    let keep_old =
        KEEP_BACKUPS.saturating_sub(1 + usize::from(existing.iter().any(|b| b.path == protected)));
    for old in existing
        .iter()
        .filter(|b| b.path != protected)
        .skip(keep_old)
    {
        let _ = p.clear_backup_dir(&old.path);
    }
    Ok(Some(dir))
}

pub fn delete_backup(p: &dyn Platform, b: &BackupInfo) -> Result<(), String> {
    p.clear_backup_dir(&b.path)
}

/// Opens a save folder, `main.dat` or `.zip` from the SD card (backing it up first).
pub fn open_sd(p: &dyn Platform, path: &Path) -> Result<Session, String> {
    let save = HorizonSave::open(path).map_err(err)?;
    let backup = create_backup(
        p,
        &save.source,
        &save.town_name(),
        &path.display().to_string(),
        save.source.location(),
    )?;
    Ok(Session {
        written: content_hashes(&save),
        save,
        origin: Origin::Sd,
        dirty: false,
        backup,
    })
}

/// Copies every file under `from` into `to` (created as needed). Returns the bytes copied.
fn copy_tree(p: &dyn Platform, from: &Path, to: &Path) -> Result<u64, String> {
    create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let mut n = 0;
    for (name, directory) in p.game_entries(from)? {
        let src = from.join(&name);
        let dst = to.join(&name);
        if directory {
            n += copy_tree(p, &src, &dst)?;
        } else {
            let data = read_file(&src).map_err(|e| format!("{}: {e}", src.display()))?;
            n += data.len() as u64;
            fs::write(&dst, data).map_err(|e| format!("{}: {e}", dst.display()))?;
        }
    }
    Ok(n)
}

/// Copies the game's save of `user` to the working folder (and a backup) and opens it.
pub fn open_game(p: &mut dyn Platform, user: &UserAccount) -> Result<Session, String> {
    let work = work_dir(p, user);
    p.clear_work_dir(&work)?;
    let root = p.mount_game_save(user)?;
    let copied = copy_tree(p, &root, &work);
    p.unmount_game_save();
    copied.map_err(|e| format!("Could not copy the game's save to the SD card: {e}"))?;
    let entries = p.game_entries(&work)?;
    if !entries
        .iter()
        .any(|(name, directory)| name == "main.dat" && !directory)
    {
        let entries = entries
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "The mounted island save has no main.dat. Files found: {}.",
            if entries.is_empty() { "none" } else { &entries }
        ));
    }
    let save = HorizonSave::open(&work).map_err(err)?;
    let backup = create_backup(
        p,
        &save.source,
        &save.town_name(),
        &format!("Game save of {}", user.name),
        &work,
    )?;
    Ok(Session {
        written: content_hashes(&save),
        save,
        origin: Origin::Game(user.clone()),
        dirty: false,
        backup,
    })
}

/// Copies `files` (relative paths) from `from` into the game's save of `user` and commits them.
/// On any error nothing is committed.
fn write_game_files(
    p: &mut dyn Platform,
    user: &UserAccount,
    from: &Path,
    files: &[String],
) -> Result<(), String> {
    let root = p.mount_game_save(user)?;
    let res = (|| -> Result<(), String> {
        for rel in files {
            let data = read_file(&from.join(rel)).map_err(|e| format!("{rel}: {e}"))?;
            let dst = root.join(rel);
            if let Some(parent) = dst.parent() {
                create_dir_all(parent).map_err(|e| format!("{rel}: {e}"))?;
            }
            fs::write(&dst, &data).map_err(|e| format!("{rel}: {e}"))?;
        }
        p.commit_game_save()
    })();
    p.unmount_game_save();
    res.map_err(|e| format!("Writing the game's save failed, so it was not changed.\n{e}"))
}

fn seed(p: &dyn Platform) -> u32 {
    let t = p.local_time() as u64;
    ((t.wrapping_mul(0x9E37_79B9_7F4A_7C15)) >> 32) as u32 ^ 0x5DEE_CE66
}

/// Writes the edits. Returns a message for the user.
pub fn save(p: &mut dyn Platform, s: &mut Session) -> Result<String, String> {
    let seed = seed(p);
    s.save.save(seed).map_err(err)?;
    match s.origin.clone() {
        Origin::Sd => {
            s.dirty = false;
            Ok(format!("Saved to {}", s.save.source.location().display()))
        }
        Origin::Game(user) => {
            // each data/header pair stands alone, so only changed pairs go to the save data:
            // this keeps the write well inside the save data's journal
            let files = changed_files(&s.save, &s.written);
            let work = s.save.source.location().to_path_buf();
            if !files.is_empty() {
                write_game_files(p, &user, &work, &files)?;
            }
            s.written = content_hashes(&s.save);
            s.dirty = false;
            Ok(format!("Saved to the game. {} can play now.", user.name))
        }
    }
}

/// Re-reads the save, discarding unsaved edits.
pub fn revert(p: &mut dyn Platform, s: &mut Session) -> Result<(), String> {
    if let Origin::Game(user) = &s.origin {
        *s = open_game(p, user)?;
        return Ok(());
    }
    let path = s.save.source.location().to_path_buf();
    s.save = HorizonSave::open(&path).map_err(err)?;
    s.written = content_hashes(&s.save);
    s.dirty = false;
    Ok(())
}

/// Replaces the game's save of `user` with a backup (backing up the current one first).
pub fn restore_backup(
    p: &mut dyn Platform,
    b: &BackupInfo,
    user: &UserAccount,
) -> Result<String, String> {
    let src = SaveSource::folder(&b.path).map_err(err)?;
    // the backup must be a complete, readable save
    HorizonSave::from_source(SaveSource::folder(&b.path).map_err(err)?)
        .map_err(|e| format!("This backup cannot be restored: {e}"))?;
    let files = src.save_files().map_err(err)?;
    // back up what is in the game now, if there is a save
    let work = work_dir(p, user);
    p.clear_work_dir(&work)?;
    let root = p.mount_game_save(user)?;
    let copied = copy_tree(p, &root, &work);
    p.unmount_game_save();
    copied.map_err(|e| format!("Could not back up the current game save: {e}"))?;
    if !p.game_entries(&work)?.is_empty() {
        let cur = HorizonSave::open(&work)
            .map_err(|e| format!("Current game save could not be read; restore stopped: {e}"))?;
        if cur.source.save_files().map_err(err)? != files {
            return Err("The backup and current save have different file sets; restore stopped to avoid leaving stale game files.".into());
        }
        create_backup(
            p,
            &cur.source,
            &cur.town_name(),
            &format!("Game save of {} (before a restore)", user.name),
            &b.path,
        )?;
    }
    write_game_files(p, user, &b.path, &files)?;
    Ok(format!(
        "Restored {} to the game save of {}.",
        b.town, user.name
    ))
}
