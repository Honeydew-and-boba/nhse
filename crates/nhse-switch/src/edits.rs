//! Bulk edits, as the desktop app does them.
use nhse_core::data::tables::{enum_values, tables};
use nhse_core::editing::fieldtools::{self, ClearKind};
use nhse_core::editing::itemsets::{self, CATEGORIES, STORY_RECIPES};
use nhse_core::{HorizonSave, Item, Result};

/// Recipes learned by a player, and how many recipes exist.
pub fn recipe_progress(s: &HorizonSave, p: usize) -> Result<(usize, usize)> {
    let flags = s.recipes(p)?;
    let all = &tables().recipes;
    let known = all
        .iter()
        .filter(|(r, _)| flags.get(*r as usize).is_some_and(|f| f[0]))
        .count();
    Ok((known, all.len()))
}

/// Learns every DIY recipe (marked as new), except the story's construction kits.
pub fn learn_all_recipes(s: &mut HorizonSave, p: usize) -> Result<usize> {
    let mut flags = s.recipes(p)?;
    let mut n = 0;
    for (r, _) in &tables().recipes {
        if STORY_RECIPES.contains(r) {
            continue;
        }
        if let Some(f) = flags.get_mut(*r as usize) {
            if !f[0] {
                f[0] = true;
                f[2] = true;
                n += 1;
            }
        }
    }
    s.set_recipes(p, &flags)?;
    Ok(n)
}

/// Catalog entries collected by a player.
pub fn catalog_count(s: &HorizonSave, p: usize) -> Result<usize> {
    Ok(s.received_items(p)?.0.iter().filter(|b| **b).count())
}

/// Marks every safe catalog item (and its color variants) as collected. Internal and
/// event-only entries, which crash Nook Shopping, are left out.
pub fn fill_catalog(s: &mut HorizonSave, p: usize) -> Result<usize> {
    let (mut items, mut remake) = s.received_items(p)?;
    let t = tables();
    let mut n = 0;
    for (cat, _) in CATEGORIES {
        for it in itemsets::item_set(cat, false) {
            if let Some(slot) = items.get_mut(it.id as usize) {
                if !*slot {
                    *slot = true;
                    n += 1;
                }
            }
            if let (Some(ri), Some(r)) = (t.remake_map.get(&it.id), t.remake_for_item(it.id)) {
                for v in 0..8 {
                    if r.has_body_color(v) {
                        if let Some(b) = remake.get_mut(*ri as usize * 8 + v) {
                            *b = true;
                        }
                    }
                }
            }
        }
    }
    s.set_received_items(p, &items, &remake)?;
    Ok(n)
}

fn known_reactions() -> impl Iterator<Item = u8> {
    enum_values("Reaction")
        .iter()
        .filter(|(name, v)| *v != 0 && !name.starts_with("UNUSED"))
        .map(|(_, v)| *v as u8)
}

/// Reactions a player knows, and how many exist.
pub fn reaction_progress(s: &HorizonSave, p: usize) -> Result<(usize, usize)> {
    let r = s.reactions(p)?;
    let all: Vec<u8> = known_reactions().collect();
    let known = all.iter().filter(|v| r.learned.contains(v)).count();
    Ok((known, all.len()))
}

/// Learns every reaction, filling free slots.
pub fn learn_all_reactions(s: &mut HorizonSave, p: usize) -> Result<usize> {
    let mut r = s.reactions(p)?;
    let mut n = 0;
    for v in known_reactions() {
        if r.learned.contains(&v) {
            continue;
        }
        let Some(slot) = r.learned.iter().position(|x| *x == 0) else {
            break;
        };
        r.learned[slot] = v;
        n += 1;
    }
    s.set_reactions(p, &r)?;
    Ok(n)
}

pub const MUSEUM_CATEGORIES: [&str; 5] = ["fish", "bugs", "sea", "fossils", "art"];

/// Pieces on display, and how many can be donated.
pub fn museum_progress(s: &HorizonSave) -> (usize, usize) {
    let m = s.museum();
    let have: std::collections::HashSet<u16> = m
        .items
        .iter()
        .filter(|i| !i.is_none())
        .map(|i| i.id)
        .collect();
    let all: std::collections::HashSet<u16> = MUSEUM_CATEGORIES
        .iter()
        .flat_map(|c| itemsets::museum_ids(c))
        .collect();
    (all.iter().filter(|i| have.contains(i)).count(), all.len())
}

/// Donates every fish, bug, sea creature, fossil and genuine artwork not yet on display,
/// credited to `player`, spread at 20 per day like the desktop app.
pub fn donate_all(s: &mut HorizonSave, player: u8) -> usize {
    let ids: Vec<u16> = MUSEUM_CATEGORIES
        .iter()
        .flat_map(|c| itemsets::museum_ids(c))
        .collect();
    let mut m = s.museum();
    let n = m.donate_all(&ids, player, 20);
    s.set_museum(&m);
    n
}

/// Field tools on the island's main item layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldTool {
    Clear(ClearKind),
    /// Gold-watering: every flower watered by all visitors.
    WaterFlowers,
}

pub fn field_tool(s: &mut HorizonSave, tool: FieldTool) -> usize {
    let mut g = s.field_layer(0);
    let (w, h) = (g.width, g.height);
    let n = match tool {
        FieldTool::Clear(kind) => fieldtools::clear(&mut g, kind, 0, 0, w, h),
        FieldTool::WaterFlowers => fieldtools::water_flowers(&mut g, true, 0, 0, w, h),
    };
    s.set_field_layer(0, &g);
    n
}

/// Number of items in the recycle bin.
pub fn recycle_bin_count(s: &HorizonSave) -> usize {
    s.recycle_bin().iter().filter(|i| !i.is_none()).count()
}

pub fn empty_recycle_bin(s: &mut HorizonSave) -> usize {
    let n = recycle_bin_count(s);
    let len = s.recycle_bin().len();
    s.set_recycle_bin(&vec![Item::NONE; len]);
    n
}

/// Display text of an item stack ("Wood × 30").
pub fn item_label(it: &Item) -> String {
    let s = nhse_core::data::strings("en");
    if it.is_none() {
        return "(empty)".into();
    }
    let name = it.name(s);
    match stack_size(it) {
        Some(_) if it.count() > 0 => format!("{name}  × {}", it.count() as u32 + 1),
        _ => name,
    }
}

/// Maximum stack of a stackable item (`None` when it does not stack).
pub fn stack_size(it: &Item) -> Option<u16> {
    nhse_core::data::items::max_stack(it.id).filter(|m| *m > 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save() -> (tempfile::TempDir, HorizonSave) {
        let dir = tempfile::tempdir().unwrap();
        let rev = nhse_core::save::REVISIONS.len() - 1;
        nhse_core::testing::write_synthetic_save(dir.path(), rev, 2, 7);
        let s = HorizonSave::open(dir.path()).unwrap();
        (dir, s)
    }

    #[test]
    fn bulk_edits_are_idempotent() {
        let (_d, mut s) = save();
        let n = learn_all_recipes(&mut s, 0).unwrap();
        assert!(n > 0);
        assert_eq!(learn_all_recipes(&mut s, 0).unwrap(), 0);
        let (known, all) = recipe_progress(&s, 0).unwrap();
        assert!(known + STORY_RECIPES.len() >= all && known <= all);

        assert!(fill_catalog(&mut s, 1).unwrap() > 0);
        assert_eq!(fill_catalog(&mut s, 1).unwrap(), 0);

        learn_all_reactions(&mut s, 0).unwrap();
        assert_eq!(learn_all_reactions(&mut s, 0).unwrap(), 0);

        donate_all(&mut s, 0);
        assert_eq!(donate_all(&mut s, 0), 0);

        empty_recycle_bin(&mut s);
        assert_eq!(recycle_bin_count(&s), 0);
        field_tool(&mut s, FieldTool::Clear(ClearKind::Weeds));
        assert_eq!(field_tool(&mut s, FieldTool::Clear(ClearKind::Weeds)), 0);
    }
}
