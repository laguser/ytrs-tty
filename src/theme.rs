use std::fs;
use ratatui::style::Color;

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
    pub fn load() -> Self {
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
