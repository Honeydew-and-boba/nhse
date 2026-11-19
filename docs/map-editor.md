# Map editor

Notes on the building layer of `src/pages/MapEditor.tsx`. Footprints and doors are draw-only:
no save layout changed. The code lives in
`crates/nhse-core/src/structs/building.rs`, `src/pages/MapEditor.tsx` and
`src-tauri/src/cmd/data.rs`.

## Coordinate conventions

- Building `x`/`y` are the **centre** of the structure in absolute map item tiles (32 per acre,
  whole map 288×256).
- One terrain tile = 2 item tiles. The map editor's bottom-left readout is in layer item tiles:
  `layer Y + 32 = absolute Y` for the 9×6 field layer.
- `building.rs::footprint` returns `(width, height)` in **terrain** tiles, so the plot covers
  `x-w … x+w-1` and `y-h … y+h-1` item tiles (2w × 2h items). The UI reads the table from
  `gd.building_sizes` via `buildingFootprint()` in `src/lib/items.ts`.
- Plots are terrain-aligned: `x % 2 == w % 2` and `y % 2 == h % 2`. Dragging moves in 2-item
  steps, which preserves that.
- `Plaza { x, z }` stores the top-left in absolute item tiles. The editor draws a neutral 24×20
  item tile (12×10 terrain) overlay from it. The plaza is not selectable or editable. The probe
  save's value is X108, Y130 in layer coords (absolute z 162).

## Footprints

| Kinds | Structure | Plot (terrain tiles) |
|---|---|---|
| 1–8 | Player houses | 5×4 |
| 9–18 | Villager houses | 4×4 |
| 19 | Nook's Cranny | 7×4 |
| 20 | Resident Services | 6×3 (the 12×10 terrain plaza is separate) |
| 21 | Museum | 7×4 |
| 22 | Airport | 10×6 (approximate) |
| 23 | Resident Services tent | 6×4 (approximate) |
| 24 | Able Sisters | 5×4 |
| 25 | Campsite | 4×4 |
| 26 | Bridge | 4×6 (approximate) |
| 27 | Incline | 2×5 (approximate) |
| 28 | Redd's Treasure Trawler | 6×4 (approximate) |
| 29 | Photo studio | 5×4 (approximate) |
| 42 | Hotel | 8×6 (approximate) |
| other | Unknown | 2×2 |

Player houses, villager houses, Nook's Cranny, the museum, Able Sisters and the campsite are
verified; the rest are drawn as approximations and never written back.

## Doors and move handles

`building.rs::doors(kind)` returns `DoorRect { dx, dy, w, h }` in item tiles from the building
centre. The UI reads them through `gd.building_doors` / `buildingDoors()`. They are amber rects
with a circular drag handle.

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
  Select hit-tests the whole plot.
- Drag a door marker (or the neutral centre handle) with the Select tool to move the building in
  2-item (one terrain tile) steps, clamped to the field layer. The position is committed once on
  release, so a drag only writes the save when the pointer is lifted.
- Moveable kinds (`building.rs::movable`, mirrored as `gd.movable_buildings`): player and villager
  houses, Nook's Cranny, the museum, Able Sisters and the campsite. They show an amber warning that
  terrain, rivers, cliffs, trees, furniture and other buildings do not move with them.
- Fixed kinds (Resident Services, airport, tent, bridge, incline, Redd's trawler, studio, hotel)
  show a red warning. Dragging is not blocked because this is a save editor; the numeric X/Y fields
  and "Pick position" work for every building.
- Warnings are static text. There is no live scan for rivers, trees or overlapping plots.

## Verified examples

From the real save on 2026-11-19:

- PlayerHouse1 at (121,134) → plot 116..125 × 130..137, door 119..122 × 136..138.
- Nook's Cranny at (75,190) → plot 68..81 × 186..193, door 73..76 × 192..193.
- Museum at (93,164) → plot 86..99 × 160..167, door 91..94 × 168.

## Status

Builds clean and the rectangles/positions look right in the app. Doors and drag were added from
in-game measurements. No in-game test yet, and no save data layout changed.

Next: test in-game, verify the remaining building sizes/positions, fill in door data for the fixed
structures, and check dragging on buildings with a non-zero `angle`.

Polish backlog: shop colours blend into the grass (green), door markers look neither grabbable nor
like doors, and custom design path patterns are not drawn correctly in the map editor.
