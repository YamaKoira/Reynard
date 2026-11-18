//! "Download as ...": how each kind of download changes the yt-dlp flags.
//!
//!  * default: exactly your settings (what "Download" in the menu does)
//!  * video:   your settings, with audio-only switched off
//!  * audio:   your settings, but audio only, in the chosen audio format
//!  * gif:     your settings, then ffmpeg turns the video into a GIF (see jobs.rs)

use crate::settings::General;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Default,
    Video,
    Audio,
    Gif,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "default" => Some(Mode::Default),
            "video" => Some(Mode::Video),
            "audio" => Some(Mode::Audio),
            "gif" => Some(Mode::Gif),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Default => "default",
            Mode::Video => "video",
            Mode::Audio => "audio",
            Mode::Gif => "gif",
        }
    }
}

/// Removes flags. `bare` flags take no value; `valued` ones take one, either as
/// `--flag value` or `--flag=value`.
fn strip(tokens: &[String], bare: &[&str], valued: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i].as_str();
        if bare.contains(&t) {
            i += 1;
        } else if valued.contains(&t) {
            i += 2; // the flag and its value
        } else if valued.iter().any(|n| t.strip_prefix(n).map_or(false, |rest| rest.starts_with('='))) {
            i += 1;
        } else {
            out.push(tokens[i].clone());
            i += 1;
        }
    }
    out
}

const AUDIO_ONLY: [&str; 2] = ["-x", "--extract-audio"];
const AUDIO_VALUED: [&str; 2] = ["--audio-format", "--audio-quality"];

/// True when the flags pick a specific format (-f / --format).
pub fn has_format_flag(args: &[String]) -> bool {
    args.iter().any(|a| a == "-f" || a == "--format" || a.starts_with("--format="))
}

/// The same flags without any format choice: yt-dlp then picks the best it can find.
pub fn without_format(args: &[String]) -> Vec<String> {
    strip(args, &[], &["-f", "--format"])
}

/// True when the flags already ask for audio only.
pub fn is_audio_only(args: &[String]) -> bool {
    args.iter().any(|a| AUDIO_ONLY.contains(&a.as_str()))
}

/// What to call a download in the popup: "video", "audio" or "gif".
pub fn kind_label(mode: Mode, base: &[String]) -> &'static str {
    match mode {
        Mode::Video => "video",
        Mode::Audio => "audio",
        Mode::Gif => "gif",
        Mode::Default => {
            if is_audio_only(base) {
                "audio"
            } else {
                "video"
            }
        }
    }
}

/// The flags for a download: the site's flags (`base`), adjusted for the kind.
pub fn args_for(mode: Mode, base: &[String], general: &General) -> Vec<String> {
    match mode {
        Mode::Default => base.to_vec(),
        Mode::Video => strip(base, &AUDIO_ONLY, &AUDIO_VALUED),
        Mode::Audio => {
            // Throw away what only makes sense for video, then ask for the best audio.
            let mut args = strip(
                base,
                &["-x", "--extract-audio", "--embed-subs"],
                &["--audio-format", "--audio-quality", "-f", "--format", "--recode-video", "--remux-video", "--merge-output-format"],
            );
            args.extend(["-f", "bestaudio/best", "-x"].map(String::from));
            if general.audio_format != "best" {
                args.push("--audio-format".into());
                args.push(general.audio_format.clone());
            }
            args
        }
        Mode::Gif => strip(
            base,
            // a GIF carries no thumbnail, subtitles or tags, and each of these makes
            // yt-dlp call ffmpeg for nothing
            &["-x", "--extract-audio", "--embed-thumbnail", "--embed-subs", "--embed-metadata", "--embed-chapters", "--add-metadata"],
            &["--audio-format", "--audio-quality", "--recode-video", "--remux-video"],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::split_args;

    fn general(fmt: &str) -> General {
        General { audio_format: fmt.into(), ..General::default() }
    }

    #[test]
    fn modes_parse() {
        assert_eq!(Mode::parse("AUDIO"), Some(Mode::Audio));
        assert_eq!(Mode::parse(""), Some(Mode::Default));
        assert_eq!(Mode::parse("gif "), Some(Mode::Gif));
        assert_eq!(Mode::parse("mp3"), None);
    }

    #[test]
    fn default_changes_nothing_and_is_labelled_by_the_flags() {
        let video = split_args("--no-playlist --recode-video mp4 --embed-thumbnail");
        assert_eq!(args_for(Mode::Default, &video, &general("mp3")), video);
        assert_eq!(kind_label(Mode::Default, &video), "video");
        let audio = split_args("--no-playlist -x --audio-format m4a");
        assert_eq!(kind_label(Mode::Default, &audio), "audio");
        assert_eq!(args_for(Mode::Default, &audio, &general("mp3")), audio, "your own audio flags are respected");
    }

    #[test]
    fn format_helpers() {
        let a = split_args("--no-playlist -f bv*[height<=720]+ba/b --embed-metadata");
        assert!(has_format_flag(&a));
        assert_eq!(without_format(&a), split_args("--no-playlist --embed-metadata"));
        assert!(!has_format_flag(&without_format(&a)));
        assert!(has_format_flag(&split_args("--format=best")));
        assert_eq!(without_format(&split_args("--format=best -x")), split_args("-x"));
    }

    #[test]
    fn video_switches_audio_only_off() {
        let base = split_args("--no-playlist -x --audio-format=m4a --embed-metadata --audio-quality 0");
        assert_eq!(args_for(Mode::Video, &base, &general("mp3")), split_args("--no-playlist --embed-metadata"));
    }

    #[test]
    fn audio_keeps_site_flags_but_drops_video_only_ones() {
        let base = split_args("--no-playlist --recode-video mp4 --embed-thumbnail --embed-metadata --embed-subs -f bv*+ba/b --cookies-from-browser firefox");
        let got = args_for(Mode::Audio, &base, &general("mp3"));
        assert_eq!(got, split_args("--no-playlist --embed-thumbnail --embed-metadata --cookies-from-browser firefox -f bestaudio/best -x --audio-format mp3"));
        let best = args_for(Mode::Audio, &base, &general("best"));
        assert!(best.ends_with(&split_args("-f bestaudio/best -x")));
        assert!(!best.contains(&"--audio-format".to_string()));
    }

    #[test]
    fn gif_drops_what_is_pointless_before_ffmpeg() {
        let base = split_args("--no-playlist --recode-video mp4 --embed-thumbnail --embed-subs -x --embed-metadata -f bv*[height<=720]+ba/b");
        assert_eq!(args_for(Mode::Gif, &base, &general("mp3")), split_args("--no-playlist -f bv*[height<=720]+ba/b"));
        assert_eq!(kind_label(Mode::Gif, &base), "gif");
    }
}
