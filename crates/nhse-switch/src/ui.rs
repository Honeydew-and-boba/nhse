//! The app shell: a stack of list screens, with a header, an info panel, button hints, dialogs
//! and a "working…" overlay for long operations.
use crate::gfx::{Color, Gfx, Rect, Weight, HEIGHT, WIDTH};
use crate::platform::{btn, Input, KeyboardMode, KeyboardRequest, Platform, UserAccount};
use crate::session::{self, Session};
use nhse_core::{HorizonSave, Item};

pub mod theme {
    use crate::gfx::Color;
    pub const BG: Color = Color(0xF6F1DE);
    pub const HEADER: Color = Color(0x2FB49F);
    pub const HEADER_DARK: Color = Color(0x239684);
    pub const CARD: Color = Color(0xFFFCF3);
    pub const SHADOW: Color = Color(0xD9CFB0);
    pub const TEXT: Color = Color(0x5A4636);
    pub const MUTED: Color = Color(0x9C8B75);
    pub const SELECT: Color = Color(0xFFE27A);
    pub const SELECT_EDGE: Color = Color(0xF2B83B);
    pub const ACCENT: Color = Color(0x1E9C89);
    pub const DANGER: Color = Color(0xD9534F);
    pub const OK: Color = Color(0x3FA66B);
    pub const FOOTER: Color = Color(0xECE4C9);
    pub const WHITE: Color = Color(0xFFFFFF);
    pub const BUTTON: Color = Color(0x5A4636);
}
use theme::*;

/// How a row looks and whether it can be selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Normal,
    /// Runs something (shown in the accent color).
    Action,
    /// Changes or deletes data in bulk.
    Danger,
    /// A heading; not selectable.
    Section,
    /// Shown but inactive.
    Muted,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub help: String,
    pub style: Style,
}

impl Row {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: String::new(),
            help: String::new(),
            style: Style::Normal,
        }
    }
    pub fn section(label: impl Into<String>) -> Self {
        Self::new(label).style(Style::Section)
    }
    pub fn value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }
    pub fn help(mut self, h: impl Into<String>) -> Self {
        self.help = h.into();
        self
    }
    pub fn style(mut self, s: Style) -> Self {
        self.style = s;
        self
    }
    fn selectable(&self) -> bool {
        self.style != Style::Section
    }
}

/// Answers delivered back to the screen below.
pub enum Reply {
    Confirmed(u32),
    Item(Item),
    User(UserAccount),
}

/// What to do after handling input.
pub enum Nav {
    Stay,
    Push(Box<dyn View>),
    Pop,
    /// Pops this screen and hands a reply to the screen below.
    PopReply(Reply),
    Replace(Box<dyn View>),
    /// Back to the home screen.
    Root,
    /// Back to the home screen, then opens a screen.
    Reset(Box<dyn View>),
    /// Asks the user; on A the current screen gets `Reply::Confirmed(tag)`.
    Confirm {
        title: String,
        text: String,
        tag: u32,
        danger: bool,
    },
    Message {
        title: String,
        text: String,
    },
    /// Shows `text` over the screen while `job` runs.
    Busy {
        text: String,
        job: Box<dyn FnOnce(&mut Cx) -> Nav>,
    },
    Quit,
}

impl Nav {
    pub fn message(title: impl Into<String>, text: impl Into<String>) -> Nav {
        Nav::Message {
            title: title.into(),
            text: text.into(),
        }
    }
    pub fn confirm(title: impl Into<String>, text: impl Into<String>, tag: u32) -> Nav {
        Nav::Confirm {
            title: title.into(),
            text: text.into(),
            tag,
            danger: false,
        }
    }
    pub fn confirm_danger(title: impl Into<String>, text: impl Into<String>, tag: u32) -> Nav {
        Nav::Confirm {
            title: title.into(),
            text: text.into(),
            tag,
            danger: true,
        }
    }
    pub fn busy(text: impl Into<String>, job: impl FnOnce(&mut Cx) -> Nav + 'static) -> Nav {
        Nav::Busy {
            text: text.into(),
            job: Box::new(job),
        }
    }
}

/// The side panel of the selected row.
#[derive(Default)]
pub struct Info {
    pub title: String,
    pub text: String,
    pub facts: Vec<(String, String)>,
}

/// A screen: a list of rows.
pub trait View {
    fn title(&self, cx: &Cx) -> String;
    fn rows(&self, cx: &Cx) -> Vec<Row>;
    /// A on a row.
    fn activate(&mut self, cx: &mut Cx, row: usize) -> Nav;
    /// Other buttons (X, Y, L/R, left/right, +, −). `None` lets the shell handle the button.
    fn button(&mut self, _cx: &mut Cx, _b: u32, _row: usize) -> Option<Nav> {
        None
    }
    fn reply(&mut self, _cx: &mut Cx, _r: Reply) -> Nav {
        Nav::Stay
    }
    /// B.
    fn back(&mut self, _cx: &mut Cx) -> Nav {
        Nav::Pop
    }
    /// Extra button hints for the footer, e.g. `[("Y", "Copy")]`.
    fn hints(&self, _cx: &Cx, _row: usize) -> Vec<(&'static str, &'static str)> {
        Vec::new()
    }
    /// The label of A for a row.
    fn a_label(&self, _cx: &Cx, _row: usize) -> &'static str {
        "Select"
    }
    /// The side panel. The default shows the row's label and help.
    fn info(&self, _cx: &Cx, rows: &[Row], row: usize) -> Info {
        rows.get(row)
            .map(|r| Info {
                title: r.label.clone(),
                text: r.help.clone(),
                facts: Vec::new(),
            })
            .unwrap_or_default()
    }
    fn empty_text(&self) -> &str {
        "Nothing here."
    }
}

/// State shared by every screen.
pub struct Cx {
    pub platform: Box<dyn Platform>,
    pub session: Option<Session>,
    /// An item copied in an item list.
    pub clipboard: Option<Item>,
    toast: Option<(String, bool, u32)>,
}

impl Cx {
    pub fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), false, 180));
    }
    pub fn toast_error(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), true, 300));
    }

    /// The open save. Screens that need one only exist while a save is open.
    pub fn save(&self) -> &HorizonSave {
        &self.session.as_ref().expect("a save is open").save
    }

    /// The open save, marked as changed.
    pub fn save_mut(&mut self) -> &mut HorizonSave {
        let s = self.session.as_mut().expect("a save is open");
        s.dirty = true;
        &mut s.save
    }

    pub fn dirty(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.dirty)
    }

    pub fn ask_text(
        &mut self,
        header: &str,
        guide: &str,
        initial: &str,
        max_len: usize,
    ) -> Option<String> {
        self.platform.keyboard(&KeyboardRequest {
            header,
            guide,
            initial,
            mode: KeyboardMode::Text,
            max_len,
        })
    }

    /// Asks for a whole number up to `max`. `None` when cancelled or not a number.
    pub fn ask_number(&mut self, header: &str, current: u64, max: u64) -> Option<u64> {
        let guide = format!("0 to {}", crate::fmt::thousands(max));
        let digits = max.to_string().len();
        let t = self.platform.keyboard(&KeyboardRequest {
            header,
            guide: &guide,
            initial: &current.to_string(),
            mode: KeyboardMode::Number,
            max_len: digits,
        })?;
        let t: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
        match t.parse::<u64>() {
            Ok(v) => Some(v.min(max)),
            Err(_) => {
                self.toast_error("That is not a number.");
                None
            }
        }
    }
}

/// Saves the open save, showing the result.
pub fn save_nav() -> Nav {
    Nav::busy("Saving…", |cx| {
        let Cx {
            platform, session, ..
        } = cx;
        let Some(s) = session.as_mut() else {
            return Nav::Stay;
        };
        match session::save(platform.as_mut(), s) {
            Ok(msg) => {
                cx.toast(msg);
                Nav::Stay
            }
            Err(e) => Nav::message("The save failed", e),
        }
    })
}

struct Entry {
    view: Box<dyn View>,
    sel: usize,
    scroll: usize,
    rows: Vec<Row>,
    stale: bool,
}

impl Entry {
    fn new(view: Box<dyn View>) -> Self {
        Self {
            view,
            sel: 0,
            scroll: 0,
            rows: Vec::new(),
            stale: true,
        }
    }
}

enum Modal {
    Confirm {
        title: String,
        text: String,
        tag: u32,
        danger: bool,
    },
    Message {
        title: String,
        text: String,
    },
}

// layout
const HEADER_H: i32 = 76;
const FOOTER_H: i32 = 58;
const LIST: Rect = Rect::new(
    28,
    HEADER_H + 22,
    780,
    HEIGHT as i32 - HEADER_H - FOOTER_H - 44,
);
const INFO: Rect = Rect::new(832, HEADER_H + 22, WIDTH as i32 - 832 - 28, LIST.h);
const ROW_H: i32 = 52;
const LIST_PAD: i32 = 14;

fn visible_rows() -> usize {
    ((LIST.h - 2 * LIST_PAD) / ROW_H) as usize
}

pub struct App {
    pub cx: Cx,
    stack: Vec<Entry>,
    modal: Option<Modal>,
    gfx: Gfx,
    /// Frames each direction has been held, for key repeat.
    held_frames: [u32; 4],
    redraw: bool,
    quit: bool,
}

impl App {
    pub fn new(platform: Box<dyn Platform>, root: Box<dyn View>) -> Self {
        Self {
            cx: Cx {
                platform,
                session: None,
                clipboard: None,
                toast: None,
            },
            stack: vec![Entry::new(root)],
            modal: None,
            gfx: Gfx::new(),
            held_frames: [0; 4],
            redraw: true,
            quit: false,
        }
    }

    /// Runs until the platform or the user closes the app.
    pub fn run(&mut self) {
        while !self.quit {
            let Some(input) = self.cx.platform.next_frame() else {
                break;
            };
            self.frame(input);
        }
    }

    /// Directions pressed this frame, with key repeat while held.
    fn repeat(&mut self, input: &Input) -> u32 {
        let mut out = input.down & !btn::DIRECTIONS;
        for (i, b) in [btn::UP, btn::DOWN, btn::LEFT, btn::RIGHT]
            .into_iter()
            .enumerate()
        {
            if input.down & b != 0 {
                // a new press always counts and restarts the repeat delay
                out |= b;
                self.held_frames[i] = 1;
            } else if input.held & b != 0 {
                let f = self.held_frames[i];
                if f >= 18 && (f - 18).is_multiple_of(if f > 60 { 2 } else { 4 }) {
                    out |= b;
                }
                self.held_frames[i] += 1;
            } else {
                self.held_frames[i] = 0;
            }
        }
        out
    }

    /// Rebuilds the rows of the current screen after a change.
    fn refresh(&mut self) {
        if let Some(e) = self.stack.last_mut() {
            if e.stale {
                e.rows = e.view.rows(&self.cx);
                e.stale = false;
                fix_selection(e);
                self.redraw = true;
            }
        }
    }

    pub fn frame(&mut self, input: Input) {
        let pressed = self.repeat(&input);
        if pressed != 0 {
            // a screen opened last frame needs its rows before it can take input
            self.refresh();
            self.redraw = true;
            self.handle(pressed);
        }
        self.refresh();
        if let Some(t) = self.cx.toast.as_mut() {
            t.2 = t.2.saturating_sub(1);
            if t.2 == 0 {
                self.cx.toast = None;
                self.redraw = true;
            }
        }
        if self.redraw {
            self.draw();
            self.redraw = false;
        }
        self.cx.platform.present(&self.gfx.canvas);
    }

    fn handle(&mut self, pressed: u32) {
        if let Some(m) = self.modal.take() {
            match m {
                Modal::Confirm { tag, .. } if pressed & btn::A != 0 => {
                    let nav = self
                        .stack
                        .last_mut()
                        .expect("stack")
                        .view
                        .reply(&mut self.cx, Reply::Confirmed(tag));
                    self.apply(nav);
                }
                Modal::Message { .. } if pressed & (btn::A | btn::B) != 0 => {}
                _ if pressed & btn::B != 0 => {}
                m => self.modal = Some(m),
            }
            self.top().stale = true;
            return;
        }
        let vis = visible_rows();
        let e = self.top();
        let (n, sel) = (e.rows.len(), e.sel);
        let mut nav = Nav::Stay;
        for b in [
            btn::A,
            btn::B,
            btn::X,
            btn::Y,
            btn::PLUS,
            btn::MINUS,
            btn::L,
            btn::R,
            btn::ZL,
            btn::ZR,
            btn::UP,
            btn::DOWN,
            btn::LEFT,
            btn::RIGHT,
        ] {
            if pressed & b == 0 {
                continue;
            }
            let e = self.stack.last_mut().expect("stack");
            let selectable = e.rows.get(e.sel).is_some_and(|r| r.selectable());
            nav = match b {
                btn::A if selectable => e.view.activate(&mut self.cx, sel),
                btn::A => Nav::Stay,
                btn::B => e.view.back(&mut self.cx),
                btn::UP | btn::DOWN => {
                    let fresh = self.held_frames[if b == btn::UP { 0 } else { 1 }] <= 1;
                    move_sel(self.top(), if b == btn::UP { -1 } else { 1 }, fresh);
                    Nav::Stay
                }
                _ => match e.view.button(&mut self.cx, b, sel) {
                    Some(nav) => nav,
                    None => match b {
                        btn::LEFT | btn::L => {
                            move_sel(self.top(), -(vis as i32), false);
                            Nav::Stay
                        }
                        btn::RIGHT | btn::R => {
                            move_sel(self.top(), vis as i32, false);
                            Nav::Stay
                        }
                        btn::ZL => {
                            move_sel(self.top(), -(n as i32), false);
                            Nav::Stay
                        }
                        btn::ZR => {
                            move_sel(self.top(), n as i32, false);
                            Nav::Stay
                        }
                        btn::PLUS if self.cx.session.is_some() => save_nav(),
                        _ => Nav::Stay,
                    },
                },
            };
            break;
        }
        self.top().stale = true;
        self.apply(nav);
    }

    fn top(&mut self) -> &mut Entry {
        self.stack
            .last_mut()
            .expect("the home screen is never popped")
    }

    fn apply(&mut self, nav: Nav) {
        match nav {
            Nav::Stay => {}
            Nav::Push(v) => self.stack.push(Entry::new(v)),
            Nav::Pop => {
                if self.stack.len() > 1 {
                    self.stack.pop();
                }
            }
            Nav::PopReply(r) => {
                if self.stack.len() > 1 {
                    self.stack.pop();
                }
                let nav = self
                    .stack
                    .last_mut()
                    .expect("stack")
                    .view
                    .reply(&mut self.cx, r);
                self.apply(nav);
            }
            Nav::Replace(v) => {
                if self.stack.len() > 1 {
                    self.stack.pop();
                }
                self.stack.push(Entry::new(v));
            }
            Nav::Root => self.stack.truncate(1),
            Nav::Reset(v) => {
                self.stack.truncate(1);
                self.stack.push(Entry::new(v));
            }
            Nav::Confirm {
                title,
                text,
                tag,
                danger,
            } => {
                self.modal = Some(Modal::Confirm {
                    title,
                    text,
                    tag,
                    danger,
                })
            }
            Nav::Message { title, text } => self.modal = Some(Modal::Message { title, text }),
            Nav::Busy { text, job } => {
                self.draw();
                self.draw_busy(&text);
                self.cx.platform.present(&self.gfx.canvas);
                let nav = job(&mut self.cx);
                self.apply(nav);
            }
            Nav::Quit => self.quit = true,
        }
        if let Some(e) = self.stack.last_mut() {
            e.stale = true;
        }
        self.redraw = true;
    }

    // --- drawing -------------------------------------------------------------

    fn draw(&mut self) {
        let Self { gfx, stack, cx, .. } = self;
        let e = stack.last().expect("stack");
        let g = gfx;
        g.canvas.set_clip(None);
        g.canvas.clear(BG);
        draw_header(g, cx, &e.view.title(cx));
        draw_list(g, e, e.view.empty_text());
        let info = e.view.info(cx, &e.rows, e.sel);
        draw_info(g, &info);
        let mut hints: Vec<(&str, &str)> = Vec::new();
        if e.rows
            .get(e.sel)
            .is_some_and(|r| r.selectable() && r.style != Style::Muted)
        {
            hints.push(("A", e.view.a_label(cx, e.sel)));
        }
        hints.extend(e.view.hints(cx, e.sel));
        hints.push(("B", if stack.len() > 1 { "Back" } else { "Exit" }));
        if cx.session.is_some() && !hints.iter().any(|(b, _)| *b == "+") {
            hints.push(("+", "Save"));
        }
        draw_footer(g, cx, &hints);
        match &self.modal {
            Some(Modal::Confirm {
                title,
                text,
                danger,
                ..
            }) => draw_dialog(
                g,
                title,
                text,
                &[
                    ("A", if *danger { "Yes, do it" } else { "OK" }),
                    ("B", "Cancel"),
                ],
                *danger,
            ),
            Some(Modal::Message { title, text }) => {
                draw_dialog(g, title, text, &[("A", "OK")], false)
            }
            None => {}
        }
    }

    fn draw_busy(&mut self, text: &str) {
        let g = &mut self.gfx;
        g.canvas.fill_alpha(
            Rect::new(0, 0, WIDTH as i32, HEIGHT as i32),
            Color(0x2A2018),
            120,
        );
        let r = Rect::new(WIDTH as i32 / 2 - 240, HEIGHT as i32 / 2 - 60, 480, 120);
        g.canvas.round_rect(r, 24, CARD);
        // three dots
        for i in 0..3 {
            g.canvas.circle(
                r.x + 56 + i * 22,
                r.y + 60,
                7,
                if i == 1 { SELECT_EDGE } else { HEADER },
            );
        }
        g.text(r.x + 140, r.y + 44, text, 26, Weight::Bold, TEXT);
    }
}

fn fix_selection(e: &mut Entry) {
    let n = e.rows.len();
    if n == 0 {
        e.sel = 0;
        e.scroll = 0;
        return;
    }
    e.sel = e.sel.min(n - 1);
    if !e.rows[e.sel].selectable() {
        // nearest selectable row, downwards first
        if let Some(i) = (e.sel..n)
            .chain((0..e.sel).rev())
            .find(|i| e.rows[*i].selectable())
        {
            e.sel = i;
        }
    }
    ensure_visible(e);
}

fn ensure_visible(e: &mut Entry) {
    let vis = visible_rows();
    if e.sel < e.scroll {
        e.scroll = e.sel;
        // show a heading right above the selection
        if e.scroll > 0 && e.rows[e.scroll - 1].style == Style::Section {
            e.scroll -= 1;
        }
    } else if e.sel >= e.scroll + vis {
        e.scroll = e.sel + 1 - vis;
    }
    e.scroll = e.scroll.min(e.rows.len().saturating_sub(vis));
}

fn move_sel(e: &mut Entry, delta: i32, wrap: bool) {
    let n = e.rows.len() as i32;
    if n == 0 {
        return;
    }
    let mut i = e.sel as i32;
    let step = delta.signum();
    let mut left = delta.abs();
    let mut target = i;
    while left > 0 {
        let next = i + step;
        if next < 0 || next >= n {
            if wrap && delta.abs() == 1 {
                i = if next < 0 { n } else { -1 };
                continue;
            }
            break;
        }
        i = next;
        if e.rows[i as usize].selectable() {
            target = i;
            left -= 1;
        }
    }
    e.sel = target as usize;
    ensure_visible(e);
}

fn card(g: &mut Gfx, r: Rect) {
    g.canvas
        .round_rect(Rect::new(r.x, r.y + 4, r.w, r.h), 22, SHADOW);
    g.canvas.round_rect(r, 22, CARD);
}

fn draw_header(g: &mut Gfx, cx: &Cx, title: &str) {
    g.canvas
        .fill(Rect::new(0, 0, WIDTH as i32, HEADER_H), HEADER);
    g.canvas
        .fill(Rect::new(0, HEADER_H - 4, WIDTH as i32, 4), HEADER_DARK);
    // leaf mark
    g.canvas.circle(52, HEADER_H / 2, 22, WHITE);
    g.canvas.round_rect(Rect::new(42, 22, 20, 30), 10, HEADER);
    g.canvas.fill(Rect::new(51, 30, 2, 22), WHITE);
    let w = g.text(88, 20, "NHSE", 30, Weight::Bold, WHITE);
    let x = 88 + w + 18;
    g.canvas.fill_alpha(Rect::new(x, 24, 2, 30), WHITE, 120);
    let right_limit = WIDTH as i32 - 330;
    let t = g
        .fonts
        .ellipsize(title, 26, Weight::Medium, right_limit - x - 20);
    g.text(x + 18, 22, &t, 26, Weight::Medium, WHITE);
    if let Some(s) = &cx.session {
        let (label, bg, fg) = if s.dirty {
            ("Unsaved changes", SELECT, TEXT)
        } else {
            ("Saved", HEADER_DARK, WHITE)
        };
        let lw = g.fonts.measure(label, 18, Weight::Bold) + 28;
        let px = WIDTH as i32 - 28 - lw;
        g.canvas.round_rect(Rect::new(px, 23, lw, 32), 16, bg);
        g.text(px + 14, 28, label, 18, Weight::Bold, fg);
        let town = g.fonts.ellipsize(&s.town(), 22, Weight::Bold, 200);
        g.text_right(px - 16, 25, &town, 22, Weight::Bold, WHITE);
    }
}

fn draw_list(g: &mut Gfx, e: &Entry, empty: &str) {
    card(g, LIST);
    if e.rows.is_empty() {
        g.text_center(
            LIST.x + LIST.w / 2,
            LIST.y + LIST.h / 2 - 16,
            empty,
            22,
            Weight::Medium,
            MUTED,
        );
        return;
    }
    let vis = visible_rows();
    let inner = Rect::new(
        LIST.x + LIST_PAD,
        LIST.y + LIST_PAD,
        LIST.w - 2 * LIST_PAD,
        LIST.h - 2 * LIST_PAD,
    );
    g.canvas.set_clip(Some(inner));
    let overflow = e.rows.len() > vis;
    let right = inner.right() - if overflow { 22 } else { 6 };
    for (k, row) in e.rows.iter().enumerate().skip(e.scroll).take(vis) {
        let y = inner.y + (k - e.scroll) as i32 * ROW_H;
        let selected = k == e.sel;
        if selected {
            let r = Rect::new(inner.x, y + 2, right - inner.x + 4, ROW_H - 4);
            g.canvas.round_rect(r, 14, SELECT_EDGE);
            g.canvas
                .round_rect(Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4), 12, SELECT);
        }
        let ty = y + (ROW_H - 28) / 2;
        match row.style {
            Style::Section => {
                g.text(
                    inner.x + 12,
                    y + 18,
                    &row.label.to_uppercase(),
                    16,
                    Weight::Bold,
                    MUTED,
                );
                g.canvas.fill(
                    Rect::new(inner.x + 12, y + ROW_H - 8, right - inner.x - 12, 2),
                    Color(0xEDE5CC),
                );
            }
            _ => {
                let color = match row.style {
                    Style::Action => ACCENT,
                    Style::Danger => DANGER,
                    Style::Muted => MUTED,
                    _ => TEXT,
                };
                let value_w = if row.value.is_empty() {
                    0
                } else {
                    g.fonts.measure(&row.value, 21, Weight::Bold).min(300) + 20
                };
                let label = g.fonts.ellipsize(
                    &row.label,
                    23,
                    Weight::Medium,
                    right - inner.x - 28 - value_w,
                );
                g.text(inner.x + 18, ty, &label, 23, Weight::Medium, color);
                if !row.value.is_empty() {
                    let v = g.fonts.ellipsize(&row.value, 21, Weight::Bold, 300);
                    g.text_right(
                        right - 10,
                        ty + 2,
                        &v,
                        21,
                        Weight::Bold,
                        if selected { TEXT } else { MUTED },
                    );
                }
            }
        }
    }
    g.canvas.set_clip(None);
    if overflow {
        let track = Rect::new(inner.right() - 10, inner.y + 4, 6, inner.h - 8);
        g.canvas.round_rect(track, 3, Color(0xEDE5CC));
        let n = e.rows.len() as i32;
        let h = (track.h * vis as i32 / n).max(30);
        let y = track.y + (track.h - h) * e.scroll as i32 / (n - vis as i32).max(1);
        g.canvas
            .round_rect(Rect::new(track.x, y, track.w, h), 3, MUTED);
    }
}

fn draw_info(g: &mut Gfx, info: &Info) {
    card(g, INFO);
    let x = INFO.x + 28;
    let w = INFO.w - 56;
    let mut y = INFO.y + 26;
    if !info.title.is_empty() {
        for line in g
            .fonts
            .wrap(&info.title, 26, Weight::Bold, w)
            .into_iter()
            .take(3)
        {
            g.text(x, y, &line, 26, Weight::Bold, TEXT);
            y += 34;
        }
        y += 6;
        g.canvas.round_rect(Rect::new(x, y, 48, 5), 2, HEADER);
        y += 22;
    }
    for (k, v) in &info.facts {
        g.text(x, y, k, 18, Weight::Medium, MUTED);
        let vv = g.fonts.ellipsize(
            v,
            20,
            Weight::Bold,
            w - g.fonts.measure(k, 18, Weight::Medium) - 16,
        );
        g.text_right(x + w, y - 1, &vv, 20, Weight::Bold, TEXT);
        y += 32;
    }
    if !info.facts.is_empty() {
        y += 10;
    }
    let bottom = INFO.bottom() - 20;
    for line in g.fonts.wrap(&info.text, 20, Weight::Medium, w) {
        if y + 26 > bottom {
            break;
        }
        g.text(x, y, &line, 20, Weight::Medium, TEXT);
        y += 28;
    }
}

/// A round (or, for two letters, pill shaped) button glyph. Returns its width.
fn button_glyph(g: &mut Gfx, x: i32, cy: i32, label: &str) -> i32 {
    let tw = g.fonts.measure(label, 17, Weight::Bold);
    let w = (tw + 16).max(30);
    g.canvas
        .round_rect(Rect::new(x, cy - 15, w, 30), 15, BUTTON);
    g.text(x + (w - tw) / 2, cy - 11, label, 17, Weight::Bold, WHITE);
    w
}

fn hints_width(g: &Gfx, hints: &[(&str, &str)]) -> i32 {
    hints
        .iter()
        .map(|(b, t)| {
            (g.fonts.measure(b, 17, Weight::Bold) + 16).max(30)
                + 8
                + g.fonts.measure(t, 19, Weight::Medium)
                + 24
        })
        .sum()
}

fn draw_hints(g: &mut Gfx, right: i32, cy: i32, hints: &[(&str, &str)]) {
    let mut x = right - hints_width(g, hints) + 24;
    for (b, t) in hints {
        x += button_glyph(g, x, cy, b) + 8;
        x += g.text(x, cy - 12, t, 19, Weight::Medium, TEXT) + 24;
    }
}

fn draw_footer(g: &mut Gfx, cx: &Cx, hints: &[(&str, &str)]) {
    let y = HEIGHT as i32 - FOOTER_H;
    g.canvas
        .fill(Rect::new(0, y, WIDTH as i32, FOOTER_H), FOOTER);
    let cy = y + FOOTER_H / 2;
    let hw = hints_width(g, hints);
    draw_hints(g, WIDTH as i32 - 28, cy, hints);
    let room = WIDTH as i32 - 28 - hw - 60;
    if let Some((msg, error, _)) = &cx.toast {
        g.canvas.circle(40, cy, 7, if *error { DANGER } else { OK });
        let m = g.fonts.ellipsize(msg, 19, Weight::Bold, room);
        g.text(
            56,
            cy - 12,
            &m,
            19,
            Weight::Bold,
            if *error { DANGER } else { TEXT },
        );
    } else if let Some(s) = &cx.session {
        let m = g.fonts.ellipsize(&s.location(), 18, Weight::Medium, room);
        g.text(32, cy - 11, &m, 18, Weight::Medium, MUTED);
    }
}

fn draw_dialog(g: &mut Gfx, title: &str, text: &str, hints: &[(&str, &str)], danger: bool) {
    g.canvas.fill_alpha(
        Rect::new(0, 0, WIDTH as i32, HEIGHT as i32),
        Color(0x2A2018),
        130,
    );
    let w = 680;
    let lines = g.fonts.wrap(text, 21, Weight::Medium, w - 80);
    let lines: Vec<String> = lines.into_iter().take(12).collect();
    let h = 150 + lines.len() as i32 * 30;
    let r = Rect::new((WIDTH as i32 - w) / 2, (HEIGHT as i32 - h) / 2, w, h);
    g.canvas
        .round_rect(Rect::new(r.x, r.y + 6, r.w, r.h), 28, Color(0x1E1610));
    g.canvas.round_rect(r, 28, CARD);
    g.canvas.round_rect(
        Rect::new(r.x, r.y, r.w, 10),
        5,
        if danger { DANGER } else { HEADER },
    );
    g.canvas.fill(Rect::new(r.x, r.y + 5, r.w, 23), CARD);
    let mut y = r.y + 30;
    g.text(
        r.x + 40,
        y,
        title,
        27,
        Weight::Bold,
        if danger { DANGER } else { TEXT },
    );
    y += 50;
    for l in &lines {
        g.text(r.x + 40, y, l, 21, Weight::Medium, TEXT);
        y += 30;
    }
    draw_hints(g, r.right() - 32, r.bottom() - 36, hints);
}
