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
> corners, straight, recessed and consecutive falls, painted paths and ground designs, three-tile
> L bends (`2B`/`2C`), and moving buildings. Not verified in game: the river junction models the
> sample save never used (see Measured river table), and the template/design gates below, which
> landed after the 2026-11-24 pass. **Back up the save before terrain work.**

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
  `River3B` (rounded) or `River3C` (sharp) for new bends, and a bend that already exists keeps its
  style. Painting over a cliff face is allowed only when that face drops one level into water; it
  becomes a waterfall. Water clears the road layer and any ground design on the painted tiles.
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
| 4 sides, all four land diagonals (a 1×1 pond) | `River4C` | 0 |
| 4 sides, one land diagonal | `River7A` | land corner: SW 0, SE 1, NE 2, NW 3 |

`River3C` (sharp bend) is written when **Corners** is set to Sharp; an existing bend keeps its
style. Four water sides with two or three land diagonals still need junction models this save never
showed; the generator fills them as `River8A` (open water) and the shape fixer can replace them.
`River4C` was measured on the test acre's 3×3 pad, whose middle is water with falls on all four
sides and cliff corners on all four diagonals; the top of a one-tile-wide cascade has the same shape.

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
`Fall101`. For example, a south-dropping 4-wide fall is `300, 400, 400, 200` west to east.

**A fall needs no water behind it and no pool below it.** A player creates a waterfall by standing
on the plateau and painting water on the top edge: the water tile becomes the fall, the tile behind
it stays dry grass or cliff, and the drop can end on dry land. The game then draws the fall recessed
into the cliff face. Water behind the fall is the ordinary river case, not a requirement, and it was
wrong to refuse the dry one.

The model follows what is behind and below the fall. All of these were measured on a dedicated test
acre of a real 3.0.3 save (acre G6: four 2×4 pads with a 2-wide fall on each side, a 3×3 pad with a
1×1 fall on each side, and a two-tier "cake" with 2-wide falls on every side of both tiers), plus
the two-level gorge:

| Behind the fall | Below the fall | Ends (CCW / CW) | Middle |
|---|---|---|---|
| river at the same height, or a fall one level up | water, or the fall below in a cascade | `Fall200` / `Fall300` | `Fall400` |
| dry land or cliff at the same height (recessed) | dry land | `Fall208` / `Fall308` | `Fall400` (unmeasured) |
| dry land or cliff at the same height | the fall below (cascade top) | `Fall204` / `Fall304` | `Fall408` |
| a fall one level up (cascade bottom) | dry land | `Fall208` (east drop) / `Fall302`; `Fall202` (west drop) / `Fall308` | `Fall403`, with `Fall422` (east drop) or `Fall421` (west drop) second from the north end |

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

Re-deriving every fall tile of the sample save from the rules above reproduces all 74 of them,
model and angle, including both gorge tiers and every pad in the test acre. Contexts the save never
showed (a north- or south-dropping cascade bottom, a recessed strip longer than two tiles, a dry
back over a pool) fall back to the plain `Fall200`/`Fall300`/`Fall400` strip, which is verified in
game, so those are the cases to look at first if a fall looks wrong.

A corner fall (water dropping in two directions at once) is not generated: there is no corner-fall
model, so the tile stays a river and the toast counts it. A drop of more than one level is refused
(add a one-tile terrace); the height brush already refuses a two-level step, so through the editor
this refusal is unreachable and only protects a hand-edited dump.

Consecutive falls (one fall landing on the next) are generated on every level. The measured cascade
variants are in the model list; the lower strip of a cascade keeps `Fall403` in its middle on every
level.


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
  ring, so a hand-picked `River3C` survives. Hand-set fall models (any model the autotiler never
  writes, such as `Fall205`) and `Cliff3C` corners are kept the same way. Use **Fix one tile** to
  change a bend or a corner by hand.
- A saddle (two low diagonals, no low side) or a strip with no connected same-height side is still
  refused. Single tiles, 2×1 strips, 1-wide ridges and 2×2-or-larger areas all work.
- A one-tile fall that drops into a multi-tile pool can sit over an unmeasured 4-water-side/2-land-
  diagonal river junction. The generator fills that base as `River8A`, so open-water waves can show
  under the fall even though the fall model is correct. Measuring the missing junction models is the
  fix.
- The generator writes the rounded `Cliff3B` style for every 2-adjacent-low corner unless the
  **Corners** choice is Sharp (`Cliff3C`).
- **Corners** styles a path bend whose inside diagonal is empty with `2B` (rounded) or `2C` (sharp),
  and one whose inside diagonal is also path with `3B`/`3C`. A neighbour-only stroke keeps the
  existing style.
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
that designs floated on water, and that a three-tile L stayed square because the editor wrote `2C`.
The game rewrote those Ls to `2B` when it loaded the save and they render rounded, which is why the
painter now uses `2B`/`2C`. The template gate, the design gate and the top-bar brush deselection
landed after the pass and have not been re-tested in game; the two water/dry intersections under
Open questions remain.

## Open questions

- Two water/dry intersection glitches seen in game on 2026-11-24, both model-table gaps rather than
  refused strokes: (a) water to the north and west meeting dry land to the south and east draws no
  backing terrain at the north-west corner; (b) a cascade lower strip with a higher waterfall to the
  west, dry land to the north, a same-height waterfall to the south and lower water to the east
  shows grass where the west edge meets the higher fall.
- Unmeasured river junction models (`4A`, `4B`, `5A`, `6A`, `6B`) and the `4A`/`5A`/`6A` path junction
  notches. The generator fills them with open water or the fully filled crossing, so a base can show
  waves under a fall and a crossing can lose its notch. Measuring them is the fix.
- Unmeasured `Cliff2C`/`Cliff3A`/`Cliff4x`/`Cliff5A`/`Cliff6x` style variants.
- Building sizes and door data are incomplete for some structures. The angle exemption for bridges
  and inclines is real data, but rotating one without rotating its river or cliff leaves a structure
  the player cannot use.

