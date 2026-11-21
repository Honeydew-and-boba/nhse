//! Map accessors on the save (field item layers, flags, terrain, acres, plaza).
use super::grid::ItemGrid;
use super::terrain::{TerrainTile, TILES_H, TILES_W, TILE_SIZE};
use super::ITEM_TILES_PER_ACRE;
use crate::data::items::{item_kind, kinds};
use crate::save::HorizonSave;
use crate::structs::{building, Item};
use crate::util::bits::{get_flag, set_flag};
use crate::util::bytes::*;
use serde::{Deserialize, Serialize};

pub const ACRES_W: usize = 9;
pub const ACRES_H: usize = 8;
const ACRE_BYTES: usize = ACRES_W * ACRES_H * 2;
pub const MAP_DESIGN_NONE: u16 = 0xF800;

/// Geometry of the field item layers for this save.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FieldLayout {
    /// Field item layer size in acres (7×6 before 3.0, 9×6 afterwards).
    pub acres_w: usize,
    pub acres_h: usize,
    /// Size in tiles.
    pub width: usize,
    pub height: usize,
    /// Offset of the layer inside the whole 9×8-acre map, in item tiles.
    pub offset_x: usize,
    pub offset_y: usize,
}

/// Plaza position (top-left), in item tile units of the whole map.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Plaza {
    pub x: u32,
    pub z: u32,
}

impl HorizonSave {
    pub fn field_layout(&self) -> FieldLayout {
        let aw = self.main_ofs.field_item_acre_width;
        let ah = 6;
        FieldLayout {
            acres_w: aw,
            acres_h: ah,
            width: aw * ITEM_TILES_PER_ACRE,
            height: ah * ITEM_TILES_PER_ACRE,
            offset_x: ((9 - aw) / 2) * ITEM_TILES_PER_ACRE,
            offset_y: ITEM_TILES_PER_ACRE,
        }
    }

    fn field_tile_count(&self) -> usize {
        let l = self.field_layout();
        l.width * l.height
    }

    fn field_layer_offset(&self, layer: usize) -> usize {
        self.main_ofs.field_item + layer * self.field_tile_count() * Item::SIZE
    }

    fn field_flag_offset(&self, layer: usize) -> usize {
        let n = self.field_tile_count();
        self.main_ofs.field_item + 2 * n * Item::SIZE + layer * (n / 8)
    }

    pub fn field_layer(&self, layer: usize) -> ItemGrid {
        let l = self.field_layout();
        let tiles = Item::read_array(
            &self.main.data,
            self.field_layer_offset(layer.min(1)),
            l.width * l.height,
        );
        ItemGrid::new(l.width, l.height, tiles)
    }

    pub fn set_field_layer(&mut self, layer: usize, g: &ItemGrid) {
        let o = self.field_layer_offset(layer.min(1));
        Item::write_array(&mut self.main.data, o, &g.tiles);
    }

    /// Raw layer bytes (for fast transfer to the UI).
    pub fn field_layer_bytes(&self, layer: usize) -> &[u8] {
        let o = self.field_layer_offset(layer.min(1));
        &self.main.data[o..o + self.field_tile_count() * Item::SIZE]
    }

    pub fn set_field_layer_bytes(&mut self, layer: usize, bytes: &[u8]) -> crate::Result<()> {
        let n = self.field_tile_count() * Item::SIZE;
        if bytes.len() != n {
            return Err(crate::Error::Invalid(format!(
                "field layer must be {n} bytes, got {}",
                bytes.len()
            )));
        }
        let o = self.field_layer_offset(layer.min(1));
        self.main.data[o..o + n].copy_from_slice(bytes);
        Ok(())
    }

    /// "Active" (animated) flag of a field item tile. Flags are row-major over the tile grid.
    pub fn field_flag(&self, layer: usize, x: usize, y: usize) -> bool {
        let l = self.field_layout();
        get_flag(
            &self.main.data,
            self.field_flag_offset(layer),
            y * l.width + x,
        )
    }

    pub fn set_field_flag(&mut self, layer: usize, x: usize, y: usize, v: bool) {
        let l = self.field_layout();
        let o = self.field_flag_offset(layer);
        set_flag(&mut self.main.data, o, y * l.width + x, v);
    }

    pub fn clear_field_flags(&mut self, layer: usize) {
        let o = self.field_flag_offset(layer);
        let n = self.field_tile_count() / 8;
        self.main.data[o..o + n].fill(0);
    }

    // --- terrain ------------------------------------------------------------------

    pub fn terrain(&self) -> Vec<TerrainTile> {
        let o = self.main_ofs.land_making_map;
        (0..TILES_W * TILES_H)
            .map(|i| TerrainTile::read(&self.main.data, o + i * TILE_SIZE))
            .collect()
    }

    pub fn set_terrain(&mut self, tiles: &[TerrainTile]) {
        let o = self.main_ofs.land_making_map;
        for (i, t) in tiles.iter().enumerate().take(TILES_W * TILES_H) {
            t.write(&mut self.main.data, o + i * TILE_SIZE);
        }
    }

    pub fn terrain_bytes(&self) -> &[u8] {
        let o = self.main_ofs.land_making_map;
        &self.main.data[o..o + TILES_W * TILES_H * TILE_SIZE]
    }

    pub fn set_terrain_bytes(&mut self, bytes: &[u8]) -> crate::Result<()> {
        let n = TILES_W * TILES_H * TILE_SIZE;
        if bytes.len() != n {
            return Err(crate::Error::Invalid(format!(
                "terrain must be {n} bytes, got {}",
                bytes.len()
            )));
        }
        let o = self.main_ofs.land_making_map;
        self.main.data[o..o + n].copy_from_slice(bytes);
        Ok(())
    }

    // --- acres ---------------------------------------------------------------------

    pub fn acres(&self) -> Vec<u16> {
        let o = self.main_ofs.outside_field;
        read_u16_array(&self.main.data[o..o + ACRE_BYTES])
    }

    pub fn set_acres(&mut self, acres: &[u16]) {
        let o = self.main_ofs.outside_field;
        let n = acres.len().min(ACRES_W * ACRES_H);
        write_u16_array(&mut self.main.data[o..o + n * 2], &acres[..n]);
    }

    pub fn plaza(&self) -> Plaza {
        let o = self.main_ofs.outside_field + ACRE_BYTES;
        Plaza {
            x: u32le(&self.main.data, o + 4),
            z: u32le(&self.main.data, o + 8),
        }
    }

    pub fn set_plaza(&mut self, p: Plaza) {
        let o = self.main_ofs.outside_field + ACRE_BYTES;
        put_u32(&mut self.main.data, o + 4, p.x);
        put_u32(&mut self.main.data, o + 8, p.z);
    }

    pub fn outside_field_template(&self) -> (u16, u16) {
        let o = self.main_ofs.outside_field + ACRE_BYTES;
        (u16le(&self.main.data, o), u16le(&self.main.data, o + 2))
    }

    // --- custom design paths ("my design" tiles on the ground) ------------------------

    /// Size of the ground-design grid in terrain tiles: (acre width × 16, 96).
    /// It grew from 7 to 9 acres wide with 3.0, like the field item layers.
    pub fn map_design_dims(&self) -> (usize, usize) {
        (self.main_ofs.field_item_acre_width * 16, 6 * 16)
    }

    /// One u16 per tile, column-major (`x * 96 + y`): design index, or 0xF800 for none.
    pub fn map_design_tiles(&self) -> Vec<u16> {
        let (w, h) = self.map_design_dims();
        let o = self.main_ofs.my_design_map;
        read_u16_array(&self.main.data[o..o + w * h * 2])
    }

    pub fn set_map_design_tiles(&mut self, v: &[u16]) {
        let (w, h) = self.map_design_dims();
        let o = self.main_ofs.my_design_map;
        let n = v.len().min(w * h);
        write_u16_array(&mut self.main.data[o..o + n * 2], &v[..n]);
    }

    /// Imports a `.nhmd` dump, converting between the 7- and 9-acre layouts if needed.
    pub fn import_map_design_bytes(&mut self, bytes: &[u8]) -> crate::Result<()> {
        let (w, h) = self.map_design_dims();
        let expect = w * h * 2;
        let small = 7 * 16 * h * 2;
        let large = 9 * 16 * h * 2;
        let column = 16 * h * 2;
        let data: Vec<u8> = if bytes.len() == expect {
            bytes.to_vec()
        } else if expect == large && bytes.len() == small {
            let pad: Vec<u8> = MAP_DESIGN_NONE.to_le_bytes().repeat(column / 2);
            [pad.as_slice(), bytes, pad.as_slice()].concat()
        } else if expect == small && bytes.len() == large {
            bytes[column..column + small].to_vec()
        } else if expect == large && bytes.len() > large {
            // older dumps appended unrelated trailing data
            bytes[..large].to_vec()
        } else {
            return Err(crate::Error::Invalid(format!(
                "design map dump must be {expect} bytes (got {})",
                bytes.len()
            )));
        };
        let o = self.main_ofs.my_design_map;
        self.main.data[o..o + expect].copy_from_slice(&data);
        Ok(())
    }

    pub fn map_design_bytes(&self) -> Vec<u8> {
        let (w, h) = self.map_design_dims();
        let o = self.main_ofs.my_design_map;
        self.main.data[o..o + w * h * 2].to_vec()
    }

    // --- painting helpers ------------------------------------------------------------

    /// Trees and building plots within one terrain tile of the given terrain tiles.
    /// The game normally keeps both clear of cliffs and rivers; painting allows it and warns.
    pub fn nearby_objects(&self, tiles: &[(usize, usize)]) -> (usize, usize) {
        let mut bounds: Option<(usize, usize, usize, usize)> = None;
        for &(x, y) in tiles {
            let b = bounds.get_or_insert((x, y, x, y));
            *b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
        }
        let Some((x0, y0, x1, y1)) = bounds else {
            return (0, 0);
        };
        // terrain tile (0,0) sits at absolute item (32,32); one terrain tile is 2 items
        let (ax0, ay0) = (32 + x0.saturating_sub(1) * 2, 32 + y0.saturating_sub(1) * 2);
        let (ax1, ay1) = (32 + (x1 + 2) * 2, 32 + (y1 + 2) * 2);
        let hit = |x: usize, y: usize, w: usize, h: usize| {
            x < ax1 && x + w > ax0 && y < ay1 && y + h > ay0
        };

        let layout = self.field_layout();
        let tree = kinds().tree;
        let mut trees = 0;
        for layer in 0..2 {
            let g = self.field_layer(layer);
            for x in 0..layout.width {
                for y in 0..layout.height {
                    let it = g.get(x, y);
                    if it.is_none() || it.is_extension() || item_kind(it.id) != tree {
                        continue;
                    }
                    let (w, h) = ItemGrid::footprint(&it);
                    if hit(x + layout.offset_x, y + layout.offset_y, w, h) {
                        trees += 1;
                    }
                }
            }
        }

        let mut buildings = 0;
        for b in self.buildings() {
            if b.kind == 0 {
                continue;
            }
            // the whole plot: the buffer applies to the plot edge, not the measured body
            let (fw, fh) = building::footprint(b.kind);
            let (fw, fh) = (fw as usize, fh as usize);
            if hit(
                (b.x as usize).saturating_sub(fw),
                (b.y as usize).saturating_sub(fh),
                fw * 2,
                fh * 2,
            ) {
                buildings += 1;
            }
        }
        (trees, buildings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::tables::tables;
    use crate::editing::fieldtools::{matches, ClearKind};
    use crate::save::revision::REVISIONS;
    use crate::testing::write_synthetic_save;

    fn tree_id() -> u16 {
        *tables()
            .field_items
            .keys()
            .find(|&&id| matches(ClearKind::Trees, &Item::new(id)))
            .expect("the item table has a tree")
    }

    #[test]
    fn counts_trees_and_buildings_within_one_tile() {
        let dir = std::env::temp_dir().join(format!("nhse-nearby-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_synthetic_save(&dir, REVISIONS.len() - 1, 1, 1);
        let mut s = HorizonSave::open(&dir).unwrap();
        let l = s.field_layout();
        let tree = Item::new(tree_id());
        let mut g = s.field_layer(0);
        // terrain tile 4 covers absolute items 40..41, so its one-tile buffer starts at 38
        g.set(38 - l.offset_x, 40 - l.offset_y, tree);
        g.set(36 - l.offset_x, 40 - l.offset_y, tree); // two terrain tiles away
        s.set_field_layer(0, &g);
        assert_eq!(s.nearby_objects(&[(4, 4)]), (1, 0));

        let mut b = s.buildings();
        b[0].kind = 1; // player house, 5×4 terrain tiles around the centre
        b[0].x = 40;
        b[0].y = 40;
        s.set_buildings(&b);
        assert_eq!(s.nearby_objects(&[(4, 4)]), (1, 1));
        assert_eq!(s.nearby_objects(&[(20, 20)]), (0, 0));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
