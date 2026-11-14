//! What the app needs from the system: frames, input, the software keyboard and the game's save.
//!
//! `nx.rs` implements it on the console (through `switch/source/glue.c`), `host.rs` on a
//! desktop for the preview and the tests.
use crate::gfx::Canvas;
use std::path::PathBuf;

/// Buttons, as bits of [`Input`]. The left stick also moves like the D-pad.
pub mod btn {
    pub const A: u32 = 1 << 0;
    pub const B: u32 = 1 << 1;
    pub const X: u32 = 1 << 2;
    pub const Y: u32 = 1 << 3;
    pub const L: u32 = 1 << 4;
    pub const R: u32 = 1 << 5;
    pub const ZL: u32 = 1 << 6;
    pub const ZR: u32 = 1 << 7;
    pub const PLUS: u32 = 1 << 8;
    pub const MINUS: u32 = 1 << 9;
    pub const UP: u32 = 1 << 10;
    pub const DOWN: u32 = 1 << 11;
    pub const LEFT: u32 = 1 << 12;
    pub const RIGHT: u32 = 1 << 13;

    pub const DIRECTIONS: u32 = UP | DOWN | LEFT | RIGHT;
}

/// Buttons pressed this frame (`down`) and held (`held`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Input {
    pub down: u32,
    pub held: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardMode {
    Text,
    Number,
}

/// A request for the system's software keyboard.
pub struct KeyboardRequest<'a> {
    pub header: &'a str,
    pub guide: &'a str,
    pub initial: &'a str,
    pub mode: KeyboardMode,
    pub max_len: usize,
}

/// A user account of the console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserAccount {
    pub uid: [u8; 16],
    pub name: String,
}

impl UserAccount {
    pub fn uid_hex(&self) -> String {
        self.uid.iter().map(|b| format!("{b:02x}")).collect()
    }
}

pub trait Platform {
    /// Waits for the next frame. `None` when the app must close (HOME menu exit, …).
    fn next_frame(&mut self) -> Option<Input>;
    /// Shows a finished frame.
    fn present(&mut self, canvas: &Canvas);
    /// Shows the software keyboard. `None` when cancelled.
    fn keyboard(&mut self, req: &KeyboardRequest) -> Option<String>;
    /// The console's user accounts.
    fn users(&mut self) -> Result<Vec<UserAccount>, String>;
    /// Mounts Animal Crossing's save data of `user` and returns its root folder. Writes only
    /// take effect after [`Platform::commit_game_save`].
    fn mount_game_save(&mut self, user: &UserAccount) -> Result<PathBuf, String>;
    fn commit_game_save(&mut self) -> Result<(), String>;
    fn unmount_game_save(&mut self);
    /// The app's own folder (backups, working copies).
    fn data_dir(&self) -> PathBuf;
    /// Where the SD card browser starts.
    fn sd_root(&self) -> PathBuf;
    /// Seconds since 1970 in the console's local time zone.
    fn local_time(&self) -> i64;
    /// Whether the console's theme (System Settings → Themes) is dark.
    fn system_dark(&mut self) -> bool;
}
