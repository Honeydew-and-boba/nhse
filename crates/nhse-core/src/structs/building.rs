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

/// Plot the game requires for a structure, in terrain tiles (16 per acre).
/// [`Building::x`]/[`Building::y`] is the centre of this plot in item tiles (32 per acre);
/// draw item spans as `x ± width`, `y ± height`.
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
}
