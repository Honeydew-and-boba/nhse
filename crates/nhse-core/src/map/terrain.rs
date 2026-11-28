//! Terrain tiles (16 per acre) and their appearance.
use crate::util::bytes::*;
use serde::{Deserialize, Serialize};

pub const TILE_SIZE: usize = 0xE;
pub const TILES_W: usize = 7 * 16; // 112
pub const TILES_H: usize = 6 * 16; // 96

/// `[model, variation, angle]` for the ground and the road layer, plus elevation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerrainTile {
    pub model: u16,
    pub variation: u16,
    pub angle: u16,
    pub road: u16,
    pub road_variation: u16,
    pub road_angle: u16,
    pub elevation: u16,
}

impl TerrainTile {
    pub fn read(d: &[u8], o: usize) -> Self {
        Self {
            model: u16le(d, o),
            variation: u16le(d, o + 2),
            angle: u16le(d, o + 4),
            road: u16le(d, o + 6),
            road_variation: u16le(d, o + 8),
            road_angle: u16le(d, o + 10),
            elevation: u16le(d, o + 12),
        }
    }
    pub fn write(&self, d: &mut [u8], o: usize) {
        put_u16(d, o, self.model);
        put_u16(d, o + 2, self.variation);
        put_u16(d, o + 4, self.angle);
        put_u16(d, o + 6, self.road);
        put_u16(d, o + 8, self.road_variation);
        put_u16(d, o + 10, self.road_angle);
        put_u16(d, o + 12, self.elevation);
    }
}

// --- model classification -----------------------------------------------------

pub fn is_river(m: u16) -> bool {
    (0x01..=0x15).contains(&m)
}
pub fn is_cliff(m: u16) -> bool {
    (0x16..=0x29).contains(&m) || m == 0x93
}
pub fn is_fall(m: u16) -> bool {
    (0x3A..=0x46).contains(&m) || (0x71..=0x92).contains(&m)
}
pub fn is_road(m: u16) -> bool {
    m >= 0x94 || (0x47..=0x70).contains(&m)
}

/// Road family used for coloring and for the road brush.
pub fn road_family(m: u16) -> Option<&'static str> {
    Some(match m {
        0x47..=0x5B => "soil",
        0x5C..=0x70 => "stone",
        0x94..=0xA8 => "brick",
        0xA9..=0xBD => "darksoil",
        0xBE..=0xD2 => "pattern",
        0xD3..=0xE7 => "sand",
        0xE8..=0xFC => "tile",
        0xFD..=0x111 => "wood",
        _ => return None,
    })
}

pub type Rgb = [u8; 3];

// One clear step per elevation level: the map editor must be readable at a glance.
const GRASS: [Rgb; 4] = [
    [70, 150, 72],
    [104, 176, 88],
    [146, 202, 104],
    [190, 224, 130],
];
const WATER: Rgb = [104, 196, 214];
const WATER_DEEP: Rgb = [84, 176, 200];
const SHORE: Rgb = [214, 199, 140];
// The game's cliffs are dirt/peach, not green. Keeping them distinct from grass also makes
// height differences obvious in the map preview.
const CLIFF: [Rgb; 4] = [
    [172, 128, 96],
    [186, 142, 108],
    [200, 156, 120],
    [214, 170, 134],
];
const CLIFF_EDGE: Rgb = [120, 86, 64];
const FALL: Rgb = [134, 214, 236];
const FOAM: Rgb = [235, 250, 255];

fn road_color(m: u16) -> Rgb {
    match road_family(m) {
        Some("brick") => [178, 70, 60],
        Some("darksoil") => [120, 82, 52],
        Some("soil") => [196, 150, 98],
        Some("stone") => [158, 158, 160],
        Some("pattern") => [236, 230, 210],
        Some("tile") => [94, 132, 178],
        Some("sand") => [226, 196, 140],
        Some("wood") => [170, 120, 74],
        _ => [200, 170, 120],
    }
}

fn tri_area2(a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> i32 {
    (a.0 * (b.1 - c.1) + b.0 * (c.1 - a.1) + c.0 * (a.1 - b.1)).abs()
}

fn in_tri(px: i32, py: i32, a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> bool {
    let p = (px, py);
    tri_area2(a, b, c) == tri_area2(p, b, c) + tri_area2(a, p, c) + tri_area2(a, b, p)
}

fn in_multi(
    px: i32,
    py: i32,
    a: (i32, i32),
    b: (i32, i32),
    c: (i32, i32),
    va: (i32, i32),
    vb: (i32, i32),
) -> bool {
    in_tri(px, py, a, va, b) || in_tri(px, py, a, b, c) || in_tri(px, py, c, b, vb)
}

fn nub_tl(x: i32, y: i32) -> bool {
    in_tri(x, y, (0, 4), (0, 0), (4, 0))
}
fn nub_tr(x: i32, y: i32) -> bool {
    in_tri(x, y, (12, 0), (15, 0), (15, 4))
}
fn nub_bl(x: i32, y: i32) -> bool {
    in_tri(x, y, (0, 12), (0, 15), (4, 15))
}
fn nub_br(x: i32, y: i32) -> bool {
    in_tri(x, y, (12, 15), (15, 15), (15, 12))
}

/// True if pixel (x, y) in 0..16 of a river tile is land (bank).
/// Shapes follow the original editor's river tile atlas.
pub fn river_is_land(m: u16, angle: u16, x: i32, y: i32) -> bool {
    let a = angle & 3;
    let edge = |lo: bool, hi: bool| lo || hi;
    match m {
        0x01 => edge(!(4..12).contains(&x), !(4..12).contains(&y)),
        0x04 => match a {
            0 => !(4..12).contains(&x) || y < 4,
            1 => x < 4 || !(4..12).contains(&y),
            2 => !(4..12).contains(&x) || y >= 12,
            _ => !(4..12).contains(&y) || x >= 12,
        },
        0x07 => match a {
            0 | 2 => !(4..12).contains(&x),
            _ => !(4..12).contains(&y),
        },
        0x08 => match a {
            0 => {
                in_multi(x, y, (4, 15), (0, 0), (15, 4), (0, 15), (15, 0))
                    || nub_br(x, y)
                    || x < 4
                    || y < 4
            }
            1 => {
                in_multi(x, y, (4, 0), (0, 15), (15, 12), (0, 0), (15, 15))
                    || nub_tr(x, y)
                    || x < 4
                    || y >= 12
            }
            2 => {
                in_multi(x, y, (0, 12), (15, 15), (12, 0), (0, 15), (15, 0))
                    || nub_tl(x, y)
                    || x >= 12
                    || y >= 12
            }
            _ => {
                in_multi(x, y, (0, 4), (15, 0), (12, 15), (0, 0), (15, 15))
                    || nub_bl(x, y)
                    || x >= 12
                    || y < 4
            }
        },
        0x09 => match a {
            0 => x < 4 || y < 4 || nub_br(x, y),
            1 => x < 4 || y >= 12 || nub_tr(x, y),
            2 => x >= 12 || y >= 12 || nub_tl(x, y),
            _ => x >= 12 || y < 4 || nub_bl(x, y),
        },
        0x0A => match a {
            0 => x < 4 || nub_tr(x, y) || nub_br(x, y),
            1 => y >= 12 || nub_tl(x, y) || nub_tr(x, y),
            2 => x >= 12 || nub_bl(x, y) || nub_tl(x, y),
            _ => y < 4 || nub_br(x, y) || nub_bl(x, y),
        },
        0x0B => match a {
            0 => in_multi(x, y, (4, 15), (0, 0), (15, 4), (0, 15), (15, 0)),
            1 => in_multi(x, y, (4, 0), (0, 15), (15, 12), (0, 0), (15, 15)),
            2 => in_multi(x, y, (0, 12), (15, 15), (12, 0), (0, 15), (15, 0)),
            _ => in_multi(x, y, (0, 4), (15, 0), (12, 15), (0, 0), (15, 15)),
        },
        0x0C => match a {
            0 => x < 4 || y < 4,
            1 => x < 4 || y >= 12,
            2 => x >= 12 || y >= 12,
            _ => x >= 12 || y < 4,
        },
        0x0D => match a {
            0 => x < 4 || nub_tr(x, y),
            1 => y >= 12 || nub_tl(x, y),
            2 => x >= 12 || nub_bl(x, y),
            _ => y < 4 || nub_br(x, y),
        },
        0x0E => match a {
            0 => x < 4 || nub_br(x, y),
            1 => y >= 12 || nub_tr(x, y),
            2 => x >= 12 || nub_tl(x, y),
            _ => y < 4 || nub_bl(x, y),
        },
        0x0F => nub_bl(x, y) || nub_br(x, y) || nub_tl(x, y) || nub_tr(x, y),
        0x10 => match a {
            0 => nub_tl(x, y) || nub_tr(x, y) || nub_br(x, y),
            1 => nub_bl(x, y) || nub_tl(x, y) || nub_tr(x, y),
            2 => nub_bl(x, y) || nub_br(x, y) || nub_tl(x, y),
            _ => nub_bl(x, y) || nub_br(x, y) || nub_tr(x, y),
        },
        0x11 => match a {
            0 => x < 4,
            1 => y >= 12,
            2 => x >= 12,
            _ => y < 4,
        },
        0x12 => match a {
            0 | 2 => nub_tl(x, y) || nub_br(x, y),
            _ => nub_bl(x, y) || nub_tr(x, y),
        },
        0x13 => match a {
            0 => nub_bl(x, y) || nub_br(x, y),
            1 => nub_br(x, y) || nub_tr(x, y),
            2 => nub_tr(x, y) || nub_tl(x, y),
            _ => nub_tl(x, y) || nub_bl(x, y),
        },
        0x14 => match a {
            0 => nub_bl(x, y),
            1 => nub_br(x, y),
            2 => nub_tr(x, y),
            _ => nub_tl(x, y),
        },
        _ => false,
    }
}

fn grass(elev: u16) -> Rgb {
    GRASS[(elev as usize).min(3)]
}

fn shade(c: Rgb, f: f32) -> Rgb {
    [
        (c[0] as f32 * f).min(255.0) as u8,
        (c[1] as f32 * f).min(255.0) as u8,
        (c[2] as f32 * f).min(255.0) as u8,
    ]
}

/// What a cliff tile's pixel shows: the wall face, the plateau body, or the lower ground a
/// rounded corner cuts back to.
#[derive(Clone, Copy, PartialEq, Debug)]
enum CliffPixel {
    Wall,
    Body,
    Below,
}

/// Pixels of a cliff tile from its measured model and angle. `None` means the shape is not
/// measured, so the caller keeps the neutral all-four-edges look.
fn cliff_wall(m: u16, angle: u16, x: i32, y: i32) -> Option<CliffPixel> {
    const N: u8 = 1;
    const E: u8 = 2;
    const S: u8 = 4;
    const W: u8 = 8;
    const T: i32 = 3; // wall band thickness
    const R: f32 = 8.0; // rounded outside corner radius
    let a = angle & 3;
    let (low, rounded) = match m {
        0x16 => (0xF, false),                                      // Cliff0A, pillar
        0x19 => (0xF & ![S, E, N, W][a as usize], false),          // Cliff1A, one connected side
        0x1C => (if a == 0 { N | S } else { E | W }, false),       // Cliff2A, opposite sides
        0x1F => ([N | W, S | W, S | E, N | E][a as usize], true),  // Cliff3B, rounded corner
        0x20 => ([N | W, S | W, S | E, N | E][a as usize], false), // Cliff3C, sharp corner
        0x25 => ([W, S, E, N][a as usize], false),                 // Cliff5B, one low side
        0x28 => {
            // Cliff7A: no low side, one sunken diagonal corner
            let nub = match a {
                0 => nub_bl(x, y),
                1 => nub_br(x, y),
                2 => nub_tr(x, y),
                _ => nub_tl(x, y),
            };
            return Some(if nub {
                CliffPixel::Wall
            } else {
                CliffPixel::Body
            });
        }
        _ => return None,
    };
    let mut wall = (low & N != 0 && y < T)
        || (low & S != 0 && y >= 16 - T)
        || (low & W != 0 && x < T)
        || (low & E != 0 && x >= 16 - T);
    if rounded {
        for (bits, px, py) in [
            ((N | W), x, y),           // outside corner NW
            ((N | E), 15 - x, y),      // NE
            ((S | E), 15 - x, 15 - y), // SE
            ((S | W), x, 15 - y),      // SW
        ] {
            if low & bits != bits || px >= R as i32 || py >= R as i32 {
                continue;
            }
            // A rounded corner pulls the wall back from the tile corner as a quarter arc; the
            // wedge inside it is the ground below, not the plateau.
            let d = ((px * px + py * py) as f32).sqrt();
            if d <= R - T as f32 {
                return Some(CliffPixel::Below);
            }
            wall = d <= R;
        }
    }
    Some(if wall {
        CliffPixel::Wall
    } else {
        CliffPixel::Body
    })
}

/// Sign of the quarter of a rounded ground-path bend that shows grass, or `None` when the tile is
/// not a rounded bend. The game stores the same silhouette as `3B` (rounded outside corner) and
/// `3C` (sharp, so the whole tile is path); the model name is the only difference.
pub fn road_rounded_corner(model: u16, angle: u16) -> Option<(i32, i32)> {
    let name = model_name(model);
    if !(name.ends_with("3B") || name.ends_with("2B")) {
        return None;
    }
    // A bend fills the diagonal between its two connected sides: SE at angle 0, rotating
    // counter-clockwise. The grass is the opposite corner.
    Some(match [2u8, 1, 8, 4][(angle & 3) as usize] {
        2 => (-1, -1), // SE filled: grass NW
        1 => (-1, 1),  // NE filled: grass SW
        8 => (1, 1),   // NW filled: grass SE
        _ => (1, -1),  // SW filled: grass NE
    })
}

/// Color of pixel (x, y) (0..16) inside a terrain tile. `road_corner` is the tile's rounded bend
/// corner from `road_rounded_corner`, precomputed once per tile by the renderer.
pub fn tile_pixel(t: &TerrainTile, x: i32, y: i32, road_corner: Option<(i32, i32)>) -> Rgb {
    if is_road(t.road) {
        let c = road_color(t.road);
        if let Some((sx, sy)) = road_corner {
            // The outside of a rounded bend is a quarter disc of grass in the tile corner.
            const R: i32 = 8; // shortcut: radius guessed from the game's look, tweak if it reads wrong
            let px = if sx < 0 { x } else { 15 - x };
            let py = if sy < 0 { y } else { 15 - y };
            if px < R && py < R && (R - px).pow(2) + (R - py).pow(2) > R * R {
                return grass(t.elevation);
            }
        }
        // subtle texture so road patches read as paths
        return if (x + y) % 8 == 0 { shade(c, 0.93) } else { c };
    }
    let m = t.model;
    if is_river(m) {
        let land = |px: i32, py: i32| -> bool {
            if !(0..16).contains(&px) || !(0..16).contains(&py) {
                return false;
            }
            river_is_land(m, t.angle, px, py)
        };
        if land(x, y) {
            // a narrow sandy band where the bank meets the water
            let near_water = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                (0..16).contains(&nx) && (0..16).contains(&ny) && !land(nx, ny)
            });
            return if near_water {
                SHORE
            } else {
                grass(t.elevation)
            };
        }
        return if (x * 3 + y * 5) % 11 == 0 {
            WATER_DEEP
        } else {
            WATER
        };
    }
    if is_fall(m) {
        return if (y + (x % 4)) % 5 == 0 { FOAM } else { FALL };
    }
    if is_cliff(m) {
        let base = CLIFF[(t.elevation as usize).min(3)];
        return match cliff_wall(m, t.angle, x, y) {
            Some(CliffPixel::Wall) => CLIFF_EDGE,
            // inside the arc the wall is pulled back, so the level below shows through
            Some(CliffPixel::Below) => grass(t.elevation.saturating_sub(1)),
            Some(CliffPixel::Body) => base,
            None if x == 0 || y == 0 || x == 15 || y == 15 => CLIFF_EDGE,
            None => base,
        };
    }
    grass(t.elevation)
}

/// Short label for a terrain model (e.g. `River2B`).
pub fn model_name(m: u16) -> &'static str {
    crate::data::tables::enum_name("TerrainUnitModel", m as i64).unwrap_or("?")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rounded_road_bend_shows_its_grass_corner() {
        let model =
            |n: &str| crate::data::tables::enum_value("TerrainUnitModel", n).unwrap() as u16;
        let tile = |m: u16, angle: u16| TerrainTile {
            road: m,
            road_angle: angle,
            ..Default::default()
        };
        let corner = road_rounded_corner(model("RoadFanPattern3B"), 0);
        assert_eq!(corner, Some((-1, -1))); // SE filled, so the NW corner is grass
        let rounded = tile(model("RoadFanPattern3B"), 0);
        assert_eq!(tile_pixel(&rounded, 0, 0, corner), grass(0));
        assert_eq!(tile_pixel(&rounded, 8, 9, corner), road_color(rounded.road));
        // sharp bends and non-bends fill the whole tile
        let sharp = tile(model("RoadFanPattern3C"), 0);
        assert_eq!(road_rounded_corner(sharp.road, 0), None);
        assert_eq!(tile_pixel(&sharp, 8, 9, None), road_color(sharp.road));
        assert_eq!(road_rounded_corner(model("RoadFanPattern2A"), 0), None);
        // a three-tile L: 2B is rounded, 2C sharp
        let l_round = tile(model("RoadFanPattern2B"), 0);
        let l_corner = road_rounded_corner(l_round.road, 0);
        assert_eq!(l_corner, Some((-1, -1)));
        assert_eq!(tile_pixel(&l_round, 0, 0, l_corner), grass(0));
        assert_eq!(road_rounded_corner(model("RoadFanPattern2C"), 0), None);
    }

    #[test]
    fn cliff_wall_shows_rounded_and_sharp_corners() {
        use CliffPixel::{Below, Body, Wall};
        // NW outside corner: rounded pulls the wall back, sharp keeps the right angle
        assert_eq!(cliff_wall(0x1F, 0, 0, 0), Some(Below)); // Cliff3B, the cut-back wedge
        assert_eq!(cliff_wall(0x1F, 0, 1, 5), Some(Wall)); // the rounded face itself
        assert_eq!(cliff_wall(0x1F, 0, 7, 7), Some(Body)); // plateau inside the arc
        assert_eq!(cliff_wall(0x20, 0, 0, 0), Some(Wall)); // Cliff3C
                                                           // straight face: Cliff5B angle 1 has the low side south
        assert_eq!(cliff_wall(0x25, 1, 8, 15), Some(Wall));
        assert_eq!(cliff_wall(0x25, 1, 8, 2), Some(Body));
        // pillar: body in the middle, wall on every side
        assert_eq!(cliff_wall(0x16, 0, 8, 8), Some(Body));
        assert_eq!(cliff_wall(0x16, 0, 8, 0), Some(Wall));
        // the cut-back wedge is the level below, so a height-2 corner shows height-1 grass
        let corner = TerrainTile {
            model: 0x1F,
            angle: 0,
            elevation: 2,
            ..Default::default()
        };
        assert_eq!(tile_pixel(&corner, 0, 0, None), grass(1));
        assert_eq!(tile_pixel(&corner, 7, 7, None), CLIFF[2]);
        // an unmeasured model keeps the neutral fallback
        assert_eq!(cliff_wall(0x1D, 0, 0, 0), None);
    }
}
