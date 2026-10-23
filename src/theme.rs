use crate::config::ThemeChoice;
use ratatui::style::Color;
use std::fs;

#[derive(Clone, Copy, Debug)]
pub struct SystemPalette {
    pub primary: Color,
    pub secondary: Color,
    pub tertiary: Color,
    pub on_surface: Color,
    pub muted: Color,
}

impl Default for SystemPalette {
    fn default() -> Self {
        Self {
            primary: Color::Rgb(155, 208, 204),    // #9bd0cc
            secondary: Color::Rgb(176, 204, 201),  // #b0ccc9
            tertiary: Color::Rgb(255, 184, 120),   // Caelestia sunset peach
            on_surface: Color::Rgb(220, 232, 230), // #dce8e6
            muted: Color::Rgb(110, 121, 120),      // #6e7978
        }
    }
}

impl SystemPalette {
    pub fn load_for_theme(choice: ThemeChoice) -> Self {
        match choice {
            ThemeChoice::Auto => Self::load_system(),
            ThemeChoice::Catppuccin => Self {
                primary: Color::Rgb(203, 166, 247),   // Mocha Mauve
                secondary: Color::Rgb(180, 190, 254), // Lavender
                tertiary: Color::Rgb(245, 194, 231),  // Pink
                on_surface: Color::Rgb(205, 214, 244),
                muted: Color::Rgb(108, 112, 134),
            },
            ThemeChoice::TokyoNight => Self {
                primary: Color::Rgb(125, 207, 255),  // Neon Cyan
                secondary: Color::Rgb(187, 154, 247), // Purple
                tertiary: Color::Rgb(255, 158, 100), // Orange
                on_surface: Color::Rgb(192, 202, 245),
                muted: Color::Rgb(86, 95, 137),
            },
            ThemeChoice::Nord => Self {
                primary: Color::Rgb(136, 192, 208),  // Frost Cyan
                secondary: Color::Rgb(129, 161, 193), // Frost Blue
                tertiary: Color::Rgb(235, 203, 139),  // Nord Yellow
                on_surface: Color::Rgb(236, 239, 244),
                muted: Color::Rgb(76, 86, 106),
            },
            ThemeChoice::Gruvbox => Self {
                primary: Color::Rgb(254, 128, 25),  // Gruvbox Orange
                secondary: Color::Rgb(250, 189, 47), // Yellow
                tertiary: Color::Rgb(184, 187, 38),  // Green
                on_surface: Color::Rgb(235, 219, 178),
                muted: Color::Rgb(146, 131, 116),
            },
            ThemeChoice::Dracula => Self {
                primary: Color::Rgb(189, 147, 249),  // Purple
                secondary: Color::Rgb(255, 121, 198), // Pink
                tertiary: Color::Rgb(80, 250, 123),   // Green
                on_surface: Color::Rgb(248, 248, 242),
                muted: Color::Rgb(98, 114, 164),
            },
            ThemeChoice::Amber => Self {
                primary: Color::Rgb(255, 176, 0),    // Vintage Amber
                secondary: Color::Rgb(255, 196, 64),
                tertiary: Color::Rgb(255, 140, 0),
                on_surface: Color::Rgb(240, 220, 180),
                muted: Color::Rgb(130, 100, 50),
            },
            ThemeChoice::White => Self {
                primary: Color::Rgb(240, 240, 240),
                secondary: Color::Rgb(200, 200, 200),
                tertiary: Color::Rgb(255, 255, 255),
                on_surface: Color::Rgb(230, 230, 230),
                muted: Color::Rgb(90, 90, 90),
            },
        }
    }

    pub fn load_system() -> Self {
        let mut palette = Self::default();
        if let Some(mut path) = dirs::config_dir() {
            path.push("hypr/scheme/current.conf");
            if let Ok(content) = fs::read_to_string(path) {
                for line in content.lines() {
                    let line = line.trim();
                    if let Some(rest) = line.strip_prefix("$primary =") {
                        if let Some(c) = parse_hex(rest) { palette.primary = c; }
                    } else if let Some(rest) = line.strip_prefix("$secondary =") {
                        if let Some(c) = parse_hex(rest) { palette.secondary = c; }
                    } else if let Some(rest) = line.strip_prefix("$onSurface =") {
                        if let Some(c) = parse_hex(rest) { palette.on_surface = c; }
                    } else if let Some(rest) = line.strip_prefix("$outline =") {
                        if let Some(c) = parse_hex(rest) { palette.muted = c; }
                    }
                }
            }
        }
        palette
    }
}

fn parse_hex(hex: &str) -> Option<Color> {
    let clean = hex.trim().trim_start_matches('#').trim_start_matches('$');
    if clean.len() >= 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
        let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
        let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
        return Some(Color::Rgb(r, g, b));
    }
    None
}
