//! Pockets, home storage, one item, and the item search.
use crate::edits::{item_label, stack_size};
use crate::platform::btn;
use crate::ui::{Cx, Info, Nav, Reply, Row, Style, View};
use nhse_core::data::items::item_kind;
use nhse_core::data::tables::{enum_name, tables};
use nhse_core::save::player::PlayerList;
use nhse_core::Item;

fn items(cx: &Cx, p: usize, list: PlayerList) -> Vec<Item> {
    cx.save().player_items(p, list).unwrap_or_default()
}

fn set_item(cx: &mut Cx, p: usize, list: PlayerList, slot: usize, it: Item) -> Result<(), String> {
    let mut v = items(cx, p, list);
    let Some(x) = v.get_mut(slot) else {
        return Err("No such slot.".into());
    };
    *x = it;
    cx.save_mut()
        .set_player_items(p, list, &v)
        .map_err(|e| e.to_string())
}

fn hex(it: &Item) -> String {
    format!("{:016X}", it.raw())
}

fn item_facts(it: &Item) -> Vec<(String, String)> {
    if it.is_none() {
        return Vec::new();
    }
    let mut f = vec![("Item id".into(), format!("0x{:04X}", it.id))];
    if let Some(k) = enum_name("ItemKind", item_kind(it.id) as i64) {
        f.push(("Kind".into(), super::prettify(k)));
    }
    if let Some(m) = stack_size(it) {
        f.push((
            "Quantity".into(),
            format!("{} of {m}", it.count() as u32 + 1),
        ));
    }
    f.push(("Raw value".into(), hex(it)));
    f
}

/// Pockets or home storage.
pub struct ItemList {
    p: usize,
    list: PlayerList,
}

impl ItemList {
    pub fn new(p: usize, list: PlayerList) -> Self {
        Self { p, list }
    }

    /// The slot of each row (`None` for headings).
    fn slots(&self, cx: &Cx) -> Vec<Option<usize>> {
        match self.list {
            PlayerList::Pocket => {
                let n = items(cx, self.p, self.list).len();
                let first = n.min(20);
                let mut v = vec![None];
                v.extend((0..first).map(Some));
                if n > first {
                    v.push(None);
                    v.extend((first..n).map(Some));
                }
                v
            }
            PlayerList::Storage => {
                let s = cx.save();
                let all = items(cx, self.p, self.list).len();
                // the unlocked storage size, or every slot when it is not plausible
                let unlocked = s
                    .player_profile(self.p)
                    .map(|pr| pr.storage_count as usize)
                    .unwrap_or(0);
                let n = if (1..=all).contains(&unlocked) {
                    unlocked
                } else {
                    all
                };
                (0..n).map(Some).collect()
            }
        }
    }

    fn slot_at(&self, cx: &Cx, row: usize) -> Option<usize> {
        self.slots(cx).get(row).copied().flatten()
    }
}

impl View for ItemList {
    fn title(&self, cx: &Cx) -> String {
        let name = cx.save().player_name(self.p);
        match self.list {
            PlayerList::Pocket => format!("{name} — Pockets"),
            PlayerList::Storage => format!("{name} — Home storage"),
        }
    }

    fn rows(&self, cx: &Cx) -> Vec<Row> {
        let v = items(cx, self.p, self.list);
        let mut section = 0;
        self.slots(cx)
            .into_iter()
            .map(|s| match s {
                None => {
                    section += 1;
                    Row::section(if section == 1 {
                        "Pocket"
                    } else {
                        "Bag (pocket upgrade)"
                    })
                }
                Some(i) => {
                    let it = v[i];
                    Row::new(item_label(&it))
                        .value(format!("{}", i + 1))
                        .style(if it.is_none() {
                            Style::Muted
                        } else {
                            Style::Normal
                        })
                }
            })
            .collect()
    }

    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        match self.slot_at(cx, row) {
            Some(slot) => Nav::Push(Box::new(ItemEditor {
                p: self.p,
                list: self.list,
                slot,
            })),
            None => Nav::Stay,
        }
    }

    fn button(&mut self, cx: &mut Cx, b: u32, row: usize) -> Option<Nav> {
        let slot = self.slot_at(cx, row)?;
        let it = items(cx, self.p, self.list).get(slot).copied()?;
        match b {
            btn::Y => {
                cx.clipboard = Some(it);
                cx.toast(format!("Copied {}.", item_label(&it)));
                Some(Nav::Stay)
            }
            btn::X => {
                let c = cx.clipboard?;
                if let Err(e) = set_item(cx, self.p, self.list, slot, c) {
                    cx.toast_error(e);
                }
                Some(Nav::Stay)
            }
            btn::MINUS => {
                if !it.is_none() {
                    if let Err(e) = set_item(cx, self.p, self.list, slot, Item::NONE) {
                        cx.toast_error(e);
                    }
                }
                Some(Nav::Stay)
            }
            _ => None,
        }
    }

    fn hints(&self, cx: &Cx, _row: usize) -> Vec<(&'static str, &'static str)> {
        let mut h = vec![("Y", "Copy")];
        if cx.clipboard.is_some() {
            h.push(("X", "Paste"));
        }
        h.push(("−", "Clear"));
        h
    }

    fn a_label(&self, _cx: &Cx, _row: usize) -> &'static str {
        "Edit"
    }

    fn info(&self, cx: &Cx, _rows: &[Row], row: usize) -> Info {
        let Some(slot) = self.slot_at(cx, row) else {
            return Info::default();
        };
        let it = items(cx, self.p, self.list)[slot];
        let mut i = Info {
            title: item_label(&it),
            text: if it.is_none() {
                "An empty slot. Press A to put an item here, or X to paste a copied item.".into()
            } else {
                "A: change the item or quantity. Y: copy, X: paste, −: clear the slot.".into()
            },
            facts: item_facts(&it),
        };
        if let Some(c) = &cx.clipboard {
            i.text = format!("{}\n\nCopied: {}", i.text, item_label(c));
        }
        i
    }
}

/// One item slot.
pub struct ItemEditor {
    p: usize,
    list: PlayerList,
    slot: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IRow {
    Item,
    Quantity,
    Fill,
    Variant,
    Raw,
    Remove,
}

impl ItemEditor {
    fn item(&self, cx: &Cx) -> Item {
        items(cx, self.p, self.list)
            .get(self.slot)
            .copied()
            .unwrap_or(Item::NONE)
    }

    fn variants(it: &Item) -> Vec<u32> {
        tables()
            .remake_for_item(it.id)
            .map(|r| {
                (0..8)
                    .filter(|v| r.has_body_color(*v))
                    .map(|v| v as u32)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn layout(&self, cx: &Cx) -> Vec<(IRow, Row)> {
        let it = self.item(cx);
        let mut v = vec![(
            IRow::Item,
            Row::new("Item")
                .value(nhse_core::data::strings("en").item_name(it.id))
                .help("Search for an item to put in this slot."),
        )];
        if it.is_none() {
            return v;
        }
        if let Some(max) = stack_size(&it) {
            v.push((
                IRow::Quantity,
                Row::new("Quantity")
                    .value(format!("{} of {max}", it.count() as u32 + 1))
                    .help(format!("How many are in the stack (1 to {max}).")),
            ));
            v.push((
                IRow::Fill,
                Row::new("Fill the stack")
                    .style(Style::Action)
                    .help(format!("Sets the quantity to {max}.")),
            ));
        }
        let variants = Self::variants(&it);
        if variants.len() > 1 {
            let k = variants
                .iter()
                .position(|x| *x == it.body_type())
                .map(|k| k + 1)
                .unwrap_or(0);
            v.push((
                IRow::Variant,
                Row::new("Color variant")
                    .value(format!("{k} of {}", variants.len()))
                    .help("Press A, or left and right, to switch between the item's colors."),
            ));
        }
        v.push((
            IRow::Raw,
            Row::new("Raw value").value(hex(&it)).help(
                "The item's 8 bytes as 16 hexadecimal digits, as other tools and cheat codes write them (id, flags, then the extra value).",
            ),
        ));
        v.push((
            IRow::Remove,
            Row::new("Remove the item")
                .style(Style::Danger)
                .help("Empties this slot."),
        ));
        v
    }

    fn put(&self, cx: &mut Cx, it: Item) {
        if let Err(e) = set_item(cx, self.p, self.list, self.slot, it) {
            cx.toast_error(e);
        }
    }

    fn cycle_variant(&self, cx: &mut Cx, step: i32) {
        let mut it = self.item(cx);
        let vs = Self::variants(&it);
        if vs.len() < 2 {
            return;
        }
        let k = vs.iter().position(|x| *x == it.body_type()).unwrap_or(0) as i32;
        let n = vs.len() as i32;
        it.set_body_type(vs[((k + step).rem_euclid(n)) as usize]);
        self.put(cx, it);
    }
}

impl View for ItemEditor {
    fn title(&self, cx: &Cx) -> String {
        let name = cx.save().player_name(self.p);
        let where_ = match self.list {
            PlayerList::Pocket => "Pockets",
            PlayerList::Storage => "Home storage",
        };
        format!("{name} — {where_}, slot {}", self.slot + 1)
    }

    fn rows(&self, cx: &Cx) -> Vec<Row> {
        self.layout(cx).into_iter().map(|(_, r)| r).collect()
    }

    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        let Some((id, _)) = self.layout(cx).into_iter().nth(row) else {
            return Nav::Stay;
        };
        let mut it = self.item(cx);
        match id {
            IRow::Item => {
                match cx.ask_text("Search for an item", "Name or id (e.g. 0x0A1B)", "", 40) {
                    Some(q) if !q.trim().is_empty() => Nav::Push(Box::new(ItemSearch::new(&q))),
                    _ => Nav::Stay,
                }
            }
            IRow::Quantity => {
                let max = stack_size(&it).unwrap_or(1) as u64;
                if let Some(n) = cx.ask_number("Quantity", it.count() as u64 + 1, max) {
                    it.set_count(n.max(1) as u16 - 1);
                    self.put(cx, it);
                }
                Nav::Stay
            }
            IRow::Fill => {
                if let Some(max) = stack_size(&it) {
                    it.set_count(max - 1);
                    self.put(cx, it);
                }
                Nav::Stay
            }
            IRow::Variant => {
                self.cycle_variant(cx, 1);
                Nav::Stay
            }
            IRow::Raw => {
                if let Some(t) = cx.ask_text("Raw value", "16 hexadecimal digits", &hex(&it), 16) {
                    let t: String = t.chars().filter(|c| !c.is_whitespace()).collect();
                    match u64::from_str_radix(t.trim_start_matches("0x"), 16) {
                        Ok(v) => self.put(cx, Item::from_raw(v)),
                        Err(_) => cx.toast_error("That is not a hexadecimal number."),
                    }
                }
                Nav::Stay
            }
            IRow::Remove => {
                self.put(cx, Item::NONE);
                Nav::Pop
            }
        }
    }

    fn button(&mut self, cx: &mut Cx, b: u32, row: usize) -> Option<Nav> {
        let id = self.layout(cx).get(row)?.0;
        if id == IRow::Variant && (b == btn::LEFT || b == btn::RIGHT) {
            self.cycle_variant(cx, if b == btn::LEFT { -1 } else { 1 });
            return Some(Nav::Stay);
        }
        None
    }

    fn reply(&mut self, cx: &mut Cx, r: Reply) -> Nav {
        if let Reply::Item(new) = r {
            let old = self.item(cx);
            let mut it = new;
            // keep the quantity when switching between stackable items
            if let (Some(max), Some(_)) = (stack_size(&it), stack_size(&old)) {
                it.set_count(old.count().min(max - 1));
            }
            self.put(cx, it);
            cx.toast(format!(
                "Slot {} now holds {}.",
                self.slot + 1,
                item_label(&it)
            ));
        }
        Nav::Stay
    }

    fn a_label(&self, cx: &Cx, row: usize) -> &'static str {
        match self.layout(cx).get(row).map(|x| x.0) {
            Some(IRow::Item) => "Search",
            Some(IRow::Variant) => "Next color",
            Some(IRow::Remove) => "Remove",
            Some(IRow::Fill) => "Fill",
            _ => "Change",
        }
    }

    fn info(&self, cx: &Cx, rows: &[Row], row: usize) -> Info {
        let it = self.item(cx);
        Info {
            title: item_label(&it),
            text: rows.get(row).map(|r| r.help.clone()).unwrap_or_default(),
            facts: item_facts(&it),
        }
    }
}

/// Results of an item search.
pub struct ItemSearch {
    query: String,
    results: Vec<u16>,
}

/// Item ids matching `query`: every word must appear in the name; exact and leading matches
/// come first. `0x1A2B` or `#1A2B` looks up an id.
pub fn search_items(query: &str) -> Vec<u16> {
    let st = nhse_core::data::strings("en");
    let q = query.trim().to_lowercase();
    let hexq = q.strip_prefix("0x").or_else(|| q.strip_prefix('#'));
    if let Some(h) = hexq {
        if let Ok(id) = u16::from_str_radix(h, 16) {
            return vec![id];
        }
    }
    let words: Vec<&str> = q.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, usize, u16)> = Vec::new();
    for (id, name) in st.items.iter().enumerate().skip(1) {
        if name.trim().is_empty() || id >= 0xFFFD {
            continue;
        }
        let n = name.to_lowercase();
        if words.iter().all(|w| n.contains(w)) {
            let rank = if n == q {
                0
            } else if n.starts_with(&q) {
                1
            } else {
                2
            };
            hits.push((rank, n.len(), id as u16));
        }
    }
    hits.sort();
    hits.truncate(500);
    hits.into_iter().map(|h| h.2).collect()
}

impl ItemSearch {
    pub fn new(q: &str) -> Self {
        Self {
            query: q.to_string(),
            results: search_items(q),
        }
    }
}

impl View for ItemSearch {
    fn title(&self, _cx: &Cx) -> String {
        format!("Items matching “{}”", self.query)
    }

    fn rows(&self, _cx: &Cx) -> Vec<Row> {
        let st = nhse_core::data::strings("en");
        self.results
            .iter()
            .map(|id| Row::new(st.item_name(*id)).value(format!("0x{id:04X}")))
            .collect()
    }

    fn activate(&mut self, _cx: &mut Cx, row: usize) -> Nav {
        match self.results.get(row) {
            Some(id) => Nav::PopReply(Reply::Item(Item::new(*id))),
            None => Nav::Stay,
        }
    }

    fn button(&mut self, cx: &mut Cx, b: u32, _row: usize) -> Option<Nav> {
        if b != btn::Y {
            return None;
        }
        if let Some(q) = cx.ask_text(
            "Search for an item",
            "Name or id (e.g. 0x0A1B)",
            &self.query,
            40,
        ) {
            if !q.trim().is_empty() {
                *self = ItemSearch::new(&q);
            }
        }
        Some(Nav::Stay)
    }

    fn hints(&self, _cx: &Cx, _row: usize) -> Vec<(&'static str, &'static str)> {
        vec![("Y", "New search")]
    }

    fn a_label(&self, _cx: &Cx, _row: usize) -> &'static str {
        "Choose"
    }

    fn empty_text(&self) -> &str {
        "No item has that name. Press Y to search again."
    }

    fn info(&self, _cx: &Cx, _rows: &[Row], row: usize) -> Info {
        let Some(id) = self.results.get(row) else {
            return Info::default();
        };
        let it = Item::new(*id);
        Info {
            title: nhse_core::data::strings("en").item_name(*id),
            text: format!(
                "{} results. Press A to put this item in the slot.",
                self.results.len()
            ),
            facts: item_facts(&it),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_finds_items() {
        let r = search_items("wood");
        assert!(!r.is_empty());
        let st = nhse_core::data::strings("en");
        assert_eq!(
            st.item_name(r[0]).to_lowercase(),
            "wood",
            "exact match first"
        );
        assert_eq!(search_items("0x16A2"), vec![0x16A2]);
        assert!(search_items("   ").is_empty());
    }
}
