//! Light and dark themes, from the desktop app's design tokens (src/styles/theme.css).
use crate::gfx::Color;

/// The theme setting: follow the console, or always light or dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeChoice {
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub fn next(self) -> Self {
        match self {
            ThemeChoice::System => ThemeChoice::Light,
            ThemeChoice::Light => ThemeChoice::Dark,
            ThemeChoice::Dark => ThemeChoice::System,
        }
    }
    pub fn prev(self) -> Self {
        self.next().next()
    }
    pub fn name(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "system" => Some(ThemeChoice::System),
            "light" => Some(ThemeChoice::Light),
            "dark" => Some(ThemeChoice::Dark),
            _ => None,
        }
    }
    /// The theme to draw with; `system_dark` is the console's own setting.
    pub fn theme(self, system_dark: bool) -> &'static Theme {
        match self {
            ThemeChoice::Light => &LIGHT,
            ThemeChoice::Dark => &DARK,
            ThemeChoice::System if system_dark => &DARK,
            ThemeChoice::System => &LIGHT,
        }
    }
}

pub struct Theme {
    pub dark: bool,
    pub bg: Color,
    pub header: Color,
    /// The line under the header.
    pub header_edge: Color,
    pub header_text: Color,
    /// The logo on the header.
    pub logo: Color,
    pub card: Color,
    pub shadow: Color,
    pub text: Color,
    /// Values and secondary text.
    pub text2: Color,
    /// Headings, hints and inactive rows.
    pub muted: Color,
    pub select: Color,
    pub select_edge: Color,
    /// Rows that run something.
    pub accent: Color,
    pub danger: Color,
    pub ok: Color,
    pub warn: Color,
    pub divider: Color,
    pub footer: Color,
    pub button: Color,
    pub button_text: Color,
    /// The veil behind dialogs, and its opacity.
    pub overlay: Color,
    pub overlay_alpha: u32,
    pub pill_dirty: Color,
    pub pill_dirty_text: Color,
    pub pill_saved: Color,
    pub pill_saved_text: Color,
}

pub static LIGHT: Theme = Theme {
    dark: false,
    bg: Color(0xF4F3EE),
    header: Color(0x23A26F),
    header_edge: Color(0x1B8A5D),
    header_text: Color(0xFFFFFF),
    logo: Color(0xFFFFFF),
    card: Color(0xFFFFFF),
    shadow: Color(0xDEDCD3),
    text: Color(0x1D2420),
    text2: Color(0x56605A),
    muted: Color(0x8B938E),
    select: Color(0xE2F3EC),
    select_edge: Color(0x23A26F),
    accent: Color(0x1B8A5D),
    danger: Color(0xD14B4B),
    ok: Color(0x23A26F),
    warn: Color(0xC98A16),
    divider: Color(0xEAE8E1),
    footer: Color(0xECEAE2),
    button: Color(0x1D2420),
    button_text: Color(0xFFFFFF),
    overlay: Color(0x141E14),
    overlay_alpha: 110,
    pill_dirty: Color(0xFFE08A),
    pill_dirty_text: Color(0x5A3E00),
    pill_saved: Color(0x1B8A5D),
    pill_saved_text: Color(0xFFFFFF),
};

pub static DARK: Theme = Theme {
    dark: true,
    bg: Color(0x121514),
    header: Color(0x171B19),
    header_edge: Color(0x2A322E),
    header_text: Color(0xE8ECE9),
    logo: Color(0x34C789),
    card: Color(0x1C211F),
    shadow: Color(0x0B0D0C),
    text: Color(0xE8ECE9),
    text2: Color(0xA8B1AC),
    muted: Color(0x7D8681),
    select: Color(0x203C30),
    select_edge: Color(0x34C789),
    accent: Color(0x4FD99D),
    danger: Color(0xEC6B6B),
    ok: Color(0x34C789),
    warn: Color(0xE0A83A),
    divider: Color(0x2A302D),
    footer: Color(0x171B19),
    button: Color(0xE8ECE9),
    button_text: Color(0x121514),
    overlay: Color(0x000000),
    overlay_alpha: 150,
    pill_dirty: Color(0xE0A83A),
    pill_dirty_text: Color(0x121514),
    pill_saved: Color(0x203C30),
    pill_saved_text: Color(0x4FD99D),
};
