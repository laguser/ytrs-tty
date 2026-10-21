use std::collections::HashSet;

pub struct Summarizer;

struct SubCue {
    start_sec: u64,
    text: String,
}

impl Summarizer {
    /// Extracts a cohesive, timeline-distributed TL;DR summary with timestamps from SRT/VTT subtitles
    pub fn summarize(raw_subtitles: &str, max_points: usize) -> Vec<String> {
        let cues = Self::parse_subtitles(raw_subtitles);
        if cues.is_empty() {
            return vec!["No speech or dialogue found in this video.".to_string()];
        }

        // Clean and filter noise/promotions
        let filtered: Vec<SubCue> = cues
            .iter()
            .filter(|c| !Self::is_filler(&c.text))
            .cloned()
            .collect();

        // If filtered is too small (e.g. music or brief talk), fallback to original cues
        let working_cues = if filtered.len() >= 3 { filtered } else { cues };

        // Group into complete semantic sentences/thoughts
        let thoughts = Self::assemble_thoughts(&working_cues);
        if thoughts.is_empty() {
            return vec!["Dialogue is too short to generate a summary.".to_string()];
        }

        // Score thoughts based on informative density and keywords
        let mut scored: Vec<(f32, SubCue)> = thoughts
            .into_iter()
            .map(|c| {
                let score = Self::score_thought(&c.text);
                (score, c)
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Select top points distributed across video timeline (avoid clustering in one minute)
        let mut selected: Vec<SubCue> = Vec::new();
        for (_, cue) in scored {
            let too_close = selected
                .iter()
                .any(|s| (s.start_sec as i64 - cue.start_sec as i64).abs() < 40);
            if !too_close {
                selected.push(cue);
                if selected.len() >= max_points {
                    break;
                }
            }
        }

        // Sort chosen points chronologically by timestamp
        selected.sort_by_key(|c| c.start_sec);

        selected
            .into_iter()
            .map(|c| {
                let m = c.start_sec / 60;
                let s = c.start_sec % 60;
                let clean = Self::capitalize_first(&c.text);
                format!("[{:02}:{:02}] {}", m, s, clean)
            })
            .collect()
    }

    /// Parse SRT / VTT content into clean cues with start timestamps
    fn parse_subtitles(content: &str) -> Vec<SubCue> {
        let mut cues = Vec::new();
        let mut current_sec: Option<u64> = None;
        let mut text_buf = String::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if let (Some(sec), false) = (current_sec, text_buf.is_empty()) {
                    let cleaned = Self::clean_line(&text_buf);
                    if !cleaned.is_empty() {
                        cues.push(SubCue {
                            start_sec: sec,
                            text: cleaned,
                        });
                    }
                    text_buf.clear();
                    current_sec = None;
                }
                continue;
            }

            // Timestamp line: 00:01:23,456 --> 00:01:25,789 or 00:01:23.456 --> ...
            if trimmed.contains("-->") {
                if let Some(start_str) = trimmed.split("-->").next() {
                    current_sec = Self::parse_timestamp_to_sec(start_str.trim());
                }
                continue;
            }

            // Skip headers or numeric index lines
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

        if let (Some(sec), false) = (current_sec, text_buf.is_empty()) {
            let cleaned = Self::clean_line(&text_buf);
            if !cleaned.is_empty() {
                cues.push(SubCue {
                    start_sec: sec,
                    text: cleaned,
                });
            }
        }

        cues
    }

    fn parse_timestamp_to_sec(ts: &str) -> Option<u64> {
        let clean = ts.replace(',', ".").replace(' ', "");
        let parts: Vec<&str> = clean.split(':').collect();
        if parts.len() == 3 {
            let h: u64 = parts[0].parse().ok()?;
            let m: u64 = parts[1].parse().ok()?;
            let s: f64 = parts[2].parse().ok()?;
            Some(h * 3600 + m * 60 + s as u64)
        } else if parts.len() == 2 {
            let m: u64 = parts[0].parse().ok()?;
            let s: f64 = parts[1].parse().ok()?;
            Some(m * 60 + s as u64)
        } else {
            None
        }
    }

    fn clean_line(text: &str) -> String {
        // Strip XML/HTML tags (<c>), music symbols, and bracket annotations ([Applause])
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

    fn is_filler(text: &str) -> bool {
        let lower = text.to_lowercase();
        let fillers = [
            "like and subscribe",
            "subscribe to",
            "hit the bell",
            "leave a comment",
            "thank you for watching",
            "thanks for watching",
            "see you next time",
            "link in the description",
            "sponsored by",
            "patreon",
            "подписывайтесь",
            "подпишитесь",
            "ставьте лайк",
            "колокольчик",
            "ссылка в описании",
            "спонсор сегодняшнего",
            "спасибо за просмотр",
            "всем пока",
            "до встречи в следующем",
            "не забудьте поставить",
        ];
        fillers.iter().any(|&f| lower.contains(f))
    }

    /// Merge rolling subtitle fragments into complete sentences
    fn assemble_thoughts(cues: &[SubCue]) -> Vec<SubCue> {
        let mut thoughts = Vec::new();
        let mut cur_time = 0;
        let mut cur_text = String::new();
        let mut last_added = String::new();

        for cue in cues {
            // Deduplicate rolling auto-subtitles
            if cue.text == last_added {
                continue;
            }
            last_added = cue.text.clone();

            if cur_text.is_empty() {
                cur_time = cue.start_sec;
                cur_text = cue.text.clone();
            } else {
                cur_text.push(' ');
                cur_text.push_str(&cue.text);
            }

            let ends_with_punct = cur_text.ends_with('.')
                || cur_text.ends_with('!')
                || cur_text.ends_with('?');
            let is_sufficient_len =
                cur_text.split_whitespace().count() >= 8 && cur_text.len() >= 45;

            if ends_with_punct || (is_sufficient_len && cur_text.len() >= 75) {
                if cur_text.len() <= 240 {
                    thoughts.push(SubCue {
                        start_sec: cur_time,
                        text: cur_text.clone(),
                    });
                }
                cur_text.clear();
            }
        }

        if !cur_text.is_empty() && cur_text.len() >= 25 && cur_text.len() <= 240 {
            thoughts.push(SubCue {
                start_sec: cur_time,
                text: cur_text,
            });
        }

        thoughts
    }

    /// Score semantic importance of sentence
    fn score_thought(text: &str) -> f32 {
        let mut score: f32 = 0.0;
        let words: Vec<&str> = text.split_whitespace().collect();
        let word_count = words.len();

        if (8..=25).contains(&word_count) {
            score += 10.0;
        }

        let lower = text.to_lowercase();

        let key_indicators = [
            "главное",
            "особенность",
            "потому что",
            "проблема",
            "решение",
            "результат",
            "оказалось",
            "в итоге",
            "важно",
            "разница",
            "работает",
            "архитектура",
            "because",
            "important",
            "feature",
            "problem",
            "solution",
            "result",
            "difference",
            "how it works",
            "key",
            "actually",
            "reason",
            "benefit",
            "crucial",
            "architecture",
            "explained",
        ];

        for ind in &key_indicators {
            if lower.contains(ind) {
                score += 8.0;
            }
        }

        let unique: HashSet<&str> = words.iter().copied().collect();
        score += unique.len() as f32 * 0.5;

        score
    }

    fn capitalize_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }
}
impl Clone for SubCue {
    fn clone(&self) -> Self {
        Self {
            start_sec: self.start_sec,
            text: self.text.clone(),
        }
    }
}
