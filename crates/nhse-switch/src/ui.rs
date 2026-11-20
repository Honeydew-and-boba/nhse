//! The app shell: a stack of list screens, with a header, an info panel, button hints, dialogs
//! and a "working…" overlay for long operations.
use crate::gfx::{Color, Gfx, Rect, Weight, HEIGHT, WIDTH};
use crate::platform::{btn, Input, KeyboardMode, KeyboardRequest, Platform, UserAccount};
use crate::session::{self, Session};
use nhse_core::{HorizonSave, Item};

use crate::gfx::Mask;
use crate::theme::{Theme, ThemeChoice};

/// How the logo at the top left looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoStyle {
    /// The palm tree of the app icon, then "NHSE".
    Palm,
    /// The whole app icon (frame, palm tree and pencil), then "NHSE".
    Icon,
    /// The palm tree on a rounded tile, like a small app icon, then "NHSE".
    Badge,
    /// "NHSE" only.
    Text,
}

/// The logo masks, scaled for the header.
struct Logos {
    palm: Mask,
    icon: Mask,
    badge_palm: Mask,
}

impl Logos {
    fn new() -> Self {
        let palm = Mask::from_bytes(include_bytes!("../assets/palm.mask"));
        let icon = Mask::from_bytes(include_bytes!("../assets/logo.mask"));
        Self {
            palm: palm.scaled(46),
            icon: icon.scaled(50),
            badge_palm: palm.scaled(34),
        }
    }
}

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
    /// Optional visual preview in the side panel.
    fn preview(&self, _cx: &Cx, _g: &mut Gfx) {}
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
    pub theme_choice: ThemeChoice,
    /// Whether the console's own theme is dark (for [`ThemeChoice::System`]).
    pub system_dark: bool,
    pub logo: LogoStyle,
    toast: Option<(String, bool, u32)>,
}

const SETTINGS_FILE: &str = "settings.txt";

impl Cx {
    pub fn theme(&self) -> &'static Theme {
        self.theme_choice.theme(self.system_dark)
    }

    /// Changes the theme and remembers it on the SD card.
    pub fn set_theme(&mut self, choice: ThemeChoice) {
        self.theme_choice = choice;
        if choice == ThemeChoice::System {
            self.system_dark = self.platform.system_dark();
        }
        let dir = self.platform.data_dir();
        let saved = nhse_core::save::provider::create_dir_all(&dir).and_then(|_| {
            std::fs::write(
                dir.join(SETTINGS_FILE),
                format!("theme={}\n", choice.name()),
            )
        });
        if let Err(e) = saved {
            self.toast_error(format!("The setting could not be saved: {e}"));
        }
    }

    fn load_settings(&mut self) {
        let path = self.platform.data_dir().join(SETTINGS_FILE);
        if let Ok(text) = nhse_core::save::provider::read_text(&path) {
            for line in text.lines() {
                if let Some(t) = line.strip_prefix("theme=").and_then(ThemeChoice::parse) {
                    self.theme_choice = t;
                }
            }
        }
    }

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
    logos: Logos,
    /// Frames each direction has been held, for key repeat.
    held_frames: [u32; 4],
    redraw: bool,
    quit: bool,
}

impl App {
    pub fn new(mut platform: Box<dyn Platform>, root: Box<dyn View>) -> Self {
        let system_dark = platform.system_dark();
        let mut cx = Cx {
            platform,
            session: None,
            clipboard: None,
            theme_choice: ThemeChoice::System,
            system_dark,
            logo: LogoStyle::Palm,
            toast: None,
        };
        cx.load_settings();
        Self {
            cx,
            stack: vec![Entry::new(root)],
            modal: None,
            gfx: Gfx::new(),
            logos: Logos::new(),
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
        let Self {
            gfx,
            stack,
            cx,
            logos,
            ..
        } = self;
        let e = stack.last().expect("stack");
        let g = gfx;
        g.theme = cx.theme();
        g.canvas.set_clip(None);
        g.canvas.clear(g.theme.bg);
        draw_header(g, cx, logos, &e.view.title(cx));
        draw_list(g, e, e.view.empty_text());
        let info = e.view.info(cx, &e.rows, e.sel);
        draw_info(g, &info);
        e.view.preview(cx, g);
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
        let t = g.theme;
        g.canvas.fill_alpha(
            Rect::new(0, 0, WIDTH as i32, HEIGHT as i32),
            t.overlay,
            t.overlay_alpha,
        );
        let r = Rect::new(WIDTH as i32 / 2 - 240, HEIGHT as i32 / 2 - 60, 480, 120);
        g.canvas.round_rect(r, 24, t.card);
        // three dots
        for i in 0..3 {
            g.canvas.circle(
                r.x + 56 + i * 22,
                r.y + 60,
                7,
                if i == 1 { t.warn } else { t.ok },
            );
        }
        g.text(r.x + 140, r.y + 44, text, 26, Weight::Bold, t.text);
    }

    /// Draws the current screen with a given logo (for the logo previews).
    pub fn render_with_logo(&mut self, logo: LogoStyle) -> &crate::gfx::Canvas {
        let old = self.cx.logo;
        self.cx.logo = logo;
        self.refresh();
        self.draw();
        self.cx.logo = old;
        &self.gfx.canvas
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
    let t = g.theme;
    g.canvas
        .round_rect(Rect::new(r.x, r.y + 4, r.w, r.h), 22, t.shadow);
    g.canvas.round_rect(r, 22, t.card);
}

/// Draws the logo at the top left. Returns where the title can start.
fn draw_logo(g: &mut Gfx, logos: &Logos, style: LogoStyle) -> i32 {
    let t = g.theme;
    let mid = HEADER_H / 2 - 2;
    let x = match style {
        LogoStyle::Palm => {
            let m = &logos.palm;
            g.canvas.mask(26, mid - m.h as i32 / 2, m, t.logo);
            26 + m.w as i32 + 12
        }
        LogoStyle::Icon => {
            let m = &logos.icon;
            g.canvas.mask(22, mid - m.h as i32 / 2, m, t.logo);
            22 + m.w as i32 + 12
        }
        LogoStyle::Badge => {
            let r = Rect::new(22, mid - 26, 52, 52);
            let (tile, palm) = if t.dark {
                (t.logo, t.header)
            } else {
                (t.header_text, t.header)
            };
            g.canvas.round_rect(r, 15, tile);
            let m = &logos.badge_palm;
            g.canvas.mask(
                r.x + (r.w - m.w as i32) / 2,
                r.y + (r.h - m.h as i32) / 2,
                m,
                palm,
            );
            r.right() + 14
        }
        LogoStyle::Text => 30,
    };
    let w = g.text(x, mid - 18, "NHSE", 30, Weight::Bold, t.header_text);
    x + w
}

fn draw_header(g: &mut Gfx, cx: &Cx, logos: &Logos, title: &str) {
    let t = g.theme;
    g.canvas
        .fill(Rect::new(0, 0, WIDTH as i32, HEADER_H), t.header);
    g.canvas
        .fill(Rect::new(0, HEADER_H - 4, WIDTH as i32, 4), t.header_edge);
    let x = draw_logo(g, logos, cx.logo) + 18;
    g.canvas
        .fill_alpha(Rect::new(x, 22, 2, 30), t.header_text, 110);
    let right_limit = WIDTH as i32 - 330;
    let title = g
        .fonts
        .ellipsize(title, 26, Weight::Medium, right_limit - x - 20);
    g.text(x + 18, 20, &title, 26, Weight::Medium, t.header_text);
    if let Some(s) = &cx.session {
        let (label, bg, fg) = if s.dirty {
            ("Unsaved changes", t.pill_dirty, t.pill_dirty_text)
        } else {
            ("Saved", t.pill_saved, t.pill_saved_text)
        };
        let lw = g.fonts.measure(label, 18, Weight::Bold) + 28;
        let px = WIDTH as i32 - 28 - lw;
        g.canvas.round_rect(Rect::new(px, 21, lw, 32), 16, bg);
        g.text(px + 14, 26, label, 18, Weight::Bold, fg);
        let town = g.fonts.ellipsize(&s.town(), 22, Weight::Bold, 200);
        g.text_right(px - 16, 23, &town, 22, Weight::Bold, t.header_text);
    }
}

fn draw_list(g: &mut Gfx, e: &Entry, empty: &str) {
    let t = g.theme;
    card(g, LIST);
    if e.rows.is_empty() {
        g.text_center(
            LIST.x + LIST.w / 2,
            LIST.y + LIST.h / 2 - 16,
            empty,
            22,
            Weight::Medium,
            t.muted,
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
            g.canvas.round_rect(r, 14, t.select_edge);
            g.canvas
                .round_rect(Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4), 12, t.select);
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
                    t.muted,
                );
                g.canvas.fill(
                    Rect::new(inner.x + 12, y + ROW_H - 8, right - inner.x - 12, 2),
                    t.divider,
                );
            }
            _ => {
                let color = match row.style {
                    Style::Action => t.accent,
                    Style::Danger => t.danger,
                    Style::Muted => t.muted,
                    _ => t.text,
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
                        if selected { t.text } else { t.text2 },
                    );
                }
            }
        }
    }
    g.canvas.set_clip(None);
    if overflow {
        let track = Rect::new(inner.right() - 10, inner.y + 4, 6, inner.h - 8);
        g.canvas.round_rect(track, 3, t.divider);
        let n = e.rows.len() as i32;
        let h = (track.h * vis as i32 / n).max(30);
        let y = track.y + (track.h - h) * e.scroll as i32 / (n - vis as i32).max(1);
        g.canvas
            .round_rect(Rect::new(track.x, y, track.w, h), 3, t.muted);
    }
}

fn draw_info(g: &mut Gfx, info: &Info) {
    let t = g.theme;
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
            g.text(x, y, &line, 26, Weight::Bold, t.text);
            y += 34;
        }
        y += 6;
        g.canvas.round_rect(Rect::new(x, y, 48, 5), 2, t.ok);
        y += 22;
    }
    for (k, v) in &info.facts {
        g.text(x, y, k, 18, Weight::Medium, t.muted);
        let vv = g.fonts.ellipsize(
            v,
            20,
            Weight::Bold,
            w - g.fonts.measure(k, 18, Weight::Medium) - 16,
        );
        g.text_right(x + w, y - 1, &vv, 20, Weight::Bold, t.text);
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
        g.text(x, y, &line, 20, Weight::Medium, t.text2);
        y += 28;
    }
}

/// A round (or, for two letters, pill shaped) button glyph. Returns its width.
fn button_glyph(g: &mut Gfx, x: i32, cy: i32, label: &str) -> i32 {
    let t = g.theme;
    let tw = g.fonts.measure(label, 17, Weight::Bold);
    let w = (tw + 16).max(30);
    g.canvas
        .round_rect(Rect::new(x, cy - 15, w, 30), 15, t.button);
    g.text(
        x + (w - tw) / 2,
        cy - 11,
        label,
        17,
        Weight::Bold,
        t.button_text,
    );
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
    let color = g.theme.text;
    let mut x = right - hints_width(g, hints) + 24;
    for (b, label) in hints {
        x += button_glyph(g, x, cy, b) + 8;
        x += g.text(x, cy - 12, label, 19, Weight::Medium, color) + 24;
    }
}

fn draw_footer(g: &mut Gfx, cx: &Cx, hints: &[(&str, &str)]) {
    let t = g.theme;
    let y = HEIGHT as i32 - FOOTER_H;
    g.canvas
        .fill(Rect::new(0, y, WIDTH as i32, FOOTER_H), t.footer);
    let cy = y + FOOTER_H / 2;
    let hw = hints_width(g, hints);
    draw_hints(g, WIDTH as i32 - 28, cy, hints);
    let room = WIDTH as i32 - 28 - hw - 60;
    if let Some((msg, error, _)) = &cx.toast {
        g.canvas
            .circle(40, cy, 7, if *error { t.danger } else { t.ok });
        let m = g.fonts.ellipsize(msg, 19, Weight::Bold, room);
        g.text(
            56,
            cy - 12,
            &m,
            19,
            Weight::Bold,
            if *error { t.danger } else { t.text },
        );
    } else if let Some(s) = &cx.session {
        let m = g.fonts.ellipsize(&s.location(), 18, Weight::Medium, room);
        g.text(32, cy - 11, &m, 18, Weight::Medium, t.muted);
    }
}

fn draw_dialog(g: &mut Gfx, title: &str, text: &str, hints: &[(&str, &str)], danger: bool) {
    let t = g.theme;
    g.canvas.fill_alpha(
        Rect::new(0, 0, WIDTH as i32, HEIGHT as i32),
        t.overlay,
        t.overlay_alpha,
    );
    let w = 680;
    let lines = g.fonts.wrap(text, 21, Weight::Medium, w - 80);
    let lines: Vec<String> = lines.into_iter().take(12).collect();
    let h = 150 + lines.len() as i32 * 30;
    let r = Rect::new((WIDTH as i32 - w) / 2, (HEIGHT as i32 - h) / 2, w, h);
    g.canvas
        .round_rect_alpha(Rect::new(r.x, r.y + 6, r.w, r.h), 28, Color(0x000000), 90);
    g.canvas.round_rect(r, 28, t.card);
    // a colored strip along the top edge, inside the rounded corners
    let strip = if danger { t.danger } else { t.ok };
    g.canvas.fill(Rect::new(r.x + 28, r.y, r.w - 56, 6), strip);
    let mut y = r.y + 30;
    g.text(
        r.x + 40,
        y,
        title,
        27,
        Weight::Bold,
        if danger { t.danger } else { t.text },
    );
    y += 50;
    for l in &lines {
        g.text(r.x + 40, y, l, 21, Weight::Medium, t.text2);
        y += 30;
    }
    draw_hints(g, r.right() - 32, r.bottom() - 36, hints);
}
