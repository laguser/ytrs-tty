mod config;
mod player;
mod search;
mod storage;
mod summary;
mod theme;
mod thumbnail;
mod ui;
mod vot;

use config::AppConfig;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use player::Player;
use ratatui::{backend::CrosstermBackend, Terminal};
use search::{SearchEngine, VideoItem};
use std::{
    error::Error,
    io,
    sync::mpsc::{channel, Receiver, Sender},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use storage::{AppStorage, HistoryEntry};
use summary::Summarizer;
use theme::SystemPalette;
use thumbnail::ThumbnailManager;

enum AppEvent {
    SearchResults(Vec<VideoItem>),
    SummaryReady(String, Vec<String>), // video_id, summary_lines
    ThumbnailReady(String),
    ProgressUpdate(f32, String),
}

struct App {
    query_input: String,
    is_typing_search: bool,
    is_searching: bool,
    items: Vec<VideoItem>,
    selected_idx: usize,
    status_msg: String,
    status_time: Instant,
    palette: SystemPalette,
    tx: Sender<AppEvent>,
    rx: Receiver<AppEvent>,
    progress: Option<(f32, String)>,
    progress_time: Instant,
    show_settings: bool,
    settings_idx: usize,
    cfg: AppConfig,
    storage: AppStorage,
    summary_map: std::collections::HashMap<String, Vec<String>>,
    thumbnail_mgr: ThumbnailManager,
    last_thumb_req: Option<(String, u16, u16)>,
}

impl App {
    fn new(initial_query: Option<String>) -> Self {
        let (tx, rx) = channel();
        let palette = SystemPalette::load();
        let cfg = AppConfig::load();
        let storage = AppStorage::load();

        let tx_thumb = tx.clone();
        let (thumb_tx, thumb_rx) = channel();
        thread::spawn(move || {
            while let Ok(vid) = thumb_rx.recv() {
                let _ = tx_thumb.send(AppEvent::ThumbnailReady(vid));
            }
        });

        let thumbnail_mgr = ThumbnailManager::new(thumb_tx);

        let query = initial_query.unwrap_or_default();
        let has_query = !query.is_empty();

        let mut app = Self {
            query_input: query,
            is_typing_search: false,
            is_searching: false,
            items: Vec::new(),
            selected_idx: 0,
            status_msg: String::new(),
            status_time: Instant::now(),
            palette,
            tx,
            rx,
            progress: None,
            progress_time: Instant::now(),
            show_settings: false,
            settings_idx: 0,
            cfg,
            storage,
            summary_map: std::collections::HashMap::new(),
            thumbnail_mgr,
            last_thumb_req: None,
        };

        if has_query {
            app.trigger_search();
        } else {
            // Automatically populate with trending / recommendations (реки) on launch
            app.show_trending();
        }

        app
    }

    fn set_status(&mut self, msg: &str) {
        self.status_msg = msg.to_string();
        self.status_time = Instant::now();
    }

    fn trigger_search(&mut self) {
        let query = self.query_input.trim().to_string();
        if query.is_empty() {
            return;
        }

        self.is_searching = true;
        self.is_typing_search = false;
        self.set_status(&format!("Searching: {}", query));
        self.progress = Some((0.3, "Searching YouTube...".to_string()));
        self.progress_time = Instant::now();

        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = SearchEngine::search(&query, 30);
            let _ = tx.send(AppEvent::SearchResults(res));
        });
    }

    fn show_history(&mut self) {
        if !self.cfg.enable_history {
            self.set_status("History is disabled in settings [s]");
            return;
        }
        if self.storage.history.is_empty() {
            self.set_status("History is empty. Watch some videos first!");
            return;
        }
        self.items = self
            .storage
            .history
            .iter()
            .map(|h| VideoItem {
                id: h.id.clone(),
                title: h.title.clone(),
                channel: h.channel.clone(),
                channel_id: None,
                channel_avatar: None,
                duration: h.duration.clone(),
                views: "History".to_string(),
                published: "Recent".to_string(),
            })
            .collect();
        self.selected_idx = 0;
        self.set_status(&format!("Loaded {} videos from History", self.items.len()));
    }

    fn show_bookmarks(&mut self) {
        if !self.cfg.enable_history {
            self.set_status("Bookmarks are disabled in settings [s]");
            return;
        }
        if self.storage.bookmarks.is_empty() {
            self.set_status("No bookmarks yet. Press [b] on any video to bookmark!");
            return;
        }
        self.items = self
            .storage
            .bookmarks
            .iter()
            .map(|b| VideoItem {
                id: b.id.clone(),
                title: b.title.clone(),
                channel: b.channel.clone(),
                channel_id: None,
                channel_avatar: None,
                duration: b.duration.clone(),
                views: "Bookmark".to_string(),
                published: "Saved".to_string(),
            })
            .collect();
        self.selected_idx = 0;
        self.set_status(&format!("Loaded {} bookmarks", self.items.len()));
    }

    fn toggle_current_bookmark(&mut self) {
        if let Some(item) = self.items.get(self.selected_idx) {
            let entry = HistoryEntry {
                id: item.id.clone(),
                title: item.title.clone(),
                channel: item.channel.clone(),
                duration: item.duration.clone(),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            };
            let added = self.storage.toggle_bookmark(entry);
            if added {
                self.set_status(&format!("★ Saved to bookmarks: {}", item.title));
            } else {
                self.set_status(&format!("Removed bookmark: {}", item.title));
            }
        }
    }

    // Реки / Trending feed
    fn show_trending(&mut self) {
        self.is_searching = true;
        self.set_status("Loading Recommendations & Trending (Реки)...");
        self.progress = Some((0.35, "Fetching feed...".to_string()));
        self.progress_time = Instant::now();

        let tx = self.tx.clone();
        thread::spawn(move || {
            let res = SearchEngine::fetch_trending(30);
            let _ = tx.send(AppEvent::SearchResults(res));
        });
    }

    // Related / Recommended
    fn show_related(&mut self) {
        if let Some(item) = self.items.get(self.selected_idx) {
            let vid_id = item.id.clone();
            let title = item.title.clone();
            self.is_searching = true;
            self.set_status(&format!("Finding recommendations for: {}", title));
            self.progress = Some((0.35, "Finding related videos...".to_string()));
            self.progress_time = Instant::now();

            let tx = self.tx.clone();
            thread::spawn(move || {
                let res = SearchEngine::fetch_related(&vid_id, &title, 30);
                let _ = tx.send(AppEvent::SearchResults(res));
            });
        }
    }

    // AI Summary (TL;DR)
    fn generate_summary(&mut self) {
        if let Some(item) = self.items.get(self.selected_idx) {
            let vid_id = item.id.clone();
            if self.summary_map.contains_key(&vid_id) {
                return;
            }

            self.set_status(&format!("Generating summary for: {}", item.title));
            self.progress = Some((0.3, "Extracting video chapters & content...".to_string()));
            self.progress_time = Instant::now();

            let tx = self.tx.clone();
            let from_lang = self.cfg.translation_from.clone();
            let to_lang = self.cfg.translation_to.clone();
            let yt_url = format!("https://www.youtube.com/watch?v={}", vid_id);

            thread::spawn(move || {
                let lines = Summarizer::extract_video_summary(&yt_url, &vid_id, &from_lang, &to_lang);
                let _ = tx.send(AppEvent::SummaryReady(vid_id, lines));
                let _ = tx.send(AppEvent::ProgressUpdate(1.0, "Summary ready!".to_string()));
            });
        }
    }

    // Channel view
    fn open_channel(&mut self) {
        if let Some(item) = self.items.get(self.selected_idx) {
            let ch_ref = if let Some(ref cid) = item.channel_id {
                cid.clone()
            } else {
                format!("@{}", item.channel)
            };

            self.is_searching = true;
            self.set_status(&format!("Loading channel: {}", item.channel));
            self.progress = Some((0.3, "Loading channel videos...".to_string()));
            self.progress_time = Instant::now();

            let tx = self.tx.clone();
            thread::spawn(move || {
                let res = SearchEngine::fetch_channel_videos(&ch_ref, 30);
                let _ = tx.send(AppEvent::SearchResults(res));
            });
        }
    }

    fn record_history(&mut self) {
        if self.cfg.enable_history {
            if let Some(item) = self.items.get(self.selected_idx) {
                let entry = HistoryEntry {
                    id: item.id.clone(),
                    title: item.title.clone(),
                    channel: item.channel.clone(),
                    duration: item.duration.clone(),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };
                self.storage.add_history(entry);
            }
        }
    }

    fn update(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                AppEvent::SearchResults(items) => {
                    self.is_searching = false;
                    let count = items.len();
                    self.items = items;
                    self.selected_idx = 0;
                    self.progress = None;
                    self.set_status(&format!("Loaded {} videos", count));
                }
                AppEvent::SummaryReady(vid_id, lines) => {
                    self.summary_map.insert(vid_id, lines);
                }
                AppEvent::ThumbnailReady(_vid_id) => {
                    // Ready to render
                }
                AppEvent::ProgressUpdate(val, label) => {
                    self.progress = Some((val, label));
                    self.progress_time = Instant::now();
                }
            }
        }

        if let Some((val, _)) = self.progress {
            if (val >= 1.0 || !self.is_searching)
                && self.progress_time.elapsed() > Duration::from_secs(3)
            {
                self.progress = None;
            }
        }

        if !self.status_msg.is_empty() && self.status_time.elapsed() > Duration::from_secs(5) {
            self.status_msg.clear();
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    let initial_query = if args.len() > 1 {
        if args[1] == "-h" || args[1] == "--help" {
            println!("ytrs — Minimalist fast YouTube CLI viewer in Rust\n");
            println!("USAGE:");
            println!("  ytrs [QUERY or @CHANNEL]\n");
            println!("KEYBINDINGS:");
            println!("  t           Browse Recommendations / Trending (Реки)");
            println!("  r           View Related videos for current selection");
            println!("  u / c       Browse channel videos of current video");
            println!("  x           Generate Smart AI Summary (TL;DR with timestamps)");
            println!("  v           Watch with Voice-Over Translation (VOT) + Subtitles");
            println!("  enter       Watch original video in mpv");
            println!("  a           Listen to audio stream only (no video window)");
            println!("  h           View watch History");
            println!("  b           View Bookmarks / Toggle bookmark on video");
            println!("  s           Open Settings & Language configuration");
            println!("  /           Focus search input bar");
            println!("  j / k       Navigate videos (Down / Up)");
            println!("  q / esc     Quit");
            return Ok(());
        }
        Some(args[1..].join(" "))
    } else {
        None
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(initial_query);

    let tick_rate = Duration::from_millis(30);
    let mut last_tick = Instant::now();

    loop {
        app.update();

        let cur_id = app
            .items
            .get(app.selected_idx)
            .map(|i| i.id.clone())
            .unwrap_or_default();
        let summary_text = app.summary_map.get(&cur_id).map(|v| v.as_slice());
        let is_bm = app.storage.bookmarks.iter().any(|b| b.id == cur_id);

        let thumb_cached = app.thumbnail_mgr.get(&cur_id);
        let has_thumbnail = thumb_cached.is_some();

        let mut preview_rect: Option<ratatui::layout::Rect> = None;

        terminal.draw(|f| {
            preview_rect = ui::render(
                f,
                &app.query_input,
                app.is_typing_search,
                app.is_searching,
                &app.items,
                app.selected_idx,
                &app.status_msg,
                app.progress.clone(),
                app.show_settings,
                app.settings_idx,
                summary_text,
                is_bm,
                has_thumbnail,
                &app.cfg,
                &app.palette,
            );
        })?;

        // Handle thumbnail loading & drawing
        if let Some(rect) = preview_rect {
            if !cur_id.is_empty() && app.cfg.graphics_mode != "off" {
                if let Some(ref thumb) = thumb_cached {
                    thumbnail::draw_thumbnail_to_terminal(thumb, rect.x, rect.y);
                } else {
                    let req = (cur_id.clone(), rect.width, rect.height);
                    if app.last_thumb_req.as_ref() != Some(&req) {
                        app.last_thumb_req = Some(req);
                        app.thumbnail_mgr.request(&cur_id, rect.width, rect.height, &app.cfg.graphics_mode);
                    }
                }
            }
        }

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Release && handle_input(&mut app, key) {
                    break;
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn handle_input(app: &mut App, key: KeyEvent) -> bool {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return true;
    }

    if app.show_settings {
        let from_langs = ["en", "auto", "ru", "zh", "de", "fr", "es", "ja", "ko", "it"];
        let to_langs = ["ru", "en", "kk", "uk", "de", "es", "zh", "fr", "uz"];
        let gfx_modes = ["auto", "kitty", "sixels", "iterm", "symbols", "off"];

        match key.code {
            KeyCode::Esc | KeyCode::Char('s') | KeyCode::Char('q') => {
                app.show_settings = false;
                app.cfg.save();
                app.set_status("Settings saved!");
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if app.settings_idx > 0 {
                    app.settings_idx -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if app.settings_idx < 9 {
                    app.settings_idx += 1;
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                match app.settings_idx {
                    0 => {
                        let cur_i = from_langs.iter().position(|&l| l == app.cfg.translation_from).unwrap_or(0);
                        let next_i = if cur_i == 0 { from_langs.len() - 1 } else { cur_i - 1 };
                        app.cfg.translation_from = from_langs[next_i].to_string();
                    }
                    1 => {
                        let cur_i = to_langs.iter().position(|&l| l == app.cfg.translation_to).unwrap_or(0);
                        let next_i = if cur_i == 0 { to_langs.len() - 1 } else { cur_i - 1 };
                        app.cfg.translation_to = to_langs[next_i].to_string();
                    }
                    2 => {
                        app.cfg.subs_lang = match app.cfg.subs_lang.as_str() {
                            "ru" => "off".to_string(),
                            "en" => "ru".to_string(),
                            _ => "en".to_string(),
                        };
                    }
                    3 => {
                        let cur_i = gfx_modes.iter().position(|&m| m == app.cfg.graphics_mode).unwrap_or(0);
                        let next_i = if cur_i == 0 { gfx_modes.len() - 1 } else { cur_i - 1 };
                        app.cfg.graphics_mode = gfx_modes[next_i].to_string();
                    }
                    4 => {
                        app.cfg.max_res_height = match app.cfg.max_res_height {
                            2160 => 1440,
                            1440 => 1080,
                            1080 => 720,
                            _ => 720,
                        };
                    }
                    5 => app.cfg.orig_volume = (app.cfg.orig_volume - 0.05).max(0.0),
                    6 => app.cfg.vot_volume = (app.cfg.vot_volume - 0.05).max(0.1),
                    7 => app.cfg.sponsorblock = !app.cfg.sponsorblock,
                    8 => app.cfg.enable_trending = !app.cfg.enable_trending,
                    9 => app.cfg.enable_history = !app.cfg.enable_history,
                    _ => {}
                }
            }
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter | KeyCode::Char(' ') => {
                match app.settings_idx {
                    0 => {
                        let cur_i = from_langs.iter().position(|&l| l == app.cfg.translation_from).unwrap_or(0);
                        let next_i = (cur_i + 1) % from_langs.len();
                        app.cfg.translation_from = from_langs[next_i].to_string();
                    }
                    1 => {
                        let cur_i = to_langs.iter().position(|&l| l == app.cfg.translation_to).unwrap_or(0);
                        let next_i = (cur_i + 1) % to_langs.len();
                        app.cfg.translation_to = to_langs[next_i].to_string();
                    }
                    2 => {
                        app.cfg.subs_lang = match app.cfg.subs_lang.as_str() {
                            "ru" => "en".to_string(),
                            "en" => "off".to_string(),
                            _ => "ru".to_string(),
                        };
                    }
                    3 => {
                        let cur_i = gfx_modes.iter().position(|&m| m == app.cfg.graphics_mode).unwrap_or(0);
                        let next_i = (cur_i + 1) % gfx_modes.len();
                        app.cfg.graphics_mode = gfx_modes[next_i].to_string();
                    }
                    4 => {
                        app.cfg.max_res_height = match app.cfg.max_res_height {
                            720 => 1080,
                            1080 => 1440,
                            1440 => 2160,
                            _ => 2160,
                        };
                    }
                    5 => app.cfg.orig_volume = (app.cfg.orig_volume + 0.05).min(1.0),
                    6 => app.cfg.vot_volume = (app.cfg.vot_volume + 0.05).min(2.0),
                    7 => app.cfg.sponsorblock = !app.cfg.sponsorblock,
                    8 => app.cfg.enable_trending = !app.cfg.enable_trending,
                    9 => app.cfg.enable_history = !app.cfg.enable_history,
                    _ => {}
                }
            }
            _ => {}
        }
        return false;
    }

    if app.is_typing_search {
        match key.code {
            KeyCode::Enter => {
                app.trigger_search();
            }
            KeyCode::Esc => {
                app.is_typing_search = false;
            }
            KeyCode::Backspace => {
                app.query_input.pop();
            }
            KeyCode::Char(c)
                if !c.is_control()
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                app.query_input.push(c);
            }
            _ => {}
        }
        return false;
    }

    match key.code {
        KeyCode::Char('q') => {
            return true;
        }
        KeyCode::Esc => {
            if !app.query_input.is_empty() {
                app.query_input.clear();
                app.show_trending();
            } else {
                return true;
            }
        }
        KeyCode::Char('s') => {
            app.show_settings = true;
        }
        KeyCode::Char('/') => {
            app.is_typing_search = true;
        }
        // Реки / Trending
        KeyCode::Char('t') => {
            app.show_trending();
        }
        // Related
        KeyCode::Char('r') => {
            app.show_related();
        }
        // AI Summary
        KeyCode::Char('x') => {
            app.generate_summary();
        }
        // History
        KeyCode::Char('h') => {
            app.show_history();
        }
        // Bookmarks
        KeyCode::Char('b') => {
            if app.items.is_empty() {
                app.show_bookmarks();
            } else {
                app.toggle_current_bookmark();
            }
        }
        // Channel
        KeyCode::Char('u') | KeyCode::Char('c') => {
            app.open_channel();
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if !app.items.is_empty() && app.selected_idx + 1 < app.items.len() {
                app.selected_idx += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.selected_idx > 0 {
                app.selected_idx -= 1;
            }
        }
        KeyCode::Enter => {
            if let Some(item) = app.items.get(app.selected_idx) {
                let id = item.id.clone();
                let title = item.title.clone();
                app.record_history();
                Player::play_video(&id, &app.cfg);
                app.set_status(&format!("Playing: {}", title));
            }
        }
        KeyCode::Char('v') => {
            if let Some(item) = app.items.get(app.selected_idx) {
                let id = item.id.clone();
                let title = item.title.clone();
                app.record_history();
                let tx = app.tx.clone();
                app.set_status(&format!(
                    "Translating [{} → {}]: {}",
                    app.cfg.translation_from.to_uppercase(),
                    app.cfg.translation_to.to_uppercase(),
                    title
                ));
                app.progress = Some((0.15, "Connecting to VOT service...".to_string()));
                app.progress_time = Instant::now();

                let cfg_clone = app.cfg.clone();
                Player::play_with_vot(&id, cfg_clone, move |ratio, msg| {
                    let _ = tx.send(AppEvent::ProgressUpdate(ratio, msg.to_string()));
                });
            }
        }
        KeyCode::Char('a') => {
            if let Some(item) = app.items.get(app.selected_idx) {
                let id = item.id.clone();
                let title = item.title.clone();
                app.record_history();
                Player::play_audio_only(&id);
                app.set_status(&format!("Audio stream started: {}", title));
            }
        }
        KeyCode::Char('y') => {
            if let Some(item) = app.items.get(app.selected_idx) {
                if Player::copy_url(&item.id) {
                    app.set_status("URL copied to clipboard!");
                }
            }
        }
        _ => {}
    }

    false
}
