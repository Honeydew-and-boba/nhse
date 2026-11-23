//! Painting the road layer: picking the right road model and angle from neighbours.

use super::terrain::{is_road, road_family, TerrainTile};
use crate::data::tables::enum_value;

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
