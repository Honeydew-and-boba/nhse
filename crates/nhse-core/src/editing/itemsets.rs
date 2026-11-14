//! Item lists for bulk actions (catalog, museum, storage fill), shared by every frontend.
use crate::data::items::{item_count, item_kind, kinds, max_stack};
use crate::data::strings;
use crate::data::tables::{enum_name, tables};
use crate::structs::Item;

/// Categories understood by [`item_set`], with a display label.
pub const CATEGORIES: &[(&str, &str)] = &[
    ("fish", "Fish"),
    ("bugs", "Bugs"),
    ("sea", "Sea creatures"),
    ("fossils", "Fossils"),
    ("art", "Art"),
    ("furniture", "Furniture"),
    ("wearables", "Clothing & accessories"),
    ("interior", "Wallpaper, floors & rugs"),
    ("tools", "Tools"),
    ("music", "Songs"),
];

/// Recipes that unlock through the story (bridge and campsite construction kits).
pub const STORY_RECIPES: [u16; 2] = [0x1CA, 0x1CD];

/// `ItemKind` name of an item id.
pub fn kind_of(id: u16) -> &'static str {
    enum_name("ItemKind", item_kind(id) as i64).unwrap_or("")
}

/// Kinds that must never be handed out in bulk (internal/dummy entries that can crash the game).
pub fn is_unsafe_kind(k: &str) -> bool {
    k.starts_with("Kind_Dummy")
        || matches!(
            k,
            "Kind_SequenceOnly"
                | "Kind_NpcOutfit"
                | "Kind_PlayerDemoOutfit"
                | "Kind_EventObjFtr"
                | "Kind_NnpcRoomMarker"
                | "Kind_PhotoStudioList"
                | "Kind_ShopTorso"
                | "Kind_GardenEditList"
                | "Kind_OneRoomBox"
                | "Kind_MyDesignObject"
                | "Kind_MyDesignTexture"
                | "Kind_CommonFabricRug"
                | "Kind_CommonFabricObject"
                | "Kind_CommonFabricTexture"
                | "Kind_SmartPhone"
                | "Kind_DummyFtr"
                | "Unknown"
                | ""
        )
}

/// Builds a list of items for bulk actions: `wearables`, `furniture`, `interior`, `tools`, `fossils`,
/// `art`, `fish`, `bugs`, `sea`, `music`, `recipes`, `materials`. With `variants`, every body color is included.
pub fn item_set(category: &str, variants: bool) -> Vec<Item> {
    let s = strings("en");
    let t = tables();
    let kd = kinds();
    let skip: std::collections::HashSet<u16> = t.list("NoCheckReceived").iter().copied().collect();
    let named = |id: u16| {
        s.items
            .get(id as usize)
            .is_some_and(|n| !n.trim().is_empty())
            && !skip.contains(&id)
    };
    let mut ids: Vec<u16> = match category {
        "fish" => t.list("Fish").to_vec(),
        "bugs" => t.list("Bugs").to_vec(),
        "sea" => t.list("Dive").to_vec(),
        "fossils" => t.list("Fossils").to_vec(),
        "art" => t.list("Art").to_vec(),
        "recipes" => {
            return t
                .recipes
                .iter()
                .filter(|(r, _)| *r != 0x1CA && *r != 0x1CD)
                .map(|(r, _)| Item {
                    id: Item::DIY_RECIPE,
                    system: 0,
                    additional: 0,
                    free: *r as u32,
                })
                .collect();
        }
        _ => (1..item_count() as u16)
            .filter(|id| named(*id))
            .filter(|id| {
                let k = kind_of(*id);
                if is_unsafe_kind(k) {
                    return false;
                }
                match category {
                    "wearables" => {
                        kd.is_clothing(item_kind(*id))
                            || matches!(
                                k,
                                "Kind_Cap"
                                    | "Kind_Helmet"
                                    | "Kind_Accessory"
                                    | "Kind_Bag"
                                    | "Kind_Socks"
                                    | "Kind_Umbrella"
                                    | "Kind_HandBag"
                            )
                    }
                    "furniture" => {
                        kd.is_furniture(item_kind(*id))
                            || matches!(k, "Kind_FtrWall" | "Kind_Counter" | "Kind_Pillar")
                    }
                    "interior" => matches!(k, "Kind_RoomWall" | "Kind_RoomFloor" | "Kind_Rug"),
                    "tools" => matches!(
                        k,
                        "Kind_FishingRod"
                            | "Kind_Net"
                            | "Kind_Shovel"
                            | "Kind_Axe"
                            | "Kind_Watering"
                            | "Kind_Slingshot"
                            | "Kind_Ladder"
                            | "Kind_ChangeStick"
                    ),
                    "music" => k == "Kind_Music",
                    "materials" => kd.is_crafting(item_kind(*id)),
                    _ => false,
                }
            })
            .collect(),
    };
    ids.dedup();
    let mut out = Vec::new();
    for id in ids {
        let mut it = Item::new(id);
        if let Some(max) = max_stack(id) {
            if max > 1 && matches!(category, "materials") {
                it.set_count(max - 1);
            }
        }
        match (variants, t.remake_for_item(id)) {
            (true, Some(r)) => {
                for v in 0..8 {
                    if r.has_body_color(v) {
                        let mut x = it;
                        x.set_body_type(v as u32);
                        out.push(x);
                    }
                }
            }
            _ => out.push(it),
        }
    }
    out
}

/// Museum-donatable ids of a category (`fish`, `bugs`, `sea`, `fossils`, `art`).
/// Art only lists genuine pieces (forgeries have a parenthesised suffix).
pub fn museum_ids(category: &str) -> Vec<u16> {
    let t = tables();
    match category {
        "fish" => t.list("Fish").to_vec(),
        "bugs" => t.list("Bugs").to_vec(),
        "sea" => t.list("Dive").to_vec(),
        "fossils" => t.list("Fossils").to_vec(),
        "art" => {
            let en = strings("en");
            t.list("Art")
                .iter()
                .copied()
                .filter(|i| !en.items.get(*i as usize).is_some_and(|n| n.contains('(')))
                .collect()
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_are_non_empty() {
        for (c, _) in CATEGORIES {
            assert!(!item_set(c, false).is_empty(), "{c}");
        }
        assert!(!item_set("recipes", false).is_empty());
        assert!(!museum_ids("art").is_empty());
    }
}
