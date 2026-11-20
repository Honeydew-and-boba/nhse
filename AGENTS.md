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

Not done yet on Windows:

- Nothing is published for Windows: `release.yml` still builds macOS + `nhse.nro` only. Adding a
  Windows job there needs care so two jobs don't race to create the same release.

## Goals and status (do not assume the rest are done)

- **Map editor QoL**: footprints, doors, drag handles and fixed-building warnings are in. Doors
  draw as a translucent plate with a one-terrain-tile glyph and a four-way move badge; only the
  first part of a multi-part door gets the glyph. Villager houses and Able Sisters draw their
  measured built body (`building.rs::body`), leaving their open front ground clear. Buildings cannot
  be rotated: the Angle field and `set_buildings` reject a change (`building.rs::angle_ok`), except
  for bridges and inclines whose angle is real terrain data. The Item panel runs Brush → Selected
  tile → Ground. Next: verify in-game, fill in the remaining building sizes/doors, and check
  dragging the bridge.
- **Map editor: custom design patterns**: ground designs (the save's `MyDesignMap`) are drawn from
  their normal 32×32 thumbnails, with a hover index and a View toggle. PRO designs cannot be placed
  on the ground in game (mannequin/painting only), so values at or above `pattern_count` are an
  unknown value and keep the magenta marker. Notes in `docs/map-editor.md`.
- **Map editor: path painting**: the Item panel's Ground section paints the eight default path
  families with 8-way autotiling (`crates/nhse-core/src/map/paint.rs`, `paint_road` command) and the
  save's non-empty normal designs; painting one clears the other and each stroke is one undo entry.
  Next: test painted paths and designs in-game. Notes in `docs/map-editor.md`.
- **Map editor: terrain painting**: in — rivers (8-way banks), raise/lower with generated faces,
  straight one-level falls, the 0–3 limit, the two-level-step refusal, the tree/building warning, the
  Rounded/Sharp corner choice and semi-manual fall models; see `docs/map-editor.md`.
  **Experimental:** only a single 1×1 cliff raise has been loaded in game (2026-11-20). Rivers,
  multi-tile cliffs, heights 2–3, waterfalls and the sharp corner style are untested in game. Open:
  in-game verification of painted paths/designs and terrain; the unmeasured river nub/junction shapes
  (`4A`, `4B`, `5A`, `6A`, `6B`, `4C`); automatic corner and multi-level waterfalls; the unmeasured
  `Cliff2C`/`3A`/`4x`/`5A`/`6x` variants.

Branches (2026-11-20): `origin/master` is `3ef7009`, the merge of `design-pattern-qol` (PR #1:
`.acnh` imports, usage-flag fix, design editor QoL). `windows-build` and `design-pattern-qol` were
merged and deleted on `origin`; both still exist locally. Local `master` tracks `upstream/master` at
`f85aee6` and trails `origin/master`. `map-editor-qol` carries the footprint, door, drag, warning,
ground-design, path-painting, rotation-guard and Able Sisters body work described in
`docs/map-editor.md`.

Remotes: `origin` is the project fork on GitHub; `upstream` is `sh00kapic/nhse`.
Do not push, open PRs or touch upstream unless explicitly asked.
