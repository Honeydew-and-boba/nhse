# Map editor

Reference for the map editor: building plots and doors, ground paths and custom designs, rivers,
cliffs and waterfalls. The code lives in `crates/nhse-core/src/structs/building.rs`,
`crates/nhse-core/src/map/paint.rs`, `crates/nhse-core/src/map/field.rs`,
`crates/nhse-core/src/save/island.rs`, `src/pages/MapEditor.tsx`, `src-tauri/src/cmd/island.rs`
and `src-tauri/src/cmd/map.rs`.

No save layout changed: the editor writes only the existing terrain, road and ground-design
fields and building positions. Footprints and doors are drawn from measured data and never
written. The building sections document work that shipped separately (#7); they are kept here
because this is the map editor's reference.

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
absolute Y). Other movable kinds with no measured door get a neutral 2×2 centre handle; fixed kinds
draw no handle and are selected by clicking their plot. Door drawing ignores `angle`; the bridge has
no door data and, being fixed, no handle either.

Dragging is limited to `movable_buildings`. Clicking a fixed structure (bridge, incline, airport,
tent, Resident Services, Redd's trawler, the studio, the hotel) selects it and nothing more. The
numeric X/Y fields and "Pick position" still work for every building, because this is a save editor;
leave a fixed structure where the game put it. A cancelled pointer restores the drag preview and
writes nothing, so only a completed release reaches `set_buildings`.

## Drawing and interaction

- Each building is a category-coloured rectangle with corner ticks and a name label underneath.
  It is drawn from `buildingRects()` and selected by clicking anywhere in those rectangles, so free
  ground inside the plot (the villager-house front row, say) is not selectable.
- Drag a door marker (or the neutral centre handle) with the Select tool to move a movable building
  in 2-item (one terrain tile) steps, clamped to the field layer. The position is committed once on
  release, so a drag only writes the save when the pointer is lifted; a cancelled pointer restores
  the preview without writing.
- Moveable kinds (`building.rs::movable`, mirrored as `gd.movable_buildings`): player and villager
  houses, Nook's Cranny, the museum, Able Sisters and the campsite. They show an amber warning that
  terrain, rivers, cliffs, trees, furniture and other buildings do not move with them.
- Fixed kinds (Resident Services, airport, tent, bridge, incline, Redd's trawler, studio, hotel)
  show a red warning and cannot be dragged: their door or handle only selects them. The numeric X/Y
  fields and "Pick position" still edit every building.
- Buildings cannot be rotated. Changing a selected building's Angle (advanced mode) raises an error
  and is not saved, and `set_buildings` rejects it too (`building.rs::angle_ok`). Bridges and
  inclines are exempt because the game stores their real angle. Measured in game: changing a bridge
  from angle 1 to 0 rotated it 90°, but the river did not rotate with it, so the bridge lay parallel
  to the river and could not be crossed. The angle has to agree with the terrain; only change it
  together with the river and cliff work.
- Warnings are static text. There is no live scan for rivers, trees or overlapping plots.

## Ground designs (placed custom patterns)

`HorizonSave::map_design_tiles()` (exposed as `MapMeta.design_tiles`, `design_dims`) is the
game's `MyDesignMap`: one `u16` per terrain tile, column-major (`x * 96 + y`), `0xF800` = empty.
It overlays the terrain, so the tile under a design can still be plain grass or a default path,
which is drawn under the design's transparent pixels. The editor draws a
normal design's 32×32 thumbnail (from `design/normal/<index>`) into its `2s × 2s` screen pixels,
with normal alpha so transparent pixels reveal the ground. The bottom-left readout appends
`Ground design #<index>`; the View menu has a **Ground designs** toggle.

PRO designs cannot be placed on the ground in game; they are only used as mannequins or wall
paintings, so `MyDesignMap` never holds one. Values at or above `pattern_count` are therefore an
unknown value of some other kind; they keep the magenta marker and show as
`Unknown ground value 0x…` in the readout instead of indexing the wrong list.

The editor refuses to write a design where it would float in game: a river or fall terrain model,
or an acre-template pixel the game draws as sea, sea edge, river mouth or tide pool
(`render.rs::design_tile_is_water`). Beach sand and solid template ground still accept a design.
There is no red preview; the skipped tiles are simply not written. Erasing a design on water is
still allowed, so an earlier stroke can be cleaned up.

Preview notes:

- The image protocol serves one design: `design/normal/<index>` returns its 32×32 RGBA as PNG,
  and `design/pro/<index>?sheet=0` returns a single 32×32 sheet (without `sheet` a PRO design is a
  2×2 overview). `asset()` in `src/lib/items.ts` yields the right URL on Windows and macOS.
- Thumbnails are cached per design index and cache-busted once per map-editor mount (`?r=`), so an
  edited design reloads when the page is reopened but not while it stays open.
- Normal design slots are proven: `0 … pattern_count-1` (100 on 3.0) is a normal design index.

## Painting paths and designs

The Item panel runs **Brush → Selected tile → Ground**. The **Ground** section holds the eight
default path families, the save's non-empty normal designs (five to a row, like the in-game
editor), three erase brushes and Stop. Choosing one puts the Place tool into ground mode; clicking
or dragging paints one terrain tile (2×2 item tiles). Choosing an item, **Paint with it** or **Use as
brush** returns to item placement. A stroke commits once on pointer release and is one normal undo
entry (terrain and `MyDesignMap` are snapshotted together).

A default path is written only where the game lets the terrain layer own the tile: not a river or
fall, and the acre template's collision type must be 7 (`terrain_editable`). The paint command drops
the other tiles, so a stroke across a pond, the beach or the sea paints the dry editable tiles and
leaves the rest alone. The preview shows the path colour everywhere and never a red invalid marker;
beach, sea and water are silently skipped. The game stores no road on water or on template ground.

**A tile holds both layers.** `TerrainTile.road` and the `MyDesignMap` entry are separate save
fields, and the game stores a default path and a ground design on the same tile; the design is drawn
on top with its own transparency. Measured in acre G6 of the sample save: terrain (85,76) is
`RoadFanPattern0A` plus design 9, (87,76) is `RoadStone0A` plus design 9, and (89,76) is
`RoadBrick0A` with no design. So painting a path only writes the road layer and keeps the design,
and painting a design only writes the design. **Erase ground** clears both; **Erase path** and
**Erase design** clear one layer.

Default path models come from the 8-way same-family neighbourhood:
`map/paint.rs::road_autotile` turns the connected sides and filled outer corners into a
`TerrainUnitModel` and `road_angle`. The table was measured from a real 3.0.3 save (70 placed path
tiles) and matches all of them; a unit test covers ends, straights, corners, T-junctions, crossings
and a painted run. Roads of different families do not join, which is also what the save shows. The
game has no model for a crossing with exactly three filled corners, so that rare case uses the
fully-filled crossing (`8A`).

### Path corner style

Every road family ships the same bend twice: `...3B` has a rounded outside corner and `...3C` a
sharp one, same silhouette, same angles. The game picks per tile, and not from the geometry: acre G6
of the sample save has a rounded 2×2 and a sharp 2×2 side by side, plus a sharp 3×3 and a rounded
3×3. The Ground section's **Corners** choice (default Sharp, the style the editor wrote before the
choice existed) styles the bends a stroke creates and restyles the bends it paints over. A bend that
is only re-tiled as a neighbour of the stroke keeps the style already in the save, so extending a
path does not restyle an existing patch. `the_g6_test_acre_patches_match_the_save` in `paint.rs`
re-derives the four G6 patches from the dump, and `a_stroke_restyles_only_the_tiles_it_paints`
covers the neighbour rule.

The terrain preview draws a rounded bend as a quarter-tile arc of grass in its outside corner
(`terrain.rs::road_rounded_corner`), so the choice is visible before saving; a sharp bend fills the
whole tile. Junction and crossing notches are not drawn: their art is unmeasured, and the game's
`4A`/`5A`/`6A` variants are not modelled.

A three-tile L (two connected sides with an empty inside diagonal) has its own pair: `...2B` is
rounded and `...2C` sharp. The game re-tiled the 2026-11-24 test acre's editor-written `2C` Ls to
`2B` when it loaded the save, and those corners render rounded in game, so **Corners** picks `2B`
for Rounded and `2C` for Sharp here too. The preview arcs `2B` (`road_rounded_corner`), and a
neighbour-only stroke keeps an L's existing style. A 2×2 block (inside diagonal is also path) uses
the `3B`/`3C` pair instead, which is the case the earlier acre measurements showed.

Coordinates: a layer tile maps to terrain tile
`((x + field.offset_x) / 2 - 16, (y + field.offset_y) / 2 - 16)` because the terrain data covers the
middle 7×6 acres. Designs use `(x / 2, y / 2)`, because `MyDesignMap` spans the field acres
(144×96 on 3.0) — the same 9×6 block as the field layer, one acre into the 9×8 map. Painting
outside the terrain data therefore does nothing for paths; the pixel preview shows that.

Only normal designs are offered, because the game cannot place a PRO design on the ground. Normal
design slots from `api.designs("normal")` are filtered to non-empty ones.

## Painting rivers, cliffs and waterfalls

> **Status.** Verified in game: cliffs, heights 0–3, rivers, sharp and rounded cliff *and* river
> corners, all four waterfall families in all four directions, recessed and consecutive falls,
> painted paths and ground designs, three-tile river L bends (`2C`), and moving buildings. The river
> junction table was measured from water shapes painted in game on acre G6 (2026-11-25, two dumps),
> including `River6A`; every flat model the editor emits came out of those dumps, and editor-written
> L, T and plus junctions then loaded in game with no broken bank. Rotated orientations were not
> loaded separately. The fall table came from a two-level cake, a recessed pad and the sample save's
> gorge (2026-11-25). A 13-tile one-corner build (2026-11-28) then confirmed the six one-corner
> middle states, so the generator now reads both edge masks and writes the catalogue model;
> `probe check` re-derives all 55 fall tiles of that build save and all 42 of `save/` with no
> mismatch.
> **Back up the save before terrain work.**

The Item panel's **Terrain** section paints `TerrainTile.model`, `angle` and `elevation` through
`map/paint.rs` (`paint_water` / `paint_height`). It works in terrain tiles (2×2 item tiles) with a
square brush from 1×1 to 5×5 (default 1×1, matching the game's one-tile tool; the shape fixer is
always one tile). A drag is one
stroke and one undo entry (terrain bytes snapshot). The backend works on a copy and only commits
if every tile is supported, so a rejected stroke writes nothing.

Terrain painting is limited to tiles the acre template owns: collision type 7
(`render.rs::terrain_editable`). Beach sand, sea, rocks, tide pools and river mouths are template
pixels, so the game draws them and ignores terrain edits there; in game the edits were invisible
and had no collision. The paint commands drop those targets, the UI leaves them out of the drag
preview, and the commands restore every fixed tile after the 8-neighbour re-tile. Erase is limited
the same way.

- **Water** sets the tile to a river at its current height and re-tiles every water tile in the
  stroke plus its 8-neighbour ring. Banks come from the water mask. The **Corners** choice picks
  `River3B` (rounded) or `River3C` (sharp) for a new 2×2 bend; a three-tile L is `River2C`, which is
  what the game uses, and a bend that already exists keeps its style. Painting over a cliff face is
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
  and any fall model the autotiler never writes is treated as hand-set: later water strokes leave it
  alone.

### Measured river table

Checked against two rounds of water shapes painted by the player in acre G6 of a real 3.0.3 save and
dumped back (model, angle and all eight neighbours). `sides` are the four water neighbours; `corners`
are the four diagonals. Only a diagonal between two water sides is a shape corner: a water diagonal
on the dry side is part of that bank and does not change the model.

| Water neighbours | Model | Angle |
|---|---|---|
| no water sides | `River0A` | 0 |
| 1 water side | `River1A` | open side: S 0, E 1, N 2, W 3 |
| 2 opposite sides | `River2A` | 0 = N/S, 1 = E/W |
| 2 adjacent sides, corner between them dry (three-tile L) | `River2C` | water corner: SE 0, NE 1, NW 2, SW 3 |
| 2 adjacent sides, corner between them water (2×2 block) | `River3B` (Rounded) / `River3C` (Sharp) | water corner: SE 0, NE 1, NW 2, SW 3 |
| 3 sides, both corners between water sides dry | `River3A` | land side: W 0, S 1, E 2, N 3 |
| 3 sides, one corner between water sides water | `River4A` (SE filled) / `River4B` (NE filled) | land side: W 0, S 1, E 2, N 3 |
| 3 sides, both corners between water sides water | `River5B` | land side: W 0, S 1, E 2, N 3 |
| 4 sides, no water diagonals | `River4C` | 0 |
| 4 sides, one water diagonal | `River5A` | water corner: SW 0, SE 1, NE 2, NW 3 |
| 4 sides, two opposite water diagonals | `River6A` | NE/SW 0, NW/SE 1 |
| 4 sides, two adjacent water diagonals | `River6B` | dry side: S 0, E 1, N 2, W 3 |
| 4 sides, three water diagonals | `River7A` | dry corner: SW 0, SE 1, NE 2, NW 3 |
| 4 sides, four water diagonals | `River8A` | 0 |

The save's three-tile L bends are all `River2C`; the 2×2 block corners are `River3C`, and a full
3×3 water body has `River3B` corners. The 3×3 band in the second dump is a square with two opposite
corners dry, so its corners are `River3C` and its centre is `River6A`. So B and C are the
rounded/square bank of the same silhouette, not a different shape. The game wrote `River2C` for all
four L orientations, so the painter always uses `2C` for an L; **Corners** writes `River3B` when
Rounded and `River3C` when Sharp for a 2×2 corner. The 3-sided `River4A` and `River4B` differ only
in which of the two water-side corners is filled; the land-side corners never matter, because the
game stored `River3A` for a T with zero, one and two wet land-side corners. An existing bend keeps
its rounded/sharp style when a later stroke re-tiles the ring; an existing `River2B` from an older
build is also kept.

The second dump measured `River6A` on a plus with opposite water corners (NE and SW water, NW and SE
dry). `River4C` is the 1×1 water tile with all four sides water and all four diagonals dry, measured
on the new G6 acre and previously on the 3×3 pad's middle tile. Four water sides with two or three
dry diagonals are now `River6A`/`River6B`/`River5A` instead of open water.

The 15 base models taken together cover all 47 distinct side/corner classes, and therefore all 256
8-neighbour water masks: the other 209 masks differ only in diagonals on a land side, which are
bank. `the_river_table_covers_every_flat_water_mask` in `paint.rs` enumerates the classes, and
`measured_river_shapes_match_the_game_dump` pins the measured models. The only river model never
produced by the game is `River2B`, the atlas's 45° channel bank; the editor does not emit it. There
is no flat water shape left untested: the editor's L, T and plus output for these classes loaded in
game on 2026-11-25 without a broken bank, corner or void.

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
rotation; the other three were measured). `ccw` below is the strip end 90° counter-clockwise from
the drop: west for a north drop, north for an east drop, east for a south drop, south for a west
drop.

**A fall needs no water behind it and no pool below it.** A player creates a waterfall by standing
on the plateau and painting water on the top edge: the water tile becomes the fall, the tile behind
it stays dry grass or cliff, and the drop can end on dry land. The game then draws the fall recessed
into the cliff face. Water behind the fall is the ordinary river case, not a requirement.

Two neighbours pick each tile's family: whether there is water behind it (a river at the same
height, or a fall one level up) and whether there is water at its base (a river, or the fall below
in a cascade). The position in the strip then picks the variant.

Those two neighbours are only the *middle* of the two edges the game models. Each edge is a 4-bit
mask whose corner bits are the two diagonals: a fall whose lip or base meets land on a diagonal needs
a different model, and the four measured families above are only the all-water and no-water corners
of that table. The catalogue is in **Fall model catalogue** below.

| Behind the fall | Base | ccw end | cw end | Middle |
|---|---|---|---|---|
| water, or a fall one level up | water, or a fall below | `Fall200` | `Fall300` | `Fall400` |
| water, or a fall one level up | dry land | `Fall202` | `Fall302` | `Fall403` |
| dry land or cliff | water, or a fall below | `Fall204` | `Fall304` | `Fall408` |
| dry land or cliff | dry land | `Fall208` | `Fall308` | `Fall424` |

So a plain 4-wide south-dropping fall into a river is `300, 400, 400, 200` west to east, the same
fall onto dry land is `308, 403, 403, 208`, and a dry-back fall into a river is
`304, 408, 408, 204`.

The family is picked per tile, so a strip can begin recessed and continue as a cascade: the tile
directly inside an end whose model is `Fall208`/`Fall308` (dry land behind that end) becomes
`Fall422` on the ccw side or `Fall421` on the cw side.

Measured in game on a real 3.0.3 save (2026-11-25): a two-level cake with 4-wide falls on all four
sides of both tiers gave every direction and the flush cascade, a recessed pad gave the `Fall424`
middle and the dry-back-into-water ends, the two-level gorge gave the staggered ends and the
`422`/`421` blends, and earlier pads gave the plain strip. Re-deriving every fall tile of the
sample save from these rules matches all 81 of them, model and angle.

A cascade that drops into water uses the plain family even with a higher fall behind it. That is the
2026-11-24 seam context: the editor wrote `Fall208`/`Fall422` at a flush cascade end, which is the
recessed art, and that mismatch was the grass seam. The models now match the game's own.

A one-tile fall (a strip of length 1) does not use the strip table. Its model is a 2×2 table of
whether water is behind and below it, measured from a game-made reference save in all four
directions:

| Behind the fall | Below the fall | Model |
|---|---|---|
| water at the same height, or a fall one level up | water, or a fall one level down | `Fall100` |
| water at the same height, or a fall one level up | dry land | `Fall101` |
| dry land or cliff at the same height | water, or a fall one level down | `Fall102` |
| dry land or cliff at the same height | dry land | `Fall103` |

The two levels of a one-tile cascade are the second and third rows in practice: the upper fall has a
dry back and a fall below (`Fall102`), the lower one has a fall behind and dry land below (`Fall101`).

Every one-tile context was dumped from game-placed copies, including the recessed fall (`Fall103`),
the fall with a dry back over water (`Fall102`) and both levels of a one-tile cascade. Writing
`Fall101` for every dry back, the earlier behaviour, makes a recessed fall's back transparent and
picks the wrong model at a one-tile cascade top.

A few states are inferred rather than dumped: a fall with a *same-height* river behind rather than a
higher fall (the one-tile table groups the two, and both masks read the same), `Fall421` on a north
or south drop (the ccw `Fall422` is measured on both axes; the cw blend only on east/west), and the
`Fall408` middle of a dry-back strip over a river (its ends are identical to the cascade tops',
whose middles are `Fall408`).

A corner fall (water dropping in two directions at once) is not generated: there is no corner-fall
model, so the tile stays a river and the toast counts it. A drop of more than one level is refused
(add a one-tile terrace); the height brush already refuses a two-level step, so through the editor
this refusal is unreachable and only protects a hand-edited dump.

Consecutive falls (one fall landing on the next) are generated on every level. The lower strip of a
cascade keeps `Fall403` in its middle, and a strip that drops into water keeps `Fall400`.

### Fall model catalogue

The game ships a parameter table for every terrain model (`FieldLandMakingUnitModelParam`, CSV at
`https://wuffs.org/acnh/bcsv_150/csv/FieldLandMakingUnitModelParam.csv`). Each row carries a
32-value signature: two layers × 4×4 sub-cells (ground layer `0` base / `1` river; wall layer `1`
fall water / `3` cliff / `4` nothing above). The 47 fall models are grouped by strip position (one
tile, the two ends, the middle), and inside each group they are a product of two edge masks,
`[W corner, W mid, E mid, E corner]`:

- the **back mask**: the lip's four sub-cells. The two middle bits are the tile's own column above
  (water behind or not); the corner bits are the two diagonals behind it.
- the **base mask**: the same four sub-cells below the tile, with the diagonals below in the corners.

A dry middle forces `0000`, so each mask has exactly five states: `1111` (both diagonals wet), `1110`
(west diagonal dry), `0111` (east diagonal dry), `0110` (both dry), `0000` (no water at all). Those
five states, not a free choice of art, are the model variants. `G` = base index, `U` = back index:

| | U0 `1111` | U1 `1110` | U2 `0111` | U3 `0110` | U4 `0000` |
|---|---|---|---|---|---|
| **G0 `1111`** | 400 | 405 | 406 | 407 | 408 |
| **G1 `1110`** | 401 | 414 | 413 | 415 | 416 |
| **G2 `0111`** | 402 | 410 | 409 | 411 | 412 |
| **G3 `0110`** | 404 | 418 | 417 | 419 | 420 |
| **G4 `0000`** | 403 | 422 | 421 | 423 | 424 |

The strip ends use a 3×3 subset: every end model has the sub-cell on the strip's closed side dry, so
`2xx` takes masks 1, 3, 4 and `3xx` takes 2, 3, 4, both as `index = 3·G + U`. One-tile falls use the
`0110`/`0000` pairs that are the measured `Fall100`–`Fall103` table, `index = 2·G + U`. The `4xx`
names are not a formula — the sheet order is 9 + 16 and four name pairs are swapped against the
product order (409/410, 413/414, 417/418, 421/422) — so the matrix above is the reference.

**The generator reads both edge masks.** A middle tile's model is the matrix entry for its own
`(base, back)` masks, read from the 3×3 neighbourhood; a dry middle bit forces `0000`. The six
one-corner middles (`401`/`402`/`404`/`405`/`406`/`407`) were confirmed by the 2026-11-28 build: all
13 predicted models came back, including the `400`/`403`/`408` controls and both `300`/`200` ends.
The ends still use the family table above, and the one-tile table is unchanged.

The middle cross terms (`409`–`420`, `423`, where both edges have a dry diagonal) come from the
catalogue's signature table and are not separately dumped; the mask reading and the sub-cell order
are. The ends' one-corner states (`201`/`203`/`205`–`207`, `301`/`303`/`305`–`307`) are unmeasured
and not written: an end whose closed-side corner is dry keeps its family's end model.

Checked against `save-ref-1wide`: T34,23 is `Fall300` with back `0111` and base `0111`, T37,23 is
`Fall200` with `1110`/`1110`, and both match the matrix exactly. That also fixes the sub-cell order:
sub-cell 0 is the west side for a south drop (angle 0). The 13-tile build in
`docs/local/fall-corner-probes.md` (fork-only) then confirmed all six one-corner middles in one
dump; every column matched its predicted model.


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
- `paint_road` skips water tiles, so a path stroke cannot write a road layer where the game has
  none. Erasing a path on water still works, so a save painted before this rule can be cleaned up.
- Only acre-template type-7 tiles are painted. `paint_road`, `paint_water` and `paint_height` filter
  the target list, the UI leaves the rest out of the preview, and the two terrain commands restore
  every fixed tile after the ring pass, so a stroke cannot change beach, sea, rocks, a tide pool or a
  river mouth. Erase is filtered the same way. Ground designs use `design_tile_is_water` instead, so
  sand keeps designs while rivers, sea and tide pools do not. Undo still restores a whole snapshot.
- The Files tab's whole-terrain dump import and the Fix one tile stamp write bytes directly and skip
  both checks.

### Height display

The terrain preview uses four clearly stepped grass colours (dark low, pale high) and dirt/peach
cliff faces. Cliff tiles draw their wall on the low sides: `Cliff3B` shows a rounded outside corner
and `Cliff3C` a sharp one, so the **Corners** choice is visible before saving. The map canvas draws
a contour line where two terrain tiles differ in elevation, the bottom-left readout shows
`Height N`, and the View menu has a **Height contours** toggle.

The save field is a `u16`, but the game only supports elevations 0–3. The brush refuses to write
above 3 and refuses to go below 0. A save that already contains a higher value is left untouched:
the map editor warns on load, names the value in the Terrain panel, and keeps showing the tile.

### Limits

- Painting water within one tile of an existing multi-level waterfall fails the >1-level check
  (nothing is written). Paint away from it, or fix the tiles by hand.
- A tile that is already a river bend keeps its rounded/sharp style when a later stroke re-tiles the
  ring, so a hand-picked `River3C` survives; this covers both the L pair (`2B`/`2C`) and the 2×2
  pair (`3B`/`3C`). Hand-set fall models (any model the autotiler never writes, such as `Fall205`)
  and `Cliff3C` corners are kept the same way. Use **Fix one tile** to change a bend or a corner by
  hand.
- A saddle (two low diagonals, no low side) or a strip with no connected same-height side is still
  refused. Single tiles, 2×1 strips, 1-wide ridges and 2×2-or-larger areas all work.
- A one-tile fall that drops into a multi-tile pool can sit over a 4-water-side/2-land-diagonal
  river junction. The generator fills it as `River6A`/`River6B`, both measured in game.
- A fall's model comes from both edge masks, so a lip or base that meets land on a diagonal gets the
  catalogue's model. The middle cross terms (`409`–`420`, `423`) and the ends' one-corner states are
  catalogue-only, not separately dumped; see **Fall model catalogue**.
- The generator writes the rounded `Cliff3B` style for every 2-adjacent-low corner unless the
  **Corners** choice is Sharp (`Cliff3C`).
- **Corners** styles a path bend whose inside diagonal is empty with `2B` (rounded) or `2C` (sharp),
  and one whose inside diagonal is also path with `3B`/`3C`. The river brush uses `River2C` for a
  three-tile L and `3B`/`3C` for a 2×2 corner. A neighbour-only stroke keeps the existing style.
- Default paths cannot be written on beach sand or the open sea, because the acre template owns
  those tiles; ground designs can go on sand but not on water, sea or a tide pool (see above).

## Verified in game

Building examples measured from the sample save:

| Structure | Centre | Plot | Door |
|---|---|---|---|
| PlayerHouse1 | (121,134) | 116..125 × 130..137 | 119..122 × 136..138 |
| Nook's Cranny | (75,190) | 68..81 × 186..193 | 73..76 × 192..193 |
| Museum | (93,164) | 86..99 × 160..167 | 91..94 × 168 |

Confirmed in game: single and multi-tile cliffs, heights 2–3, rivers, sharp and rounded cliff and
river corners, straight, recessed and consecutive waterfalls, painted paths and ground designs,
moved buildings and the tree/building warning. Building drag handles only move movable kinds: a
fixed structure no longer moves when its door is dragged, and a villager house dragged by its door
loaded on its new plot. A bridge angle change of 1 → 0 rotated the model 90° in game but left the
river in place, so the bridge lay parallel to the river and could not be crossed.

The generator's cliff shapes were checked against an emulator shape sheet: 1×1 stays `Cliff0A` at
elevations 1–3, 2×1 stays `Cliff1A`, a 3×1 ridge stays `Cliff2A`, 3×3 stays `3B`/`5B`/`Base`. The
only re-derive differences are angular `Cliff2C`/`Cliff3C` corners that the generator draws as
rounded `3B`. No save data layout changed.

The 2026-11-24 pass re-checked the ground and terrain brushes in game (3.0.3): all eight default
path families and their joins, designs over paths, the erase brushes, the outer-edge refusal,
raise/lower including the 0–3 limit, rivers with banks, and recessed and consecutive falls. It found
that terrain and water could be painted on beach and sea (invisible in game, with no collision),
that designs floated on water, and that a three-tile *path* L stayed square because the editor wrote
`2C`. The game rewrote those path Ls to `2B` when it loaded the save and they render rounded, which
is why the path painter now uses `2B`/`2C`. The waterfall models were fixed the next day (see
below); the open seam turned out to be the editor's `Fall208`/`Fall422` at a flush cascade end.

The 2026-11-25 pass painted 17 water shapes with the in-game water tool on a flat acre G6 and
dumped the save back: four three-tile Ls (one per orientation), a T with no water-side corners, a T
with one and with two, two T variants with wet land-side corners, a plus with no diagonals, a plus
with one, two adjacent and three water diagonals, a full 2×2-armed junction, open water, and a
3×3 body with rounded corners. A second dump added a plus with opposite water corners and rebuilt
the 3×3 body as a 3×3 band. Every flat model in the river table above came out of those dumps; the
L bends were `River2C` in all four orientations, and the old NW-corner void was the editor writing
the 2×2 `River3B` for an L. The 3×3 band is a 3×3 square with two opposite corners dry: its four
corner tiles are `River3C` and its centre is `River6A`.

The same day, editor-written L, T and plus junctions loaded in game (3.0.3) with no broken bank,
corner or void, which closes the flat-water work. Rotated orientations were not loaded separately:
`4A` was dumped at two angles, `6A` at two and `2C` in all four, and the other rotations follow the
same side/corner mapping.

The 2026-11-25 fall pass then built a two-level cake and a recessed pad on acre G5, loaded them in
game, and dumped the result. The cake (4-wide falls on every side of both levels) gave every
direction's cascade family and proved the flush ends are `Fall202`/`Fall302` with `Fall403` between,
not the `Fall208`/`Fall422` the editor had been writing for a flush strip. The pad gave `Fall424`
as the recessed middle and `Fall304`/`Fall204` for a dry-back fall into a river. A staggered upper
strip gave `Fall422` next to a recessed end for a north drop as well. The pool added at the cake's
foot showed a cascade over water is the plain `Fall200`/`Fall400`/`Fall300` family, and re-deriving
every fall tile of the save matches all 81 of them, model and angle (`probe -- save check`).

The 2026-11-28 pass built the 13-tile one-corner sheet from `docs/local/fall-corner-probes.md` on
acre G8 and saved in game. Every tile matched its prediction, `300, 401, 403, 404, 403, 402, 405,
408, 407, 408, 406, 400, 200` west to east, which confirms the mask reading, the sub-cell order and
the six one-corner middle models. The generator now reads both masks; `probe check` re-derives all 55
fall tiles of that build save and all 42 of `save/` with no mismatch.

## Open questions

- The middle cross terms (`409`–`420`, `423`, where both fall edges have a dry diagonal) come from
  the catalogue's signature table, not a separate dump, and the ends' one-corner states
  (`201`/`203`/`205`–`207`, `301`/`303`/`305`–`307`) are unmeasured. The six middle one-corner
  states were confirmed on 2026-11-28 and are now generated; loading an editor-written cross term in
  game would close the middle group.
- No `2xx`/`3xx` model has a wet sub-cell on the strip's closed side, so an end tile whose pool or
  river behind continues past the strip's end has no model in its own family. What the game writes
  there is unknown; the probe layout avoids the case.
- The `4A`/`5A`/`6A` *path* junction notches are still unmeasured; a crossing loses its notch.
  Measuring them is the fix.
- Unmeasured `Cliff2C`/`Cliff3A`/`Cliff4x`/`Cliff5A`/`Cliff6x` style variants.
- Building sizes and door data are incomplete for some structures. The angle exemption for bridges
  and inclines is real data, but rotating one without rotating its river or cliff leaves a structure
  the player cannot use.

