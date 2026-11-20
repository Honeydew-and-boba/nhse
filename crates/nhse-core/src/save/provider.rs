//! Storage backends for a save: an unpacked folder, or a ZIP archive.
use crate::error::{Error, Result};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[cfg(target_os = "horizon")]
fn path_kind(path: &Path) -> i32 {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    extern "C" {
        fn nhse_path_kind(path: *const std::ffi::c_char) -> i32;
    }
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return 0;
    };
    unsafe { nhse_path_kind(path.as_ptr()) }
}

#[cfg(not(target_os = "horizon"))]
fn path_kind(path: &Path) -> i32 {
    if path.is_file() {
        1
    } else if path.is_dir() {
        2
    } else {
        0
    }
}

pub fn path_is_file(path: &Path) -> bool {
    path_kind(path) == 1
}
pub fn path_is_dir(path: &Path) -> bool {
    path_kind(path) == 2
}
pub fn path_exists(path: &Path) -> bool {
    path_kind(path) != 0
}

/// Rust's recursive mkdir checks `Path::is_dir`, which uses the wrong Horizon stat layout.
pub fn create_dir_all(path: &Path) -> std::io::Result<()> {
    if path.as_os_str().is_empty() || path.parent().is_none() {
        return Ok(());
    }
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && path_is_dir(path) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            create_dir_all(path.parent().unwrap())?;
            match fs::create_dir(path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && path_is_dir(path) => {
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

/// Read without Rust's file-size hint, whose stat layout is wrong on Horizon.
pub fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let mut out = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        match file.read(&mut chunk) {
            Ok(0) => return Ok(out),
            Ok(n) => {
                out.try_reserve(n).map_err(std::io::Error::other)?;
                out.extend_from_slice(&chunk[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
}

pub fn read_text(path: &Path) -> std::io::Result<String> {
    String::from_utf8(read_file(path)?).map_err(std::io::Error::other)
}

#[cfg(target_os = "horizon")]
pub fn path_len(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    extern "C" {
        fn nhse_path_size(path: *const std::ffi::c_char, size: *mut u64) -> i32;
    }
    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut size = 0;
    (unsafe { nhse_path_size(path.as_ptr(), &mut size) } == 0).then_some(size)
}

#[cfg(not(target_os = "horizon"))]
pub fn path_len(path: &Path) -> Option<u64> {
    fs::metadata(path).ok().map(|m| m.len())
}

/// Where a save was loaded from, and where it is written back to.
#[derive(Debug)]
pub enum SaveSource {
    /// Folder containing `main.dat`, `mainHeader.dat` and `Villager*` sub folders.
    Folder(PathBuf),
    /// ZIP archive held entirely in memory; rebuilt on flush.
    Zip {
        path: PathBuf,
        /// All entries of the archive keyed by normalized path.
        files: BTreeMap<String, Vec<u8>>,
        /// Prefix (e.g. `"savedata/"`) of the folder containing `main.dat` inside the archive.
        root: String,
    },
}

fn normalize(p: &str) -> String {
    p.replace('\\', "/").trim_start_matches('/').to_string()
}

impl SaveSource {
    pub fn folder(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if !path_is_file(&path.join("main.dat")) {
            return Err(Error::MissingFile(format!(
                "main.dat not found in {}",
                path.display()
            )));
        }
        Ok(SaveSource::Folder(path))
    }

    /// Opens a ZIP. The save may sit at the archive root or inside any sub folder.
    pub fn zip(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let file = fs::File::open(&path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut files = BTreeMap::new();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if entry.is_dir() {
                continue;
            }
            let name = normalize(entry.name());
            // Skip macOS resource fork junk.
            if name.starts_with("__MACOSX/")
                || name.rsplit('/').next().is_some_and(|n| n.starts_with("._"))
            {
                continue;
            }
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf)?;
            files.insert(name, buf);
        }
        // Locate the shallowest main.dat.
        let root = files
            .keys()
            .filter(|k| k.rsplit('/').next() == Some("main.dat"))
            .min_by_key(|k| k.matches('/').count())
            .map(|k| k[..k.len() - "main.dat".len()].to_string())
            .ok_or_else(|| {
                Error::MissingFile("main.dat not found inside the ZIP archive".into())
            })?;
        Ok(SaveSource::Zip { path, files, root })
    }

    /// Human readable location.
    pub fn location(&self) -> &Path {
        match self {
            SaveSource::Folder(p) => p,
            SaveSource::Zip { path, .. } => path,
        }
    }

    pub fn is_zip(&self) -> bool {
        matches!(self, SaveSource::Zip { .. })
    }

    pub fn read(&self, rel: &str) -> Result<Vec<u8>> {
        match self {
            SaveSource::Folder(root) => {
                let p = root.join(rel);
                read_file(&p).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        Error::MissingFile(rel.to_string())
                    } else {
                        Error::Io(e)
                    }
                })
            }
            SaveSource::Zip { files, root, .. } => files
                .get(&format!("{root}{}", normalize(rel)))
                .cloned()
                .ok_or_else(|| Error::MissingFile(rel.to_string())),
        }
    }

    pub fn exists(&self, rel: &str) -> bool {
        match self {
            SaveSource::Folder(root) => path_is_file(&root.join(rel)),
            SaveSource::Zip { files, root, .. } => {
                files.contains_key(&format!("{root}{}", normalize(rel)))
            }
        }
    }

    pub fn write(&mut self, rel: &str, data: &[u8]) -> Result<()> {
        match self {
            SaveSource::Folder(root) => {
                let p = root.join(rel);
                write_atomic(&p, data)
            }
            SaveSource::Zip { files, root, .. } => {
                files.insert(format!("{root}{}", normalize(rel)), data.to_vec());
                Ok(())
            }
        }
    }

    /// Player folders (`Villager0` … `Villager7`), sorted numerically.
    pub fn player_dirs(&self) -> Result<Vec<String>> {
        let mut dirs: Vec<String> = match self {
            SaveSource::Folder(_root) => {
                #[cfg(target_os = "horizon")]
                {
                    // Horizon's Rust dirent layout truncates names; ACNH has Villager0–7.
                    (0..8).map(|i| format!("Villager{i}")).collect()
                }
                #[cfg(not(target_os = "horizon"))]
                {
                    fs::read_dir(_root)?
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                        .filter_map(|e| e.file_name().to_str().map(str::to_string))
                        .filter(|n| is_player_dir(n))
                        .collect()
                }
            }
            SaveSource::Zip { files, root, .. } => {
                let mut set = std::collections::BTreeSet::new();
                for k in files.keys() {
                    if let Some(rest) = k.strip_prefix(root.as_str()) {
                        if let Some((dir, _)) = rest.split_once('/') {
                            if is_player_dir(dir) {
                                set.insert(dir.to_string());
                            }
                        }
                    }
                }
                set.into_iter().collect()
            }
        };
        dirs.sort_by_key(|d| player_dir_index(d));
        // Only keep folders that actually hold a player.
        dirs.retain(|d| {
            self.exists(&format!("{d}/personal.dat"))
                && self.exists(&format!("{d}/personalHeader.dat"))
        });
        Ok(dirs)
    }

    /// Persists pending writes (rebuilds the archive for ZIP sources).
    pub fn flush(&mut self) -> Result<()> {
        if let SaveSource::Zip { path, files, .. } = self {
            let tmp = path.with_extension("zip.tmp");
            {
                let f = fs::File::create(&tmp)?;
                let mut w = zip::ZipWriter::new(f);
                let opts = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated)
                    .compression_level(Some(6));
                for (k, v) in files.iter() {
                    w.start_file(k.as_str(), opts)?;
                    w.write_all(v)?;
                }
                w.finish()?;
            }
            if let Err(e) = rename_replace(&tmp, path) {
                let _ = fs::remove_file(&tmp);
                return Err(e.into());
            }
        }
        Ok(())
    }

    /// All files that make up the save (relative paths), used for backups.
    pub fn save_files(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for name in ["main.dat", "mainHeader.dat"] {
            if self.exists(name) {
                out.push(name.to_string());
            }
        }
        for d in self.player_dirs()? {
            for f in PLAYER_FILES {
                for suffix in [".dat", "Header.dat"] {
                    let rel = format!("{d}/{f}{suffix}");
                    if self.exists(&rel) {
                        out.push(rel);
                    }
                }
            }
        }
        Ok(out)
    }
}

/// Files stored per player folder.
pub const PLAYER_FILES: [&str; 5] = [
    "personal",
    "photo_studio_island",
    "postbox",
    "profile",
    "wherearen",
];

fn is_player_dir(name: &str) -> bool {
    name.len() > 8
        && name.as_bytes()[..8].eq_ignore_ascii_case(b"villager")
        && name[8..].chars().all(|c| c.is_ascii_digit())
}

fn player_dir_index(name: &str) -> u32 {
    name[8..].parse().unwrap_or(u32::MAX)
}

/// Writes via a temporary file + rename so a crash never leaves a half written save.
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let tmp = path.with_extension("nhse-tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    rename_replace(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        Error::Io(e)
    })
}

/// Renames `from` onto `to`, replacing `to`. Some file systems (the Switch's SD card and save
/// data) cannot rename onto an existing file; the data is complete in `from` at this point, so
/// the old file is removed and the rename retried.
fn rename_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    match fs::rename(from, to) {
        Err(_) if path_is_file(to) => {
            fs::remove_file(to)?;
            fs::rename(from, to)
        }
        r => r,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_across_chunks_without_a_size_hint() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.dat");
        let data = (0..131_073).map(|i| i as u8).collect::<Vec<_>>();
        std::fs::write(&path, &data).unwrap();
        assert_eq!(super::read_file(&path).unwrap(), data);
    }

    #[test]
    fn skips_unrelated_unicode_folders() {
        assert!(!super::is_player_dir("1234567島"));
        assert!(super::is_player_dir("Villager0"));
    }

    #[test]
    fn creates_existing_and_nested_directories_but_rejects_a_file() {
        let dir = tempfile::tempdir().unwrap();
        super::create_dir_all(dir.path()).unwrap();
        let nested = dir.path().join("a/b");
        super::create_dir_all(&nested).unwrap();
        super::create_dir_all(&nested).unwrap();
        let file = nested.join("file");
        std::fs::write(&file, b"x").unwrap();
        assert!(super::create_dir_all(&file).is_err());
    }
}
