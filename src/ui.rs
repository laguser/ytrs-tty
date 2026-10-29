use crate::config::AppConfig;
use crate::search::VideoItem;
use crate::theme::SystemPalette;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

#[allow(clippy::too_many_arguments)]
pub fn render(
    f: &mut Frame,
    query_input: &str,
    is_typing_search: bool,
    is_searching: bool,
    items: &[VideoItem],
    selected_idx: usize,
    status_msg: &str,
    progress: Option<(f32, String)>,
    show_settings: bool,
    settings_idx: usize,
    show_download: Option<usize>,
    summary_text: Option<&[String]>,
    is_bookmarked: bool,
    has_thumbnail: bool,
    cfg: &AppConfig,
    palette: &SystemPalette,
) -> Option<Rect> {
    let size = f.area();
    if size.width < 45 || size.height < 12 {
        let p = Paragraph::new("Terminal too small for ytrs (needs at least 45x12)")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(p, size);
        return None;
    }

    // Stable 3-row layout structure: zero layout jumping on progress or search!
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top search bar
            Constraint::Fill(1),   // Main content (List + Preview)
            Constraint::Length(1), // Bottom unified status / progress / hotkeys bar
        ])
        .split(size);

    // Top Search / Header: modern minimal line with subtle cues
    render_search_bar(f, query_input, is_typing_search, is_searching, chunks[0], palette, cfg);

    let mut thumb_rect = None;
    if items.is_empty() && !is_searching {
        render_home_screen(f, chunks[1], cfg, palette);
    } else {
        thumb_rect = render_results_area(
            f,
            items,
            selected_idx,
            chunks[1],
            summary_text,
            is_bookmarked,
            has_thumbnail,
            cfg,
            palette,
        );
    }

    render_bottom_bar(
        f,
        status_msg,
        items.is_empty(),
        progress,
        is_searching,
        cfg,
        chunks[2],
        palette,
    );

    if show_settings {
        render_settings_modal(f, settings_idx, cfg, palette);
    }

    if let Some(dl_idx) = show_download {
        render_download_modal(f, dl_idx, items.get(selected_idx), cfg, palette);
    }

    thumb_rect
}

fn render_search_bar(
    f: &mut Frame,
    input: &str,
    is_typing: bool,
    is_searching: bool,
    area: Rect,
    p: &SystemPalette,
    cfg: &AppConfig,
) {
    let prompt_icon = if is_typing { "⌕ " } else { "› " };
    let prompt_color = if is_typing { p.primary } else { p.muted };

    let mut left_spans = vec![
        Span::styled(format!("  {}", prompt_icon), Style::default().fg(prompt_color).add_modifier(Modifier::BOLD)),
    ];

    if input.is_empty() && !is_typing {
        let hint = if cfg.enable_trending {
            "Search or press '/' · [t] реки · [s] settings"
        } else {
            "Search or press '/' · [s] settings"
        };
        left_spans.push(Span::styled(
            hint,
            Style::default().fg(p.muted),
        ));
    } else {
        left_spans.push(Span::styled(
            input,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
        if is_typing {
            left_spans.push(Span::styled("▋", Style::default().fg(p.primary)));
        }
    }

    if is_searching {
        left_spans.push(Span::styled(
            "   [fetching...]",
            Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD),
        ));
    }

    let right_info = format!("VOT: {} → {}  ", cfg.translation_from.to_uppercase(), cfg.translation_to.to_uppercase());
    let left_len = left_spans.iter().map(|s| s.content.chars().count()).sum::<usize>();
    let total_w = area.width as usize;

    if total_w > left_len + right_info.chars().count() + 2 {
        let pad_w = total_w.saturating_sub(left_len + right_info.chars().count());
        left_spans.push(Span::raw(" ".repeat(pad_w)));
        left_spans.push(Span::styled(
            right_info,
            Style::default().fg(p.muted),
        ));
    }

    let p_widget = Paragraph::new(Line::from(left_spans));
    f.render_widget(p_widget, area);
}

fn render_home_screen(f: &mut Frame, area: Rect, cfg: &AppConfig, p: &SystemPalette) {
    let mut lines = Vec::new();
    let pad = (area.height as usize).saturating_sub(6) / 2;
    for _ in 0..pad {
        lines.push(Line::from(""));
    }

    lines.push(
        Line::from(vec![
            Span::styled("ytrs ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("· Minimalist YouTube TUI", Style::default().fg(p.muted)),
        ])
        .alignment(Alignment::Center),
    );
    lines.push(Line::from(""));

    let mut actions = vec![
        Span::styled("[/]", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
        Span::styled(" search  ·  ", Style::default().fg(p.muted)),
    ];
    if cfg.enable_trending {
        actions.push(Span::styled("[t]", Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD)));
        actions.push(Span::styled(" реки / trending  ·  ", Style::default().fg(p.muted)));
    }
    if cfg.enable_history {
        actions.push(Span::styled("[h]", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)));
        actions.push(Span::styled(" history  ·  ", Style::default().fg(p.muted)));
    }
    actions.push(Span::styled("[s]", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)));
    actions.push(Span::styled(" settings", Style::default().fg(p.muted)));

    lines.push(Line::from(actions).alignment(Alignment::Center));

    f.render_widget(Paragraph::new(lines), area);
}

#[allow(clippy::too_many_arguments)]
fn render_results_area(
    f: &mut Frame,
    items: &[VideoItem],
    selected_idx: usize,
    area: Rect,
    summary_text: Option<&[String]>,
    is_bookmarked: bool,
    has_thumbnail: bool,
    cfg: &AppConfig,
    p: &SystemPalette,
) -> Option<Rect> {
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55), // Left: Clean video list
            Constraint::Length(1),      // Low-contrast vertical divider
            Constraint::Percentage(45), // Right: Picture Preview & Metadata & Summary
        ])
        .split(area);

    // 1. Render Left list
    render_video_list(f, items, selected_idx, split[0], p);

    // 2. Render Divider
    let mut div_lines = Vec::new();
    for _ in 0..split[1].height {
        div_lines.push(Line::from(Span::styled("│", Style::default().fg(Color::Rgb(36, 38, 42)))));
    }
    f.render_widget(Paragraph::new(div_lines), split[1]);

    // 3. Render Right Pane (Returns inner thumbnail rect for hardware graphic blit)
    render_details_pane(
        f,
        items,
        selected_idx,
        split[2],
        summary_text,
        is_bookmarked,
        has_thumbnail,
        cfg,
        p,
    )
}

fn render_video_list(
    f: &mut Frame,
    items: &[VideoItem],
    selected_idx: usize,
    area: Rect,
    p: &SystemPalette,
) {
    if items.is_empty() {
        let msg = vec![Line::from(Span::styled(
            "   No videos found.",
            Style::default().fg(p.muted),
        ))];
        f.render_widget(Paragraph::new(msg), area);
        return;
    }

    let h = area.height as usize;
    let start_i = selected_idx.saturating_sub(h / 2);
    let end_i = (start_i + h).min(items.len());

    let mut lines = Vec::new();

    for (i, it) in items.iter().enumerate().take(end_i).skip(start_i) {
        let is_sel = i == selected_idx;

        let dur = if it.is_playlist {
            "PLAYLIST".to_string()
        } else if !it.duration.is_empty() {
            it.duration.clone()
        } else {
            "LIVE".to_string()
        };

        let cursor_marker = if is_sel { "› " } else { "  " };

        let total_w = area.width as usize;
        let left_w = 13; // marker (2) + dur (9) + spaces (2)
        let channel_max_w = 16.min(total_w.saturating_sub(25));
        let title_avail_w = total_w.saturating_sub(left_w + channel_max_w + 2);

        let title_clean: String = if it.title.chars().count() > title_avail_w {
            it.title.chars().take(title_avail_w.saturating_sub(1)).collect::<String>() + "…"
        } else {
            it.title.clone()
        };

        let channel_clean: String = if it.channel.chars().count() > channel_max_w {
            it.channel.chars().take(channel_max_w.saturating_sub(1)).collect::<String>() + "…"
        } else {
            it.channel.clone()
        };

        let dur_color = if it.is_playlist { p.primary } else { p.tertiary };

        let row = if is_sel {
            Line::from(vec![
                Span::styled(format!(" {}", cursor_marker), Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<9} ", dur), Style::default().fg(dur_color).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:<width$} ", title_clean, width = title_avail_w),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("@{}", channel_clean), Style::default().fg(p.secondary)),
            ])
        } else {
            Line::from(vec![
                Span::styled(format!(" {}", cursor_marker), Style::default().fg(p.muted)),
                Span::styled(format!("{:<9} ", dur), Style::default().fg(if it.is_playlist { p.primary } else { p.muted })),
                Span::styled(
                    format!("{:<width$} ", title_clean, width = title_avail_w),
                    Style::default().fg(p.on_surface),
                ),
                Span::styled(format!("@{}", channel_clean), Style::default().fg(p.muted)),
            ])
        };

        lines.push(row);
    }

    f.render_widget(Paragraph::new(lines), area);
}

#[allow(clippy::too_many_arguments)]
fn render_details_pane(
    f: &mut Frame,
    items: &[VideoItem],
    selected_idx: usize,
    area: Rect,
    summary_text: Option<&[String]>,
    is_bookmarked: bool,
    has_thumbnail: bool,
    cfg: &AppConfig,
    p: &SystemPalette,
) -> Option<Rect> {
    let item = items.get(selected_idx)?;

    // Calculate preview box dimensions (proportional 16:9 ratio in terminal cells)
    let thumb_w = (area.width.saturating_sub(2)).clamp(20, 48);
    let thumb_h = ((thumb_w as f32 * 0.42).round() as u16).min(area.height.saturating_sub(8)).max(7);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(thumb_h), // Picture preview box
            Constraint::Length(5),       // Metadata & Channel block
            Constraint::Fill(1),         // Smart Summary & Chapters
        ])
        .split(area);

    let thumb_box = chunks[0];
    let meta_box = chunks[1];
    let summary_box = chunks[2];

    // 1. Picture Preview Box
    let preview_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(40, 43, 48)))
        .title(Span::styled(" preview ", Style::default().fg(p.muted)));

    let inner_thumb = preview_block.inner(thumb_box);
    f.render_widget(preview_block, thumb_box);
    f.render_widget(Clear, inner_thumb);

    if !has_thumbnail && cfg.graphics_mode != "off" {
        let load_msg = vec![
            Line::from(""),
            Line::from(Span::styled("   ⠋ loading picture...", Style::default().fg(p.muted))),
        ];
        f.render_widget(Paragraph::new(load_msg), inner_thumb);
    }

    // 2. Metadata & Channel Info Block
    let bm_label = if is_bookmarked { " ★ saved" } else { "" };
    let mut meta_lines = Vec::new();

    meta_lines.push(Line::from(vec![
        Span::styled(&item.title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(bm_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]));

    meta_lines.push(Line::from(vec![
        Span::styled(format!("@{}", item.channel), Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
        Span::styled("  ·  [u] channel", Style::default().fg(p.muted)),
    ]));

    let mut stats_spans = Vec::new();
    if !item.views.is_empty() {
        stats_spans.push(Span::styled(&item.views, Style::default().fg(p.on_surface)));
    }
    if !item.published.is_empty() {
        if !stats_spans.is_empty() {
            stats_spans.push(Span::styled(" · ", Style::default().fg(p.muted)));
        }
        stats_spans.push(Span::styled(&item.published, Style::default().fg(p.muted)));
    }
    if !stats_spans.is_empty() {
        stats_spans.push(Span::styled(" · ", Style::default().fg(p.muted)));
    }
    stats_spans.push(Span::styled(
        format!("VOT: {} → {}", cfg.translation_from.to_uppercase(), cfg.translation_to.to_uppercase()),
        Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD),
    ));

    meta_lines.push(Line::from(stats_spans));

    // Hotkey actions row for current item
    let play_label = if item.is_playlist { "enter play all" } else { "enter play" };
    meta_lines.push(Line::from(vec![
        Span::styled(" [a] ", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
        Span::styled("audio-only  · ", Style::default().fg(p.muted)),
        Span::styled(format!(" [{}] ", play_label), Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD)),
        Span::styled(" · [d] ", Style::default().fg(p.muted)),
        Span::styled("download", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
    ]));

    f.render_widget(Paragraph::new(meta_lines), meta_box);

    // 3. Smart Summary & Chapters Block
    let mut sum_lines = Vec::new();
    sum_lines.push(Line::from(vec![
        Span::styled("tl;dr & chapters ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("·  [x] refresh", Style::default().fg(p.muted)),
    ]));

    if let Some(points) = summary_text {
        for pt in points {
            let mut parts = pt.splitn(2, ' ');
            let first = parts.next().unwrap_or("");
            let rest = parts.next().unwrap_or("");

            if first.starts_with('[') && first.ends_with(']') {
                sum_lines.push(Line::from(vec![
                    Span::styled(format!(" {} ", first), Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD)),
                    Span::styled(rest, Style::default().fg(p.on_surface)),
                ]));
            } else if pt.starts_with("Обзор:") {
                sum_lines.push(Line::from(vec![
                    Span::styled(" · ", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
                    Span::styled(pt, Style::default().fg(Color::White)),
                ]));
            } else {
                sum_lines.push(Line::from(vec![
                    Span::styled(" · ", Style::default().fg(p.primary)),
                    Span::styled(pt, Style::default().fg(p.on_surface)),
                ]));
            }
        }
    } else {
        sum_lines.push(Line::from(Span::styled(
            "   Press [x] to extract chapters & summary...",
            Style::default().fg(p.muted),
        )));
    }

    f.render_widget(Paragraph::new(sum_lines), summary_box);

    Some(inner_thumb)
}

fn render_settings_modal(f: &mut Frame, selected: usize, cfg: &AppConfig, p: &SystemPalette) {
    let area = f.area();
    let modal_w = 70.min(area.width.saturating_sub(4));
    let modal_h = 18.min(area.height.saturating_sub(2));

    let modal_rect = Rect {
        x: (area.width.saturating_sub(modal_w)) / 2,
        y: (area.height.saturating_sub(modal_h)) / 2,
        width: modal_w,
        height: modal_h,
    };

    f.render_widget(Clear, modal_rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" settings & translation ")
        .border_style(Style::default().fg(p.primary));

    let orig_vol_pct = (cfg.orig_volume * 100.0).round() as u32;
    let vot_vol_pct = (cfg.vot_volume * 100.0).round() as u32;

    let rows: Vec<(&str, String)> = vec![
        ("Theme / Color Palette", format!("‹  {}  ›", cfg.theme.name())),
        ("Progress Bar Style", format!("‹  {}  ›", cfg.progress_style.name())),
        ("Audio Source Lang (From)", format!("‹  {}  ›", lang_display(&cfg.translation_from))),
        ("Voiceover Target Lang (To)", format!("‹  {}  ›", lang_display(&cfg.translation_to))),
        ("Subtitles", format!("‹  {}  ›", cfg.subs_lang.to_uppercase())),
        ("Picture Graphics Mode", format!("‹  {}  ›", cfg.graphics_mode)),
        ("Max Resolution", format!("‹  {}p  ›", cfg.max_res_height)),
        ("Original Video Volume", render_slider(orig_vol_pct, 100, cfg.progress_style)),
        ("Voiceover (VOT) Volume", render_slider(vot_vol_pct, 200, cfg.progress_style)),
        ("SponsorBlock (Auto-Skip)", render_toggle(cfg.sponsorblock)),
        ("Trending on Launch (Реки)", render_toggle(cfg.enable_trending)),
        ("History & Bookmarks", render_toggle(cfg.enable_history)),
    ];

    let mut lines = Vec::new();
    lines.push(Line::from(""));

    for (i, (label, val_str)) in rows.iter().enumerate() {
        let is_sel = i == selected;
        let marker = if is_sel { " › " } else { "   " };

        let label_style = if is_sel {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.on_surface)
        };

        let val_style = if is_sel {
            Style::default().fg(p.primary).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.tertiary)
        };

        let label_fmt = format!("{:<28}", label);

        let row = Line::from(vec![
            Span::styled(marker, Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
            Span::styled(label_fmt, label_style),
            Span::styled(" ", Style::default()),
            Span::styled(val_str, val_style),
        ]);
        lines.push(row);
    }

    lines.push(Line::from(""));
    lines.push(
        Line::from(Span::styled(
            "   ↑/↓ select  ·  ←/→/enter adjust  ·  esc/s close & save",
            Style::default().fg(p.muted),
        )),
    );

    let p_widget = Paragraph::new(lines).block(block);
    f.render_widget(p_widget, modal_rect);
}

fn render_download_modal(
    f: &mut Frame,
    selected: usize,
    cur_item: Option<&VideoItem>,
    cfg: &AppConfig,
    p: &SystemPalette,
) {
    let area = f.area();
    let modal_w = 64.min(area.width.saturating_sub(4));
    let modal_h = 12.min(area.height.saturating_sub(2));

    let modal_rect = Rect {
        x: (area.width.saturating_sub(modal_w)) / 2,
        y: (area.height.saturating_sub(modal_h)) / 2,
        width: modal_w,
        height: modal_h,
    };

    f.render_widget(Clear, modal_rect);

    let title = if let Some(it) = cur_item {
        let t: String = it.title.chars().take(35).collect();
        format!(" download: {}… ", t)
    } else {
        " download (yt-dlp) ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .border_style(Style::default().fg(p.primary));

    let options = [
        "1. Video (MP4 / Best 1080p)",
        "2. Audio Only (MP3 / Best 320k)",
        &format!("3. Video + Yandex VOT ({})", cfg.translation_to.to_uppercase()),
    ];

    let mut lines = Vec::new();
    lines.push(Line::from(""));

    for (i, opt) in options.iter().enumerate() {
        let is_sel = i == selected;
        let marker = if is_sel { " › " } else { "   " };
        let style = if is_sel {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(p.on_surface)
        };

        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
            Span::styled(*opt, style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "   1/2/3 or enter to download  ·  esc/d cancel",
        Style::default().fg(p.muted),
    )));

    let p_widget = Paragraph::new(lines).block(block);
    f.render_widget(p_widget, modal_rect);
}

fn lang_display(code: &str) -> String {
    let name = match code {
        "auto" => "Auto Detect",
        "en" => "English",
        "ru" => "Russian",
        "zh" => "Chinese",
        "de" => "German",
        "fr" => "French",
        "es" => "Spanish",
        "ja" => "Japanese",
        "ko" => "Korean",
        "it" => "Italian",
        "kk" => "Kazakh",
        "uk" => "Ukrainian",
        "uz" => "Uzbek",
        _ => code,
    };
    format!("{:<4} ({})", code.to_uppercase(), name)
}

fn render_slider(val: u32, max: u32, style: crate::config::ProgressStyle) -> String {
    let width = 12;
    let ratio = (val as f32 / max as f32).clamp(0.0, 1.0);
    let filled = (width as f32 * ratio).round() as usize;

    let mut bar = String::new();
    match style {
        crate::config::ProgressStyle::Tape => {
            for i in 0..width {
                if i < filled {
                    bar.push('━');
                } else if i == filled {
                    bar.push('●');
                } else {
                    bar.push('─');
                }
            }
        }
        crate::config::ProgressStyle::Blocky => {
            for i in 0..width {
                if i < filled {
                    bar.push('█');
                } else if i == filled {
                    bar.push('▓');
                } else {
                    bar.push('░');
                }
            }
        }
        crate::config::ProgressStyle::Minimal => {
            for i in 0..width {
                if i < filled {
                    bar.push('·');
                } else if i == filled {
                    bar.push('◆');
                } else {
                    bar.push(' ');
                }
            }
        }
    }
    format!("[{:>3}%] {}", val, bar)
}

fn render_toggle(on: bool) -> String {
    if on {
        "[ ON  ]".to_string()
    } else {
        "[ OFF ]".to_string()
    }
}

#[allow(clippy::too_many_arguments)]
fn render_bottom_bar(
    f: &mut Frame,
    status: &str,
    is_empty: bool,
    progress: Option<(f32, String)>,
    is_searching: bool,
    _cfg: &AppConfig,
    area: Rect,
    p: &SystemPalette,
) {
    let w = area.width as usize;

    // 1. If progress or search is active, render sleek conveyor progress bar!
    if let Some((val, ref label)) = progress {
        let line = render_minimal_progress_bar(val, label, w, p, _cfg.progress_style);
        f.render_widget(Paragraph::new(line), area);
        return;
    } else if is_searching {
        let line = render_minimal_progress_bar(0.45, "Loading YouTube...", w, p, _cfg.progress_style);
        f.render_widget(Paragraph::new(line), area);
        return;
    }

    // 2. If transient status message is set:
    if !status.is_empty() {
        let line = Line::from(vec![
            Span::styled("  ⚡ ", Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD)),
            Span::styled(status, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }

    // 3. Clean minimal hotkeys bar with soft dot separators:
    let key_s = Style::default().fg(p.primary).add_modifier(Modifier::BOLD);
    let vot_key_s = Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD);
    let desc_s = Style::default().fg(p.muted);
    let dot = Span::styled(" · ", Style::default().fg(Color::Rgb(45, 48, 52)));

    let spans = if is_empty {
        let mut v = vec![
            Span::styled("  / ", key_s),
            Span::styled("search", desc_s),
            dot.clone(),
        ];
        if _cfg.enable_trending {
            v.push(Span::styled("t ", vot_key_s));
            v.push(Span::styled("реки", desc_s));
            v.push(dot.clone());
        }
        v.extend(vec![
            Span::styled("h ", key_s),
            Span::styled("history", desc_s),
            dot.clone(),
            Span::styled("s ", vot_key_s),
            Span::styled("settings", desc_s),
            dot.clone(),
            Span::styled("q ", key_s),
            Span::styled("quit", desc_s),
        ]);
        v
    } else {
        let mut v = vec![
            Span::styled("  v ", vot_key_s),
            Span::styled("watch (vot)", desc_s),
            dot.clone(),
            Span::styled("enter ", key_s),
            Span::styled("play", desc_s),
            dot.clone(),
        ];
        if _cfg.enable_trending {
            v.push(Span::styled("t ", vot_key_s));
            v.push(Span::styled("реки", desc_s));
            v.push(dot.clone());
        }
        v.extend(vec![
            Span::styled("r ", key_s),
            Span::styled("related", desc_s),
            dot.clone(),
            Span::styled("x ", vot_key_s),
            Span::styled("summary", desc_s),
            dot.clone(),
            Span::styled("u ", key_s),
            Span::styled("channel", desc_s),
            dot.clone(),
            Span::styled("b ", key_s),
            Span::styled("bookmark", desc_s),
            dot.clone(),
            Span::styled("d ", vot_key_s),
            Span::styled("download", desc_s),
            dot.clone(),
            Span::styled("s ", vot_key_s),
            Span::styled("settings", desc_s),
            dot.clone(),
            Span::styled("q ", key_s),
            Span::styled("quit", desc_s),
        ]);
        v
    };

    let p_widget = Paragraph::new(Line::from(spans));
    f.render_widget(p_widget, area);
}

fn render_minimal_progress_bar(
    progress: f32,
    label: &str,
    total_w: usize,
    p: &SystemPalette,
    style: crate::config::ProgressStyle,
) -> Line<'static> {
    let ratio = progress.clamp(0.0, 1.0) as f64;
    let pct = (ratio * 100.0) as u32;

    let bar_w = 24.min(total_w.saturating_sub(label.chars().count() + 18)).max(8);
    let filled_w = (bar_w as f64 * ratio).round() as usize;

    let mut spans = Vec::new();
    spans.push(Span::styled("  ⠋ ", Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD)));
    spans.push(Span::styled(
        format!("{:<width$}  ", label, width = label.chars().count().min(35)),
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::styled("[", Style::default().fg(Color::Rgb(60, 64, 70))));

    match style {
        crate::config::ProgressStyle::Tape => {
            for i in 0..bar_w {
                if i < filled_w {
                    spans.push(Span::styled("━", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)));
                } else if i == filled_w {
                    spans.push(Span::styled("●", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
                } else {
                    spans.push(Span::styled("─", Style::default().fg(Color::Rgb(45, 48, 52))));
                }
            }
        }
        crate::config::ProgressStyle::Blocky => {
            for i in 0..bar_w {
                if i < filled_w {
                    spans.push(Span::styled("█", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)));
                } else if i == filled_w {
                    spans.push(Span::styled("▓", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
                } else {
                    spans.push(Span::styled("░", Style::default().fg(Color::Rgb(45, 48, 52))));
                }
            }
        }
        crate::config::ProgressStyle::Minimal => {
            for i in 0..bar_w {
                if i < filled_w {
                    spans.push(Span::styled("·", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)));
                } else if i == filled_w {
                    spans.push(Span::styled("◆", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)));
                } else {
                    spans.push(Span::styled(" ", Style::default().fg(Color::Rgb(45, 48, 52))));
                }
            }
        }
    }

    spans.push(Span::styled("] ", Style::default().fg(Color::Rgb(60, 64, 70))));
    spans.push(Span::styled(
        format!("{:>3}%", pct),
        Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD),
    ));

    Line::from(spans)
}
