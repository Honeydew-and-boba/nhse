# AGENTS.md

Handoff notes for AI sessions in this repo.

## Working rules (from my global AGENTS.md)

- I am a BSEE student and hobbyist developer. Explain an unfamiliar concept briefly the first time it matters.
- When you are stuck, facing a design trade-off, or unsure after one failed attempt, stop and ask (`ask_advisor`) with a specific question instead of guessing.
- Prefer the standard library and tools already installed in the project over new dependencies.

## What this project is

NHSE is a save editor for **Animal Crossing: New Horizons**, rewritten in Rust with a native
[Tauri](https://tauri.app) desktop app (React + TypeScript UI), plus a Switch 2 homebrew build.
It edits save data dumped from a console; it never dumps saves itself. Covers game versions
1.0.0–3.0.3, verifies hashes, backs up on open, and only writes when the user saves.

## Layout

- `crates/nhse-core` — save format library: encryption, hashes, revisions, data structures, game data, map and editing tools. All save logic lives here; keep it GUI-free.
- `src-tauri` — desktop backend (Tauri commands, backups, image protocol). Entry: `src-tauri/src/lib.rs`.
- `src` — React + TypeScript UI (`pages/`, `components/`, `lib/`).
- `crates/nhse-switch` + `switch` — Switch 2 homebrew app (`nhse.nro`), see `switch/README.md`.
- `docs` — longer reference notes; design pattern format: `docs/designs.md`.

## Commands

```sh
npm ci
npx tauri dev                       # run desktop app in development
npx tauri build                     # release build for this platform (bundle targets: "all")
npx tauri build --bundles nsis      # Windows: NSIS installer only
cargo test -p nhse-core             # core library tests
cargo test -p nhse-switch
cargo run -p nhse-switch --example preview -- screenshots/
cargo fmt                           # must stay clean
cargo clippy                        # must stay clean
npx tsc --noEmit                    # must stay clean
```

Requirements: Rust stable, Node.js 22, Tauri prerequisites. Switch build needs devkitPro +
ounce-devkit; `make -C switch` or `sh switch/build.sh`. On Windows the Tauri prerequisites are the
MSVC linker (Visual Studio Build Tools, *Desktop development with C++*) and WebView2 (built into
Windows 10/11).

## Conventions

- Spaces, not tabs.
- Keep non-GUI logic in `nhse-core`; the UI and Switch frontends are thin layers over it.
- Model game data the way the game does — it makes research reproducible.
- The save format is byte-exact and cross-checked against the original C# NHSE. Do not change read/write layout without evidence; add a test in `nhse-core` when touching it.
- GPL-3.0-or-later. This is a rewrite of Kaphotics' NHSE; upstream exchange file formats are intentionally compatible.
- Never silently destroy user save data: the existing backup-on-open and single-commit-on-save behaviour is a hard requirement.
- `save/` is a real v3.0.3 save (revision 34) excluded through `.git/info/exclude`, used for local probing; never commit it or other save data.

## Design editor

- Design files are raw save slices: `.nhd` = `0x2A8`, `.nhpd` = `0x8A8` (`NORMAL_SIZE`/`PRO_SIZE` in
  `crates/nhse-core/src/structs/design.rs`). acpatterns.com's `.acnh` is the same layout and is
  accepted by the import dialog (`F.nhd` in `src/lib/dialog.ts`).
- `import_design` rewrites the author to player 1, then `design::normalize_usage` fixes the
  ownership/placement flag (`0x70`) to `0xEE00`/`0xEE02`. The island flag slot is skipped because the
  game writes `0xEE04` there. Full byte layout: `docs/designs.md`.
- The editor paints transparency as palette index 15; the checkerboard swatch beside the palette
  selects it and Pick reads it from empty pixels. Pixel-only undo/redo is `Ctrl/Cmd+Z`,
  `Ctrl/Cmd+Shift+Z`, `Ctrl+Y` (one step per stroke, 64 deep, reset on reload).

## Map editor

- Building `x`/`y` are the **centre** of the structure in absolute map item tiles (32 per acre, whole map 288×256). Plot sizes live in `crates/nhse-core/src/structs/building.rs::footprint` in **terrain** tiles (16 per acre), so the footprint spans `x ± w`, `y ± h` item tiles. The UI reads the table from `gd.building_sizes` (set in `src-tauri/src/cmd/data.rs`) via `buildingFootprint()` in `src/lib/items.ts`.
- `src/pages/MapEditor.tsx` draws each building as a filled rectangle with a white edge and a label underneath, and Select hit-tests the whole plot. Fill/border use the plot coordinates directly, so a moved X/Y shows up immediately.
- Verified against the real save on 2026-11-19: PlayerHouse1 at (121,134) → item 116..126 × 130..138 = terrain 58..63 × 65..69, exactly the 5×4 inside the 7×6 path boundary around the house, with the dark-soil door at the bottom-centre tile. Nook's Cranny (7×4) and the museum (7×4) line up with their darksoil entrance paths the same way.
- Dimensions from the in-game plots: player 5×4, villager 4×4, Nook's Cranny 7×4, museum 7×4, Able Sisters 5×4, campsite 4×4, Resident Services 6×3 (its 12×10-item plaza is a separate structure). Airport, tent, Redd, studio, hotel, bridge and incline are still approximations — verify before trusting them.
- Status (2026-11-19): builds clean and the user confirmed the rectangles/positions look right in the app. **Not yet tested in-game** and no save data layout changed — the footprint table is draw-only.
- Next steps for this feature: visually verify the remaining building positions/sizes in the editor, then add door/entrance markers and click-and-drag moving (the Buildings panel still edits X/Y numerically). Snapping a moved building to the terrain grid is also not implemented.
- Data quirk: the plaza is 12×10 item tiles (6×5 terrain) at `Plaza { x, z }` top-left; do not confuse it with the 6×3 Resident Services plot. Player-house plots have parity `x % 2 == w % 2` and `y % 2 == h % 2` (item vs terrain alignment); keep that in mind for future drag/snap code.

## Platform status

- **macOS (Apple Silicon)** — primary target; releases ship `.dmg` + `.app.zip` (ad-hoc signed).
- **Windows (x64)** — builds and runs from source (Tauri 2 + WebView2). Build with
  `npx tauri build --bundles nsis`, or develop with `npx tauri dev`.
- **Nintendo Switch 2** — `nhse.nro`, see `switch/README.md`.

What was done for Windows (2026-11-16):

- `src-tauri/tauri.conf.json`: `bundle.targets` was macOS-only `["app", "dmg"]`; it is now `"all"`
  (dmg + app on macOS, NSIS + MSI on Windows).
- Shortcut hints in the UI were hard-coded `⌘`. They now use `MOD` from `src/lib/util.ts`
  (`⌘` on macOS, `Ctrl` on Windows). Use `MOD` for any new hint.
- `.github/workflows/build-macos.yml` (file kept, workflow renamed "Build desktop app") has a
  `windows` job that builds the NSIS installer and uploads it as an artifact.
- Item images already used the right custom-protocol URL per platform: `asset()` in
  `src/lib/items.ts` returns `http://nhse.localhost/...` on Windows. Don't "simplify" that back to
  `nhse://` — custom schemes are only reachable as `http://<scheme>.localhost` on Windows.
- `crates/nhse-switch/tests/flow.rs`: one test compared paths from `Path::display()`, which uses
  `\` on Windows. The helper now normalizes to `/`. Write portable path assertions the same way.

Verified on Windows 11 (Rust 1.100, MSVC 14.51, Node 26, WebView2 157):

- `npm ci`, `npx tsc --noEmit`, `npm run build` — clean.
- `cargo check --workspace --all-targets`, `cargo fmt --check` — clean.
- `cargo clippy --workspace --all-targets` — no errors. Rust 1.100's clippy emits pre-existing
  style warnings in `nhse-core` (`chunks_exact` → `as_chunks`, loop index); not from Windows work.
  Keep the diff free of new warnings.
- `cargo test -p nhse-core -p nhse-switch` — all green, including
  `open_edit_save_reopen_all_revisions`, so save read/write is byte-exact on Windows.
- `npx tauri build --bundles nsis` — produces `target/release/bundle/nsis/NHSE_4.0.0_x64-setup.exe`.
- `target\release\nhse.exe` starts and shows a responsive window, sprites load.
- Opened, edited and saved a real **Switch 1** save with the app on Windows, and the changes loaded in
  an emulator without a crash (user-tested, one dump).

Windows quirks worth knowing:

- `titleBarStyle: "Overlay"` and `hiddenTitle` are macOS-only; on Windows the window keeps the
  native title bar and the `.drag` CSS regions just drag it. Cosmetic difference only.
- A fresh Rust install is not on `PATH` in already-open shells: use `%USERPROFILE%\.cargo\bin`.
- `npm ci` may report esbuild's postinstall script as blocked; the build still works (the binary
  ships as a platform optional dependency).
- Backups already avoid Windows-illegal characters in file names (`clean_file_name`).

Not done yet on Windows:

- Nothing is published for Windows: `release.yml` still builds macOS + `nhse.nro` only. Adding a
  Windows job there needs care so two jobs don't race to create the same release.
- `revealItemInDir(info.backups_dir + "/.")` in `src/pages/Backups.tsx` is a macOS idiom; check the
  Backups page "Show folder" button on Windows.

## My goals for this fork (NOT implemented yet — do not assume they are done)

- Save compatibility with **Switch 1** saves (`main.dat` from the original Switch, not just Switch 2 dumps). One real Switch 1 dump round-tripped on Windows (open → edit → save → emulator loads it), so this looks good; still verify other revisions before claiming support.
- Possible feature expansion: **custom designs** and **map editing** (both already exist on desktop — extend rather than rebuild).
- **Map editor QoL** (in progress on `map-editor-qol`): buildings now draw as their real footprint rectangles. Next: verify the remaining building dimensions in the app, then add entrance/door markers and click-and-drag moving.

Branches (2026-11-19): local `master` tracks `upstream/master` at `f85aee6` (the `windows-build`
merge); `windows-build` is a stale local branch. `design-pattern-qol` is 4 commits ahead of
`master`, local only (the old `origin` branch is gone), and carries the `.acnh` import, usage-flag
fix and editor QoL work; details in `docs/designs.md`. `map-editor-qol` is branched from
`design-pattern-qol` and adds the building footprint rectangles described under “Map editor”.
`origin` (`Honeydew-and-boba/nhse`) currently has only `master`; neither QoL branch has been pushed.

Remotes: `origin` is my fork `Honeydew-and-boba/nhse`; `upstream` is `sh00kapic/nhse`.
Do not push, open PRs or touch upstream unless explicitly asked.
