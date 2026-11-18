//! yt-dlp housekeeping: which version is installed, and updating it.

use crate::{jobs, paths, settings::Settings};
use std::process::{Command, Stdio};

fn ytdlp_command() -> Result<Command, String> {
    let settings = Settings::load().map_err(|e| format!("settings.toml: {e}"))?;
    let mut cmd = Command::new(paths::resolve_tool(&settings.general.yt_dlp));
    cmd.arg("--ignore-config").env("PYTHONIOENCODING", "utf-8").stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    Ok(cmd)
}

pub fn ytdlp_version() -> Option<String> {
    let out = ytdlp_command().ok()?.arg("--version").stderr(Stdio::null()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if v.is_empty() { None } else { Some(v) }
}

/// Runs `yt-dlp -U`. Refuses while downloads are running, because on Windows
/// the update replaces the very exe those downloads are using.
pub fn update_ytdlp() -> Result<String, String> {
    if jobs::any_active() {
        return Err("Downloads are running - wait for them to finish, then update.".into());
    }
    let out = ytdlp_command()?
        .arg("-U")
        .output()
        .map_err(|e| format!("could not run yt-dlp: {e}"))?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let tail = lines[lines.len().saturating_sub(8)..].join("\n");
    if out.status.success() { Ok(tail) } else { Err(tail) }
}
