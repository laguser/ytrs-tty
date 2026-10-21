# ytrs ⚡

> **Fast, ultra-minimalist YouTube CLI & TUI client in Rust with true picture thumbnails (Kitty/Sixel/Chafa), real-time Voice-Over translation, channel browser, and smart timeline summaries.**

---

### Features

- 🖼️ **Real Picture Previews**: Renders true bitmap thumbnails directly in terminal via modern graphics protocols (**Kitty Graphics**, **Sixel**, **iTerm2**) and crisp high-resolution Unicode symbols fallback using `chafa`.
- 🌐 **Instant Voice-Over Translation (VOT)**: Stream with parallel AI voice translation (from English to Russian by default, or configurable to any language pair) mixed with original audio track at customizable volumes.
- 🔥 **Рекомендации / Trending Feed**: Opens directly with trending/recommended videos on launch, with quick hotkeys (`t` for feed, `r` for related videos).
- 📺 **Channel Browser**: Press `u` or `c` to instantly inspect any creator's channel videos.
- 🧠 **Smart AI Summaries (TL;DR)**: Semantic subtitle extraction that filters out sponsor fluff and creates chronological key takeaways with timestamps (`[MM:SS]`).
- ⚡ **Zero-bloat & Fast**: Native Rust binary built on `ratatui` and `crossterm`. Uses `mpv` for smooth hardware-accelerated playback and SponsorBlock integration.

---

### Keybindings

| Key | Action |
| :--- | :--- |
| `enter` | Watch original video in `mpv` |
| `v` | Watch with Voice-Over Translation (VOT) + Subtitles |
| `a` | Listen to audio stream only (no video window) |
| `t` | Browse Recommendations / Trending (Реки) |
| `r` | View Related videos for current selection |
| `u` / `c` | Browse Channel of selected video |
| `x` | Generate Smart Summary (TL;DR with timestamps) |
| `b` | Toggle Bookmark / View Bookmarks |
| `h` | View History |
| `s` | Open Settings & Language configuration |
| `/` | Focus search bar |
| `j` / `k` (or `↓` / `↑`) | Navigate videos |
| `q` / `esc` | Quit / clear search |

---

### Installation

#### Using Cargo
```bash
cargo install ytrs
```

#### From Source
```bash
git clone https://github.com/laguser/ytrs.git
cd ytrs
cargo build --release
install -m 755 target/release/ytrs ~/.local/bin/
```

### Dependencies
- `mpv` (for hardware-accelerated video/audio playback)
- `chafa` (for true terminal picture thumbnail rendering)
- `vot-cli-live` (optional, for Yandex voice-over stream resolution)
