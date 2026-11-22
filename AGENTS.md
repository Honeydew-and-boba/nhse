# AGENTS.md

Handoff notes for AI sessions in this repo.

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
- `docs` — longer reference notes; design patterns: `docs/designs.md`; map editor: `docs/map-editor.md`.

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

Design file format, import rules and editor behaviour: see `docs/designs.md`.

## Map editor

Coordinates, footprints, doors, drag behaviour and warnings: see `docs/map-editor.md`.

## Platform status

- **macOS (Apple Silicon)** — primary target; releases ship `.dmg` + `.app.zip` (ad-hoc signed).
- **Windows (x64)** — builds and runs from source (Tauri 2 + WebView2). Build with
  `npx tauri build --bundles nsis`, or develop with `npx tauri dev`.
- **Nintendo Switch 2** — `nhse.nro`, see `switch/README.md`.

What was done for Windows (2026-11-16):

- `src-tauri/tauri.conf.json`: `bundle.targets` was macOS-only `["app", "dmg"]`; it is now `"all"`
  (dmg + app on macOS, NSIS + MSI on Windows).
- Shortcut hints in the UI were hard-coded `⌘`. Hints now use `MOD` from `src/lib/util.ts`
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
  an emulator without a crash (tested manually, one dump).

Windows quirks worth knowing:

- `titleBarStyle: "Overlay"` and `hiddenTitle` are macOS-only; on Windows the window keeps the
  native title bar and the `.drag` CSS regions just drag it. Cosmetic difference only.
- A fresh Rust install is not on `PATH` in already-open shells: use `%USERPROFILE%\.cargo\bin`.
- `npm ci` may report esbuild's postinstall script as blocked; the build still works (the binary
  ships as a platform optional dependency).
- Backups already avoid Windows-illegal characters in file names (`clean_file_name`).
- The Backups page "Show folder" button uses `revealItemInDir(backups_dir + "/.")`. The trailing
  `/.` is a macOS idiom, but on Windows the path goes through `GetFullPathNameW`, which collapses it,
  so the button works: it opens the parent folder with the backups folder selected. It selects the
  folder rather than opening its contents; switching to `openPath` would need `opener:allow-open-path`
  in `src-tauri/capabilities/default.json`.

Release: `release.yml` has a `windows` job that builds the NSIS installer (unsigned, so SmartScreen
warns on first run). Its output joins the macOS and `nhse.nro` files in the one `release` job, so no
two jobs race to create the release. It has not been run yet.

## Goals and status (do not assume the rest are done)

- **Map editor QoL**: footprints, doors, drag handles and fixed-building warnings are in and
  verified in game (2026-11-22), including moving buildings. Doors
  draw as a translucent plate with a one-terrain-tile glyph and a four-way move badge; only the
  first part of a multi-part door gets the glyph. Villager houses and Able Sisters draw their
  measured built body (`building.rs::body`), leaving their open front ground clear. Buildings cannot
  be rotated: the Angle field and `set_buildings` reject a change (`building.rs::angle_ok`), except
  for bridges and inclines whose angle is real terrain data. The Item panel runs Brush → Selected
  tile → Ground. **Fixed structures can still be dragged** (the bridge included, by its neutral
  centre handle): dragging ignores `movable_buildings`, and the danger Callout in the building panel
  is the only warning. Next: fill in the remaining building sizes/doors.
- **Map editor: custom design patterns**: ground designs (the save's `MyDesignMap`) are drawn from
  their normal 32×32 thumbnails, with a hover index and a View toggle. PRO designs cannot be placed
  on the ground in game (mannequin/painting only), so values at or above `pattern_count` are an
  unknown value and keep the magenta marker. Notes in `docs/map-editor.md`.
- **Map editor: path painting**: the Item panel's Ground section paints the eight default path
  families with 8-way autotiling (`crates/nhse-core/src/map/paint.rs`, `paint_road` command) and the
  save's non-empty normal designs; painting one clears the other and each stroke is one undo entry.
  Painted paths and designs verified in game (2026-11-22). Notes in `docs/map-editor.md`.
- **Map editor: terrain painting**: in — rivers (8-way banks, Rounded/Sharp bends), raise/lower with
  generated faces and rounded/sharp corners, straight one-level and consecutive falls, the 0–3 limit,
  the two-level-step refusal, the tree/building warning and semi-manual fall models; see
  `docs/map-editor.md`. Verified in game (2026-11-22): cliffs, heights, rivers, sharp/round cliff and
  river corners, consecutive falls and the tree/building warning. Waterfalls were re-done on
  2026-11-21 (hot fix): a fall needs **no water behind it and no pool below** — the player paints
  water on a plateau edge and the game draws the fall recessed into the cliff face — and the fall
  model now follows the measured table for recessed strips, cascade tops/bottoms and single-tile
  falls (`Fall101` on land, `Fall100` into water). That re-derives all 74 fall tiles of the sample
  save exactly. Verified in game (2026-11-21): a recessed fall with no source and a one-tile-wide
  two-level cascade built with the editor. `Fall100` still matches the game's own family but was not
  dumped from a game-placed copy. Corner falls do not exist in the game, and a fall dropping more
  than one level is refused unless a terrace buffers it. Open: the unmeasured river junction shapes
  (`4A`, `4B`, `5A`, `6A`, `6B`), which the generator fills as open water (`4C` is now used for the
  1×1 pond inside a cross of falls), and the unmeasured `Cliff2C`/`3A`/`4x`/`5A`/`6x` variants.
  **Back up the save before terrain work.**

Branches (2026-11-22): the fork holds `master` only. `origin/master` was `da1ffb0`, the merge of PR #3
(`map-editor-qol`: footprints, doors, drag, warnings, ground designs, path painting, terrain
painting). Local `master` tracks `origin/master`; `windows-build`, `design-pattern-qol` and
`map-editor-qol` were merged and deleted locally and on `origin`.

`design-shape-tools` is the open branch for the design editor (shape tools, Fill all, brush sizes,
pixel-grid overlay, left/right mirror, canvas rotate and wrap-drag). It is pushed to `origin` with no
PR yet; `master` does not have it.

On 2026-11-21 the waterfall hot fix (three commits: recessed falls and the measured model table, the
`Fall100`/`River4C` follow-up, the verification docs) was committed and pushed straight to
`master` on `origin` with no PR. `save/` in the working tree is that day's dump, acre G6 being the
waterfall test acre.

Remotes: `origin` is the project fork on GitHub; `upstream` is `sh00kapic/nhse`.
Do not push, open PRs or touch upstream unless explicitly asked.
