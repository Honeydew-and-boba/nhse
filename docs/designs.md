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

- Tools: Draw (1×1 / 2×2 / 3×3 brush), Fill (flood the clicked area), Fill all (the whole sheet, one
  undo step), Pick and Erase, plus shapes — Line, Rect and Ellipse (1 px outlines), and the filled
  stamps Circle, Star and Heart in three sizes. Shapes are previewed while dragging and
  written as one undo step on release; the ellipse is inscribed in the dragged box, so a non-square
  drag gives an ellipse and a square drag a circle. The stamps follow the cursor while dragging and
  are centred on the release pixel. Stamp radii are 2 / 5 / 16 px (in-game diameters 4 / 10 / 36;
  the large stamp is capped at the 32 px sheet). Rasterizers live in `src/lib/shapes.ts`.
- Transparency is selectable as a colour (checkerboard swatch to the right of the palette), so Draw,
  Fill and the shape tools can paint empty space; Pick selects it from an empty pixel.
- Grid (on by default): a thin line on every pixel boundary, drawn as a CSS overlay on the 448 px
  canvas. View only, never written to the save.
- Mirror (off by default): every painting tool also paints the mirrored column, `x → 31−x`, so the
  axis sits between columns 15 and 16. Applies to Draw, Erase, Fill (each seed floods its own area),
  Fill All, Line, Rect, Ellipse and the stamps, and the shape preview shows the mirrored result while
  dragging. The game's guide cross also has a horizontal line, but only left↔right is mirrored here.
  Both toggles are per editor session and reset when the editor closes.
- Whole-sheet edits act on the visible sheet only (PRO sheets stay independent), as in game:
  - Rotate: the two buttons beside Grid/Mirror turn the sheet 90° counter-clockwise / clockwise
    (the game's ZL/ZR). The 32×32 square wraps, so nothing is lost; each press is one undo step.
  - Move: the Move tool drags the whole sheet by whole pixels. Content that leaves one edge
    reappears on the opposite edge while dragging, so a drag of one row up moves the top row to
    the bottom. Offsets are recomputed from the sheet captured at pointer-down, so the drag never
    compounds rounding; the whole drag is one undo step. Grid and Mirror do not apply.
- Undo/redo: `Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z` or `Ctrl+Y`. History is pixel-only, one step per
  stroke, 64 steps, and resets whenever a design is reloaded or imported. Shortcuts are ignored
  while a text input has focus.
- Save design is enabled only while the design differs from what was loaded or last saved (name,
  palette or pixels). Undoing every stroke, rotating four times or dragging the canvas back leaves
  it disabled, so an unchanged design is never rewritten.
- PNG: import samples 32×32 (64×64 for PRO, a 2×2 grid of sheets), quantizes to 15 colours
  (median cut + k-means, nearest-colour mapping) and treats alpha < 128 as transparent. Export
  renders RGBA and scales by an integer factor (PRO is written as a 2×2 sheet grid).
- Saving a design also clears the slot's "edited" flag (`patterns_edit_flag_start` /
  `patterns_pro_edit_flag_start`) so the game picks up the new name.

## Colour space (hue / vividness / brightness)

The game's Change Color menu is a 30 × 15 × 15 grid: 30 hues, 15 vividness steps, 15 brightness
steps, 6750 colours. The save only stores the resulting RGB triple per palette slot, so this table
exists purely so the UI can offer the same picker as the game; nothing about the file layout
depends on it. Implementation: `src/lib/designColors.ts`.

The table was measured, not guessed: one design per row with a colour per palette slot, dumped and
read back from the palette bytes (the game stores the whole palette, including colours no pixel
uses). Findings:

- **Hue** (30): the anchors are stored literally, byte-exact — this is the part users copy from
  in-game tutorials, so it must match exactly. The strip runs red → yellow → green → cyan → blue →
  magenta → red; every anchor has channel values of only `0f`, `3a`, `64`, `8f`, `ba` or `e5`.
- **Brightness** (15): scales every channel; the brightest channel is 229 at step 15 and 20 at
  step 1, linear in between (`(20 + 209·b/14) / 229`).
- **Vividness** (15): lerps every channel toward the hue's brightest channel, i.e. step 1 is grey
  and step 15 is the hue anchor.
- The two ramps reproduce the measured rows to within one 8-bit unit, and reproduced 150 of the
  165 distinct colours in a real island's saved palettes exactly or within 1. The remainder are
  outside the game's range (channel values the grid cannot produce), i.e. imported artwork.

The picker shows one strip per channel, each cell painted with the colour that step actually
produces, and writes the chosen RGB into the selected palette slot. A slot whose colour is not in
the grid (imported or hand-picked) gets no strip marker; the native colour picker stays available
as a custom colour escape hatch.

## References

- Upstream C# layout: `DesignPattern.cs` / `DesignPatternPRO.cs` in
  [kwsch/NHSE](https://github.com/kwsch/NHSE).
- Ownership fix and extra detail: [vectorcmdr/ACNH-Pattern-Research](https://github.com/vectorcmdr/ACNH-Pattern-Research).
