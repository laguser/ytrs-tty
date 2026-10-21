use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Deserialize)]
struct SubtitleItem {
    text: String,
    #[serde(rename = "startMs")]
    start_ms: f64,
    #[serde(rename = "durationMs")]
    duration_ms: f64,
}

#[derive(Deserialize)]
struct SubtitleFile {
    subtitles: Vec<SubtitleItem>,
}

#[derive(Deserialize)]
struct SubsListEntry {
    language: String,
    url: String,
}

pub struct VotManager;

impl VotManager {
    pub fn get_vot_bin() -> String {
        if let Ok(home) = std::env::var("HOME") {
            let local_bin = format!("{}/.local/bin/vot-cli-live", home);
            if std::path::Path::new(&local_bin).exists() {
                return local_bin;
            }
        }
        "vot-cli-live".to_string()
    }

    /// Fetch voice translation audio stream URL via Yandex
    pub fn fetch_audio_url(yt_url: &str, from_lang: &str, to_lang: &str) -> Option<String> {
        let vot_bin = Self::get_vot_bin();
        let mut cmd = Command::new(&vot_bin);
        cmd.arg("--quiet");

        if !from_lang.is_empty() && from_lang != "auto" {
            cmd.arg("--lang").arg(from_lang);
        }
        if !to_lang.is_empty() {
            cmd.arg("--reslang").arg(to_lang);
        }

        cmd.arg(yt_url);

        let output = cmd.output().ok()?;
        let s = String::from_utf8_lossy(&output.stdout);
        let audio_url = s.trim().to_string();
        if audio_url.starts_with("http") {
            Some(audio_url)
        } else {
            None
        }
    }

    /// Fetch subtitles:
    /// Fetches translated subtitles from Yandex according to `from_lang` and `to_lang`.
    /// Fallback to YouTube captions via yt-dlp.
    pub fn fetch_and_save_srt(
        yt_url: &str,
        video_id: &str,
        from_lang: &str,
        to_lang: &str,
    ) -> Option<PathBuf> {
        if to_lang == "off" {
            return None;
        }

        // Try Yandex Subtitles API first
        if let Some(path) = Self::fetch_yandex_srt(yt_url, video_id, from_lang, to_lang) {
            return Some(path);
        }

        // Fallback to YouTube native captions
        Self::fetch_youtube_srt(yt_url, video_id, to_lang)
    }

    /// Fetch from Yandex Subtitles API
    fn fetch_yandex_srt(
        yt_url: &str,
        video_id: &str,
        from_lang: &str,
        to_lang: &str,
    ) -> Option<PathBuf> {
        let vot_bin = Self::get_vot_bin();
        let mut cmd = Command::new(&vot_bin);
        cmd.arg("--subs").arg("--quiet");

        if !from_lang.is_empty() && from_lang != "auto" {
            cmd.arg("--lang").arg(from_lang);
        }
        if !to_lang.is_empty() {
            cmd.arg("--reslang").arg(to_lang);
        }

        cmd.arg(yt_url);

        let output = cmd.output().ok()?;
        let raw = String::from_utf8_lossy(&output.stdout);
        let start = raw.find('[')?;
        let end = raw.rfind(']')? + 1;
        let json_slice = &raw[start..end];

        let entries: Vec<SubsListEntry> = serde_json::from_str(json_slice).ok()?;
        let target_entry = entries
            .iter()
            .find(|e| e.language.eq_ignore_ascii_case(to_lang))
            .or_else(|| entries.iter().find(|e| e.language == "ru"))
            .or_else(|| entries.first())?;

        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(5))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0")
            .build();

        let resp = agent.get(&target_entry.url).call().ok()?;
        let data: SubtitleFile = resp.into_json().ok()?;

        let mut srt_content = String::new();
        let mut idx = 1;

        for item in data.subtitles.iter() {
            let clean_text = item.text.trim();
            if clean_text.is_empty() {
                continue;
            }

            let chunks = split_long_subtitle(clean_text, 50);
            let chunk_count = chunks.len() as f64;
            let duration_per_chunk = item.duration_ms / chunk_count;

            for (c_i, chunk_text) in chunks.into_iter().enumerate() {
                let start_ms = item.start_ms + (c_i as f64 * duration_per_chunk);
                let end_ms = start_ms + duration_per_chunk;

                let start_time = format_timestamp_ms(start_ms);
                let end_time = format_timestamp_ms(end_ms);
                let wrapped = wrap_text(&chunk_text, 38);

                srt_content.push_str(&format!(
                    "{}\n{} --> {}\n{}\n\n",
                    idx, start_time, end_time, wrapped
                ));
                idx += 1;
            }
        }

        let mut out_path = std::env::temp_dir();
        out_path.push(format!("yt_sub_{}_{}.srt", video_id, to_lang));
        if fs::write(&out_path, srt_content).is_ok() {
            Some(out_path)
        } else {
            None
        }
    }

    /// Fetch YouTube native English or translated subtitles via yt-dlp
    fn fetch_youtube_srt(yt_url: &str, video_id: &str, lang: &str) -> Option<PathBuf> {
        let temp_dir = std::env::temp_dir();
        let sub_langs = format!("{},en,auto", lang);
        let out_template = format!("{}/yt_sub_{}_%(lang)s.%(ext)s", temp_dir.display(), video_id);

        let _ = Command::new("yt-dlp")
            .arg("--write-sub")
            .arg("--write-auto-sub")
            .arg("--sub-lang")
            .arg(&sub_langs)
            .arg("--skip-download")
            .arg("-o")
            .arg(&out_template)
            .arg(yt_url)
            .output();

        // Search temp_dir for any matching subtitle file for this video_id
        if let Ok(entries) = fs::read_dir(&temp_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&format!("yt_sub_{}_", video_id))
                    && (name.ends_with(".srt") || name.ends_with(".vtt"))
                {
                    return Some(entry.path());
                }
            }
        }

        None
    }
}

fn split_long_subtitle(text: &str, max_len: usize) -> Vec<String> {
    if text.chars().count() <= max_len {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let sentences: Vec<&str> = text.split_inclusive(&['.', '!', '?', ';', ','][..]).collect();

    let mut cur = String::new();
    for s in sentences {
        if cur.chars().count() + s.chars().count() > max_len && !cur.is_empty() {
            result.push(cur.trim().to_string());
            cur = s.to_string();
        } else {
            cur.push_str(s);
        }
    }
    if !cur.trim().is_empty() {
        result.push(cur.trim().to_string());
    }

    if result.is_empty() {
        vec![text.to_string()]
    } else {
        result
    }
}

fn wrap_text(text: &str, max_width: usize) -> String {
    let words = text.split_whitespace();
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in words {
        if current_line.is_empty() {
            current_line.push_str(word);
        } else if current_line.chars().count() + word.chars().count() < max_width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    lines.join("\n")
}

fn format_timestamp_ms(ms: f64) -> String {
    let total_secs = (ms / 1000.0).floor() as u64;
    let millis = (ms % 1000.0).round() as u64;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}:{:02},{:03}", hours, mins, secs, millis)
}
