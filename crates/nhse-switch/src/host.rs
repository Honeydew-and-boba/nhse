//! The app on a desktop: scripted input, PNG screenshots and folders standing in for the SD card
//! and the console's save data. Used by `examples/preview.rs` and the tests.
use crate::gfx::{Canvas, HEIGHT, WIDTH};
use crate::platform::{Input, KeyboardRequest, Platform, UserAccount};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// One frame of a script.
#[derive(Debug, Clone)]
pub enum Step {
    /// Press (and release) buttons.
    Press(u32),
    /// Hold buttons for a number of frames.
    Hold(u32, u32),
    /// Saves a screenshot after this frame.
    Shot(String),
    /// Idle frames.
    Wait(u32),
}

pub struct HostPlatform {
    root: PathBuf,
    script: VecDeque<Step>,
    /// Answers for the software keyboard, in order. `None` cancels.
    pub keyboard: VecDeque<Option<String>>,
    users: Vec<UserAccount>,
    shots: PathBuf,
    pending_shot: Option<String>,
    hold: Option<(u32, u32)>,
    mounted: Option<(PathBuf, PathBuf)>,
    /// Fails the next commit (to test that the game save stays untouched).
    pub fail_commit: bool,
    /// The console theme to report.
    pub system_dark: bool,
    time: i64,
}

/// Copies a folder tree.
pub fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()))?;
        } else {
            std::fs::copy(&p, to.join(e.file_name()))?;
        }
    }
    Ok(())
}

impl HostPlatform {
    /// `root/sd` is the SD card, `root/data` the app folder and `root/game/<user>` the
    /// console's save data of each user.
    pub fn new(root: &Path, users: &[&str], script: Vec<Step>, shots: &Path) -> Self {
        let users = users
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let mut uid = [0u8; 16];
                uid[0] = i as u8 + 1;
                uid[15] = 0xA5;
                UserAccount {
                    uid,
                    name: n.to_string(),
                }
            })
            .collect();
        Self {
            root: root.to_path_buf(),
            script: script.into(),
            keyboard: VecDeque::new(),
            users,
            shots: shots.to_path_buf(),
            pending_shot: None,
            hold: None,
            mounted: None,
            fail_commit: false,
            system_dark: false,
            time: 1_763_112_333,
        }
    }

    /// The folder holding the game save of a user.
    pub fn game_dir(root: &Path, user_name: &str) -> PathBuf {
        root.join("game").join(user_name)
    }

    fn write_png(&self, c: &Canvas, name: &str) {
        let _ = std::fs::create_dir_all(&self.shots);
        save_png(c, &self.shots.join(format!("{name}.png")));
    }
}

/// Writes a frame as a PNG.
pub fn save_png(c: &Canvas, path: &Path) {
    let f = std::fs::File::create(path).expect("screenshot file");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), WIDTH as u32, HEIGHT as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().expect("png header");
    let mut data = Vec::with_capacity(WIDTH * HEIGHT * 3);
    for p in &c.px {
        data.extend_from_slice(&[(*p & 0xFF) as u8, (*p >> 8) as u8, (*p >> 16) as u8]);
    }
    w.write_image_data(&data).expect("png data");
}

impl Platform for HostPlatform {
    fn next_frame(&mut self) -> Option<Input> {
        self.time += 1;
        if let Some((b, left)) = self.hold {
            self.hold = (left > 1).then_some((b, left - 1));
            return Some(Input { down: 0, held: b });
        }
        let step = self.script.pop_front()?;
        Some(match step {
            Step::Press(b) => Input { down: b, held: b },
            Step::Hold(b, n) => {
                self.hold = (n > 1).then_some((b, n - 1));
                Input { down: b, held: b }
            }
            Step::Shot(name) => {
                self.pending_shot = Some(name);
                Input::default()
            }
            Step::Wait(n) => {
                for _ in 1..n {
                    self.script.push_front(Step::Wait(1));
                }
                Input::default()
            }
        })
    }

    fn present(&mut self, c: &Canvas) {
        if let Some(name) = self.pending_shot.take() {
            self.write_png(c, &name);
        }
    }

    fn keyboard(&mut self, req: &KeyboardRequest) -> Option<String> {
        self.keyboard
            .pop_front()
            .unwrap_or_else(|| panic!("no scripted keyboard answer for \"{}\"", req.header))
    }

    fn users(&mut self) -> Result<Vec<UserAccount>, String> {
        Ok(self.users.clone())
    }

    fn mount_game_save(&mut self, user: &UserAccount) -> Result<PathBuf, String> {
        assert!(self.mounted.is_none(), "save data mounted twice");
        let real = Self::game_dir(&self.root, &user.name);
        if !real.is_dir() {
            return Err(format!("{} has no save data for the game.", user.name));
        }
        // like the console: changes land in a journal until they are committed
        let staging = self.root.join("mount");
        let _ = std::fs::remove_dir_all(&staging);
        copy_dir(&real, &staging).map_err(|e| e.to_string())?;
        self.mounted = Some((real, staging.clone()));
        Ok(staging)
    }

    fn commit_game_save(&mut self) -> Result<(), String> {
        if self.fail_commit {
            return Err("2002-0001 (simulated)".into());
        }
        let (real, staging) = self.mounted.as_ref().ok_or("not mounted")?;
        let _ = std::fs::remove_dir_all(real);
        copy_dir(staging, real).map_err(|e| e.to_string())
    }

    fn unmount_game_save(&mut self) {
        if let Some((_, staging)) = self.mounted.take() {
            let _ = std::fs::remove_dir_all(staging);
        }
    }

    fn data_dir(&self) -> PathBuf {
        self.root.join("sd").join("switch").join("nhse")
    }

    fn sd_root(&self) -> PathBuf {
        self.root.join("sd")
    }

    fn local_time(&self) -> i64 {
        self.time
    }

    fn system_dark(&mut self) -> bool {
        self.system_dark
    }
}
