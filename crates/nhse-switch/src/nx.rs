//! The console: calls into `switch/source/glue.c`, which uses libounce (libnx for Switch 2).
use crate::gfx::{Canvas, HEIGHT, WIDTH};
use crate::platform::{Input, KeyboardMode, KeyboardRequest, Platform, UserAccount};
use std::ffi::{c_char, CStr, CString};
use std::path::PathBuf;

extern "C" {
    fn nhse_frame(down: *mut u32, held: *mut u32) -> i32;
    fn nhse_present(px: *const u32, width: u32, height: u32);
    fn nhse_keyboard(
        header: *const c_char,
        guide: *const c_char,
        initial: *const c_char,
        numeric: i32,
        max_len: i32,
        out: *mut c_char,
        out_size: usize,
    ) -> i32;
    fn nhse_users(uids: *mut u8, names: *mut c_char, max: i32, rc: *mut u32) -> i32;
    fn nhse_mount_save(uid: *const u8, title_id: u64) -> u32;
    fn nhse_commit_save() -> u32;
    fn nhse_unmount_save();
    fn nhse_local_time() -> i64;
    fn nhse_system_dark() -> i32;
}

/// The mount name `nhse_mount_save` uses.
const MOUNT: &str = "acnh:/";

/// A Horizon result code as the console shows it (`2002-1234`).
fn result_code(rc: u32) -> String {
    format!("{:04}-{:04}", 2000 + (rc & 0x1FF), (rc >> 9) & 0x1FFF)
}

fn cstring(s: &str) -> CString {
    CString::new(s.replace('\0', " ")).expect("no NUL left")
}

struct Nx;

impl Platform for Nx {
    fn next_frame(&mut self) -> Option<Input> {
        let (mut down, mut held) = (0, 0);
        (unsafe { nhse_frame(&mut down, &mut held) } != 0).then_some(Input { down, held })
    }

    fn present(&mut self, c: &Canvas) {
        unsafe { nhse_present(c.px.as_ptr(), WIDTH as u32, HEIGHT as u32) }
    }

    fn keyboard(&mut self, req: &KeyboardRequest) -> Option<String> {
        let (h, g, i) = (
            cstring(req.header),
            cstring(req.guide),
            cstring(req.initial),
        );
        let mut out = vec![0 as c_char; 512];
        let r = unsafe {
            nhse_keyboard(
                h.as_ptr(),
                g.as_ptr(),
                i.as_ptr(),
                (req.mode == KeyboardMode::Number) as i32,
                req.max_len as i32,
                out.as_mut_ptr(),
                out.len(),
            )
        };
        (r == 1).then(|| {
            unsafe { CStr::from_ptr(out.as_ptr()) }
                .to_string_lossy()
                .into_owned()
        })
    }

    fn users(&mut self) -> Result<Vec<UserAccount>, String> {
        const MAX: usize = 12;
        let mut uids = [0u8; 16 * MAX];
        let mut names = [0 as c_char; 33 * MAX];
        let mut rc = 0u32;
        let n = unsafe { nhse_users(uids.as_mut_ptr(), names.as_mut_ptr(), MAX as i32, &mut rc) };
        if n < 0 {
            return Err(format!(
                "The user accounts could not be read ({}).",
                result_code(rc)
            ));
        }
        Ok((0..n as usize)
            .map(|i| {
                let mut uid = [0u8; 16];
                uid.copy_from_slice(&uids[i * 16..i * 16 + 16]);
                let name = unsafe { CStr::from_ptr(names[i * 33..].as_ptr()) };
                UserAccount {
                    uid,
                    name: name.to_string_lossy().into_owned(),
                }
            })
            .collect())
    }

    fn mount_game_save(&mut self, user: &UserAccount) -> Result<PathBuf, String> {
        let rc = unsafe { nhse_mount_save(user.uid.as_ptr(), crate::session::GAME_TITLE_ID) };
        match rc {
            0 => Ok(PathBuf::from(MOUNT)),
            _ => Err(format!(
                "The Animal Crossing save of {} could not be opened ({}). Close the game and try again. A user who has never played has no save.",
                user.name,
                result_code(rc)
            )),
        }
    }

    fn commit_game_save(&mut self) -> Result<(), String> {
        match unsafe { nhse_commit_save() } {
            0 => Ok(()),
            rc => Err(format!(
                "The save data could not be committed ({}).",
                result_code(rc)
            )),
        }
    }

    fn unmount_game_save(&mut self) {
        unsafe { nhse_unmount_save() }
    }

    fn data_dir(&self) -> PathBuf {
        PathBuf::from("sdmc:/switch/nhse")
    }

    fn sd_root(&self) -> PathBuf {
        PathBuf::from("sdmc:/")
    }

    fn local_time(&self) -> i64 {
        unsafe { nhse_local_time() }
    }

    fn system_dark(&mut self) -> bool {
        unsafe { nhse_system_dark() == 1 }
    }
}

/// Called by `main` in glue.c once the console services are up.
#[no_mangle]
pub extern "C" fn nhse_main() -> i32 {
    std::panic::set_hook(Box::new(|info| {
        // the app aborts after a panic: leave a note on the SD card
        let _ = std::fs::create_dir_all("sdmc:/switch/nhse");
        let _ = std::fs::write(
            "sdmc:/switch/nhse/crash.txt",
            format!("NHSE {}: {info}\n", env!("CARGO_PKG_VERSION")),
        );
    }));
    let _ = std::fs::create_dir_all("sdmc:/switch/nhse");
    crate::app(Box::new(Nx)).run();
    0
}
