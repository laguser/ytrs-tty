use crate::config::AppConfig;
use crate::vot::VotManager;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;

pub struct Player;

impl Player {
    /// Play standard video with ultra-fast mpv config (supports playlists and single videos)
    pub fn play_video(id_or_playlist: &str, is_playlist: bool, cfg: &AppConfig) {
        let url = if is_playlist || id_or_playlist.starts_with("PL") || id_or_playlist.starts_with("OLAK5uy") {
            format!("https://www.youtube.com/playlist?list={}", id_or_playlist)
        } else {
            format!("https://www.youtube.com/watch?v={}", id_or_playlist)
        };
        let format_arg = format!(
            "--ytdl-format=bestvideo[height<={}]+bestaudio/best[height<={}]/best",
            cfg.max_res_height, cfg.max_res_height
        );
        let title_arg = if is_playlist {
            "--title=yt [playlist]: ${media-title}".to_string()
        } else {
            "--title=yt: ${media-title}".to_string()
        };
        Self::spawn_mpv(
            &[
                &format_arg,
                &title_arg,
                &url,
            ],
            None,
            cfg,
        );
    }

    /// Play with Voice-Over Translation (VOT via Yandex) + Subtitles + SponsorBlock in parallel
    pub fn play_with_vot(
        video_id: &str,
        cfg: AppConfig,
        on_progress: impl Fn(f32, &str) + Send + 'static,
    ) {
        let vid = video_id.to_string();
        thread::spawn(move || {
            on_progress(0.2, "Connecting to Voice Translation service...");
            let yt_url = format!("https://www.youtube.com/watch?v={}", vid);

            // Parallel fetch: Voice translation audio and Subtitles at the same time
            let yt_url_clone = yt_url.clone();
            let from_lang = cfg.translation_from.clone();
            let to_lang = cfg.translation_to.clone();
            let audio_handle = thread::spawn(move || {
                VotManager::fetch_audio_url(&yt_url_clone, &from_lang, &to_lang)
            });

            let yt_url_subs = yt_url.clone();
            let vid_subs = vid.clone();
            let from_lang_subs = cfg.translation_from.clone();
            let subs_lang = cfg.subs_lang.clone();
            let subs_handle = thread::spawn(move || {
                if subs_lang != "off" {
                    VotManager::fetch_and_save_srt(&yt_url_subs, &vid_subs, &from_lang_subs, &subs_lang)
                } else {
                    None
                }
            });

            let audio_opt = audio_handle.join().unwrap_or(None);
            on_progress(0.7, "Processing audio streams & subtitles...");
            let srt_path = subs_handle.join().unwrap_or(None);

            on_progress(0.9, "Starting MPV hardware accelerated playback...");

            if let Some(audio_url) = audio_opt {
                let lavfi = format!(
                    "[aid1]volume={:.2}[vol1];[aid2]volume={:.2}[vol2];[vol1][vol2]amix=inputs=2[ao]",
                    cfg.orig_volume, cfg.vot_volume
                );

                let format_arg = format!(
                    "--ytdl-format=bestvideo[height<={}]+bestaudio/best[height<={}]/best",
                    cfg.max_res_height, cfg.max_res_height
                );

                let audio_file_arg = format!("--audio-file={}", audio_url);
                let lavfi_arg = format!("--lavfi-complex={}", lavfi);
                let title_arg = format!("--title=yt [VOT {}->{}]: ${{media-title}}", cfg.translation_from, cfg.translation_to);

                let mut args = vec![
                    format_arg.as_str(),
                    title_arg.as_str(),
                    audio_file_arg.as_str(),
                    lavfi_arg.as_str(),
                ];

                let srt_arg;
                if let Some(ref path) = srt_path {
                    srt_arg = format!("--sub-file={}", path.to_string_lossy());
                    args.push(srt_arg.as_str());
                }

                args.push(&yt_url);

                Self::spawn_mpv(&args, srt_path.as_deref(), &cfg);
                on_progress(1.0, "Playing with Voice Translation!");
            } else {
                on_progress(1.0, "VOT translation unavailable, opening standard stream");
                Self::play_video(&vid, false, &cfg);
            }
        });
    }

    /// Play audio-only stream (supports playlists and single videos)
    pub fn play_audio_only(id_or_playlist: &str, is_playlist: bool) {
        let url = if is_playlist || id_or_playlist.starts_with("PL") || id_or_playlist.starts_with("OLAK5uy") {
            format!("https://www.youtube.com/playlist?list={}", id_or_playlist)
        } else {
            format!("https://www.youtube.com/watch?v={}", id_or_playlist)
        };
        let _ = Command::new("mpv")
            .arg("--no-video")
            .arg("--ytdl-format=bestaudio/best")
            .arg("--title=yt [audio]: ${media-title}")
            .arg(&url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }

    /// Spawn mpv with hardware acceleration, monochrome dark styling, and instant cache
    fn spawn_mpv(custom_args: &[&str], _srt_path: Option<&std::path::Path>, cfg: &AppConfig) {
        let sub_font = format!("--sub-font={}", cfg.subs_font);
        let sub_size = format!("--sub-font-size={}", cfg.subs_font_size);

        let mut cmd = Command::new("mpv");
        cmd.args(custom_args)
            // Hardware acceleration & speed
            .arg("--hwdec=auto-safe")
            .arg("--vo=gpu-next,gpu")
            .arg("--profile=fast")
            .arg("--force-window=immediate")
            .arg("--demuxer-lavf-analyzeduration=0.5")
            .arg("--demuxer-lavf-probesize=1048576")
            .arg("--autofit=72%x72%")
            .arg("--geometry=50%:50%")
            .arg("--cursor-autohide=800")
            // Ultra-minimal monochrome (Black & White) OSD styling
            .arg("--osd-font=JetBrains Mono")
            .arg("--osd-font-size=18")
            .arg("--osd-color=#EAEAEA")
            .arg("--osd-border-color=#101010")
            .arg("--osd-border-size=1.5")
            .arg("--osd-shadow-offset=0")
            .arg("--osd-bar-align-y=0.92")
            .arg("--osd-bar-w=80")
            .arg("--osd-bar-h=2.5")
            // Ultra-clean compact monochrome bottom-left subtitles
            .arg(&sub_font)
            .arg(&sub_size)
            .arg("--sub-align-x=left")
            .arg("--sub-align-y=bottom")
            .arg("--sub-margin-x=40")
            .arg("--sub-margin-y=35")
            .arg("--sub-color=#FFFFFF")
            .arg("--sub-border-color=#000000")
            .arg("--sub-border-size=2.0")
            .arg("--sub-shadow-offset=0")
            .arg("--sub-back-color=#00000088");

        if cfg.sponsorblock {
            cmd.arg("--ytdl-raw-options=sponsorblock-mark=all,sponsorblock-remove=all");
        }

        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let _ = cmd.spawn();
    }

    /// Copy URL to clipboard
    pub fn copy_url(id_or_playlist: &str, is_playlist: bool) -> bool {
        let url = if is_playlist || id_or_playlist.starts_with("PL") || id_or_playlist.starts_with("OLAK5uy") {
            format!("https://www.youtube.com/playlist?list={}", id_or_playlist)
        } else {
            format!("https://www.youtube.com/watch?v={}", id_or_playlist)
        };
        if let Ok(mut child) = Command::new("wl-copy")
            .stdin(Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(url.as_bytes());
                return true;
            }
        }
        false
    }
}
