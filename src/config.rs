use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeChoice {
    Auto,       // Hyprland / system scheme
    Catppuccin, // Mocha Mauve (#cba6f7)
    TokyoNight, // Neon Cyan (#7dcfff)
    Nord,       // Frost Blue (#88c0d0)
    Gruvbox,    // Retro Orange (#fe8019)
    Dracula,    // Purple (#bd93f9)
    Amber,      // Vintage Amber (#ffb000)
    White,      // Pure Monochrome
}

impl ThemeChoice {
    pub fn name(&self) -> &'static str {
        match self {
            ThemeChoice::Auto => "auto (hypr/term)",
            ThemeChoice::Catppuccin => "catppuccin",
            ThemeChoice::TokyoNight => "tokyo night",
            ThemeChoice::Nord => "nord",
            ThemeChoice::Gruvbox => "gruvbox",
            ThemeChoice::Dracula => "dracula",
            ThemeChoice::Amber => "amber",
            ThemeChoice::White => "white",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressStyle {
    Tape,    // ━━━●───
    Blocky,  // ████▓░░
    Minimal, // ···◆   
}

impl ProgressStyle {
    pub fn name(&self) -> &'static str {
        match self {
            ProgressStyle::Tape => "tape (smooth)",
            ProgressStyle::Blocky => "blocky (solid)",
            ProgressStyle::Minimal => "minimal (dots)",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_theme")]
    pub theme: ThemeChoice,
    #[serde(default = "default_progress_style")]
    pub progress_style: ProgressStyle,
    pub vot_volume: f32,          // e.g. 1.0 (100%)
    pub orig_volume: f32,         // e.g. 0.15 (15%)
    pub max_res_height: u32,      // e.g. 1080
    pub translation_from: String, // Source audio language (default: "en")
    pub translation_to: String,   // Target translation language (default: "ru")
    pub subs_lang: String,        // "ru", "en", "off"
    pub subs_font: String,        // e.g. "JetBrains Mono"
    pub subs_font_size: u32,      // e.g. 18
    pub sponsorblock: bool,       // auto-skip sponsors
    pub graphics_mode: String,    // "auto", "kitty", "sixels", "iterm", "symbols", "off"
    pub enable_history: bool,     // History & Bookmarks
    pub enable_trending: bool,    // Trending / Recommendations feed
    pub enable_related: bool,     // Related / Recommended videos
    pub enable_summary: bool,     // Smart Summary (TL;DR)
}

fn default_theme() -> ThemeChoice {
    ThemeChoice::Auto
}

fn default_progress_style() -> ProgressStyle {
    ProgressStyle::Tape
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::Auto,
            progress_style: ProgressStyle::Tape,
            vot_volume: 1.0,
            orig_volume: 0.15,
            max_res_height: 1080,
            translation_from: "en".to_string(),
            translation_to: "ru".to_string(),
            subs_lang: "ru".to_string(),
            subs_font: "JetBrains Mono".to_string(),
            subs_font_size: 18,
            sponsorblock: true,
            graphics_mode: "auto".to_string(),
            enable_history: true,
            enable_trending: true,
            enable_related: true,
            enable_summary: true,
        }
    }
}

impl AppConfig {
    fn path() -> PathBuf {
        let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/home/lag/.config"));
        p.push("ytrs");
        let _ = fs::create_dir_all(&p);
        p.push("config.json");
        p
    }

    pub fn load() -> Self {
        let p = Self::path();
        if let Ok(data) = fs::read_to_string(&p) {
            if let Ok(cfg) = serde_json::from_str(&data) {
                return cfg;
            }
        }
        let def = Self::default();
        let _ = def.save();
        def
    }

    pub fn save(&self) -> bool {
        let p = Self::path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            return fs::write(p, json).is_ok();
        }
        false
    }
}
