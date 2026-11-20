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
| 24 | Able Sisters | 5×4 | whole plot |
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

The Item panel has a **Ground** section under Brush: the eight default path families, the save's
non-empty normal designs, an erase swatch and Stop. Choosing one puts the Place tool into ground
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

## Verified examples

From the real save on 2026-11-19:

- PlayerHouse1 at (121,134) → plot 116..125 × 130..137, door 119..122 × 136..138.
- Nook's Cranny at (75,190) → plot 68..81 × 186..193, door 73..76 × 192..193.
- Museum at (93,164) → plot 86..99 × 160..167, door 91..94 × 168.

## Status

Builds clean and the map visuals were confirmed in the app on Windows (2026-11-20), including
design thumbnails with transparency. Doors and drag were added from in-game measurements. No
in-game test yet, and no save data layout changed.

Done in this pass: translucent door plates with a one-tile door glyph and four-way move badge (only
the first part of a multi-part door gets the glyph, so Able Sisters shows one icon), villager-house
body rectangles (`building.rs::body`), shop fills moved off green — they used to blend into the
grass — to crimson `[211,61,61]` in `buildingColor()` in `src/pages/MapEditor.tsx`, normal
ground designs drawn from their thumbnails (with a View toggle and hover index), and ground
painting in the Item panel (`map/paint.rs` + the `paint_road` command): default paths autotile from
their neighbours, normal designs paint straight onto `MyDesignMap`, both replace each other, and
each stroke is one undo entry.

Next: test the painted paths and designs in-game, verify the remaining building sizes/positions,
fill in door data for the fixed structures, and check dragging on the bridge (the one building with
a non-zero `angle`).

## Remaining goals (not started — handoff notes)

1. **Terrain/water/cliff painting.** Ground painting covers the road layer and custom designs; it
   does not change `TerrainTile.model` (rivers, cliffs, grass elevation). The Files tab still
   imports and exports whole terrain dumps. No implementation started.
