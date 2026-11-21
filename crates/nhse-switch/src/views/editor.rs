//! Screens of an open save.
use super::items::ItemList;
use super::prettify;
use crate::edits::{self, FieldTool};
use crate::fmt::thousands;
use crate::session;
use crate::ui::{save_nav, Cx, Info, Nav, Reply, Row, Style, View};
use nhse_core::data::tables::enum_name;
use nhse_core::editing::fieldtools::ClearKind;
use nhse_core::save::player::{PlayerList, PlayerProfile};

const T_CLOSE: u32 = 1;
const T_REVERT: u32 = 2;
const T_DONATE: u32 = 3;
const T_RECYCLE: u32 = 4;

fn bells(n: u32) -> String {
    format!("{} Bells", thousands(n as u64))
}

/// The open save: residents, island and saving.
pub struct Editor;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ERow {
    Section,
    Player(usize),
    Map,
    Designs,
    Houses,
    Turnips,
    Villagers,
    Museum,
    Tools,
    Recycle,
    Save,
    Revert,
    Verify,
    Close,
}

impl Editor {
    fn layout(cx: &Cx) -> Vec<(ERow, Row)> {
        let Some(sess) = cx.session.as_ref() else {
            return Vec::new();
        };
        let s = &sess.save;
        let mut v = vec![(ERow::Section, Row::section("Residents"))];
        for p in 0..s.players.len() {
            let wallet = s.player_profile(p).map(|x| x.wallet).unwrap_or(0);
            v.push((
                ERow::Player(p),
                Row::new(s.player_name(p)).value(bells(wallet)).help(
                    "Bells, Nook Miles, pockets, home storage, DIY recipes, catalog and reactions.",
                ),
            ));
        }
        v.push((ERow::Section, Row::section("Island")));
        let t = s.turnips();
        v.push((
            ERow::Turnips,
            Row::new("Turnip prices")
                .value(if t.buy_price > 0 {
                    bells(t.buy_price)
                } else {
                    String::new()
                })
                .help("Daisy Mae's buying price and this week's selling prices at Nook's Cranny."),
        ));
        let villagers = s
            .villagers()
            .iter()
            .filter(|v| v.species != EMPTY_SPECIES)
            .count();
        v.push((
            ERow::Villagers,
            Row::new("Villagers")
                .value(format!("{villagers} of 10"))
                .help("The villagers living on the island. You can ask them to move out."),
        ));
        let (have, all) = edits::museum_progress(s);
        v.push((
            ERow::Museum,
            Row::new("Museum: donate everything")
                .value(format!("{have} of {all}"))
                .style(Style::Action)
                .help(format!(
                    "Donates every fish, bug, sea creature, fossil and genuine piece of art that is not on display yet, credited to {}, 20 per day (as the desktop app does).",
                    s.player_name(0)
                )),
        ));
        v.push((
            ERow::Tools,
            Row::new("Island tools")
                .help("Remove weeds, branches or shells everywhere, or water every flower."),
        ));
        let bin = edits::recycle_bin_count(s);
        v.push((
            ERow::Recycle,
            Row::new("Empty the recycle bin")
                .value(crate::fmt::count(bin, "item"))
                .style(if bin > 0 { Style::Danger } else { Style::Muted })
                .help("Deletes the items in the Resident Services recycle bin."),
        ));
        v.push((ERow::Section, Row::section("Save file")));
        v.push((
            ERow::Save,
            Row::new("Save changes").style(Style::Action).help(match &sess.origin {
                session::Origin::Game(u) => format!(
                    "Writes your changes into the game save of {}. If anything goes wrong while writing, the game's save is left as it was.",
                    u.name
                ),
                session::Origin::Sd => "Writes your changes back to the save on the SD card.".into(),
            }),
        ));
        v.push((
            ERow::Revert,
            Row::new("Discard changes")
                .style(if sess.dirty {
                    Style::Danger
                } else {
                    Style::Muted
                })
                .help("Reloads the save as it was last saved."),
        ));
        v.push((
            ERow::Verify,
            Row::new("Check the save").help(
                "Checks the hashes the game uses to detect damaged data. Saving recomputes them.",
            ),
        ));
        v.push((
            ERow::Close,
            Row::new("Close the save").help("Back to the start screen."),
        ));
        v.extend([
            (ERow::Map, Row::new("Map editor")),
            (ERow::Designs, Row::new("Custom designs")),
            (ERow::Houses, Row::new("Houses")),
        ]);
        v
    }

    fn close(cx: &mut Cx) -> Nav {
        cx.session = None;
        Nav::Root
    }
}

pub const EMPTY_SPECIES: u8 = 35;

impl View for Editor {
    fn title(&self, cx: &Cx) -> String {
        cx.session
            .as_ref()
            .map(|s| format!("{} — game version {}", s.town(), s.save.version()))
            .unwrap_or_default()
    }

    fn rows(&self, cx: &Cx) -> Vec<Row> {
        Self::layout(cx).into_iter().map(|(_, r)| r).collect()
    }

    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        let Some((id, _)) = Self::layout(cx).into_iter().nth(row) else {
            return Nav::Stay;
        };
        match id {
            ERow::Player(p) => Nav::Push(Box::new(Player { p })),
            ERow::Map => Nav::Push(Box::new(super::advanced::Map::new())),
            ERow::Designs => Nav::Push(Box::new(super::advanced::Designs)),
            ERow::Houses => Nav::Push(Box::new(super::advanced::Houses)),
            ERow::Turnips => Nav::Push(Box::new(Turnips)),
            ERow::Villagers => Nav::Push(Box::new(Villagers)),
            ERow::Museum => Nav::confirm(
                "Donate everything to the museum?",
                "Every missing fish, bug, sea creature, fossil and genuine artwork is added to the museum.",
                T_DONATE,
            ),
            ERow::Tools => Nav::Push(Box::new(IslandTools)),
            ERow::Recycle if edits::recycle_bin_count(cx.save()) > 0 => Nav::confirm_danger(
                "Empty the recycle bin?",
                "The items in the recycle bin are deleted.",
                T_RECYCLE,
            ),
            ERow::Save => save_nav(),
            ERow::Revert if cx.dirty() => Nav::confirm_danger(
                "Discard your changes?",
                "The save is reloaded as it was last saved.",
                T_REVERT,
            ),
            ERow::Verify => {
                let bad = cx.save().invalid_hashes();
                if bad.is_empty() {
                    Nav::message("The save is intact", "Every hash checks out.")
                } else if cx.dirty() {
                    Nav::message(
                        "Hashes will be updated",
                        format!(
                            "{} regions changed since the save was opened. Their hashes are recomputed when you save.",
                            bad.len()
                        ),
                    )
                } else {
                    let list: Vec<String> =
                        bad.iter().take(8).map(|h| format!("{}: {}", h.file, h.region)).collect();
                    Nav::message(
                        "Some hashes do not match",
                        format!(
                            "{} regions do not match their hash. Saving recomputes them.\n{}",
                            bad.len(),
                            list.join("\n")
                        ),
                    )
                }
            }
            ERow::Close => self.back(cx),
            _ => Nav::Stay,
        }
    }

    fn back(&mut self, cx: &mut Cx) -> Nav {
        if cx.dirty() {
            Nav::confirm_danger(
                "Close without saving?",
                "Your changes are lost if you close the save now.",
                T_CLOSE,
            )
        } else {
            Self::close(cx)
        }
    }

    fn reply(&mut self, cx: &mut Cx, r: Reply) -> Nav {
        match r {
            Reply::Confirmed(T_CLOSE) => Self::close(cx),
            Reply::Confirmed(T_REVERT) => {
                let res = cx
                    .session
                    .as_mut()
                    .map(|s| session::revert(cx.platform.as_mut(), s));
                match res {
                    Some(Err(e)) => Nav::message("The save could not be reloaded", e),
                    _ => {
                        cx.toast("Changes discarded.");
                        Nav::Stay
                    }
                }
            }
            Reply::Confirmed(T_DONATE) => {
                let n = edits::donate_all(cx.save_mut(), 0);
                cx.toast(if n > 0 {
                    format!("Donated {n} pieces to the museum.")
                } else {
                    "The museum already has everything.".into()
                });
                Nav::Stay
            }
            Reply::Confirmed(T_RECYCLE) => {
                let n = edits::empty_recycle_bin(cx.save_mut());
                cx.toast(format!("Removed {n} items from the recycle bin."));
                Nav::Stay
            }
            _ => Nav::Stay,
        }
    }

    fn info(&self, cx: &Cx, rows: &[Row], row: usize) -> Info {
        let layout = Self::layout(cx);
        let Some((id, r)) = layout.get(row) else {
            return Info::default();
        };
        let mut info = Info {
            title: r.label.clone(),
            text: r.help.clone(),
            facts: Vec::new(),
        };
        let _ = rows;
        let s = cx.save();
        match id {
            ERow::Player(p) => {
                if let Ok(pr) = s.player_profile(*p) {
                    info.facts = vec![
                        ("Wallet".into(), bells(pr.wallet)),
                        ("Bank".into(), bells(pr.bank)),
                        ("Nook Miles".into(), thousands(pr.nook_miles as u64)),
                    ];
                }
                if let Ok((k, n)) = edits::recipe_progress(s, *p) {
                    info.facts
                        .push(("DIY recipes".into(), format!("{k} of {n}")));
                }
            }
            ERow::Save | ERow::Revert | ERow::Verify | ERow::Close => {
                let sess = cx.session.as_ref().expect("session");
                info.facts = vec![
                    ("Game version".into(), s.version().into()),
                    ("Residents".into(), s.players.len().to_string()),
                ];
                if s.revision.compatibility_mode {
                    info.facts.push(("Mode".into(), "Compatibility".into()));
                }
                if let Some(b) = &sess.backup {
                    info.text = format!(
                        "{}\n\nBackup made when opened:\n{}",
                        info.text,
                        b.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default()
                    );
                }
            }
            _ => {}
        }
        info
    }

    fn a_label(&self, cx: &Cx, row: usize) -> &'static str {
        match Self::layout(cx).get(row).map(|x| x.0) {
            Some(ERow::Save) => "Save",
            Some(ERow::Museum) | Some(ERow::Recycle) | Some(ERow::Revert) | Some(ERow::Verify) => {
                "Run"
            }
            _ => "Open",
        }
    }
}

/// One resident.
pub struct Player {
    p: usize,
}

#[derive(Clone, Copy)]
enum Field {
    Wallet,
    Bank,
    Miles,
    TotalMiles,
    Hotel,
    Poki,
}

impl Field {
    fn get(self, pr: &PlayerProfile) -> Option<u32> {
        match self {
            Field::Wallet => Some(pr.wallet),
            Field::Bank => Some(pr.bank),
            Field::Miles => Some(pr.nook_miles),
            Field::TotalMiles => Some(pr.total_nook_miles),
            Field::Hotel => pr.hotel_tickets,
            Field::Poki => pr.poki,
        }
    }
    fn set(self, pr: &mut PlayerProfile, v: u32) {
        match self {
            Field::Wallet => pr.wallet = v,
            Field::Bank => pr.bank = v,
            Field::Miles => pr.nook_miles = v,
            Field::TotalMiles => pr.total_nook_miles = v,
            Field::Hotel => pr.hotel_tickets = Some(v),
            Field::Poki => pr.poki = Some(v),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Field::Wallet => "Wallet",
            Field::Bank => "Bank savings",
            Field::Miles => "Nook Miles",
            Field::TotalMiles => "Nook Miles earned (total)",
            Field::Hotel => "Hotel tickets",
            Field::Poki => "Poki",
        }
    }
    /// The game's own limits.
    fn max(self) -> u32 {
        match self {
            Field::Wallet => 99_999,
            Field::Bank => 999_999_999,
            Field::Miles | Field::TotalMiles => 999_999_999,
            Field::Hotel => 99_999,
            Field::Poki => 9_999_999,
        }
    }
    fn help(self) -> &'static str {
        match self {
            Field::Wallet => "Bells carried in the wallet (up to 99,999).",
            Field::Bank => "Bells saved at the ABD (up to 999,999,999).",
            Field::Miles => "Nook Miles that can be spent.",
            Field::TotalMiles => "Every Nook Mile ever earned (shown in the Nook Miles app).",
            Field::Hotel => "Hotel tickets from the Nook Inc. hotel (game version 3.0).",
            Field::Poki => "Poki, the currency of the Paradise Planning.",
        }
    }
    fn unit(self, v: u32) -> String {
        match self {
            Field::Wallet | Field::Bank => bells(v),
            _ => thousands(v as u64),
        }
    }
}

#[derive(Clone, Copy)]
enum PRow {
    Section,
    Field(Field),
    Items(PlayerList),
    Recipes,
    Catalog,
    Reactions,
}

const T_RECIPES: u32 = 1;
const T_CATALOG: u32 = 2;
const T_REACTIONS: u32 = 3;

impl Player {
    fn layout(&self, cx: &Cx) -> Vec<(PRow, Row)> {
        let s = cx.save();
        let p = self.p;
        let Ok(pr) = s.player_profile(p) else {
            return Vec::new();
        };
        let mut v = vec![(PRow::Section, Row::section("Money"))];
        for f in [
            Field::Wallet,
            Field::Bank,
            Field::Miles,
            Field::TotalMiles,
            Field::Hotel,
            Field::Poki,
        ] {
            if let Some(x) = f.get(&pr) {
                v.push((
                    PRow::Field(f),
                    Row::new(f.label()).value(f.unit(x)).help(f.help()),
                ));
            }
        }
        v.push((PRow::Section, Row::section("Items")));
        let pocket = s.player_items(p, PlayerList::Pocket).unwrap_or_default();
        v.push((
            PRow::Items(PlayerList::Pocket),
            Row::new("Pockets")
                .value(crate::fmt::count(
                    pocket.iter().filter(|i| !i.is_none()).count(),
                    "item",
                ))
                .help("The items the resident carries."),
        ));
        let storage = s.player_items(p, PlayerList::Storage).unwrap_or_default();
        v.push((
            PRow::Items(PlayerList::Storage),
            Row::new("Home storage")
                .value(crate::fmt::count(
                    storage.iter().filter(|i| !i.is_none()).count(),
                    "item",
                ))
                .help("The storage of the resident's house."),
        ));
        v.push((PRow::Section, Row::section("Collections")));
        let (k, n) = edits::recipe_progress(s, p).unwrap_or((0, 0));
        v.push((
            PRow::Recipes,
            Row::new("Learn every DIY recipe")
                .value(format!("{k} of {n}"))
                .style(Style::Action)
                .help("Learns every DIY recipe. The bridge and campsite construction kits are left out: they unlock through the story."),
        ));
        let c = edits::catalog_count(s, p).unwrap_or(0);
        v.push((
            PRow::Catalog,
            Row::new("Complete the catalog")
                .value(crate::fmt::count(c, "item"))
                .style(Style::Action)
                .help("Marks every furniture item, clothing, tool, song, critter, fossil and artwork (with color variants) as collected, so they can be ordered with Nook Shopping. Internal and event-only entries that crash Nook Shopping are left out."),
        ));
        let (k, n) = edits::reaction_progress(s, p).unwrap_or((0, 0));
        v.push((
            PRow::Reactions,
            Row::new("Learn every reaction")
                .value(format!("{k} of {n}"))
                .style(Style::Action)
                .help("Learns every reaction (emote)."),
        ));
        v
    }
}

impl View for Player {
    fn title(&self, cx: &Cx) -> String {
        cx.save().player_name(self.p)
    }

    fn rows(&self, cx: &Cx) -> Vec<Row> {
        self.layout(cx).into_iter().map(|(_, r)| r).collect()
    }

    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        let Some((id, _)) = self.layout(cx).into_iter().nth(row) else {
            return Nav::Stay;
        };
        let p = self.p;
        match id {
            PRow::Field(f) => {
                let Ok(mut pr) = cx.save().player_profile(p) else {
                    return Nav::Stay;
                };
                let cur = f.get(&pr).unwrap_or(0);
                if let Some(v) = cx.ask_number(f.label(), cur as u64, f.max() as u64) {
                    if v as u32 != cur {
                        f.set(&mut pr, v as u32);
                        if let Err(e) = cx.save_mut().set_player_profile(p, &pr) {
                            return Nav::message("Could not change it", e.to_string());
                        }
                        cx.toast(format!("{} set to {}.", f.label(), f.unit(v as u32)));
                    }
                }
                Nav::Stay
            }
            PRow::Items(list) => Nav::Push(Box::new(ItemList::new(p, list))),
            PRow::Recipes => Nav::confirm(
                "Learn every DIY recipe?",
                "Every recipe is marked as learned (and new).",
                T_RECIPES,
            ),
            PRow::Catalog => Nav::confirm(
                "Complete the catalog?",
                "Every safe catalog item is marked as collected.",
                T_CATALOG,
            ),
            PRow::Reactions => Nav::confirm(
                "Learn every reaction?",
                "Every reaction is added to the reaction list.",
                T_REACTIONS,
            ),
            PRow::Section => Nav::Stay,
        }
    }

    fn reply(&mut self, cx: &mut Cx, r: Reply) -> Nav {
        let p = self.p;
        let res = match r {
            Reply::Confirmed(T_RECIPES) => {
                edits::learn_all_recipes(cx.save_mut(), p).map(|n| format!("Learned {n} recipes."))
            }
            Reply::Confirmed(T_CATALOG) => edits::fill_catalog(cx.save_mut(), p)
                .map(|n| format!("Added {n} items to the catalog.")),
            Reply::Confirmed(T_REACTIONS) => edits::learn_all_reactions(cx.save_mut(), p)
                .map(|n| format!("Learned {n} reactions.")),
            _ => return Nav::Stay,
        };
        match res {
            Ok(m) => cx.toast(m),
            Err(e) => cx.toast_error(e.to_string()),
        }
        Nav::Stay
    }

    fn a_label(&self, cx: &Cx, row: usize) -> &'static str {
        match self.layout(cx).get(row).map(|x| x.0) {
            Some(PRow::Field(_)) => "Change",
            Some(PRow::Items(_)) => "Open",
            _ => "Run",
        }
    }

    fn info(&self, cx: &Cx, rows: &[Row], row: usize) -> Info {
        let mut i = Info {
            title: rows.get(row).map(|r| r.label.clone()).unwrap_or_default(),
            text: rows.get(row).map(|r| r.help.clone()).unwrap_or_default(),
            facts: Vec::new(),
        };
        if let Ok(pr) = cx.save().player_profile(self.p) {
            if !pr.invalid_values.is_empty() {
                i.text = format!(
                    "{}\n\nWarning: these values failed their checksum when the save was loaded: {}.",
                    i.text,
                    pr.invalid_values.join(", ")
                );
            }
            i.facts.push(("Resident".into(), pr.name.clone()));
            i.facts.push(("Folder".into(), pr.dir.clone()));
        }
        i
    }
}

/// Turnip prices.
pub struct Turnips;

const DAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

impl View for Turnips {
    fn title(&self, _cx: &Cx) -> String {
        "Turnip prices".into()
    }

    fn rows(&self, cx: &Cx) -> Vec<Row> {
        let t = cx.save().turnips();
        let mut v = vec![
            Row::new("Daisy Mae's price (Sunday)")
                .value(bells(t.buy_price))
                .help("What Daisy Mae asks for a turnip on Sunday morning."),
            Row::new("Set every selling price…")
                .style(Style::Action)
                .help("Sets the same price for every half-day from Monday to Saturday."),
            Row::section("Nook's Cranny buys at"),
        ];
        for (i, price) in t.sell.iter().enumerate().skip(2) {
            let half = if i % 2 == 0 { "morning" } else { "afternoon" };
            v.push(
                Row::new(format!("{} {half}", DAYS[i / 2]))
                    .value(bells(*price))
                    .help("The price Timmy and Tommy pay for a turnip."),
            );
        }
        v
    }

    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        let mut t = cx.save().turnips();
        match row {
            0 => {
                if let Some(v) = cx.ask_number("Daisy Mae's price", t.buy_price as u64, 999) {
                    t.buy_price = v as u32;
                    cx.save_mut().set_turnips(&t);
                }
            }
            1 => {
                if let Some(v) =
                    cx.ask_number("Selling price for the whole week", t.sell[2] as u64, 999)
                {
                    for s in t.sell.iter_mut().skip(2) {
                        *s = v as u32;
                    }
                    cx.save_mut().set_turnips(&t);
                    cx.toast(format!("Every selling price is now {}.", bells(v as u32)));
                }
            }
            r if r >= 3 => {
                let i = r - 3 + 2;
                if let Some(v) = cx.ask_number("Selling price", t.sell[i] as u64, 999) {
                    t.sell[i] = v as u32;
                    cx.save_mut().set_turnips(&t);
                }
            }
            _ => {}
        }
        Nav::Stay
    }

    fn a_label(&self, _cx: &Cx, row: usize) -> &'static str {
        if row == 1 {
            "Set"
        } else {
            "Change"
        }
    }
}

/// The island's villagers.
pub struct Villagers;

impl Villagers {
    fn list(cx: &Cx) -> Vec<nhse_core::structs::villager::Villager> {
        cx.save()
            .villagers()
            .into_iter()
            .filter(|v| v.species != EMPTY_SPECIES)
            .collect()
    }
}

impl View for Villagers {
    fn hints(&self, _: &Cx, _: usize) -> Vec<(&'static str, &'static str)> {
        vec![("X", "Replace")]
    }
    fn button(&mut self, cx: &mut Cx, b: u32, row: usize) -> Option<Nav> {
        if b == crate::platform::btn::X {
            Self::list(cx)
                .get(row)
                .map(|v| Nav::Push(Box::new(super::advanced::Replacement::new(v.index))))
        } else {
            None
        }
    }
    fn title(&self, _cx: &Cx) -> String {
        "Villagers".into()
    }

    fn rows(&self, cx: &Cx) -> Vec<Row> {
        let st = nhse_core::data::strings("en");
        Self::list(cx)
            .iter()
            .map(|v| {
                Row::new(st.villager_name(&v.internal_name)).value(if v.moving_out {
                    "Moving out".to_string()
                } else {
                    enum_name("VillagerPersonality", v.personality as i64)
                        .map(prettify)
                        .unwrap_or_default()
                })
            })
            .collect()
    }

    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        let Some(mut v) = Self::list(cx).into_iter().nth(row) else {
            return Nav::Stay;
        };
        v.moving_out = !v.moving_out;
        let name = nhse_core::data::strings("en").villager_name(&v.internal_name);
        match cx.save_mut().set_villager(&v) {
            Ok(()) => cx.toast(if v.moving_out {
                format!("{name} will move out at the next day change.")
            } else {
                format!("{name} stays.")
            }),
            Err(e) => cx.toast_error(e.to_string()),
        }
        Nav::Stay
    }

    fn a_label(&self, cx: &Cx, row: usize) -> &'static str {
        match Self::list(cx).get(row) {
            Some(v) if v.moving_out => "Let them stay",
            _ => "Ask to move out",
        }
    }

    fn info(&self, cx: &Cx, _rows: &[Row], row: usize) -> Info {
        let st = nhse_core::data::strings("en");
        let Some(v) = Self::list(cx).into_iter().nth(row) else {
            return Info::default();
        };
        Info {
            title: st.villager_name(&v.internal_name),
            text: if v.moving_out {
                "Flagged to move out: they leave the island at the next day change. Press A to let them stay.".into()
            } else {
                "Press A to flag this villager as moving out. They leave the island at the next day change.".into()
            },
            facts: vec![
                (
                    "Species".into(),
                    enum_name("VillagerSpecies", v.species as i64)
                        .map(prettify)
                        .unwrap_or_default(),
                ),
                (
                    "Personality".into(),
                    enum_name("VillagerPersonality", v.personality as i64)
                        .map(prettify)
                        .unwrap_or_default(),
                ),
                ("Catchphrase".into(), v.catchphrase.clone()),
                ("Internal name".into(), v.internal_name.clone()),
            ],
        }
    }

    fn empty_text(&self) -> &str {
        "No villagers live on the island."
    }
}

/// Bulk tools for the island map.
pub struct IslandTools;

const TOOLS: [(FieldTool, &str, &str, &str); 5] = [
    (
        FieldTool::Clear(ClearKind::Weeds),
        "Remove all weeds",
        "weeds",
        "Pulls every weed (and clover) on the island.",
    ),
    (
        FieldTool::Clear(ClearKind::Branches),
        "Remove all tree branches",
        "branches",
        "Removes every tree branch lying on the ground.",
    ),
    (
        FieldTool::Clear(ClearKind::Shells),
        "Remove all shells",
        "shells",
        "Removes every shell on the beaches. New ones wash up each day.",
    ),
    (
        FieldTool::Clear(ClearKind::Holes),
        "Fill every hole",
        "holes",
        "Fills every hole dug in the ground.",
    ),
    (
        FieldTool::WaterFlowers,
        "Water every flower",
        "flowers",
        "Waters every flower as if all your visitors had watered them, which helps hybrids grow.",
    ),
];

impl View for IslandTools {
    fn title(&self, _cx: &Cx) -> String {
        "Island tools".into()
    }

    fn rows(&self, _cx: &Cx) -> Vec<Row> {
        TOOLS
            .iter()
            .map(|(t, label, _, help)| {
                Row::new(*label)
                    .style(if matches!(t, FieldTool::WaterFlowers) {
                        Style::Action
                    } else {
                        Style::Danger
                    })
                    .help(*help)
            })
            .collect()
    }

    fn activate(&mut self, _cx: &mut Cx, row: usize) -> Nav {
        match TOOLS.get(row) {
            Some((_, label, _, help)) => Nav::confirm(format!("{label}?"), *help, row as u32),
            None => Nav::Stay,
        }
    }

    fn reply(&mut self, cx: &mut Cx, r: Reply) -> Nav {
        if let Reply::Confirmed(i) = r {
            if let Some((tool, _, what, _)) = TOOLS.get(i as usize) {
                let n = edits::field_tool(cx.save_mut(), *tool);
                cx.toast(match tool {
                    FieldTool::WaterFlowers => format!("Watered {n} {what}."),
                    _ => format!("Removed {n} {what}."),
                });
            }
        }
        Nav::Stay
    }

    fn a_label(&self, _cx: &Cx, _row: usize) -> &'static str {
        "Run"
    }
}
