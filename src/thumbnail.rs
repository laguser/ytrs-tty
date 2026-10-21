use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone, Debug)]
pub struct ThumbnailOutput {
    pub is_graphic: bool, // true for Kitty / Sixel / iTerm bitmaps
    pub raw_data: Vec<u8>,
    pub symbol_lines: Vec<String>, // for character/symbols fallback
}

type ThumbReq = (String, u16, u16, String);

pub struct ThumbnailManager {
    cache: Arc<Mutex<HashMap<String, ThumbnailOutput>>>,
    req_tx: Sender<ThumbReq>,
}

impl ThumbnailManager {
    pub fn new(ready_tx: Sender<String>) -> Self {
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let (req_tx, req_rx): (Sender<ThumbReq>, Receiver<ThumbReq>) = channel();

        let cache_clone = Arc::clone(&cache);
        thread::spawn(move || {
            let mut in_flight: HashSet<String> = HashSet::new();

            while let Ok((video_id, w, h, proto_cfg)) = req_rx.recv() {
                // Check if already cached
                {
                    let map = cache_clone.lock().unwrap();
                    if map.contains_key(&video_id) {
                        continue;
                    }
                }

                if in_flight.contains(&video_id) {
                    continue;
                }
                in_flight.insert(video_id.clone());

                let cache_ref = Arc::clone(&cache_clone);
                let ready_sender = ready_tx.clone();
                let vid = video_id.clone();

                thread::spawn(move || {
                    let out = render_thumbnail(&vid, w, h, &proto_cfg);
                    if let Some(thumb) = out {
                        let mut map = cache_ref.lock().unwrap();
                        map.insert(vid.clone(), thumb);
                        let _ = ready_sender.send(vid);
                    }
                });
            }
        });

        Self { cache, req_tx }
    }

    pub fn request(&self, video_id: &str, width: u16, height: u16, mode: &str) {
        let map = self.cache.lock().unwrap();
        if !map.contains_key(video_id) {
            drop(map);
            let _ = self.req_tx.send((video_id.to_string(), width, height, mode.to_string()));
        }
    }

    pub fn get(&self, video_id: &str) -> Option<ThumbnailOutput> {
        let map = self.cache.lock().unwrap();
        map.get(video_id).cloned()
    }
}

pub fn detect_graphics_format(cfg_mode: &str) -> &'static str {
    if cfg_mode != "auto" {
        return match cfg_mode {
            "kitty" => "kitty",
            "sixels" => "sixels",
            "iterm" => "iterm",
            "symbols" => "symbols",
            "off" => "off",
            _ => "symbols",
        };
    }

    // Auto-detect modern true-pixel graphics terminal protocols
    if std::env::var("KITTY_PID").is_ok()
        || std::env::var("KITTY_WINDOW_ID").is_ok()
        || std::env::var("GHOSTTY_RESOURCES_DIR").is_ok()
        || std::env::var("WEZTERM_PANE").is_ok()
        || std::env::var("TERM").map(|t| t.contains("kitty")).unwrap_or(false)
    {
        return "kitty";
    }

    if std::env::var("TERM_PROGRAM").map(|p| p == "iTerm.app").unwrap_or(false) {
        return "iterm";
    }

    if std::env::var("TERM").map(|t| t.contains("sixel") || t.contains("foot") || t.contains("mlterm")).unwrap_or(false) {
        return "sixels";
    }

    "symbols"
}

fn cache_dir() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push("ytrs_cache");
    let _ = fs::create_dir_all(&p);
    p
}

pub fn download_thumbnail(video_id: &str) -> Option<PathBuf> {
    let mut path = cache_dir();
    path.push(format!("{}.jpg", video_id));

    if path.exists() {
        return Some(path);
    }

    let url = format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", video_id);
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(4))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0")
        .build();

    let resp = agent.get(&url).call().ok()?;
    let mut bytes = Vec::new();
    let mut reader = resp.into_reader();
    let _ = std::io::copy(&mut reader, &mut bytes);

    if !bytes.is_empty() && fs::write(&path, &bytes).is_ok() {
        return Some(path);
    }

    None
}

pub fn render_thumbnail(video_id: &str, w: u16, h: u16, mode: &str) -> Option<ThumbnailOutput> {
    let proto = detect_graphics_format(mode);
    if proto == "off" {
        return None;
    }

    let img_path = download_thumbnail(video_id)?;

    // Clamp dimensions to reasonable preview box
    let req_w = w.clamp(10, 60);
    let req_h = h.clamp(5, 25);
    let size_arg = format!("{}x{}", req_w, req_h);

    let output = Command::new("chafa")
        .arg("-f")
        .arg(proto)
        .arg("-s")
        .arg(&size_arg)
        .arg(&img_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let is_graphic = proto == "kitty" || proto == "sixels" || proto == "iterm";

    if is_graphic {
        Some(ThumbnailOutput {
            is_graphic: true,
            raw_data: output.stdout,
            symbol_lines: Vec::new(),
        })
    } else {
        let text = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
        Some(ThumbnailOutput {
            is_graphic: false,
            raw_data: output.stdout,
            symbol_lines: lines,
        })
    }
}

/// Helper to blit a graphic or symbols directly into terminal at specified row, col (1-indexed)
pub fn draw_thumbnail_to_terminal(thumb: &ThumbnailOutput, x: u16, y: u16) {
    let mut stdout = std::io::stdout();

    if thumb.is_graphic {
        // Position cursor, write raw escape bytes, and restore
        let _ = write!(stdout, "\x1b7\x1b[{};{}H", y + 1, x + 1);
        let _ = stdout.write_all(&thumb.raw_data);
        let _ = write!(stdout, "\x1b8");
        let _ = stdout.flush();
    } else {
        // Line-by-line ANSI positioning for symbols so lines do not wrap to column 1
        let _ = write!(stdout, "\x1b7");
        for (i, line) in thumb.symbol_lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let _ = write!(stdout, "\x1b[{};{}H{}", y + 1 + i as u16, x + 1, line);
        }
        let _ = write!(stdout, "\x1b8");
        let _ = stdout.flush();
    }
}
