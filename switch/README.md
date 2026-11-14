NHSE for Nintendo Switch 2
==========================

The save editor as a homebrew app (`nhse.nro`) that runs on the Switch 2 itself. It is built with
[ounce-devkit](https://github.com/ouncebrew/ounce-devkit) (libounce, a libnx for Switch 2) and
reads and writes saves with `nhse-core`, the same library as the desktop app.

It can edit the game's own save directly: no dumping or copying to a computer.

## Install

1. Get `nhse.nro` from the [latest release](https://github.com/cedricroulof/nhse/releases/latest),
   the `nightly-switch2` pre-release (latest code), or build it (below).
2. Copy it to `/switch/` on the SD card.
3. Open it from the homebrew menu.

## Use

Close Animal Crossing first (press HOME, then X on the game), then choose
**Animal Crossing on this console** and the user who plays. The save is copied to the SD card and
backed up, and you edit that copy. Press **+** to save: the changed files are written into the
game's save data and committed in one go. If anything fails while writing, nothing is committed and
the game's save stays as it was.

You can also open a save from the SD card (a folder with `main.dat`, `main.dat` itself, or a `.zip`).

| Button | Action |
| --- | --- |
| D-pad / left stick | Move (hold to repeat) |
| L / R, left / right | Page up / down |
| ZL / ZR | First / last row |
| A | Select, change a value |
| B | Back |
| + | Save (exits on the start screen) |
| Y / X / − | In pockets and storage: copy / paste / clear a slot |

Numbers and item searches use the system keyboard.

**Appearance** on the start screen switches between light and dark colors (the desktop app's), or
**System**, which follows the console's theme. The choice is remembered.

### What it edits

- **Residents:** wallet, bank, Nook Miles (current and total), hotel tickets, Poki; pockets and home
  storage (search any item by name or id, quantity, color variant, raw value; copy and paste
  slots); learn every DIY recipe; complete the catalog; learn every reaction.
- **Island:** turnip prices, villagers (ask to move out, or to stay), donate everything to the
  museum, remove weeds / branches / shells, fill holes, water every flower, empty the recycle bin.
- **Saves:** check hashes, discard changes, backups (restore one to the game, open, delete).

The bulk actions behave like the desktop app's: story recipes (bridge and campsite kits) and catalog
entries that crash Nook Shopping are left out.

The map editor, villager replacement, designs and house editing are desktop only for now.

### Files on the SD card

- `/switch/nhse/backups/<Island> <date>/`: a backup of each save you open (the 30 newest are kept).
  Each is a plain save folder, so the desktop NHSE can open it too.
- `/switch/nhse/game/<user id>/`: the working copy of a game save while you edit it.
- `/switch/nhse/settings.txt`: the appearance setting.
- `/switch/nhse/crash.txt`: written if the app crashes.

## Build

Requirements: Rust 1.97 or newer with `rust-src` (`rustup component add rust-src`) and
devkitPro with ounce-devkit installed, or Docker.

```sh
make -C switch                 # devkitPro + ounce-devkit installed (sh install.sh from ounce-devkit)
sh switch/build.sh             # or: Rust here, the rest in ounce-devkit's Docker image
```

`build.sh` uses the image `ounce-devkit`; build it once in a checkout of ounce-devkit with
`docker build -t ounce-devkit .`.

How it fits together:

- `crates/nhse-switch`: the app, in Rust. It draws a 1280×720 frame in software (text in Inter,
  rasterized with fontdue) and handles the screens, saving and backups. It builds as a static
  library for `aarch64-switch-horizon`, with the standard library built from source.
- `switch/source/glue.c`: `main` and the libounce calls (framebuffer, buttons, keyboard,
  accounts, save data).
- `switch/cargo-build.sh`: the cargo command for the library. `switch/aarch64-switch-horizon.json`
  is OunceVK's Rust target for Switch 2.
- `switch/Makefile`: ounce-devkit's example Makefile, linking the library into `nhse.nro`.
- `switch/tools/make-logo.py`: makes the homebrew menu icon (`switch/icon.jpg`) and the in-app
  logo masks (`crates/nhse-switch/assets/*.mask`) from `assets/switch-logo.png`. Run it again
  after changing the logo.

The same app runs on a desktop for tests and screenshots:

```sh
cargo test -p nhse-switch
cargo run -p nhse-switch --example preview -- screenshots/
```
