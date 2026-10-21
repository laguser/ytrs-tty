use crate::config::AppConfig;
use crate::search::VideoItem;
use crate::theme::SystemPalette;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Gauge, Paragraph},
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

    let has_progress = progress.is_some() || is_searching;

    let constraints = if has_progress {
        vec![
            Constraint::Length(1), // Top search bar
            Constraint::Fill(1),   // Main content
            Constraint::Length(1), // Progress bar
            Constraint::Length(1), // Bottom hotkey status bar
        ]
    } else {
        vec![
            Constraint::Length(1), // Top search bar
            Constraint::Fill(1),   // Main content
            Constraint::Length(1), // Bottom hotkey status bar
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(size);

    render_search_bar(f, query_input, is_typing_search, is_searching, chunks[0], palette);

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

    if has_progress {
        let p_chunk = chunks[2];
        let status_chunk = chunks[3];
        if let Some((val, label)) = progress {
            render_progress_bar(f, val, &label, p_chunk, palette);
        } else if is_searching {
            render_progress_bar(f, 0.45, "Loading YouTube...", p_chunk, palette);
        }
        render_bottom_bar(f, status_msg, items.is_empty(), cfg, status_chunk, palette);
    } else {
        render_bottom_bar(f, status_msg, items.is_empty(), cfg, chunks[2], palette);
    }

    if show_settings {
        render_settings_modal(f, settings_idx, cfg, palette);
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
) {
    let mut spans = vec![
        Span::styled(" ❯ ", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
        Span::styled("yt", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(" ", Style::default()),
    ];

    if input.is_empty() && !is_typing {
        spans.push(Span::styled(
            "Search or [/]  •  [t] реки/trending  •  [r] related  •  [s] settings",
            Style::default().fg(p.muted),
        ));
    } else {
        spans.push(Span::styled(
            input,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
        if is_typing {
            spans.push(Span::styled("▋", Style::default().fg(p.primary)));
        }
    }

    if is_searching {
        spans.push(Span::styled(
            "   [fetching...]",
            Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD),
        ));
    }

    let p_widget = Paragraph::new(Line::from(spans));
    f.render_widget(p_widget, area);
}

fn render_progress_bar(f: &mut Frame, progress: f32, label: &str, area: Rect, p: &SystemPalette) {
    let ratio = progress.clamp(0.0, 1.0) as f64;
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(p.primary).bg(Color::Rgb(24, 26, 28)))
        .ratio(ratio)
        .label(Span::styled(
            format!(" {} ({:.0}%)", label, ratio * 100.0),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
    f.render_widget(gauge, area);
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
            Span::styled("• Minimalist YouTube TUI", Style::default().fg(p.muted)),
        ])
        .alignment(Alignment::Center),
    );
    lines.push(Line::from(""));

    let mut actions = vec![
        Span::styled("[/]", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
        Span::styled(" Search  •  ", Style::default().fg(p.muted)),
    ];
    if cfg.enable_trending {
        actions.push(Span::styled("[t]", Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD)));
        actions.push(Span::styled(" Реки / Trending  •  ", Style::default().fg(p.muted)));
    }
    if cfg.enable_history {
        actions.push(Span::styled("[h]", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)));
        actions.push(Span::styled(" History  •  ", Style::default().fg(p.muted)));
    }
    actions.push(Span::styled("[s]", Style::default().fg(p.primary).add_modifier(Modifier::BOLD)));
    actions.push(Span::styled(" Settings", Style::default().fg(p.muted)));

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
            Constraint::Percentage(54), // Left: Video list
            Constraint::Length(1),      // Thin vertical divider
            Constraint::Percentage(46), // Right: Picture Preview & Metadata & Summary
        ])
        .split(area);

    // 1. Render Left list
    render_video_list(f, items, selected_idx, split[0], p);

    // 2. Render Divider
    let mut div_lines = Vec::new();
    for _ in 0..split[1].height {
        div_lines.push(Line::from(Span::styled("│", Style::default().fg(Color::Rgb(40, 42, 45)))));
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

        let dur = if !it.duration.is_empty() {
            format!("{:<6}", it.duration)
        } else {
            "LIVE  ".to_string()
        };

        let prefix = if is_sel { "▎" } else { " " };

        // Available width for title and channel
        let total_w = area.width as usize;
        let left_prefix_w = 9; // prefix + dur + spaces
        let channel_max_w = 18.min(total_w.saturating_sub(25));
        let title_avail_w = total_w.saturating_sub(left_prefix_w + channel_max_w + 2);

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

        let row = if is_sel {
            Line::from(vec![
                Span::styled(prefix, Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" {:<6} ", dur), Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:<width$} ", title_clean, width = title_avail_w),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
                Span::styled(channel_clean, Style::default().fg(p.primary)),
            ])
        } else {
            Line::from(vec![
                Span::raw(prefix),
                Span::styled(format!(" {:<6} ", dur), Style::default().fg(p.muted)),
                Span::styled(
                    format!("{:<width$} ", title_clean, width = title_avail_w),
                    Style::default().fg(p.on_surface),
                ),
                Span::styled(channel_clean, Style::default().fg(p.muted)),
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
    let thumb_w = (area.width.saturating_sub(2)).clamp(20, 46);
    let thumb_h = ((thumb_w as f32 * 0.42).round() as u16).min(area.height.saturating_sub(8)).max(7);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(thumb_h), // Picture preview box
            Constraint::Length(5),       // Metadata & Channel block
            Constraint::Fill(1),         // Smart Summary & TL;DR
        ])
        .split(area);

    let thumb_box = chunks[0];
    let meta_box = chunks[1];
    let summary_box = chunks[2];

    // 1. Picture Preview Box
    let preview_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(50, 52, 55)))
        .title(Span::styled(" Preview ", Style::default().fg(p.muted)));

    let inner_thumb = preview_block.inner(thumb_box);
    f.render_widget(preview_block, thumb_box);
    f.render_widget(Clear, inner_thumb);

    if !has_thumbnail && cfg.graphics_mode != "off" {
        let load_msg = vec![
            Line::from(""),
            Line::from(Span::styled("   Loading picture...", Style::default().fg(p.muted))),
        ];
        f.render_widget(Paragraph::new(load_msg), inner_thumb);
    }

    // 2. Metadata & Channel Info Block
    let bm_label = if is_bookmarked { " ★ Saved" } else { "" };
    let mut meta_lines = Vec::new();

    meta_lines.push(Line::from(vec![
        Span::styled(&item.title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(bm_label, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]));

    let av_icon = if item.channel_avatar.is_some() { "◈ " } else { "● " };
    meta_lines.push(Line::from(vec![
        Span::styled(av_icon, Style::default().fg(p.tertiary)),
        Span::styled(&item.channel, Style::default().fg(p.primary).add_modifier(Modifier::BOLD)),
        Span::styled("  [u: View Channel]", Style::default().fg(p.muted)),
    ]));

    let mut stats_spans = Vec::new();
    if !item.views.is_empty() {
        stats_spans.push(Span::styled(&item.views, Style::default().fg(p.on_surface)));
    }
    if !item.published.is_empty() {
        if !stats_spans.is_empty() {
            stats_spans.push(Span::styled(" • ", Style::default().fg(p.muted)));
        }
        stats_spans.push(Span::styled(&item.published, Style::default().fg(p.muted)));
    }
    if !stats_spans.is_empty() {
        stats_spans.push(Span::styled(" • ", Style::default().fg(p.muted)));
    }
    stats_spans.push(Span::styled(
        format!("VOT: {} → {}", cfg.translation_from.to_uppercase(), cfg.translation_to.to_uppercase()),
        Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD),
    ));

    meta_lines.push(Line::from(stats_spans));

    f.render_widget(Paragraph::new(meta_lines), meta_box);

    // 3. Smart Summary Block
    let mut sum_lines = Vec::new();
    sum_lines.push(Line::from(vec![
        Span::styled("TL;DR Summary ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("[x: Refresh]", Style::default().fg(p.muted)),
    ]));

    if let Some(points) = summary_text {
        for pt in points {
            sum_lines.push(Line::from(vec![
                Span::styled(" • ", Style::default().fg(p.primary)),
                Span::styled(pt, Style::default().fg(p.on_surface)),
            ]));
        }
    } else {
        sum_lines.push(Line::from(Span::styled(
            "   Press [x] to generate key takeaways with timestamps...",
            Style::default().fg(p.muted),
        )));
    }

    f.render_widget(Paragraph::new(sum_lines), summary_box);

    Some(inner_thumb)
}

fn render_settings_modal(f: &mut Frame, selected: usize, cfg: &AppConfig, p: &SystemPalette) {
    let area = f.area();
    let modal_w = 64.min(area.width.saturating_sub(4));
    let modal_h = 22.min(area.height.saturating_sub(2));

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
        .title(" Settings & Languages (↑/↓, ←/→ to cycle, [Esc/s] save) ")
        .border_style(Style::default().fg(p.primary));

    let items = vec![
        format!("Source Audio Lang (From):  {:<4} (←/→ cycle)", cfg.translation_from.to_uppercase()),
        format!("Target Voice Lang (To):    {:<4} (←/→ cycle)", cfg.translation_to.to_uppercase()),
        format!("Subtitles Language:        {:<4} (ru / en / off)", cfg.subs_lang),
        format!("Graphics Picture Protocol: {:<7} (auto/kitty/sixel/symbols)", cfg.graphics_mode),
        format!("Max Video Resolution:     {}p (720 / 1080 / 1440 / 2160)", cfg.max_res_height),
        format!("Original Video Volume:    {:<3.0}% (←/→ adjust)", cfg.orig_volume * 100.0),
        format!("Voice Translation Volume: {:<3.0}% (←/→ adjust)", cfg.vot_volume * 100.0),
        format!("SponsorBlock (Auto-Skip): {}", if cfg.sponsorblock { "Enabled" } else { "Disabled" }),
        format!("Recommendations (Реки):   {}", if cfg.enable_trending { "Enabled [ON]" } else { "Disabled" }),
        format!("History & Bookmarks:      {}", if cfg.enable_history { "Enabled [ON]" } else { "Disabled" }),
    ];

    let mut lines = Vec::new();
    lines.push(Line::from(""));
    for (i, it) in items.iter().enumerate() {
        let is_sel = i == selected;
        let line = if is_sel {
            Line::from(vec![
                Span::styled(" ▶ ", Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD)),
                Span::styled(it, Style::default().fg(Color::Black).bg(p.primary).add_modifier(Modifier::BOLD)),
            ])
        } else {
            Line::from(vec![
                Span::raw("   "),
                Span::styled(it, Style::default().fg(Color::White)),
            ])
        };
        lines.push(line);
    }

    let p_widget = Paragraph::new(lines).block(block);
    f.render_widget(p_widget, modal_rect);
}

fn render_bottom_bar(f: &mut Frame, status: &str, is_empty: bool, _cfg: &AppConfig, area: Rect, p: &SystemPalette) {
    let key_s = Style::default().fg(p.primary).add_modifier(Modifier::BOLD);
    let vot_key_s = Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD);
    let desc_s = Style::default().fg(p.muted);

    let spans = if !status.is_empty() {
        vec![
            Span::styled(" ⚡ ", Style::default().fg(p.tertiary).add_modifier(Modifier::BOLD)),
            Span::styled(status, Style::default().fg(p.on_surface).add_modifier(Modifier::BOLD)),
        ]
    } else if is_empty {
        vec![
            Span::styled(" / ", key_s),
            Span::styled("search  •  ", desc_s),
            Span::styled("t ", vot_key_s),
            Span::styled("реки/trending  •  ", desc_s),
            Span::styled("h ", key_s),
            Span::styled("history  •  ", desc_s),
            Span::styled("s ", vot_key_s),
            Span::styled("settings  •  ", desc_s),
            Span::styled("q ", key_s),
            Span::styled("quit", desc_s),
        ]
    } else {
        vec![
            Span::styled(" v ", vot_key_s),
            Span::styled("watch (VOT)  •  ", desc_s),
            Span::styled("enter ", key_s),
            Span::styled("original  •  ", desc_s),
            Span::styled("t ", vot_key_s),
            Span::styled("реки  •  ", desc_s),
            Span::styled("r ", key_s),
            Span::styled("related  •  ", desc_s),
            Span::styled("u ", key_s),
            Span::styled("channel  •  ", desc_s),
            Span::styled("x ", vot_key_s),
            Span::styled("summary  •  ", desc_s),
            Span::styled("b ", key_s),
            Span::styled("bookmark  •  ", desc_s),
            Span::styled("s ", vot_key_s),
            Span::styled("settings  •  ", desc_s),
            Span::styled("q ", key_s),
            Span::styled("quit", desc_s),
        ]
    };

    let p_widget = Paragraph::new(Line::from(spans));
    f.render_widget(p_widget, area);
}
