//! Where Reynard keeps things. Rule: everything lives next to reynard.exe,
//! except settings.toml (%APPDATA%\Reynard) and the downloads themselves.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn exe_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// History, logs, per-job scratch files.
pub fn data_dir() -> PathBuf {
    exe_dir().join("data")
}

pub fn settings_path() -> PathBuf {
    let base = env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Reynard").join("settings.toml")
}

/// How the default download folder is *written* in settings: a variable, so no
/// user name ends up in the file. Reynard expands it when it is used.
pub fn default_download_setting() -> String {
    if cfg!(windows) { r"%USERPROFILE%\Downloads".into() } else { "%HOME%/Downloads".into() }
}

/// Expands %NAME% the way Windows does, on any OS. Names that don't exist are
/// left exactly as written.
pub fn expand_env(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 => {
                let name = &after[..end];
                match env::var(name) {
                    Ok(value) => out.push_str(&value),
                    Err(_) => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn default_download_dir() -> String {
    let home = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("Downloads").to_string_lossy().into_owned()
}

/// "bin\yt-dlp.exe" is relative to the Reynard folder, a bare name like
/// "yt-dlp" is looked up on PATH, and an absolute path is used as is.
pub fn resolve_tool(p: &str) -> PathBuf {
    let path = Path::new(p);
    if path.is_absolute() {
        path.to_path_buf()
    } else if p.contains('\\') || p.contains('/') {
        exe_dir().join(path)
    } else {
        path.to_path_buf()
    }
}

/// For folder settings (ffmpeg_dir): relative paths are always relative to the
/// Reynard folder, so a bare "bin" means <reynard folder>\bin.
pub fn resolve_dir(p: &str) -> PathBuf {
    let path = Path::new(p);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        exe_dir().join(path)
    }
}

/// The log is a plain file because stdout belongs to Firefox in host mode.
pub fn log(msg: &str) {
    let dir = data_dir();
    let _ = fs::create_dir_all(&dir);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(dir.join("reynard.log")) {
        let _ = writeln!(f, "{msg}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_expand_and_unknown_ones_stay() {
        env::set_var("REYNARD_TEST_DIR", "C:\\Users\\someone");
        assert_eq!(expand_env(r"%REYNARD_TEST_DIR%\Downloads"), r"C:\Users\someone\Downloads");
        assert_eq!(expand_env(r"D:\x\%REYNARD_TEST_DIR%\y"), r"D:\x\C:\Users\someone\y");
        assert_eq!(expand_env(r"%NO_SUCH_VARIABLE_XYZ%\a"), r"%NO_SUCH_VARIABLE_XYZ%\a");
        assert_eq!(expand_env("100% sure"), "100% sure");
        assert_eq!(expand_env("%%"), "%%");
        assert_eq!(expand_env("plain"), "plain");
        assert_eq!(expand_env(""), "");
    }

    #[test]
    fn bare_folder_names_are_relative_to_the_reynard_folder() {
        assert_eq!(resolve_dir("bin"), exe_dir().join("bin"));
        assert!(resolve_dir("bin").is_absolute());
        // tools keep the PATH behaviour for bare names
        assert_eq!(resolve_tool("yt-dlp"), Path::new("yt-dlp"));
    }
}
