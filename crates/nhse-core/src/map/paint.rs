//! Painting terrain: the road layer, rivers, cliffs and waterfalls.

use super::terrain::{is_cliff, is_fall, is_river, is_road, road_family, TerrainTile};
use crate::data::tables::enum_value;
use std::collections::BTreeSet;

/// Side bits, clockwise from north.
const N: u8 = 1;
const E: u8 = 2;
const S: u8 = 4;
const W: u8 = 8;
/// Corner bits, clockwise from north-east.
const NE: u8 = 1;
const SE: u8 = 2;
const SW: u8 = 4;
const NW: u8 = 8;

/// One road model in its base orientation: suffix, connected sides, filled outer corners.
/// The set is enough to cover every 8-neighbour configuration; see `docs/map-editor.md`.
const MODELS: [(&str, u8, u8); 15] = [
    ("0A", 0, 0),
    ("1A", S, 0),
    ("2A", N | S, 0),
    ("2C", E | S, 0),
    ("3C", E | S, SE),
    ("3A", N | E | S, 0),
    ("4A", N | E | S, SE),
    ("4B", N | E | S, NE),
    ("5B", N | E | S, NE | SE),
    ("4C", N | E | S | W, 0),
    ("5A", N | E | S | W, SW),
    ("6B", N | E | S | W, NE),
    ("6A", N | E | S | W, NE | SW),
    ("7A", N | E | S | W, NE | SE),
    ("8A", N | E | S | W, NE | SE | SW | NW),
];

fn prefix(family: &str) -> Option<&'static str> {
    Some(match family {
        "soil" => "Soil",
        "stone" => "Stone",
        "brick" => "Brick",
        "darksoil" => "DarkSoil",
        "pattern" => "FanPattern",
        "sand" => "Sand",
        "tile" => "Tile",
        "wood" => "Wood",
        _ => return None,
    })
}

/// Enum id of `Road<Prefix><suffix>`, the road family's model for a shape.
fn model_id(family: &str, suffix: &str) -> Option<u16> {
    let name = format!("Road{}{}", prefix(family)?, suffix);
    Some(enum_value("TerrainUnitModel", &name)? as u16)
}

/// `road_angle` rotates the model counter-clockwise by 90 degrees per step.
fn rotate_sides(mut sides: u8, steps: u8) -> u8 {
    for _ in 0..steps {
        sides = (sides >> 1) | (sides << 3);
        sides &= 0xF;
    }
    sides
}

fn rotate_corners(mut corners: u8, steps: u8) -> u8 {
    for _ in 0..steps {
        corners = (corners >> 1) | (corners << 3);
        corners &= 0xF;
    }
    corners
}

/// Corners that touch two connected sides; outer corners next to a missing side are ground.
fn relevant_corners(sides: u8) -> u8 {
    let mut v = 0;
    for (bit, a, b) in [(NE, N, E), (SE, E, S), (SW, S, W), (NW, W, N)] {
        if sides & a != 0 && sides & b != 0 {
            v |= bit;
        }
    }
    v
}

/// Road model and angle for a tile of `family` with the given neighbours.
/// Returns `None` for an unknown family. Side and corner bits are the game's 8-way path
/// connections; only corners between two connected sides matter.
pub fn road_autotile(family: &str, sides: u8, corners: u8) -> Option<(u16, u16)> {
    let corners = corners & relevant_corners(sides);
    for (suffix, base_sides, base_corners) in MODELS {
        for angle in 0..4 {
            if rotate_sides(base_sides, angle) == sides
                && rotate_corners(base_corners, angle) == corners
            {
                return Some((model_id(family, suffix)?, angle as u16));
            }
        }
    }
    // A crossing with three diagonal neighbours is drawn as a full crossing; the game's
    // model set has no partial one, and one extra corner is invisible at map scale.
    if sides == N | E | S | W {
        return Some((model_id(family, "8A")?, 0));
    }
    None
}

fn neighbours(
    tiles: &[TerrainTile],
    w: usize,
    h: usize,
    x: usize,
    y: usize,
    family: &str,
) -> (u8, u8) {
    let mut sides = 0;
    let mut corners = 0;
    for (dx, dy, side, corner) in [
        (0i64, -1i64, N, 0),
        (1, 0, E, 0),
        (0, 1, S, 0),
        (-1, 0, W, 0),
        (1, -1, 0, NE),
        (1, 1, 0, SE),
        (-1, 1, 0, SW),
        (-1, -1, 0, NW),
    ] {
        let nx = x as i64 + dx;
        let ny = y as i64 + dy;
        if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
            continue;
        }
        let t = &tiles[nx as usize * h + ny as usize];
        if is_road(t.road) && road_family(t.road) == Some(family) {
            sides |= side;
            corners |= corner;
        }
    }
    (sides, corners)
}

/// Applies or clears the road layer on `targets` (terrain tile coordinates), then re-tiles
/// them and their 8 neighbours so joins stay correct.
///
/// `None` removes the road; the model, elevation and variation of the ground are untouched.
/// Returns the number of terrain tiles whose road data changed.
pub fn paint_road(
    tiles: &mut [TerrainTile],
    w: usize,
    h: usize,
    targets: &[(usize, usize)],
    family: Option<&str>,
) -> usize {
    if tiles.len() != w * h {
        return 0;
    }
    let marker = match family {
        Some(f) => match model_id(f, "0A") {
            Some(v) => v,
            None => return 0,
        },
        None => 0,
    };

    let mut affected = std::collections::BTreeSet::new();
    let mut changed = std::collections::BTreeSet::new();
    for &(x, y) in targets {
        if x >= w || y >= h {
            continue;
        }
        let t = &mut tiles[x * h + y];
        if t.road != marker || t.road_angle != 0 || t.road_variation != 0 {
            changed.insert((x, y));
            t.road = marker;
            t.road_angle = 0;
            t.road_variation = 0;
        }
        let x0 = x.saturating_sub(1);
        let y0 = y.saturating_sub(1);
        for nx in x0..=(x + 1).min(w - 1) {
            for ny in y0..=(y + 1).min(h - 1) {
                affected.insert((nx, ny));
            }
        }
    }

    for (x, y) in affected {
        let Some(fam) = road_family(tiles[x * h + y].road) else {
            continue;
        };
        let (sides, corners) = neighbours(tiles, w, h, x, y, fam);
        if let Some((model, angle)) = road_autotile(fam, sides, corners) {
            let t = &mut tiles[x * h + y];
            if t.road != model || t.road_angle != angle {
                t.road = model;
                t.road_angle = angle;
                changed.insert((x, y));
            }
        }
    }
    changed.len()
}

// --- rivers, cliffs and waterfalls -------------------------------------------
//
// Model and angle tables below were measured from a real 3.0.3 save; see
// `docs/map-editor.md`. Only the shapes the game actually used are emitted. A mask that would
// need one of the unmeasured junction/nub models falls back to the closest measured shape and
// is left for the shape override.

/// Highest elevation the game supports. The field is a `u16`, but the game only renders 0–3.
const MAX_ELEVATION: i32 = 3;

fn terrain_model(name: &str) -> u16 {
    enum_value("TerrainUnitModel", name)
        .unwrap_or_else(|| panic!("bundled TerrainUnitModel is missing {name}")) as u16
}

fn is_water(t: &TerrainTile) -> bool {
    is_river(t.model) || is_fall(t.model)
}

/// Fall models the autotiler manages. Any other fall model is a hand-set stamp and is left alone
/// by later water strokes.
fn is_managed_fall(model: u16) -> bool {
    model == terrain_model("Fall200")
        || model == terrain_model("Fall300")
        || model == terrain_model("Fall400")
}

fn in_bounds(x: i64, y: i64, w: usize, h: usize) -> bool {
    x >= 0 && y >= 0 && x < w as i64 && y < h as i64
}

fn tile(tiles: &[TerrainTile], w: usize, h: usize, x: i64, y: i64) -> Option<&TerrainTile> {
    in_bounds(x, y, w, h).then(|| &tiles[x as usize * h + y as usize])
}

fn side_angle(mask: u8) -> Option<u16> {
    // West 0, south 1, east 2, north 3.
    Some(match mask {
        W => 0,
        S => 1,
        E => 2,
        N => 3,
        _ => return None,
    })
}

/// `River1A` angle points at the open (water) side: south 0, east 1, north 2, west 3.
fn open_side_angle(mask: u8) -> Option<u16> {
    Some(match mask {
        S => 0,
        E => 1,
        N => 2,
        W => 3,
        _ => return None,
    })
}

/// `River3B` angle points at the water corner: south-east 0, north-east 1, north-west 2, south-west 3.
fn water_corner_angle(corner: u8) -> Option<u16> {
    Some(match corner {
        SE => 0,
        NE => 1,
        NW => 2,
        SW => 3,
        _ => return None,
    })
}

/// `River7A`/`Cliff7A` angle points at the land (nub/high) corner: SW 0, SE 1, NE 2, NW 3.
fn land_corner_angle(corner: u8) -> Option<u16> {
    Some(match corner {
        SW => 0,
        SE => 1,
        NE => 2,
        NW => 3,
        _ => return None,
    })
}

/// `Cliff1A` angle points at the one side connected to the same-height body:
/// south 0, east 1, north 2, west 3.
fn same_side_angle(mask: u8) -> Option<u16> {
    Some(match mask {
        S => 0,
        E => 1,
        N => 2,
        W => 3,
        _ => return None,
    })
}

/// `Cliff2A` angle points at the same-height pair: 0 = north/south, 1 = east/west.
fn same_axis_angle(mask: u8) -> Option<u16> {
    if mask == N | S {
        Some(0)
    } else if mask == E | W {
        Some(1)
    } else {
        None
    }
}

/// `Cliff3B` angle points at the low corner: north-west 0, south-west 1, south-east 2, north-east 3.
fn cliff_corner_angle(corner: u8) -> Option<u16> {
    Some(match corner {
        NW => 0,
        SW => 1,
        SE => 2,
        NE => 3,
        _ => return None,
    })
}

/// Water model and angle for a tile with the given water neighbours.
fn water_shape(sides: u8, corners: u8) -> (u16, u16) {
    match sides.count_ones() {
        0 => (terrain_model("River0A"), 0),
        1 => (
            terrain_model("River1A"),
            open_side_angle(sides).unwrap_or(0),
        ),
        2 => {
            let corner = if sides == N | E {
                NE
            } else if sides == E | S {
                SE
            } else if sides == S | W {
                SW
            } else if sides == W | N {
                NW
            } else {
                0
            };
            if corner != 0 {
                (
                    terrain_model("River3B"),
                    water_corner_angle(corner).unwrap_or(0),
                )
            } else {
                // N|S or E|W: straight channel. `River2A` has no extra corners either way.
                (terrain_model("River2A"), (sides == E | W) as u16)
            }
        }
        3 => (
            terrain_model("River5B"),
            side_angle(!sides & 0xF).unwrap_or(0),
        ),
        _ => {
            // Four sides water. One land diagonal is the measured `7A` nub; the other
            // combinations need models this save never used, so open water is the safe fill.
            let land = !corners & 0xF;
            match land.count_ones() {
                0 => (terrain_model("River8A"), 0),
                1 => (
                    terrain_model("River7A"),
                    land_corner_angle(land).unwrap_or(0),
                ),
                _ => (terrain_model("River8A"), 0), // shortcut: unmeasured multi-nub junction
            }
        }
    }
}

fn water_neighbours(tiles: &[TerrainTile], w: usize, h: usize, x: usize, y: usize) -> (u8, u8) {
    let mut sides = 0;
    let mut corners = 0;
    for (dx, dy, side, corner) in [
        (0i64, -1i64, N, 0),
        (1, 0, E, 0),
        (0, 1, S, 0),
        (-1, 0, W, 0),
        (1, -1, 0, NE),
        (1, 1, 0, SE),
        (-1, 1, 0, SW),
        (-1, -1, 0, NW),
    ] {
        if tile(tiles, w, h, x as i64 + dx, y as i64 + dy).is_some_and(is_water) {
            sides |= side;
            corners |= corner;
        }
    }
    (sides, corners)
}

/// Cliff model and angle for a land tile, or `None` for a shape this editor cannot build.
/// Two kinds of model exist for the same silhouette; `sharp` picks the angular corner style.
fn cliff_shape(low_sides: u8, low_corners: u8, same_sides: u8, sharp: bool) -> Option<(u16, u16)> {
    match low_sides.count_ones() {
        0 => match low_corners.count_ones() {
            0 => Some((0, 0)),
            // The measured save points the angle at the low corner (a0 = low SW), not the high one.
            1 => Some((terrain_model("Cliff7A"), land_corner_angle(low_corners)?)),
            _ => None, // saddle
        },
        1 => Some((terrain_model("Cliff5B"), side_angle(low_sides)?)),
        2 => {
            let corner = if low_sides == N | W {
                NW
            } else if low_sides == W | S {
                SW
            } else if low_sides == S | E {
                SE
            } else if low_sides == E | N {
                NE
            } else {
                // 1-wide ridge: E/W or N/S same-height pair.
                return Some((terrain_model("Cliff2A"), same_axis_angle(same_sides)?));
            };
            let model = if sharp {
                terrain_model("Cliff3C")
            } else {
                terrain_model("Cliff3B")
            };
            Some((model, cliff_corner_angle(corner)?))
        }
        // 2×1 strip: one side connected to the same-height body.
        3 => Some((terrain_model("Cliff1A"), same_side_angle(same_sides)?)),
        // A single raised tile: measured in the sample save at e1, e2 and e3. A same-height
        // diagonal neighbour only changes the corner, not the four walls, so a diagonal run of
        // 1×1 pillars is the same model.
        4 => Some((terrain_model("Cliff0A"), 0)),
        _ => None, // T-junction or saddle
    }
}

/// Re-derives the cliff/grass model of one land tile from its neighbour heights.
/// Returns the first unsupported coordinate, wrapped in `Err`.
fn retile_land_tile(
    tiles: &mut [TerrainTile],
    w: usize,
    h: usize,
    x: usize,
    y: usize,
    sharp: bool,
) -> Result<(), (usize, usize)> {
    let before = tiles[x * h + y];
    if is_water(&before) || !(before.model == 0 || is_cliff(before.model)) {
        return Ok(());
    }
    let e = before.elevation;
    let mut low_sides = 0;
    let mut low_corners = 0;
    let mut same_sides = 0;
    for (dx, dy, side, corner) in [
        (0i64, -1i64, N, 0),
        (1, 0, E, 0),
        (0, 1, S, 0),
        (-1, 0, W, 0),
        (1, -1, 0, NE),
        (1, 1, 0, SE),
        (-1, 1, 0, SW),
        (-1, -1, 0, NW),
    ] {
        match tile(tiles, w, h, x as i64 + dx, y as i64 + dy) {
            Some(n) if n.elevation < e => {
                low_sides |= side;
                low_corners |= corner;
            }
            // A face tile is at the same height as the plateau it belongs to; the connected
            // sides are the model's "high" (body) sides.
            Some(n) if n.elevation == e => same_sides |= side,
            _ => {}
        }
    }
    let Some((model, angle)) = cliff_shape(low_sides, low_corners, same_sides, sharp) else {
        return Err((x, y));
    };
    // A tile that is already a corner keeps its rounded/sharp look; the stroke's style only
    // applies to corners it creates.
    let (rounded, angular) = (terrain_model("Cliff3B"), terrain_model("Cliff3C"));
    let model = if (model == rounded || model == angular)
        && (before.model == rounded || before.model == angular)
    {
        before.model
    } else {
        model
    };
    let t = &mut tiles[x * h + y];
    if model == 0 {
        if t.model != 0 {
            t.model = 0;
            t.variation = 0;
            t.angle = 0;
        }
    } else if t.model != model || t.angle != angle {
        t.model = model;
        t.angle = angle;
        // the game randomises cliff/river variation 0–3 for texture; keep the old value
        // a road on a cliff face would draw over the wall
        t.road = 0;
        t.road_variation = 0;
        t.road_angle = 0;
    }
    Ok(())
}

/// A fall tile at this elevation and angle, if it is part of a straight fall strip.
fn fall_run(
    tiles: &[TerrainTile],
    w: usize,
    h: usize,
    x: usize,
    y: usize,
    angle: u16,
    elevation: u16,
) -> Vec<(usize, usize)> {
    let (dx, dy) = if angle & 1 == 0 { (1i64, 0i64) } else { (0, 1) };
    let same = |px: i64, py: i64| {
        tile(tiles, w, h, px, py).is_some_and(|t| {
            is_fall(t.model) && t.angle & 3 == angle & 3 && t.elevation == elevation
        })
    };
    let mut lo = (x as i64, y as i64);
    while same(lo.0 - dx, lo.1 - dy) {
        lo = (lo.0 - dx, lo.1 - dy);
    }
    let mut hi = (x as i64, y as i64);
    while same(hi.0 + dx, hi.1 + dy) {
        hi = (hi.0 + dx, hi.1 + dy);
    }
    let mut run = Vec::new();
    let mut p = lo;
    loop {
        run.push((p.0 as usize, p.1 as usize));
        if p == hi {
            break;
        }
        p = (p.0 + dx, p.1 + dy);
    }
    run
}

/// Sets the 200/300 end caps and 400 middle of one fall strip.
fn assign_fall_run(
    tiles: &mut [TerrainTile],
    h: usize,
    run: &[(usize, usize)],
    angle: u16,
    elevation: u16,
) -> usize {
    if run.is_empty() {
        return 0;
    }
    let (middle, end_200) = (terrain_model("Fall400"), terrain_model("Fall200"));
    let (first, last) = (run[0], run[run.len() - 1]);
    // The 200 cap sits at the end 90 degrees counter-clockwise from the drop. Measured on the
    // save for south, east and west drops; north follows by rotation.
    let ccw = match angle & 3 {
        0 => (1i64, 0i64),
        1 => (0, -1),
        2 => (-1, 0),
        _ => (0, 1),
    };
    let Some(second) = run.get(1) else {
        return set_fall(tiles, h, first, middle, angle, elevation) as usize;
    };
    let forward = (
        second.0 as i64 - first.0 as i64,
        second.1 as i64 - first.1 as i64,
    );
    let two_hundred = if forward.0 * ccw.0 + forward.1 * ccw.1 > 0 {
        last
    } else {
        first
    };
    let three_hundred = if two_hundred == first { last } else { first };
    let mut changed = 0;
    for &p in run {
        let model = if p == two_hundred {
            end_200
        } else if p == three_hundred {
            terrain_model("Fall300")
        } else {
            middle
        };
        changed += set_fall(tiles, h, p, model, angle, elevation) as usize;
    }
    changed
}

fn set_fall(
    tiles: &mut [TerrainTile],
    h: usize,
    p: (usize, usize),
    model: u16,
    angle: u16,
    elevation: u16,
) -> bool {
    let t = &mut tiles[p.0 * h + p.1];
    // a hand-set fall model is never overwritten by the autotiler
    if is_fall(t.model) && !is_managed_fall(t.model) {
        return false;
    }
    if t.model == model && t.angle == angle && t.variation == 0 && t.elevation == elevation {
        return false;
    }
    t.model = model;
    t.angle = angle;
    t.variation = 0;
    t.elevation = elevation;
    t.road = 0;
    t.road_variation = 0;
    t.road_angle = 0;
    true
}

/// Re-shapes every water tile in `affected` and rebuilds waterfalls at its cliff edges.
/// Existing fall strips touched by the set are re-capped so their ends stay correct.
fn retile_water(
    tiles: &mut [TerrainTile],
    w: usize,
    h: usize,
    affected: &mut BTreeSet<(usize, usize)>,
) -> crate::Result<(usize, usize)> {
    // Pull whole existing strips into the set before any of them is reshaped.
    let existing: Vec<(usize, usize)> = affected
        .iter()
        .copied()
        .filter(|&(x, y)| is_fall(tiles[x * h + y].model))
        .collect();
    for (x, y) in existing {
        let t = tiles[x * h + y];
        for p in fall_run(tiles, w, h, x, y, t.angle, t.elevation) {
            affected.insert(p);
        }
    }

    // Fall tiles are water for shaping; start them as open water.
    for &(x, y) in affected.iter() {
        let t = &mut tiles[x * h + y];
        if is_fall(t.model) && is_managed_fall(t.model) {
            t.model = terrain_model("River8A");
            t.angle = 0;
            t.variation = 0;
        }
    }

    // River/bank shapes from the water mask. Hand-set falls keep their model.
    for &(x, y) in affected.iter() {
        let t = tiles[x * h + y];
        if !is_water(&t) || (is_fall(t.model) && !is_managed_fall(t.model)) {
            continue;
        }
        let (sides, corners) = water_neighbours(tiles, w, h, x, y);
        let (model, angle) = water_shape(sides, corners);
        let t = &mut tiles[x * h + y];
        if t.model != model || t.angle != angle {
            t.model = model;
            t.angle = angle;
            // keep the existing 0–3 texture variation
            t.road = 0;
            t.road_variation = 0;
            t.road_angle = 0;
        }
    }

    // Waterfalls: a water tile one level above water in one straight direction.
    let mut warnings = 0;
    let mut candidates = Vec::new();
    for &(x, y) in affected.iter() {
        let t = tiles[x * h + y];
        if !is_water(&t) || (is_fall(t.model) && !is_managed_fall(t.model)) {
            continue;
        }
        let e = t.elevation;
        if e == 0 {
            continue;
        }
        let mut drops = Vec::new();
        let mut onto_land = false;
        let mut too_steep = false;
        for (dir, dx, dy) in [(S, 0i64, 1i64), (E, 1, 0), (N, 0, -1), (W, -1, 0)] {
            let Some(n) = tile(tiles, w, h, x as i64 + dx, y as i64 + dy) else {
                continue;
            };
            if n.elevation < e {
                if n.elevation + 1 < e {
                    too_steep = true;
                } else if is_water(n) {
                    drops.push(dir);
                } else {
                    onto_land = true;
                }
            }
        }
        if too_steep {
            return Err(crate::Error::Invalid(format!(
                "Water at {x},{y} drops more than one level. Add a one-tile terrace for the waterfall."
            )));
        }
        if drops.len() > 1 || (drops.len() == 1 && onto_land) {
            warnings += 1; // corner, or water over land: only straight falls are generated
            continue;
        }
        if let Some(dir) = drops.first().copied() {
            candidates.push((x, y, dir));
        } else if onto_land {
            warnings += 1; // water at a cliff edge with no water below
        }
    }
    for &(x, y, dir) in &candidates {
        let angle = match dir {
            S => 0,
            E => 1,
            N => 2,
            _ => 3,
        };
        let e = tiles[x * h + y].elevation;
        set_fall(tiles, h, (x, y), terrain_model("Fall400"), angle, e);
    }

    let mut changed = 0;
    let mut seen = BTreeSet::new();
    for &(x, y, dir) in &candidates {
        if seen.contains(&(x, y)) {
            continue;
        }
        let angle = match dir {
            S => 0,
            E => 1,
            N => 2,
            _ => 3,
        };
        let e = tiles[x * h + y].elevation;
        let run = fall_run(tiles, w, h, x, y, angle, e);
        for &p in &run {
            seen.insert(p);
            affected.insert(p);
        }
        changed += assign_fall_run(tiles, h, &run, angle, e);
    }
    Ok((changed, warnings))
}

fn count_changed(before: &[TerrainTile], after: &[TerrainTile]) -> usize {
    before.iter().zip(after).filter(|(a, b)| a != b).count()
}

/// Refuses a stroke that would leave two adjacent tiles more than one level apart. The game only
/// draws a cliff face for a one-level drop, so the higher tile would show the void.
/// A step that already existed before the stroke is left alone: only new ones are refused.
fn check_steps(
    before: &[TerrainTile],
    after: &[TerrainTile],
    w: usize,
    h: usize,
) -> crate::Result<()> {
    for x in 0..w {
        for y in 0..h {
            let old_t = before[x * h + y];
            let t = after[x * h + y];
            if old_t == t {
                continue;
            }
            for (dx, dy) in [(0i64, -1i64), (1, 0), (0, 1), (-1, 0)] {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                let Some(n) = tile(after, w, h, nx, ny) else {
                    continue;
                };
                let old_n = tile(before, w, h, nx, ny).copied().unwrap_or_default();
                if t.elevation.abs_diff(n.elevation) > 1
                    && old_t.elevation.abs_diff(old_n.elevation) <= 1
                {
                    return Err(crate::Error::Invalid(format!(
                        "The change at {x},{y} would leave a two-level step next to {nx},{ny}. Each step must be one level; add a one-tile terrace."
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Paints or erases rivers. Water keeps the tile's elevation; a water tile that sits one level
/// above water in a straight direction gets a waterfall strip.
///
/// Painting over a cliff face is allowed only when the face drops one level into water, which is
/// exactly the case that becomes a waterfall. Returns `(changed, fall tiles, warnings)`.
pub fn paint_water(
    tiles: &mut [TerrainTile],
    w: usize,
    h: usize,
    targets: &[(usize, usize)],
    paint: bool,
    sharp: bool,
) -> crate::Result<(usize, usize, usize)> {
    if tiles.len() != w * h {
        return Err(crate::Error::Invalid(
            "terrain size does not match its grid".into(),
        ));
    }
    let mut next = tiles.to_vec();
    for &(x, y) in targets {
        if x >= w || y >= h {
            continue;
        }
        if !paint {
            continue;
        }
        let t = &next[x * h + y];
        if !is_cliff(t.model) {
            continue;
        }
        let e = t.elevation;
        let mut low = Vec::new();
        for (dx, dy) in [(0i64, 1i64), (1, 0), (0, -1), (-1, 0)] {
            if let Some(n) = tile(&next, w, h, x as i64 + dx, y as i64 + dy) {
                if n.elevation < e {
                    low.push(n);
                }
            }
        }
        let straight_into_water = low.len() == 1 && low[0].elevation + 1 == e && is_water(low[0]);
        if !straight_into_water {
            return Err(crate::Error::Invalid(format!(
                "The cliff at {x},{y} does not drop one level into water. Lower it or paint the water below first."
            )));
        }
    }

    let mut affected = BTreeSet::new();
    for &(x, y) in targets {
        if x >= w || y >= h {
            continue;
        }
        let t = &mut next[x * h + y];
        if paint {
            t.model = terrain_model("River8A");
            t.angle = 0;
            t.variation = 0;
            t.road = 0;
            t.road_variation = 0;
            t.road_angle = 0;
        } else if is_water(t) {
            // become grass at the same height; the ring pass restores any cliff face
            t.model = 0;
            t.angle = 0;
            t.variation = 0;
        } else {
            continue;
        }
        let x0 = x.saturating_sub(1);
        let y0 = y.saturating_sub(1);
        for nx in x0..=(x + 1).min(w - 1) {
            for ny in y0..=(y + 1).min(h - 1) {
                affected.insert((nx, ny));
            }
        }
    }

    let (falls, warnings) = retile_water(&mut next, w, h, &mut affected)?;
    if !paint {
        for &(x, y) in affected.iter() {
            retile_land_tile(&mut next, w, h, x, y, sharp)
                .map_err(|(x, y)| crate::Error::Invalid(unsupported_cliff(x, y)))?;
        }
    }
    let changed = count_changed(tiles, &next);
    tiles.copy_from_slice(&next);
    Ok((changed, falls, warnings))
}

fn unsupported_cliff(x: usize, y: usize) -> String {
    format!(
        "Cliff shape at {x},{y} is not supported yet (a saddle or a strip with no connected same-height side). Use the Fix one tile stamp."
    )
}

/// Raises or lowers land by one level and re-derives the cliff faces around it.
/// Water tiles are refused: erase the water, change the height, then paint it again.
/// Returns `(changed, fall tiles, warnings)`.
pub fn paint_height(
    tiles: &mut [TerrainTile],
    w: usize,
    h: usize,
    targets: &[(usize, usize)],
    delta: i32,
    sharp: bool,
) -> crate::Result<(usize, usize, usize)> {
    if tiles.len() != w * h {
        return Err(crate::Error::Invalid(
            "terrain size does not match its grid".into(),
        ));
    }
    let mut next = tiles.to_vec();
    for &(x, y) in targets {
        if x >= w || y >= h {
            continue;
        }
        let t = &next[x * h + y];
        if is_water(t) {
            return Err(crate::Error::Invalid(format!(
                "Water is at {x},{y}. Erase it, change the height, then paint it again."
            )));
        }
        let e = t.elevation as i32 + delta;
        if !(0..=MAX_ELEVATION).contains(&e) {
            return Err(crate::Error::Invalid(format!(
                "Elevation at {x},{y} would leave the supported 0–{MAX_ELEVATION} range."
            )));
        }
    }

    let mut affected = BTreeSet::new();
    for &(x, y) in targets {
        if x >= w || y >= h {
            continue;
        }
        let t = &mut next[x * h + y];
        t.elevation = (t.elevation as i32 + delta) as u16;
        let x0 = x.saturating_sub(1);
        let y0 = y.saturating_sub(1);
        for nx in x0..=(x + 1).min(w - 1) {
            for ny in y0..=(y + 1).min(h - 1) {
                affected.insert((nx, ny));
            }
        }
    }

    for &(x, y) in affected.iter() {
        retile_land_tile(&mut next, w, h, x, y, sharp)
            .map_err(|(x, y)| crate::Error::Invalid(unsupported_cliff(x, y)))?;
    }
    let (falls, warnings) = retile_water(&mut next, w, h, &mut affected)?;
    check_steps(tiles, &next, w, h)?;
    let changed = count_changed(tiles, &next);
    tiles.copy_from_slice(&next);
    Ok((changed, falls, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(suffix: &str) -> u16 {
        model_id("pattern", suffix).unwrap()
    }

    #[test]
    fn every_road_configuration_has_a_model() {
        for sides in 0..16u8 {
            for corners in 0..16u8 {
                assert!(
                    road_autotile("pattern", sides, corners).is_some(),
                    "no model for sides={sides:04b} corners={corners:04b}"
                );
            }
        }
    }

    #[test]
    fn matches_the_game_save() {
        // Examples measured from a real save; see docs/map-editor.md.
        let cases = [
            (S, 0, "1A", 0),
            (E, 0, "1A", 1),
            (N | S, 0, "2A", 0),
            (E | S, 0, "2C", 0),
            (N | E, NE, "3C", 1),
            (N | E, 0, "2C", 1),
            (N | E | S, 0, "3A", 0),
            (E | S | W, SE, "4B", 3),
            (N | E | W, NE | NW, "5B", 1),
        ];
        for (sides, corners, suffix, angle) in cases {
            assert_eq!(
                road_autotile("pattern", sides, corners),
                Some((id(suffix), angle)),
                "sides={sides:04b} corners={corners:04b}"
            );
        }
    }

    /// Paint with the default rounded corner style.
    fn paint(
        tiles: &mut [TerrainTile],
        w: usize,
        h: usize,
        targets: &[(usize, usize)],
        delta: i32,
    ) -> crate::Result<(usize, usize, usize)> {
        paint_height(tiles, w, h, targets, delta, false)
    }

    // --- rivers, cliffs and falls -------------------------------------------

    fn water_models() -> (u16, u16, u16, u16, u16, u16) {
        (
            terrain_model("River8A"),
            terrain_model("River7A"),
            terrain_model("River5B"),
            terrain_model("River3B"),
            terrain_model("Fall200"),
            terrain_model("Fall400"),
        )
    }

    fn paint_block(
        tiles: &mut [TerrainTile],
        w: usize,
        h: usize,
        x0: usize,
        y0: usize,
        x1: usize,
        y1: usize,
    ) {
        let targets: Vec<(usize, usize)> = (x0..=x1)
            .flat_map(|x| (y0..=y1).map(move |y| (x, y)))
            .collect();
        paint_water(tiles, w, h, &targets, true, false).unwrap();
    }

    #[test]
    fn water_block_gets_banks_and_corners() {
        let (w, h) = (5, 5);
        let mut tiles = vec![TerrainTile::default(); w * h];
        paint_block(&mut tiles, w, h, 1, 1, 3, 3);
        let (open, _nub, edge, corner, _, _) = water_models();
        let at = |x: usize, y: usize| tiles[x * h + y];
        assert_eq!((at(2, 2).model, at(2, 2).angle), (open, 0));
        assert_eq!((at(2, 1).model, at(2, 1).angle), (edge, 3)); // north bank
        assert_eq!((at(2, 3).model, at(2, 3).angle), (edge, 1)); // south bank
        assert_eq!((at(1, 2).model, at(1, 2).angle), (edge, 0)); // west bank
        assert_eq!((at(3, 2).model, at(3, 2).angle), (edge, 2)); // east bank
        assert_eq!((at(1, 1).model, at(1, 1).angle), (corner, 0)); // NW, water SE
        assert_eq!((at(3, 1).model, at(3, 1).angle), (corner, 3)); // NE, water SW
        assert_eq!((at(1, 3).model, at(1, 3).angle), (corner, 1)); // SW, water NE
        assert_eq!((at(3, 3).model, at(3, 3).angle), (corner, 2)); // SE, water NW
        assert_eq!(at(0, 0).model, 0); // untouched land
    }

    #[test]
    fn erasing_water_restores_grass() {
        let (w, h) = (5, 5);
        let mut tiles = vec![TerrainTile::default(); w * h];
        paint_block(&mut tiles, w, h, 1, 1, 3, 3);
        let targets: Vec<(usize, usize)> =
            (1..=3).flat_map(|x| (1..=3).map(move |y| (x, y))).collect();
        paint_water(&mut tiles, w, h, &targets, false, false).unwrap();
        for x in 1..=3 {
            for y in 1..=3 {
                assert_eq!(tiles[x * h + y].model, 0);
            }
        }
    }

    #[test]
    fn waterfall_strip_gets_measured_caps() {
        let (w, h) = (4, 4);
        let mut tiles = vec![TerrainTile::default(); w * h];
        // the water row is one level above the water below it, with same-height land behind
        for x in 0..w {
            tiles[x * h].elevation = 1;
            tiles[x * h + 1].elevation = 1;
        }
        paint_block(&mut tiles, w, h, 0, 1, w - 1, 2);
        let (_, _, _, _, fall200, fall400) = water_models();
        let fall300 = terrain_model("Fall300");
        for x in 0..w {
            let t = &tiles[x * h + 1];
            let expected = match x {
                0 => fall300,               // west end
                n if n == w - 1 => fall200, // east end
                _ => fall400,
            };
            assert_eq!((t.model, t.angle, t.elevation), (expected, 0, 1), "x={x}");
            assert_eq!(tiles[x * h + 2].elevation, 0);
            assert!(!is_fall(tiles[x * h + 2].model));
        }
    }

    #[test]
    fn raising_a_plateau_builds_cliff_faces() {
        let (w, h) = (5, 5);
        let mut tiles = vec![TerrainTile::default(); w * h];
        let targets: Vec<(usize, usize)> =
            (1..=3).flat_map(|x| (1..=3).map(move |y| (x, y))).collect();
        paint(&mut tiles, w, h, &targets, 1).unwrap();
        let edge = terrain_model("Cliff5B");
        let corner = terrain_model("Cliff3B");
        let at = |x: usize, y: usize| tiles[x * h + y];
        assert_eq!((at(2, 2).model, at(2, 2).elevation), (0, 1)); // flat top
        assert_eq!((at(2, 1).model, at(2, 1).angle), (edge, 3)); // low north
        assert_eq!((at(2, 3).model, at(2, 3).angle), (edge, 1)); // low south
        assert_eq!((at(1, 2).model, at(1, 2).angle), (edge, 0)); // low west
        assert_eq!((at(3, 2).model, at(3, 2).angle), (edge, 2)); // low east
        assert_eq!((at(1, 1).model, at(1, 1).angle), (corner, 0)); // low NW
        assert_eq!((at(3, 1).model, at(3, 1).angle), (corner, 3)); // low NE
        assert_eq!((at(1, 3).model, at(1, 3).angle), (corner, 1)); // low SW
        assert_eq!((at(3, 3).model, at(3, 3).angle), (corner, 2)); // low SE
        assert_eq!(at(0, 0).elevation, 0);
        assert_eq!(at(0, 2).model, 0);
    }

    #[test]
    fn a_single_raised_tile_uses_cliff0a() {
        let (w, h) = (5, 5);
        let mut tiles = vec![TerrainTile::default(); w * h];
        paint(&mut tiles, w, h, &[(2, 2)], 1).unwrap();
        let t = tiles[2 * h + 2];
        assert_eq!(
            (t.model, t.angle, t.elevation),
            (terrain_model("Cliff0A"), 0, 1)
        );
        for (dx, dy) in [
            (0i64, -1i64),
            (1, 0),
            (1, -1),
            (1, 1),
            (0, 1),
            (-1, 1),
            (-1, 0),
            (-1, -1),
        ] {
            assert_eq!(
                tiles[(2 + dx) as usize * h + (2 + dy) as usize].elevation,
                0
            );
        }
        paint(&mut tiles, w, h, &[(2, 2)], -1).unwrap();
        assert_eq!(tiles[2 * h + 2].model, 0);
    }

    #[test]
    fn two_by_one_uses_cliff1a() {
        let (w, h) = (5, 5);
        let mut tiles = vec![TerrainTile::default(); w * h];
        paint(&mut tiles, w, h, &[(1, 1), (2, 1)], 1).unwrap();
        let one = terrain_model("Cliff1A");
        assert_eq!((tiles[h + 1].model, tiles[h + 1].angle), (one, 1)); // high east
        assert_eq!((tiles[2 * h + 1].model, tiles[2 * h + 1].angle), (one, 3)); // high west
    }

    #[test]
    fn one_wide_ridge_uses_cliff2a() {
        let (w, h) = (6, 5);
        let mut tiles = vec![TerrainTile::default(); w * h];
        paint(&mut tiles, w, h, &[(1, 2), (2, 2), (3, 2)], 1).unwrap();
        let one = terrain_model("Cliff1A");
        assert_eq!((tiles[h + 2].model, tiles[h + 2].angle), (one, 1));
        assert_eq!(
            (tiles[2 * h + 2].model, tiles[2 * h + 2].angle),
            (terrain_model("Cliff2A"), 1)
        );
        assert_eq!((tiles[3 * h + 2].model, tiles[3 * h + 2].angle), (one, 3));
    }

    #[test]
    fn l_shape_uses_a_curved_corner_and_cliff1a_ends() {
        let (w, h) = (5, 5);
        let mut tiles = vec![TerrainTile::default(); w * h];
        paint(&mut tiles, w, h, &[(1, 1), (2, 1), (1, 2)], 1).unwrap();
        assert_eq!(
            (tiles[h + 1].model, tiles[h + 1].angle),
            (terrain_model("Cliff3B"), 0) // low NW
        );
        assert_eq!(
            (tiles[2 * h + 1].model, tiles[2 * h + 1].angle),
            (terrain_model("Cliff1A"), 3) // high west
        );
        assert_eq!(
            (tiles[h + 2].model, tiles[h + 2].angle),
            (terrain_model("Cliff1A"), 2) // high north
        );
    }

    #[test]
    fn a_one_tile_mesa_uses_cliff0a_at_every_level() {
        let (w, h) = (7, 7);
        let mut tiles = vec![TerrainTile::default(); w * h];
        let zero = terrain_model("Cliff0A");
        let square = |a: usize, b: usize| -> Vec<(usize, usize)> {
            (a..=b).flat_map(|x| (a..=b).map(move |y| (x, y))).collect()
        };
        // terraces so every step is one level: 5×5 at 1, the centre at 2, then its 3×3 ring at 2
        // and the centre again at 3, which leaves a one-tile mesa on the plateau.
        paint(&mut tiles, w, h, &square(1, 5), 1).unwrap();
        paint(&mut tiles, w, h, &[(3, 3)], 1).unwrap();
        assert_eq!(
            (tiles[3 * h + 3].model, tiles[3 * h + 3].elevation),
            (zero, 2)
        );
        let ring: Vec<(usize, usize)> = square(2, 4).into_iter().filter(|&p| p != (3, 3)).collect();
        paint(&mut tiles, w, h, &ring, 1).unwrap();
        paint(&mut tiles, w, h, &[(3, 3)], 1).unwrap();
        assert_eq!(
            (tiles[3 * h + 3].model, tiles[3 * h + 3].elevation),
            (zero, 3)
        );
        assert!(paint_height(&mut tiles, w, h, &[(3, 3)], 1, false).is_err()); // above 3
    }

    #[test]
    fn a_new_two_level_step_is_refused_and_writes_nothing() {
        let (w, h) = (5, 5);
        let mut tiles = vec![TerrainTile::default(); w * h];
        paint(&mut tiles, w, h, &[(2, 2)], 1).unwrap();
        let before = tiles.clone();
        let err = paint_height(&mut tiles, w, h, &[(2, 2)], 1, false).unwrap_err();
        assert!(err.to_string().contains("two-level step"), "{err}");
        assert_eq!(tiles, before);
    }

    #[test]
    fn lowering_next_to_a_higher_tile_is_refused() {
        let (w, h) = (7, 7);
        let mut tiles = vec![TerrainTile::default(); w * h];
        let square = |a: usize, b: usize| -> Vec<(usize, usize)> {
            (a..=b).flat_map(|x| (a..=b).map(move |y| (x, y))).collect()
        };
        paint(&mut tiles, w, h, &square(1, 5), 1).unwrap();
        paint(&mut tiles, w, h, &square(2, 4), 1).unwrap();
        paint(&mut tiles, w, h, &[(3, 3)], 1).unwrap();
        let before = tiles.clone();
        // (2,3) is at 2 beside the height-3 centre, so lowering it leaves a two-level step
        assert!(paint_height(&mut tiles, w, h, &[(2, 3)], -1, false).is_err());
        assert_eq!(tiles, before);
    }

    #[test]
    fn an_existing_two_level_step_does_not_block_elsewhere() {
        let (w, h) = (5, 5);
        let mut tiles = vec![TerrainTile::default(); w * h];
        tiles[h + 1].elevation = 2; // a dumped or hand-edited save may hold this
        paint(&mut tiles, w, h, &[(4, 4)], 1).unwrap();
        assert_eq!(tiles[4 * h + 4].elevation, 1);
    }

    #[test]
    fn a_pillar_diagonal_to_a_cliff_uses_cliff0a() {
        let (w, h) = (7, 7);
        let mut tiles = vec![TerrainTile::default(); w * h];
        let square = |a: usize, b: usize| -> Vec<(usize, usize)> {
            (a..=b).flat_map(|x| (a..=b).map(move |y| (x, y))).collect()
        };
        paint_height(&mut tiles, w, h, &square(2, 4), 1, false).unwrap();
        // one more 1×1 pillar touching the plateau only at its north-west corner
        paint_height(&mut tiles, w, h, &[(1, 1)], 1, false).unwrap();
        let t = tiles[h + 1];
        assert_eq!((t.model, t.elevation), (terrain_model("Cliff0A"), 1));
    }

    #[test]
    fn sharp_corners_use_cliff3c() {
        let (w, h) = (5, 5);
        let mut tiles = vec![TerrainTile::default(); w * h];
        let targets: Vec<(usize, usize)> =
            (1..=3).flat_map(|x| (1..=3).map(move |y| (x, y))).collect();
        paint_height(&mut tiles, w, h, &targets, 1, true).unwrap();
        let sharp = terrain_model("Cliff3C");
        assert_eq!((tiles[h + 1].model, tiles[h + 1].angle), (sharp, 0)); // low NW
        assert_eq!((tiles[3 * h + 1].model, tiles[3 * h + 1].angle), (sharp, 3)); // low NE
        assert_eq!(tiles[2 * h + 2].model, 0); // flat top
    }

    #[test]
    fn an_existing_corner_keeps_its_style() {
        let (w, h) = (7, 7);
        let mut tiles = vec![TerrainTile::default(); w * h];
        let square = |a: usize, b: usize| -> Vec<(usize, usize)> {
            (a..=b).flat_map(|x| (a..=b).map(move |y| (x, y))).collect()
        };
        paint_height(&mut tiles, w, h, &square(2, 4), 1, true).unwrap();
        assert_eq!(tiles[2 * h + 2].model, terrain_model("Cliff3C"));
        // a rounded stroke next to the corner re-tiles it but keeps its sharp look
        paint_height(&mut tiles, w, h, &[(1, 1)], 1, false).unwrap();
        assert_eq!(tiles[2 * h + 2].model, terrain_model("Cliff3C"));
    }

    #[test]
    fn unsupported_cliff_shape_writes_nothing() {
        let (w, h) = (5, 5);
        let mut tiles = vec![
            TerrainTile {
                elevation: 1,
                ..Default::default()
            };
            w * h
        ];
        // two low diagonals and no low sides: a saddle
        tiles[h + 1].elevation = 0;
        tiles[3 * h + 3].elevation = 0;
        let before = tiles.clone();
        assert!(paint_height(&mut tiles, w, h, &[(2, 2)], 0, false).is_err());
        assert_eq!(tiles, before);
    }

    #[test]
    fn water_over_a_cliff_needs_water_below() {
        let (w, h) = (6, 6);
        let mut tiles = vec![TerrainTile::default(); w * h];
        let top: Vec<(usize, usize)> = (1..=3).flat_map(|x| (1..=3).map(move |y| (x, y))).collect();
        paint(&mut tiles, w, h, &top, 1).unwrap();
        // water one level below the north face
        tiles[2 * h].model = terrain_model("River8A");
        let before = tiles.clone();
        paint_water(&mut tiles, w, h, &[(2, 1)], true, false).unwrap();
        assert!(is_fall(tiles[2 * h + 1].model));
        assert_eq!(tiles[2 * h + 1].angle, 2); // drops north
                                               // the south face has land below, so it cannot become water
        tiles = before;
        assert!(paint_water(&mut tiles, w, h, &[(2, 3)], true, false).is_err());
    }

    #[test]
    fn erasing_a_fall_restores_the_cliff_face() {
        let (w, h) = (6, 6);
        let mut tiles = vec![TerrainTile::default(); w * h];
        let top: Vec<(usize, usize)> = (1..=3).flat_map(|x| (1..=3).map(move |y| (x, y))).collect();
        paint(&mut tiles, w, h, &top, 1).unwrap();
        tiles[2 * h].model = terrain_model("River8A");
        paint_water(&mut tiles, w, h, &[(2, 1)], true, false).unwrap();
        assert!(is_fall(tiles[2 * h + 1].model));
        paint_water(&mut tiles, w, h, &[(2, 1)], false, false).unwrap();
        let t = tiles[2 * h + 1];
        assert_eq!(
            (t.model, t.angle, t.elevation),
            (terrain_model("Cliff5B"), 3, 1)
        );
    }

    #[test]
    fn lowering_a_plateau_removes_its_faces() {
        let (w, h) = (5, 5);
        let mut tiles = vec![TerrainTile::default(); w * h];
        let top: Vec<(usize, usize)> = (1..=3).flat_map(|x| (1..=3).map(move |y| (x, y))).collect();
        paint(&mut tiles, w, h, &top, 1).unwrap();
        paint(&mut tiles, w, h, &top, -1).unwrap();
        for x in 1..=3 {
            for y in 1..=3 {
                let t = tiles[x * h + y];
                assert_eq!((t.model, t.elevation), (0, 0), "{x},{y}");
            }
        }
    }

    #[test]
    fn a_hand_set_fall_survives_a_nearby_water_stroke() {
        let (w, h) = (4, 4);
        let mut tiles = vec![TerrainTile::default(); w * h];
        for x in 0..w {
            tiles[x * h].elevation = 1;
            tiles[x * h + 1].elevation = 1;
        }
        paint_block(&mut tiles, w, h, 0, 1, w - 1, 2);
        // a model the autotiler never writes, as the shape fixer would set it
        tiles[h + 1].model = terrain_model("Fall403");
        let before = tiles.clone();
        // paint the river behind the strip: the whole strip is pulled in and re-tiled
        paint_block(&mut tiles, w, h, 0, 0, w - 1, 0);
        assert_eq!(tiles[h + 1], before[h + 1]);
        assert!(is_fall(tiles[3 * h + 1].model)); // the rest is still an automatic fall
    }

    #[test]
    fn painting_the_same_water_twice_changes_nothing() {
        let (w, h) = (5, 5);
        let mut tiles = vec![TerrainTile::default(); w * h];
        paint_block(&mut tiles, w, h, 1, 1, 3, 3);
        let before = tiles.clone();
        let targets: Vec<(usize, usize)> =
            (1..=3).flat_map(|x| (1..=3).map(move |y| (x, y))).collect();
        let (changed, falls, _) = paint_water(&mut tiles, w, h, &targets, true, false).unwrap();
        assert_eq!((changed, falls), (0, 0));
        assert_eq!(tiles, before);
    }

    #[test]
    fn painting_a_run_joins_it_up() {
        let (w, h) = (4, 3);
        let mut tiles = vec![TerrainTile::default(); w * h];
        let run = [(1, 1), (2, 1), (3, 1)];
        assert!(paint_road(&mut tiles, w, h, &run, Some("pattern")) > 0);
        assert_eq!(tiles[h + 1].road, id("1A"));
        assert_eq!(tiles[h + 1].road_angle, 1); // end pointing east
        assert_eq!(tiles[2 * h + 1].road, id("2A"));
        assert_eq!(tiles[3 * h + 1].road, id("1A"));
        assert_eq!(tiles[3 * h + 1].road_angle, 3); // end pointing west
        assert_eq!(tiles[2 * h].road, 0); // unaffected neighbour stays empty

        assert!(paint_road(&mut tiles, w, h, &run, None) > 0);
        for (x, y) in run {
            assert_eq!(tiles[x * h + y].road, 0);
        }
    }
}
