# Design patterns ("My Designs")

How ACNH design patterns are stored, exchanged and edited in NHSE. The byte layout is
implemented in `crates/nhse-core/src/structs/design.rs`; the UI is `src/pages/Designs.tsx`
and the commands are `src-tauri/src/cmd/island.rs` (`designs`, `design_pixels`,
`set_design_pixels`, `import_design`, `export_design`, …).

## Files

| File | Size | Used for |
|---|---|---|
| `.nhd` | `0x2A8` (680 B) | normal 32×32 design; also the island flag slot |
| `.nhpd` | `0x8A8` (2216 B) | PRO design (four 32×32 sheets); also Able Sisters |
| `.acnh` | same 680 B layout as `.nhd` | acpatterns.com's export name for the identical structure |

These are raw slices of the save's pattern blocks, not a container format. Import checks the
length only and copies the bytes; export writes the block verbatim. `.acnh` is accepted by the
import dialog because acpatterns exports the same layout — a game-dumped `.nhd` and an acpatterns
export use identical offsets and sizes; the differences that matter are the author fields, the
usage flag (fixed on import) and the artwork.

## Layout (header is shared by normal and PRO)

| Offset | Size | Contents |
|---|---|---|
| `0x000` | 16 B | hash/ID prefix. The game writes a per-pattern value; acpatterns leaves it zero. Not validated by NHSE. |
| `0x010` | 40 B | design name, UTF-16LE, up to 20 chars |
| `0x038` | 4 B | town ID (`u32`) |
| `0x03C` | 20 B | town name, 10 chars |
| `0x054` | 4 B | player ID (`u32`) |
| `0x058` | 20 B | player name, 10 chars |
| `0x070` | 4 B | ownership/placement flag, see below |
| `0x077` | 1 B | pattern type (`0x00` = simple pattern; garment/flag types exist) |
| `0x078` | 45 B | palette: 15 × RGB |
| `0x0A5` | 512 B | pixels, normal (PRO: 2048 B, four sheets) |
| last 3 B | 3 B | padding |

Pixels are 4-bit palette indices, two per byte: even index in the low nibble, odd in the high
nibble. Index `15` means transparent (the palette only has 15 colours, `0`–`14`).

PRO sheet order in the UI is Front, Back, Right, Left; each sheet is 512 bytes and uses the same
palette.

## Ownership/placement flag (`0x70`)

The game only accepts `0xEE00` (opaque design) or `0xEE02` (contains transparency). Other
editors write values the game rejects, which makes the design unowned/unusable in-game.
`design::normalize_usage` rewrites anything else based on whether any pixel (all four PRO sheets)
is transparent. It mirrors upstream NHSE's import behaviour and is called by `import_design`.

Exception: the island flag slot is skipped, because a game-dumped flag uses `0xEE04` and the
meaning of that extra bit is not known. Flag files keep whatever the game wrote.

Importing from the desktop UI passes `adopt = true`, which also rewrites the author (town/player ID
and name) to player 1, so the design is immediately editable in-game. Only the usage flag is left
alone for the island flag slot.

## Editor behaviour

- Tools: Draw, Fill, Pick, Erase. Transparency is selectable as a colour (checkerboard swatch to
  the right of the palette), so Draw and Fill can paint empty space; Pick selects it from an empty
  pixel.
- Undo/redo: `Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z` or `Ctrl+Y`. History is pixel-only, one step per
  stroke, 64 steps, and resets whenever a design is reloaded or imported. Shortcuts are ignored
  while a text input has focus.
- PNG: import samples 32×32 (64×64 for PRO, a 2×2 grid of sheets), quantizes to 15 colours
  (median cut + k-means, nearest-colour mapping) and treats alpha < 128 as transparent. Export
  renders RGBA and scales by an integer factor (PRO is written as a 2×2 sheet grid).
- Saving a design also clears the slot's "edited" flag (`patterns_edit_flag_start` /
  `patterns_pro_edit_flag_start`) so the game picks up the new name.

## References

- Upstream C# layout: `DesignPattern.cs` / `DesignPatternPRO.cs` in
  [kwsch/NHSE](https://github.com/kwsch/NHSE).
- Ownership fix and extra detail: [vectorcmdr/ACNH-Pattern-Research](https://github.com/vectorcmdr/ACNH-Pattern-Research).
