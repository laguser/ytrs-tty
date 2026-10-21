use std::fs;
use std::process::Command;

pub struct Summarizer;

impl Summarizer {
    /// Extracts a genuine, structured summary:
    /// 1. Creator chapters & timeline topics (from metadata/description)
    /// 2. Clean video synopsis & overview (from description)
    /// 3. Concept & topic extraction from speech transcript
    pub fn extract_video_summary(
        yt_url: &str,
        video_id: &str,
        from_lang: &str,
        to_lang: &str,
    ) -> Vec<String> {
        let mut results = Vec::new();

        // 1. Fetch metadata via yt-dlp to inspect official chapters and description
        if let Some((chapters, synopsis)) = Self::fetch_metadata_chapters_and_synopsis(yt_url) {
            if !chapters.is_empty() {
                results.extend(chapters);
                if let Some(syn) = synopsis {
                    if results.len() <= 5 {
                        results.push(format!("Обзор: {}", syn));
                    }
                }
                return results;
            }

            if let Some(syn) = synopsis {
                results.push(format!("Обзор: {}", syn));
            }
        }

        // 2. If no official chapters, try extracting key topics from subtitles
        if let Some(srt_path) = crate::vot::VotManager::fetch_and_save_srt(yt_url, video_id, from_lang, to_lang) {
            if let Ok(content) = fs::read_to_string(&srt_path) {
                let speech_topics = Self::extract_topics_from_subtitles(&content);
                if !speech_topics.is_empty() {
                    results.extend(speech_topics);
                }
            }
        }

        if results.is_empty() {
            vec!["Нет доступных глав или описания для этого видео.".to_string()]
        } else {
            results
        }
    }

    /// Inspect video metadata for official chapters or timestamped topics in description
    fn fetch_metadata_chapters_and_synopsis(yt_url: &str) -> Option<(Vec<String>, Option<String>)> {
        let output = Command::new("yt-dlp")
            .arg("--dump-single-json")
            .arg("--skip-download")
            .arg(yt_url)
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let raw = String::from_utf8_lossy(&output.stdout);
        let data: serde_json::Value = serde_json::from_str(&raw).ok()?;

        let mut chapters_out = Vec::new();

        // Check official video chapters
        if let Some(chapters) = data.get("chapters").and_then(|c| c.as_array()) {
            for ch in chapters {
                let start_sec = ch.get("start_time").and_then(|t| t.as_f64()).unwrap_or(0.0) as u64;
                let title = ch.get("title").and_then(|t| t.as_str()).unwrap_or("").trim();
                if !title.is_empty() {
                    let m = start_sec / 60;
                    let s = start_sec % 60;
                    chapters_out.push(format!("[{:02}:{:02}] {}", m, s, title));
                }
            }
        }

        let desc = data.get("description").and_then(|d| d.as_str()).unwrap_or("");

        // If no official chapters, look for timestamps in description (e.g. "01:23 Topic Name")
        if chapters_out.is_empty() && !desc.is_empty() {
            for line in desc.lines() {
                let trimmed = line.trim();
                if let Some((ts, topic)) = Self::parse_description_timecode(trimmed) {
                    chapters_out.push(format!("[{}] {}", ts, topic));
                    if chapters_out.len() >= 8 {
                        break;
                    }
                }
            }
        }

        // Extract a clean, meaningful synopsis paragraph from description (ignoring links/ads)
        let mut synopsis = None;
        if !desc.is_empty() {
            for para in desc.split("\n\n") {
                let p = para.trim();
                if p.is_empty()
                    || p.contains("http://")
                    || p.contains("https://")
                    || p.contains("t.me/")
                    || p.contains("vk.com/")
                    || p.contains("patreon")
                {
                    continue;
                }

                let clean_lines: Vec<&str> = p
                    .lines()
                    .map(|l| l.trim())
                    .filter(|l| {
                        !l.is_empty()
                            && !l.starts_with('#')
                            && !l.to_lowercase().contains("подпиш")
                            && !l.to_lowercase().contains("subscribe")
                    })
                    .collect();

                let joined = clean_lines.join(" ");
                if joined.chars().count() >= 35 {
                    let truncated = if joined.chars().count() > 160 {
                        let s: String = joined.chars().take(157).collect();
                        format!("{}…", s)
                    } else {
                        joined
                    };
                    synopsis = Some(truncated);
                    break;
                }
            }
        }

        Some((chapters_out, synopsis))
    }

    fn parse_description_timecode(line: &str) -> Option<(String, String)> {
        // Match lines like "01:23 Introduction" or "0:45 - The Problem"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let first = parts[0].trim_matches(['[', ']', '(', ')']);
        if Self::is_timecode_format(first) {
            let topic = parts[1..]
                .join(" ")
                .trim_start_matches(['-', '—', '–', ':'])
                .trim()
                .to_string();

            if topic.chars().count() >= 3 && topic.chars().count() <= 80 {
                return Some((first.to_string(), topic));
            }
        }

        None
    }

    fn is_timecode_format(s: &str) -> bool {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 || parts.len() == 3 {
            parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty())
        } else {
            false
        }
    }

    /// Extract semantic topics from subtitles
    fn extract_topics_from_subtitles(raw_subtitles: &str) -> Vec<String> {
        let mut cues = Vec::new();
        let mut current_sec = None;
        let mut text_buf = String::new();

        for line in raw_subtitles.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if let (Some(sec), false) = (current_sec, text_buf.is_empty()) {
                    let clean = Self::clean_sub_line(&text_buf);
                    if clean.chars().count() >= 15 {
                        cues.push((sec, clean));
                    }
                    text_buf.clear();
                    current_sec = None;
                }
                continue;
            }

            if trimmed.contains("-->") {
                if let Some(start_str) = trimmed.split("-->").next() {
                    let parts: Vec<&str> = start_str.trim().split(':').collect();
                    if parts.len() == 3 {
                        let h: u64 = parts[0].parse().unwrap_or(0);
                        let m: u64 = parts[1].parse().unwrap_or(0);
                        let s: f64 = parts[2].replace(',', ".").parse().unwrap_or(0.0);
                        current_sec = Some(h * 3600 + m * 60 + s as u64);
                    }
                }
                continue;
            }

            if trimmed.starts_with("WEBVTT")
                || trimmed.starts_with("Kind:")
                || trimmed.starts_with("Language:")
                || trimmed.chars().all(|c| c.is_ascii_digit())
            {
                continue;
            }

            if current_sec.is_some() {
                if !text_buf.is_empty() {
                    text_buf.push(' ');
                }
                text_buf.push_str(trimmed);
            }
        }

        if cues.is_empty() {
            return Vec::new();
        }

        // Group into timeline milestones (every 60-90 seconds)
        let mut topics = Vec::new();
        let mut last_added_sec = 0;

        for (sec, text) in cues {
            if topics.is_empty() || sec.saturating_sub(last_added_sec) >= 75 {
                let m = sec / 60;
                let s = sec % 60;
                let clean = Self::capitalize_first(&text);
                let label = if topics.is_empty() {
                    format!("[{:02}:{:02}] Тема: {}", m, s, clean)
                } else {
                    format!("[{:02}:{:02}] Разбор: {}", m, s, clean)
                };
                topics.push(label);
                last_added_sec = sec;
                if topics.len() >= 5 {
                    break;
                }
            }
        }

        topics
    }

    fn clean_sub_line(text: &str) -> String {
        let mut out = String::new();
        let mut in_tag = false;
        let mut in_bracket = false;
        for c in text.chars() {
            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
            } else if c == '[' {
                in_bracket = true;
            } else if c == ']' {
                in_bracket = false;
            } else if !in_tag && !in_bracket && c != '♪' && c != '♫' {
                out.push(c);
            }
        }
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn capitalize_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }
}
