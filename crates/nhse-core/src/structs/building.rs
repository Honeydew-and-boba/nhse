//! Island structures (houses, shops, bridges, inclines …).
use crate::util::bytes::*;
use serde::{Deserialize, Serialize};

pub const SIZE: usize = 0x14;
pub const COUNT: usize = 46;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Building {
    pub kind: u16,
    /// Center position, in field item grid units (32 per acre), absolute map coordinates.
    pub x: u16,
    pub y: u16,
    pub angle: u8,
    pub bit: i8,
    pub type_: u16,
    pub type_arg: u8,
    pub unique_id: u16,
    pub unused: u32,
}

impl Building {
    pub fn read(d: &[u8], o: usize) -> Self {
        Self {
            kind: u16le(d, o),
            x: u16le(d, o + 2),
            y: u16le(d, o + 4),
            angle: d[o + 6],
            bit: d[o + 7] as i8,
            type_: u16le(d, o + 8),
            type_arg: d[o + 0xA],
            unique_id: u16le(d, o + 0xC),
            unused: u32le(d, o + 0x10),
        }
    }
    pub fn write(&self, d: &mut [u8], o: usize) {
        put_u16(d, o, self.kind);
        put_u16(d, o + 2, self.x);
        put_u16(d, o + 4, self.y);
        d[o + 6] = self.angle;
        d[o + 7] = self.bit as u8;
        put_u16(d, o + 8, self.type_);
        d[o + 0xA] = self.type_arg;
        d[o + 0xB] = 0;
        put_u16(d, o + 0xC, self.unique_id);
        put_u32(d, o + 0x10, self.unused);
    }
}

/// One part of a building's entrance, in item tiles relative to [`Building::x`]/[`Building::y`]:
/// it covers `x + dx .. x + dx + w` × `y + dy .. y + dy + h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoorRect {
    pub dx: i16,
    pub dy: i16,
    pub w: u16,
    pub h: u16,
}

const fn door(dx: i16, dy: i16, w: u16, h: u16) -> DoorRect {
    DoorRect { dx, dy, w, h }
}

const NO_DOORS: &[DoorRect] = &[];
const PLAYER_DOORS: &[DoorRect] = &[door(-2, 2, 4, 3)];
const VILLAGER_DOORS: &[DoorRect] = &[door(-1, 2, 4, 3)];
const NOOKS_DOORS: &[DoorRect] = &[door(-2, 2, 4, 2)];
const MUSEUM_DOORS: &[DoorRect] = &[door(-2, 4, 4, 1)];
const ABLES_DOORS: &[DoorRect] = &[door(-1, 2, 6, 2), door(0, 4, 4, 1)];
const CAMPSITE_DOORS: &[DoorRect] = &[door(-2, 4, 4, 2)];

/// Measured entrances for the structures whose doors are known. Several buildings have no
/// reliable measurements yet, so this is deliberately partial.
pub fn doors(kind: u16) -> &'static [DoorRect] {
    match kind {
        1..=8 => PLAYER_DOORS,
        9..=18 => VILLAGER_DOORS,
        19 => NOOKS_DOORS,
        21 => MUSEUM_DOORS,
        24 => ABLES_DOORS,
        25 => CAMPSITE_DOORS,
        _ => NO_DOORS,
    }
}

/// Whether the game lets the player move this structure. Resident Services, the airport,
/// tent, bridge, incline, Redd's trawler, studio and hotel are fixed.
pub fn movable(kind: u16) -> bool {
    matches!(kind, 1..=19 | 21 | 24 | 25)
}

/// Plot the game requires for a structure, in terrain tiles (16 per acre).
/// [`Building::x`]/[`Building::y`] is the centre of this plot in item tiles (32 per acre);
/// the plot covers `x - width ..= x + width - 1` and `y - height ..= y + height - 1`
/// (2×width by 2×height item tiles).
///
/// Player houses, villager houses, Nook's Cranny, the museum and Able Sisters are
/// verified against paths in real saves or the widely documented plot sizes. The rest
/// are approximations and are only drawn, never written back.
pub fn footprint(kind: u16) -> (u16, u16) {
    match kind {
        1..=8 => (5, 4),  // player houses
        9..=18 => (4, 4), // villager houses
        19 => (7, 4),     // Nook's Cranny
        20 => (6, 3),     // Resident Services building (the plaza is separate)
        21 => (7, 4),     // Museum
        22 => (10, 6),    // Airport
        23 => (6, 4),     // Resident Services tent
        24 => (5, 4),     // Able Sisters
        25 => (4, 4),     // Campsite
        26 => (4, 6),     // Bridge
        27 => (2, 5),     // Incline
        28 => (6, 4),     // Redd's Treasure Trawler
        29 => (5, 4),     // Photo studio
        42 => (8, 6),     // Hotel
        _ => (2, 2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn footprints_match_the_game_plots() {
        assert_eq!(footprint(1), (5, 4)); // player house
        assert_eq!(footprint(9), (4, 4)); // villager house
        assert_eq!(footprint(19), (7, 4)); // Nook's Cranny
        assert_eq!(footprint(21), (7, 4)); // museum
        assert_eq!(footprint(24), (5, 4)); // Able Sisters
        assert_eq!(footprint(25), (4, 4)); // campsite
        assert_eq!(footprint(999), (2, 2)); // unknown kind
    }

    #[test]
    fn doors_use_the_measured_offsets() {
        // Checked against the user's in-game measurements (map layer coordinates + 32 in Y).
        let door = |kind: u16, x: i32, y: i32| {
            doors(kind)
                .iter()
                .map(|d| {
                    (
                        x + d.dx as i32,
                        y + d.dy as i32,
                        x + d.dx as i32 + d.w as i32 - 1,
                        y + d.dy as i32 + d.h as i32 - 1,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(door(1, 121, 134), vec![(119, 136, 122, 138)]); // player house
        assert_eq!(door(10, 148, 182), vec![(147, 184, 150, 186)]); // villager 2
        assert_eq!(door(19, 75, 190), vec![(73, 192, 76, 193)]); // Nook's Cranny
        assert_eq!(door(21, 93, 164), vec![(91, 168, 94, 168)]); // museum
        assert_eq!(
            door(24, 67, 134),
            vec![(66, 136, 71, 137), (67, 138, 70, 138)]
        ); // Able Sisters
        assert_eq!(door(25, 136, 106), vec![(134, 110, 137, 111)]); // campsite
        assert!(doors(20).is_empty()); // Resident Services: not measured
    }

    #[test]
    fn movable_matches_the_game() {
        for kind in 1..=19 {
            assert!(movable(kind), "kind {kind}");
        }
        assert!(movable(21));
        assert!(movable(24));
        assert!(movable(25));
        for kind in [0, 20, 22, 23, 26, 27, 28, 29, 42, 999] {
            assert!(!movable(kind), "kind {kind}");
        }
    }
}
