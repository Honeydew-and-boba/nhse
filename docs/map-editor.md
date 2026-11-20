# Map editor

Notes on the building layer of `src/pages/MapEditor.tsx`. Footprints and doors are draw-only:
no save layout changed. The code lives in
`crates/nhse-core/src/structs/building.rs`, `crates/nhse-core/src/map/paint.rs`,
`src/pages/MapEditor.tsx` and `src-tauri/src/cmd/data.rs`.

## Coordinate conventions

- Building `x`/`y` are the **centre** of the structure in absolute map item tiles (32 per acre,
  whole map 288×256).
- One terrain tile = 2 item tiles. The map editor's bottom-left readout is in layer item tiles:
  `layer Y + 32 = absolute Y` for the 9×6 field layer.
- `building.rs::footprint` returns `(width, height)` in **terrain** tiles, so the plot covers
  `x-w … x+w-1` and `y-h … y+h-1` item tiles (2w × 2h items). The UI reads the table from
  `gd.building_sizes` via `buildingFootprint()` in `src/lib/items.ts`.
- `building.rs::body` returns the part of the structure that is actually built, as rectangles in
  item tiles relative to the centre (`[dx, dy, w, h]`). An empty slice means "the whole plot". The
  UI reads it from `gd.building_bodies` via `buildingBodies()` / `buildingRects()`; `buildingRects()`
  falls back to the plot rectangle, so drawing and hit-testing always have something to work with.
- Plots are terrain-aligned: `x % 2 == w % 2` and `y % 2 == h % 2`. Dragging moves in 2-item
  steps, which preserves that.
- `Plaza { x, z }` stores the top-left in absolute item tiles. The editor draws a neutral 24×20
  item tile (12×10 terrain) overlay from it. The plaza is not selectable or editable. The probe
  save's value is X108, Y130 in layer coords (absolute z 162).

## Footprints

| Kinds | Structure | Plot (terrain tiles) | Built part |
|---|---|---|---|
| 1–8 | Player houses | 5×4 | whole plot |
| 9–18 | Villager houses | 4×4 | back 3 terrain rows (8×6 items); front rows open except the door |
| 19 | Nook's Cranny | 7×4 | whole plot |
| 20 | Resident Services | 6×3 (the 12×10 terrain plaza is separate) | whole plot |
| 21 | Museum | 7×4 | whole plot |
| 22 | Airport | 10×6 (approximate) | whole plot |
| 23 | Resident Services tent | 6×4 (approximate) | whole plot |
| 24 | Able Sisters | 5×4 | back 3 terrain rows + front row right of the door (10×6 + 6×2 items) |
| 25 | Campsite | 4×4 | whole plot |
| 26 | Bridge | 4×6 (approximate) | whole plot |
| 27 | Incline | 2×5 (approximate) | whole plot |
| 28 | Redd's Treasure Trawler | 6×4 (approximate) | whole plot |
| 29 | Photo studio | 5×4 (approximate) | whole plot |
| 42 | Hotel | 8×6 (approximate) | whole plot |
| other | Unknown | 2×2 | whole plot |

Player houses, villager houses, Nook's Cranny, the museum, Able Sisters and the campsite are
verified; the rest are drawn as approximations and never written back.

Villager 4 is the reference for kinds 9–18: plot 90..97 × 142..149, house 90..97 × 142..147, door
93..96 × 148..150. So the house fills rows 142–147 and the ground below and beside the door mat
(90..92 × 148..149 and 97 × 148..149) is open. The house body is `rect(-4, -4, 8, 6)` in
`building.rs::VILLAGER_BODY`; `body()` returns nothing for kinds whose built part has not been
measured, and those fall back to the whole plot.

Able Sisters (24) has the opposite free corner: layer 62..65 × 104..105 (absolute 62..65 × 136..137
for the save's building at 67,134) is open ground, while the rest of the front row is the shop
front beside the door. Its body is `ABLES_BODY` = `rect(-5, -4, 10, 6)` + `rect(-1, 2, 6, 2)`.

## Doors and move handles

`building.rs::doors(kind)` returns `DoorRect { dx, dy, w, h }` in item tiles from the building
centre. The UI reads them through `gd.building_doors` / `buildingDoors()`. Each door is drawn as a
translucent amber plate with a dashed edge (the plate is also the drag hit area), carrying a
one-terrain-tile door glyph whose knob is a four-way move badge. The plate brightens on hover and
selection. Only the first part of a multi-part door gets the glyph, so Able Sisters shows one door
icon over its two plates.

| Kinds | Door (item tiles from centre) | Shape |
|---|---|---|
| 1–8 | `dx -2, dy +2, 4×3` | player house |
| 9–18 | `dx -1, dy +2, 4×3` | villager house |
| 19 | `dx -2, dy +2, 4×2` | Nook's Cranny |
| 21 | `dx -2, dy +4, 4×1` | museum |
| 24 | `dx -1, dy +2, 6×2` plus `dx 0, dy +4, 4×1` | Able Sisters |
| 25 | `dx -2, dy +4, 4×2` | campsite |

The measurements came from the in-game map using the editor's bottom-left readout (layer Y + 32 =
absolute Y). Other kinds have no measured door and get a neutral 2×2 centre handle instead, so they
can still be selected and dragged. Door drawing ignores `angle`; only the bridge currently has a
non-zero angle and it has no door data.

## Drawing and interaction

- Each building is a category-coloured rectangle with corner ticks and a name label underneath.
  It is drawn from `buildingRects()` and selected by clicking anywhere in those rectangles, so free
  ground inside the plot (the villager-house front row, say) is not selectable.
- Drag a door marker (or the neutral centre handle) with the Select tool to move the building in
  2-item (one terrain tile) steps, clamped to the field layer. The position is committed once on
  release, so a drag only writes the save when the pointer is lifted.
- Moveable kinds (`building.rs::movable`, mirrored as `gd.movable_buildings`): player and villager
  houses, Nook's Cranny, the museum, Able Sisters and the campsite. They show an amber warning that
  terrain, rivers, cliffs, trees, furniture and other buildings do not move with them.
- Fixed kinds (Resident Services, airport, tent, bridge, incline, Redd's trawler, studio, hotel)
  show a red warning. Dragging is not blocked because this is a save editor; the numeric X/Y fields
  and "Pick position" work for every building.
- Buildings cannot be rotated. Changing a selected building's Angle (advanced mode) raises an error
  and is not saved, and `set_buildings` rejects it too (`building.rs::angle_ok`). Bridges and
  inclines are exempt because the game stores their real angle, which follows the terrain.
- Warnings are static text. There is no live scan for rivers, trees or overlapping plots.

## Ground designs (placed custom patterns)

`HorizonSave::map_design_tiles()` (exposed as `MapMeta.design_tiles`, `design_dims`) is the
game's `MyDesignMap`: one `u16` per terrain tile, column-major (`x * 96 + y`), `0xF800` = empty.
It overlays the terrain, so the tile under a design can still be plain grass. The editor draws a
normal design's 32×32 thumbnail (from `design/normal/<index>`) into its `2s × 2s` screen pixels,
with normal alpha so transparent pixels reveal the ground. The bottom-left readout appends
`Ground design #<index>`; the View menu has a **Ground designs** toggle.

PRO designs cannot be placed on the ground in game; they are only used as mannequins or wall
paintings, so `MyDesignMap` never holds one. Values at or above `pattern_count` are therefore an
unknown value of some other kind; they keep the magenta marker and show as
`Unknown ground value 0x…` in the readout instead of indexing the wrong list.

Preview notes:

- The image protocol serves one design: `design/normal/<index>` returns its 32×32 RGBA as PNG,
  and `design/pro/<index>?sheet=0` returns a single 32×32 sheet (without `sheet` a PRO design is a
  2×2 overview). `asset()` in `src/lib/items.ts` yields the right URL on Windows and macOS.
- Thumbnails are cached per design index and cache-busted once per map-editor mount (`?r=`), so an
  edited design reloads when the page is reopened but not while it stays open.
- Normal design slots are proven: `0 … pattern_count-1` (100 on 3.0) is a normal design index.

## Painting paths and designs

The Item panel runs **Brush → Selected tile → Ground**. The **Ground** section holds the eight
default path families, the save's non-empty normal designs, an erase swatch and Stop. Choosing one puts the Place tool into ground
mode; clicking or dragging paints one terrain tile (2×2 item tiles). Choosing an item, **Paint with
it** or **Use as brush** returns to item placement. A stroke commits once on pointer release and is
one normal undo entry (terrain and `MyDesignMap` are snapshotted together).

Painting a default path writes `TerrainTile.road` and clears the design on that physical tile;
painting a design writes `MyDesignMap` and clears the road. This matches the game, where one
replaces the other. **Erase ground** clears both.

Default path models come from the 8-way same-family neighbourhood:
`map/paint.rs::road_autotile` turns the connected sides and filled outer corners into a
`TerrainUnitModel` and `road_angle`. The table was measured from a real 3.0.3 save (70 placed path
tiles) and matches all of them; a unit test covers ends, straights, corners, T-junctions, crossings
and a painted run. Roads of different families do not join, which is also what the save shows. The
game has no model for a crossing with exactly three filled corners, so that rare case uses the
fully-filled crossing (`8A`).

Coordinates: a layer tile maps to terrain tile
`((x + field.offset_x) / 2 - 16, (y + field.offset_y) / 2 - 16)` because the terrain data covers the
middle 7×6 acres. Designs use `(x / 2, y / 2)`, because `MyDesignMap` spans the field acres
(144×96 on 3.0). Painting outside the terrain data therefore does nothing for paths; the pixel
preview shows that.

Only normal designs are offered, because the game cannot place a PRO design on the ground. Normal
design slots from `api.designs("normal")` are filtered to non-empty ones.

## Painting rivers, cliffs and waterfalls

> **In-game status (2026-11-20).** Only a single 1×1 cliff raise has been checked in game. Rivers,
> multi-tile cliffs, heights 2–3, every waterfall model and the sharp corner style are untested:
> those models and heights come from save dumps and an emulator shape sheet, not from a loaded
> island. Treat terrain painting as experimental and check the result in game after editing.

The Item panel's **Terrain** section paints `TerrainTile.model`, `angle` and `elevation` through
`map/paint.rs` (`paint_water` / `paint_height`). It works in terrain tiles (2×2 item tiles) with a
square brush from 1×1 to 5×5 (default 1×1, matching the game's one-tile tool; the shape fixer is
always one tile). A drag is one
stroke and one undo entry (terrain bytes snapshot). The backend works on a copy and only commits
if every tile is supported, so a rejected stroke writes nothing.

- **Water** sets the tile to a river at its current height and re-tiles every water tile in the
  stroke plus its 8-neighbour ring. Banks come from the water mask. Painting over a cliff face is
  allowed only when that face drops one level into water; it becomes a waterfall. Water clears the
  road layer and any ground design on the painted tiles.
- **Raise / Lower** moves elevation by one level and re-derives the cliff faces on the stroke and
  its ring. Plateau tops stay `Base` at the new height; faces are `Cliff5B` (one low side),
  `Cliff3B` (two adjacent low sides) or `Cliff7A` (one low diagonal). Erase the water first: a
  water tile is refused, so the river can be moved to the new height afterwards.
  The **Corners** choice writes `Cliff3C` instead of `Cliff3B` for corners the stroke creates; a
  tile that is already a corner keeps its own style, so a later neighbouring stroke does not
  restyle it.
- **Erase water** turns a river/fall tile back into grass at the same height and restores a cliff
  face if the tile is now at a plateau edge.
- **Fix one tile / falls** stamps one model and angle for edges the autotiler refuses, and for
  waterfall models; also one undo entry. The list uses plain-language labels (see the tables below)
  and any fall model other than `Fall200`/`Fall300`/`Fall400` is treated as hand-set: later water
  strokes leave it alone.

### Measured river table

Checked against a real 3.0.3 save (811 river tiles). `sides` are the four water neighbours;
`corners` are the four diagonal ones.

| Water neighbours | Model | Angle |
|---|---|---|
| no water sides | `River0A` | 0 |
| 1 water side | `River1A` | open side: S 0, E 1, N 2, W 3 |
| 2 opposite sides | `River2A` | 0 = N/S, 1 = E/W |
| 2 adjacent sides | `River3B` | water corner: SE 0, NE 1, NW 2, SW 3 |
| 3 sides | `River5B` | land side: W 0, S 1, E 2, N 3 |
| 4 sides, no land diagonal | `River8A` | 0 |
| 4 sides, one land diagonal | `River7A` | land corner: SW 0, SE 1, NE 2, NW 3 |

`River3C` (angular corner) is offered by the shape fixer but not generated. Four water sides with
two or more land diagonals need the junction/nub models this save never used; the generator fills
them as `River8A` (open water) and the shape fixer can replace them.

### Measured cliff table

A cliff face tile has the **higher** elevation; a plateau top is `Base` at the same height. The
angle is the low side/corner, not the high one.

| Low sides | Connected same-height sides | Model | Angle |
|---|---|---|---|
| 4 | 0 (single raised tile, same-height diagonals make no difference) | `Cliff0A` | 0 |
| 1 | 3 | `Cliff5B` | low side: W 0, S 1, E 2, N 3 |
| 2 adjacent | 2 adjacent | `Cliff3B` (rounded) or `Cliff3C` (sharp) | low corner: NW 0, SW 1, SE 2, NE 3 |
| 2 opposite | 2 opposite | `Cliff2A` | 0 = N/S, 1 = E/W |
| 3 | 1 | `Cliff1A` | connected side: S 0, E 1, N 2, W 3 |
| 0 | 4 | `Cliff7A` if exactly one low diagonal | low corner: SW 0, SE 1, NE 2, NW 3 |

The shape sheet was measured from an emulator dump:

- 1×1 pillars at elevation 1, 2 and 3 all store `Cliff0A` angle 0; the `_2`/`_3` variants are not
  used by the game for height. A same-height diagonal neighbour only changes the corner, not the
  four walls, so a diagonal run of 1×1 pillars is `Cliff0A` too (verified by raising tile 88,61
  beside the sample save's cliff; it used to be refused as an unsupported shape).
- A 2×1 strip uses `Cliff1A` with the angle pointing at the one side connected to the other raised
  tile (`a0` high/connected south, `a1` east, `a2` north, `a3` west; the last is by rotation).
- A 3×1 row uses `Cliff2A a1` for the middle tile (same-height pair E/W); a vertical ridge is `a0`.
- A 3×3 at tier 96..98 × 68..70 uses `Cliff3B` at the four corners, `Cliff5B` on the edges and
  `Base` at elevation 1 in the middle, which is exactly what the generator writes.

The game sometimes stores the square/angular `Cliff3C` version of a corner (a fresh L, or the
corners of the emulator's 2×2 block) and sometimes the rounded `Cliff3B` (the 3×3 and large
plateaus). The sample save has the same corner shape stored as `Cliff3B` at e1 and `Cliff3C` at e2,
so the letter is a free style choice rather than a height rule. The Terrain panel's **Corners**
choice picks the style for new corners; existing corners keep theirs. `Cliff2C`/`Cliff3A` are
variants of other shapes and are unmeasured. A saddle (two low diagonals and no low side) is refused
by the backend, so a rejected stroke writes nothing and names the coordinate.

Cliff/river `variation` is the game's 0–3 texture choice. Retiling keeps the value on a tile whose
model and angle do not change, so a no-op pass is byte-identical to the save. A new face built from
grass starts at 0; a face that changes shape keeps its old variation.

### Waterfalls

A fall strip replaces the water tiles at the top edge of a one-level drop. The fall tile keeps the
**high** elevation and its angle is the drop direction: S 0, E 1, N 2, W 3 (north is inferred by
rotation; the other three were measured). The strip is `Fall200` at the end 90° counter-clockwise
from the drop, `Fall300` at the opposite end, `Fall400` in the middle; a one-tile strip is
`Fall400`. For example, a south-dropping 4-wide fall is `300, 400, 400, 200` west to east.

Only those three straight-fall models are generated automatically. A drop of more than one level is
refused (add a one-tile terrace), and water that drops onto land is left as a river and counted in
the warning toast.

The sample save's two-level gorge uses the other measured cascade models. All of them are in the
model list and are treated as hand-set, so later water strokes leave them alone:

| Use | Model | Label in the Terrain panel |
|---|---|---|
| upper cascade, counter-clockwise end | `Fall204` | Cascade top, counter-clockwise end |
| upper cascade, middle | `Fall408` | Cascade top, middle |
| upper cascade, clockwise end | `Fall304` | Cascade top, clockwise end |
| lower cascade, north end, east drop | `Fall208` | Cascade bottom, north end (east drop) |
| lower cascade, 2nd from north, east drop | `Fall422` | Cascade bottom, 2nd from north (east drop) |
| lower cascade, middle | `Fall403` | Cascade bottom, middle |
| lower cascade, south end, east drop | `Fall302` | Cascade bottom, south end (east drop) |
| lower cascade, north end, west drop | `Fall308` | Cascade bottom, north end (west drop) |
| lower cascade, 2nd from north, west drop | `Fall421` | Cascade bottom, 2nd from north (west drop) |
| lower cascade, south end, west drop | `Fall202` | Cascade bottom, south end (west drop) |

The lower-strip models encode the absolute direction as well as the strip position (208 and 202 are
the same slot for opposite drops), so the labels name the measured orientation rather than a
rule. Those placements come from one gorge: corner falls and cascades of three or more levels are
not measured, and automatic cascade generation is not implemented. Place those tiles by hand with
the model list.

### Rules that keep a save loadable

- Elevations are 0–3 and every step between neighbours must be one level: the game only draws a
  cliff face for a one-level drop, so a two-level step shows the void. `paint_height` calls
  `map/paint.rs::check_steps` after retiling and refuses the whole stroke, writing nothing, when a
  tile it changed is two or more levels from an orthogonal neighbour. A two-level step that was
  already in the save is ignored, so a hand-edited dump never makes neighbouring terrain
  unpaintable. Diagonal steps are not checked. Water painting never changes elevations, so it
  cannot create one.
- Trees and buildings need a one-terrain-tile buffer from new cliffs and water. The game normally
  keeps them clear, but some players want the overlap for a particular look, so the editor allows
  it and warns instead of blocking. The paint commands in `src-tauri/src/cmd/map.rs` diff the
  terrain before and after the stroke, then call `HorizonSave::nearby_objects` (`map/field.rs`),
  which scans both field layers for tree-kind items and every building plot and returns the counts;
  the toast names them. It runs on both water painting and raise/lower.
- Nothing is deleted by terrain painting. Field items have no elevation field in the save: the game
  draws them at the terrain height, so raising or lowering land moves them for free.
- The Files tab's whole-terrain dump import and the Fix one tile stamp write bytes directly and skip
  both checks.

### Height display

The terrain preview uses four clearly stepped grass colours (dark low, pale high) and dirt/peach
cliff faces. The map canvas draws a contour line where two terrain tiles differ in elevation, the
bottom-left readout shows `Height N`, and the View menu has a **Height contours** toggle.

The save field is a `u16`, but the game only supports elevations 0–3. The brush refuses to write
above 3 and refuses to go below 0. A save that already contains a higher value is left untouched:
the map editor warns on load, names the value in the Terrain panel, and keeps showing the tile.

### Limits

- Painting water within one tile of an existing multi-level waterfall fails the >1-level check
  (nothing is written). Paint away from it, or fix the tiles by hand.
- Re-tiling the ring can replace a hand-picked `River3C` with the default `River3B` when the water
  next to it changes. Use **Fix one tile** to put it back. Hand-set fall models (anything other than
  `Fall200`/`Fall300`/`Fall400`) and `Cliff3C` corners are kept.
- A saddle (two low diagonals, no low side) or a strip with no connected same-height side is still
  refused. Single tiles, 2×1 strips, 1-wide ridges and 2×2-or-larger areas all work.
- The generator writes the rounded `Cliff3B` style for every 2-adjacent-low corner unless the
  **Corners** choice is Sharp (`Cliff3C`). Existing corners keep their style; use **Fix one tile**
  to change one by hand.
- In game (2026-11-20) a single 1×1 raise was correct: the ground rose with the items on it and the
  tree/building warning was acceptable. Painted rivers, multi-tile cliffs, heights 2–3 and falls are
  not verified in game yet.

## Verified examples

From the real save on 2026-11-19:

- PlayerHouse1 at (121,134) → plot 116..125 × 130..137, door 119..122 × 136..138.
- Nook's Cranny at (75,190) → plot 68..81 × 186..193, door 73..76 × 192..193.
- Museum at (93,164) → plot 86..99 × 160..167, door 91..94 × 168.

## Status

Builds clean and the map visuals were confirmed in the app on Windows (2026-11-20), including
design thumbnails with transparency. Doors and drag were added from in-game measurements. Terrain
painting is checked against an emulator shape sheet: a 1×1 raise stays `Cliff0A` at elevations
1–3, a 2×1 stays `Cliff1A`, a 3×1 ridge stays `Cliff2A`, a 3×3 stays `3B`/`5B`/`Base`, and the only
re-derive differences are angular `Cliff2C`/`Cliff3C` corners that the generator draws as rounded
`3B`. A single 1×1 raise was loaded in game on 2026-11-20 and behaved correctly (ground and items
moved up, no visual break); no river, fall, multi-tile cliff or height-2+ plateau has been tested in
game yet. No save data layout changed.

Done in this pass: translucent door plates with a one-tile door glyph and four-way move badge (only
the first part of a multi-part door gets the glyph, so Able Sisters shows one icon), villager-house
and Able Sisters body rectangles (`building.rs::body`; the free front corner is documented above),
shop fills moved off green — they used to blend into the grass — to crimson `[211,61,61]` in
`buildingColor()` in `src/pages/MapEditor.tsx`, normal ground designs drawn from their thumbnails
(with a View toggle and hover index), ground painting in the Item panel (`map/paint.rs` + the
`paint_road` command): default paths autotile from their neighbours, normal designs paint straight
onto `MyDesignMap`, both replace each other, and each stroke is one undo entry. The Item panel puts
the selected-tile editor above the ground palette, and building rotation is rejected in the UI and
in `set_buildings`. Terrain painting adds rivers (8-way banks), cliff raise/lower with generated
faces, straight one-level waterfalls, a 1×1–5×5 square brush, a one-tile shape fixer, and the
tree/building buffer warning; unsupported shapes and water drops over more than one level are
refused without writing. A second pass added the measured `Cliff0A`/`Cliff1A`/`Cliff2A` shapes, the
0–3 write limit with a load warning for higher saves, stronger stepped grass/dirt colours, height
contours and a hover height readout. A third pass added the **Corners** rounded/sharp choice for new
cliff corners (existing corners keep their style) and the semi-manual waterfall mode: the model list
now uses plain-language labels and carries the 13 fall models the save uses, and any hand-set fall
model survives later water strokes.

Next: test the painted paths, designs, rivers, multi-tile cliffs and falls in-game, verify the
remaining building sizes/positions, fill in door data for the fixed structures, and check dragging
on the bridge (the one building with a non-zero `angle`).

## Remaining goals (not started — handoff notes)

1. **Terrain painting follow-ups.** Rivers, cliffs, straight one-level waterfalls, the corner style
   choice and semi-manual fall models are in (see above). Still open: in-game verification, the
   unmeasured river nub/junction shapes (`4A`, `4B`, `5A`, `6A`, `6B`, `4C`), automatic corner and
   multi-level waterfalls, and the `Cliff2C`/`Cliff3A`/`Cliff4x`/`Cliff5A`/`Cliff6x` style variants.
   The Files tab still imports and exports whole terrain dumps.
