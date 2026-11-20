//! Controller editors using the same save accessors as the desktop app.
use crate::ui::{Cx, Nav, Reply, Row, View};
use nhse_core::{save::island::DesignKind, structs::design};

fn result(cx: &mut Cx, r: Result<(), impl std::fmt::Display>) {
    match r {
        Ok(()) => cx.toast("Changed. Press + in the editor to save."),
        Err(e) => cx.toast_error(e.to_string()),
    }
}

pub struct Replacement {
    pub index: usize,
    query: String,
    names: Vec<String>,
}
impl Replacement {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            query: String::new(),
            names: nhse_core::editing::villagers::available_templates().to_vec(),
        }
    }
}
impl View for Replacement {
    fn title(&self, _: &Cx) -> String {
        "Replace villager".into()
    }
    fn rows(&self, _: &Cx) -> Vec<Row> {
        let st = nhse_core::data::strings("en");
        std::iter::once(Row::new("Search villagers").value(&self.query)).chain(self.names.iter().map(|n| Row::new(st.villager_name(n)).help("Replaces this villager, their house and belongings. Player relationship identities are preserved."))).collect()
    }
    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        if row == 0 {
            if let Some(q) =
                cx.ask_text("Find a villager", "Name or internal name", &self.query, 40)
            {
                self.query = q;
                let st = nhse_core::data::strings("en");
                let q = self.query.to_lowercase();
                self.names = nhse_core::editing::villagers::available_templates()
                    .iter()
                    .filter(|n| n.contains(&q) || st.villager_name(n).to_lowercase().contains(&q))
                    .cloned()
                    .collect();
            }
            Nav::Stay
        } else if self.names.get(row - 1).is_some() {
            Nav::confirm_danger("Replace this villager?", "Their house and belongings will also be replaced. The existing backup can restore the original save.", row as u32)
        } else {
            Nav::Stay
        }
    }
    fn reply(&mut self, cx: &mut Cx, r: Reply) -> Nav {
        if let Reply::Confirmed(n) = r {
            if let Some(name) = n.checked_sub(1).and_then(|i| self.names.get(i as usize)) {
                let r = nhse_core::editing::villagers::replace_villager(
                    cx.save_mut(),
                    self.index,
                    name,
                    false,
                );
                let ok = r.is_ok();
                result(cx, r);
                if ok {
                    return Nav::Pop;
                }
            }
        }
        Nav::Stay
    }
}

pub struct Map {
    x: usize,
    y: usize,
    layer: usize,
}
impl Map {
    pub fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            layer: 0,
        }
    }
}
impl View for Map {
    fn preview(&self, cx: &Cx, g: &mut crate::gfx::Gfx) {
        use crate::gfx::{Color, Rect};
        let tiles = cx.save().terrain();
        let acres = cx.save().acres();
        let view = nhse_core::map::render::TerrainView {
            tiles: &tiles,
            acres: &acres,
        };
        let (w, h, rgba) = nhse_core::map::render::render_terrain(&view, 2);
        for y in 0..h {
            for x in 0..w {
                let p = &rgba[(y * w + x) * 4..];
                g.canvas.fill(
                    Rect::new(875 + x as i32, 240 + y as i32, 1, 1),
                    Color((p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32),
                );
            }
        }
        g.canvas.fill(
            Rect::new(
                875 + (self.x as i32 + 16) * 2,
                240 + (self.y as i32 + 16) * 2,
                3,
                3,
            ),
            Color(0xff3366),
        );
    }
    fn title(&self, _: &Cx) -> String {
        "Map editor".into()
    }
    fn rows(&self, cx: &Cx) -> Vec<Row> {
        let t = cx.save().terrain()[self.x * 96 + self.y];
        vec![Row::new("Terrain X").value(self.x.to_string()), Row::new("Terrain Y").value(self.y.to_string()), Row::new("Ground model").value(t.model.to_string()), Row::new("Ground variation").value(t.variation.to_string()), Row::new("Ground angle").value(t.angle.to_string()), Row::new("Elevation").value(t.elevation.to_string()), Row::new("Path model").value(t.road.to_string()), Row::new("Path variation").value(t.road_variation.to_string()), Row::new("Path angle").value(t.road_angle.to_string()), Row::new("Item layer").value(self.layer.to_string()), Row::new("Edit field items").help("Choose item coordinates on this layer. Placement checks prevent overlapping furniture.")]
    }
    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        if row == 10 {
            return Nav::Push(Box::new(FieldItems {
                x: 0,
                y: 0,
                layer: self.layer,
            }));
        }
        if row == 0 || row == 1 || row == 9 {
            let (v, max) = match row {
                0 => (&mut self.x, 111),
                1 => (&mut self.y, 95),
                _ => (&mut self.layer, 1),
            };
            if let Some(n) = cx.ask_number("Coordinate / layer", *v as u64, max) {
                *v = n as usize;
            }
        } else {
            let mut ts = cx.save().terrain();
            let t = &mut ts[self.x * 96 + self.y];
            let v = match row {
                2 => &mut t.model,
                3 => &mut t.variation,
                4 => &mut t.angle,
                5 => &mut t.elevation,
                6 => &mut t.road,
                7 => &mut t.road_variation,
                8 => &mut t.road_angle,
                _ => return Nav::Stay,
            };
            let max = match row {
                5 => 3,
                4 | 8 => 3,
                2 | 6 => 0x111,
                _ => u16::MAX as u64,
            };
            if let Some(n) = cx.ask_number("Terrain value", *v as u64, max) {
                *v = n as u16;
                cx.save_mut().set_terrain(&ts);
            }
        }
        Nav::Stay
    }
}
struct FieldItems {
    x: usize,
    y: usize,
    layer: usize,
}
impl View for FieldItems {
    fn title(&self, _: &Cx) -> String {
        "Map items".into()
    }
    fn rows(&self, cx: &Cx) -> Vec<Row> {
        let g = cx.save().field_layer(self.layer);
        vec![
            Row::new("X").value(self.x.to_string()),
            Row::new("Y").value(self.y.to_string()),
            Row::new("Find an item").value(crate::edits::item_label(&g.get(self.x, self.y))),
            Row::new("Remove item").help("Removes the item and its extension tiles."),
        ]
    }
    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        let mut g = cx.save().field_layer(self.layer);
        match row {
            0 | 1 => {
                let (v, max) = if row == 0 {
                    (&mut self.x, g.width - 1)
                } else {
                    (&mut self.y, g.height - 1)
                };
                if let Some(n) = cx.ask_number("Item coordinate", *v as u64, max as u64) {
                    *v = n as usize;
                }
            }
            2 => return Nav::Push(Box::new(super::items::ItemSearch::new(""))),
            3 => {
                g.remove_at(self.x, self.y);
                cx.save_mut().set_field_layer(self.layer, &g);
            }
            _ => {}
        }
        Nav::Stay
    }
    fn reply(&mut self, cx: &mut Cx, r: Reply) -> Nav {
        if let Reply::Item(it) = r {
            let mut g = cx.save().field_layer(self.layer);
            if matches!(
                g.place(it, self.x, self.y, true, false),
                nhse_core::map::grid::Placement::Free
            ) {
                cx.save_mut().set_field_layer(self.layer, &g);
            } else {
                cx.toast_error(
                    "The item does not fit here. Remove existing items or choose another position.",
                );
            }
        }
        Nav::Stay
    }
}

pub struct Designs;
const KINDS: [DesignKind; 4] = [
    DesignKind::Normal,
    DesignKind::Pro,
    DesignKind::Flag,
    DesignKind::Tailor,
];
impl View for Designs {
    fn title(&self, _: &Cx) -> String {
        "Custom designs".into()
    }
    fn rows(&self, _: &Cx) -> Vec<Row> {
        ["My Designs", "PRO designs", "Island flag", "Able Sisters"]
            .iter()
            .map(|s| Row::new(*s))
            .collect()
    }
    fn activate(&mut self, _: &mut Cx, row: usize) -> Nav {
        KINDS
            .get(row)
            .map(|k| Nav::Push(Box::new(DesignList { kind: *k })))
            .unwrap_or(Nav::Stay)
    }
}
struct DesignList {
    kind: DesignKind,
}
impl View for DesignList {
    fn title(&self, _: &Cx) -> String {
        "Choose a design".into()
    }
    fn rows(&self, cx: &Cx) -> Vec<Row> {
        (0..cx.save().design_count(self.kind))
            .map(|i| {
                let d = cx.save().design_raw(self.kind, i).unwrap();
                Row::new(format!(
                    "{}: {}",
                    i + 1,
                    design::info(d, i, self.kind == DesignKind::Pro).name
                ))
            })
            .collect()
    }
    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        if row < cx.save().design_count(self.kind) {
            Nav::Push(Box::new(DesignEdit {
                kind: self.kind,
                index: row,
                x: 0,
                y: 0,
                sheet: 0,
                color: 0,
            }))
        } else {
            Nav::Stay
        }
    }
}
struct DesignEdit {
    kind: DesignKind,
    index: usize,
    x: usize,
    y: usize,
    sheet: usize,
    color: usize,
}
impl View for DesignEdit {
    fn preview(&self, cx: &Cx, g: &mut crate::gfx::Gfx) {
        use crate::gfx::{Color, Rect};
        let d = cx.save().design_raw(self.kind, self.index).unwrap();
        let p = design::palette(d);
        for y in 0..32 {
            for x in 0..32 {
                let v = design::pixel(d, self.sheet, x, y) as usize;
                let c = if v < 15 {
                    let c = p[v];
                    Color((c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32)
                } else {
                    Color(if (x + y) % 2 == 0 { 0xcccccc } else { 0xffffff })
                };
                g.canvas.fill(
                    Rect::new(865 + x as i32 * 10, 240 + y as i32 * 10, 10, 10),
                    c,
                );
            }
        }
        let x = 865 + self.x as i32 * 10;
        let y = 240 + self.y as i32 * 10;
        g.canvas.fill(Rect::new(x, y, 10, 2), Color(0xff3366));
        g.canvas.fill(Rect::new(x, y, 2, 10), Color(0xff3366));
    }

    fn title(&self, _: &Cx) -> String {
        "Design pixel editor".into()
    }
    fn rows(&self, cx: &Cx) -> Vec<Row> {
        let d = cx.save().design_raw(self.kind, self.index).unwrap();
        let p = design::palette(d);
        vec![
            Row::new("Name").value(design::info(d, self.index, self.kind == DesignKind::Pro).name),
            Row::new("Sheet").value(self.sheet.to_string()),
            Row::new("Pixel X").value(self.x.to_string()),
            Row::new("Pixel Y").value(self.y.to_string()),
            Row::new("Brush color (15 = transparent)").value(self.color.to_string()),
            Row::new("Paint pixel").value(design::pixel(d, self.sheet, self.x, self.y).to_string()),
            Row::new("Palette color RGB").value(if self.color < 15 {
                format!(
                    "{:02X}{:02X}{:02X}",
                    p[self.color][0], p[self.color][1], p[self.color][2]
                )
            } else {
                "Transparent".into()
            }),
            Row::new("Import .nhd / .nhpd").help("Enter the full path on the SD card."),
            Row::new("Export .nhd / .nhpd")
                .help("Enter a new file path on the SD card; existing files are preserved."),
            Row::new("Import PNG (32 x 32)"),
            Row::new("Export PNG (current sheet)"),
        ]
    }
    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        let mut d = cx
            .save()
            .design_raw(self.kind, self.index)
            .unwrap()
            .to_vec();
        if row == 9 || row == 10 {
            let Some(path) = cx.ask_text("PNG file", "Full SD card path", "", 255) else {
                return Nav::Stay;
            };
            if row == 9 {
                let bytes = match nhse_core::save::provider::read_file(std::path::Path::new(&path))
                {
                    Ok(b) => b,
                    Err(e) => {
                        cx.toast_error(e.to_string());
                        return Nav::Stay;
                    }
                };
                if bytes.len() > 1024 * 1024
                    || bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n")
                    || bytes.get(16..24) != Some(&[0, 0, 0, 32, 0, 0, 0, 32])
                {
                    cx.toast_error("Choose a 32 x 32 PNG smaller than 1 MB.");
                    return Nav::Stay;
                }
                if let Some((_, _, rgba)) = nhse_core::map::render::decode_png(&bytes) {
                    let count = design::sheets(self.kind == DesignKind::Pro);
                    let mut sheets: Vec<_> =
                        (0..count).map(|i| design::render_rgba(&d, i)).collect();
                    sheets[self.sheet] = rgba;
                    design::import_rgba(&mut d, &sheets);
                } else {
                    cx.toast_error("Could not decode PNG.");
                    return Nav::Stay;
                }
            } else {
                use std::io::Write;
                let bytes = nhse_core::map::render::encode_png(
                    32,
                    32,
                    &design::render_rgba(&d, self.sheet),
                );
                let r = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .and_then(|mut f| f.write_all(&bytes));
                result(cx, r);
                return Nav::Stay;
            }
            let r = cx.save_mut().set_design_raw(self.kind, self.index, &d);
            result(cx, r);
            return Nav::Stay;
        }
        match row {
            0 => {
                if let Some(n) = cx.ask_text(
                    "Design name",
                    "Up to 20 characters",
                    &design::info(&d, self.index, self.kind == DesignKind::Pro).name,
                    20,
                ) {
                    design::set_name(&mut d, &n);
                } else {
                    return Nav::Stay;
                }
            }
            1..=4 => {
                let (v, max) = match row {
                    1 => (
                        &mut self.sheet,
                        if self.kind == DesignKind::Pro { 3 } else { 0 },
                    ),
                    2 => (&mut self.x, 31),
                    3 => (&mut self.y, 31),
                    _ => (&mut self.color, 15),
                };
                if let Some(n) = cx.ask_number("Pixel / brush", *v as u64, max) {
                    *v = n as usize;
                }
                return Nav::Stay;
            }
            5 => design::set_pixel(&mut d, self.sheet, self.x, self.y, self.color as u8),
            6 if self.color < 15 => {
                let mut p = design::palette(&d);
                if let Some(s) = cx.ask_text(
                    "RGB color",
                    "Six hexadecimal digits, for example FF8800",
                    "",
                    6,
                ) {
                    if s.len() != 6 || !s.is_ascii() {
                        cx.toast_error("Enter six hexadecimal digits.");
                        return Nav::Stay;
                    }
                    match u32::from_str_radix(&s, 16) {
                        Ok(n) => {
                            p[self.color] = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
                            design::set_palette(&mut d, &p);
                        }
                        Err(_) => {
                            cx.toast_error("Invalid RGB color.");
                            return Nav::Stay;
                        }
                    }
                } else {
                    return Nav::Stay;
                }
            }
            7 | 8 => {
                if let Some(path) = cx.ask_text("Design file", "Full SD card path", "", 255) {
                    if row == 7 {
                        match nhse_core::save::provider::read_file(std::path::Path::new(&path)) {
                            Ok(bytes) if bytes.len() == d.len() => d = bytes,
                            Ok(_) => {
                                cx.toast_error("The design file has the wrong size for this slot.");
                                return Nav::Stay;
                            }
                            Err(e) => {
                                cx.toast_error(e.to_string());
                                return Nav::Stay;
                            }
                        }
                    } else {
                        use std::io::Write;
                        let r = std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(path)
                            .and_then(|mut f| f.write_all(&d));
                        result(cx, r);
                        return Nav::Stay;
                    }
                } else {
                    return Nav::Stay;
                }
            }
            _ => return Nav::Stay,
        }
        let r = cx.save_mut().set_design_raw(self.kind, self.index, &d);
        result(cx, r);
        Nav::Stay
    }
}

pub struct Houses;
impl View for Houses {
    fn title(&self, _: &Cx) -> String {
        "Houses".into()
    }
    fn rows(&self, cx: &Cx) -> Vec<Row> {
        cx.save()
            .player_houses()
            .iter()
            .map(|h| Row::new(format!("Player house {}", h.index + 1)))
            .chain(
                cx.save()
                    .villager_houses()
                    .iter()
                    .map(|h| Row::new(format!("Villager house {}", h.index + 1))),
            )
            .collect()
    }
    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        let n = cx.save().player_houses().len();
        if row < n + cx.save().villager_houses().len() {
            Nav::Push(Box::new(House {
                index: if row < n { row } else { row - n },
                player: row < n,
            }))
        } else {
            Nav::Stay
        }
    }
}
struct House {
    index: usize,
    player: bool,
}
impl View for House {
    fn title(&self, _: &Cx) -> String {
        "House editor".into()
    }
    fn rows(&self, cx: &Cx) -> Vec<Row> {
        let e = if self.player {
            cx.save().player_houses()[self.index].exterior.clone()
        } else {
            cx.save().villager_houses()[self.index].exterior.clone()
        };
        let mut rows = vec![
            Row::new("Wall").value(e.wall.to_string()),
            Row::new("Roof").value(e.roof.to_string()),
            Row::new("Door").value(e.door.to_string()),
        ];
        if self.player {
            rows.push(
                Row::new("Loan").value(cx.save().player_houses()[self.index].loan.to_string()),
            );
            rows.extend(
                [
                    "Main room",
                    "Back room",
                    "Left room",
                    "Right room",
                    "Upstairs",
                    "Basement",
                ]
                .iter()
                .map(|r| Row::new(*r)),
            );
        }
        if !self.player && cx.save().villager_houses()[self.index].room.is_some() {
            rows.push(Row::new("Wallpaper and flooring"));
        }
        rows
    }
    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        if !self.player && row == 3 {
            return Nav::Push(Box::new(Surfaces {
                house: self.index,
                room: None,
                target: 0,
            }));
        }
        if self.player && row >= 4 && row < 10 {
            return Nav::Push(Box::new(Room {
                house: self.index,
                room: row - 4,
                x: 0,
                y: 0,
                layer: 0,
            }));
        }
        if self.player {
            let mut h = cx.save().player_houses()[self.index].clone();
            if row == 3 {
                if let Some(n) = cx.ask_number("House loan", h.loan as u64, u32::MAX as u64) {
                    h.loan = n as u32;
                } else {
                    return Nav::Stay;
                }
            } else {
                let v = match row {
                    0 => &mut h.exterior.wall,
                    1 => &mut h.exterior.roof,
                    2 => &mut h.exterior.door,
                    _ => return Nav::Stay,
                };
                if let Some(n) = cx.ask_number("Exterior style ID", *v as u64, u16::MAX as u64) {
                    *v = n as u16;
                } else {
                    return Nav::Stay;
                }
            }
            cx.save_mut().set_player_house(&h);
        } else {
            let mut h = cx.save().villager_houses()[self.index].clone();
            let v = match row {
                0 => &mut h.exterior.wall,
                1 => &mut h.exterior.roof,
                2 => &mut h.exterior.door,
                _ => return Nav::Stay,
            };
            if let Some(n) = cx.ask_number("Exterior style ID", *v as u64, u16::MAX as u64) {
                *v = n as u16;
                cx.save_mut().set_villager_house(&h);
            }
        }
        Nav::Stay
    }
}
struct Room {
    house: usize,
    room: usize,
    x: usize,
    y: usize,
    layer: usize,
}
impl View for Room {
    fn title(&self, _: &Cx) -> String {
        "Room furniture".into()
    }
    fn rows(&self, cx: &Cx) -> Vec<Row> {
        match cx.save().player_room(self.house, self.room) {
            Ok(r) => vec![
                Row::new("X").value(self.x.to_string()),
                Row::new("Y").value(self.y.to_string()),
                Row::new("Layer").value(self.layer.to_string()),
                Row::new("Find furniture").value(crate::edits::item_label(
                    &r.layers[self.layer][self.x * 20 + self.y],
                )),
                Row::new("Remove furniture"),
                Row::new("Wallpaper and flooring"),
            ],
            Err(e) => vec![Row::new(e.to_string())],
        }
    }
    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav {
        match row {
            0..=2 => {
                let (v, max) = match row {
                    0 => (&mut self.x, 19),
                    1 => (&mut self.y, 19),
                    _ => (&mut self.layer, 7),
                };
                if let Some(n) = cx.ask_number("Room coordinate / layer", *v as u64, max) {
                    *v = n as usize;
                }
            }
            3 => return Nav::Push(Box::new(super::items::ItemSearch::new(""))),
            4 => return self.reply(cx, Reply::Item(nhse_core::Item::NONE)),
            5 => {
                return Nav::Push(Box::new(Surfaces {
                    house: self.house,
                    room: Some(self.room),
                    target: 0,
                }))
            }
            _ => {}
        }
        Nav::Stay
    }
    fn reply(&mut self, cx: &mut Cx, r: Reply) -> Nav {
        if let Reply::Item(it) = r {
            match cx.save().player_room(self.house, self.room) {
                Ok(mut room) => {
                    room.layers[self.layer][self.x * 20 + self.y] = it;
                    let r = cx.save_mut().set_player_room(self.house, self.room, &room);
                    if r.is_ok() {
                        let mut raw = cx.save().player_room_raw(self.house, self.room).to_vec();
                        nhse_core::structs::house::PlayerRoom::set_active(
                            &mut raw,
                            self.layer,
                            self.x,
                            self.y,
                            !it.is_none(),
                        );
                        let r = cx
                            .save_mut()
                            .set_player_room_raw(self.house, self.room, &raw);
                        result(cx, r);
                    } else {
                        result(cx, r);
                    }
                }
                Err(e) => cx.toast_error(e.to_string()),
            }
        }
        Nav::Stay
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        host::{HostPlatform, Step},
        platform::btn,
    };
    #[test]
    fn edits_roundtrip_through_switch_views() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let rev = nhse_core::save::REVISIONS.len() - 1;
        nhse_core::testing::write_synthetic_save(
            &HostPlatform::game_dir(root, "Player"),
            rev,
            1,
            3,
        );
        let p = HostPlatform::new(
            root,
            &["Player"],
            vec![Step::Press(btn::A), Step::Press(btn::A)],
            &root.join("shots"),
        );
        let mut app = crate::app(Box::new(p));
        app.run();
        let cx = &mut app.cx;
        let mut p = HostPlatform::new(root, &[], vec![], &root.join("shots"));
        p.keyboard
            .extend(["2", "Island art", "010203", "1234", "5"].map(|s| Some(s.into())));
        cx.platform = Box::new(p);
        Map {
            x: 2,
            y: 3,
            layer: 0,
        }
        .activate(cx, 5);
        assert_eq!(cx.save().terrain()[2 * 96 + 3].elevation, 2);
        let mut d = DesignEdit {
            kind: DesignKind::Normal,
            index: 0,
            x: 2,
            y: 3,
            sheet: 0,
            color: 4,
        };
        d.activate(cx, 0);
        d.activate(cx, 6);
        d.activate(cx, 5);
        let raw = cx.save().design_raw(DesignKind::Normal, 0).unwrap();
        assert_eq!(design::info(raw, 0, false).name, "Island art");
        assert_eq!(design::pixel(raw, 0, 2, 3), 4);
        assert_eq!(design::palette(raw)[4], [1, 2, 3]);
        House {
            index: 0,
            player: true,
        }
        .activate(cx, 3);
        assert_eq!(cx.save().player_houses()[0].loan, 1234);
        House {
            index: 0,
            player: false,
        }
        .activate(cx, 1);
        assert_eq!(cx.save().villager_houses()[0].exterior.roof, 5);
        let mut room = Room {
            house: 0,
            room: 0,
            x: 2,
            y: 3,
            layer: 0,
        };
        room.reply(cx, Reply::Item(nhse_core::Item::new(1)));
        assert_eq!(cx.save().player_room(0, 0).unwrap().layers[0][43].id, 1);
        assert!(nhse_core::structs::house::PlayerRoom::is_active(
            cx.save().player_room_raw(0, 0),
            0,
            2,
            3
        ));
        room.activate(cx, 4);
        assert!(!nhse_core::structs::house::PlayerRoom::is_active(
            cx.save().player_room_raw(0, 0),
            0,
            2,
            3
        ));
        let mut replacement = Replacement::new(0);
        replacement.names = vec!["cat00".into()];
        replacement.reply(cx, Reply::Confirmed(1));
        assert_eq!(cx.save().villagers()[0].internal_name, "cat00");
        assert!(cx.dirty());
        let r = crate::session::save(cx.platform.as_mut(), cx.session.as_mut().unwrap());
        assert!(r.is_ok(), "{r:?}");
        let saved = nhse_core::HorizonSave::open(&HostPlatform::game_dir(root, "Player")).unwrap();
        assert_eq!(saved.terrain()[2 * 96 + 3].elevation, 2);
        assert_eq!(saved.player_houses()[0].loan, 1234);
        assert_eq!(saved.villagers()[0].internal_name, "cat00");
        assert_eq!(
            design::pixel(saved.design_raw(DesignKind::Normal, 0).unwrap(), 0, 2, 3),
            4
        );
        assert!(saved.invalid_hashes().is_empty());
    }
}

struct Surfaces {
    house: usize,
    room: Option<usize>,
    target: usize,
}
impl Surfaces {
    fn get(&self, cx: &Cx) -> Option<nhse_core::structs::misc::RoomFloorWall> {
        match self.room {
            Some(r) => cx
                .save()
                .player_room(self.house, r)
                .ok()
                .map(|r| r.surfaces),
            None => cx
                .save()
                .villager_houses()
                .get(self.house)
                .and_then(|h| h.room),
        }
    }
}
impl View for Surfaces {
    fn title(&self, _: &Cx) -> String {
        "Wallpaper and flooring".into()
    }
    fn rows(&self, cx: &Cx) -> Vec<Row> {
        match self.get(cx) {
            Some(s) => [s.wall, s.floor, s.accent_wall]
                .into_iter()
                .zip(["Wallpaper", "Flooring", "Accent wallpaper"])
                .map(|(i, n)| Row::new(n).value(crate::edits::item_label(&i)))
                .collect(),
            None => vec![],
        }
    }
    fn activate(&mut self, _: &mut Cx, row: usize) -> Nav {
        if row < 3 {
            self.target = row;
            Nav::Push(Box::new(super::items::ItemSearch::new(if row == 1 {
                "flooring"
            } else {
                "wall"
            })))
        } else {
            Nav::Stay
        }
    }
    fn reply(&mut self, cx: &mut Cx, r: Reply) -> Nav {
        if let Reply::Item(it) = r {
            if let Some(mut s) = self.get(cx) {
                match self.target {
                    0 => s.wall = it,
                    1 => s.floor = it,
                    _ => s.accent_wall = it,
                }
                if let Some(r) = self.room {
                    match cx.save().player_room(self.house, r) {
                        Ok(mut room) => {
                            room.surfaces = s;
                            let r = cx.save_mut().set_player_room(self.house, r, &room);
                            result(cx, r);
                        }
                        Err(e) => cx.toast_error(e.to_string()),
                    }
                } else {
                    let mut h = cx.save().villager_houses()[self.house].clone();
                    h.room = Some(s);
                    cx.save_mut().set_villager_house(&h);
                }
            }
        }
        Nav::Stay
    }
}
