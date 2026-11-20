//! Host mode: the browser started us. We speak its stdin/stdout protocol and start
//! each download as a separate background process, so it carries on by itself.

use crate::modes::Mode;
use crate::settings::{self, Settings};
use crate::{jobs, paths, shell, tools};
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn run() {
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    loop {
        // Each message = 4-byte length (native byte order) + JSON bytes.
        let mut len_buf = [0u8; 4];
        if stdin.read_exact(&mut len_buf).is_err() {
            break; // Firefox closed the pipe
        }
        let mut msg = vec![0u8; u32::from_ne_bytes(len_buf) as usize];
        if stdin.read_exact(&mut msg).is_err() {
            break;
        }
        let reply = match serde_json::from_slice::<Value>(&msg) {
            Ok(v) => handle(&v),
            Err(e) => json!({"ok": false, "error": format!("bad message: {e}")}),
        };
        let bytes = serde_json::to_vec(&reply).unwrap_or_default();
        let _ = stdout.write_all(&(bytes.len() as u32).to_ne_bytes());
        let _ = stdout.write_all(&bytes);
        let _ = stdout.flush();
    }
}

fn handle(msg: &Value) -> Value {
    match msg["type"].as_str() {
        Some("ping") => json!({"ok": true, "version": env!("CARGO_PKG_VERSION")}),
        Some("download") => {
            let url = msg["url"].as_str().unwrap_or("");
            // "kind" is what the right-click menu asked for; missing = your settings as they are
            let Some(mode) = Mode::parse(msg["kind"].as_str().unwrap_or("")) else {
                return json!({"ok": false, "error": "unknown download kind"});
            };
            match jobs::start(url, mode, msg["items"].as_str().unwrap_or("")) {
                Err(e) => json!({"ok": false, "error": e}),
                Ok(started) => match spawn_runner(started.job.id) {
                    Ok(()) => json!({
                        "ok": true,
                        "id": started.job.id,
                        "dir": started.job.dir,
                        "kind": started.job.kind
                    }),
                    Err(e) => json!({"ok": false, "error": format!("could not start the download: {e}")}),
                },
            }
        }
        // Recent jobs, newest last. `now` comes from the same clock as the job
        // timestamps, so the extension can tell "finished 3 seconds ago".
        Some("jobs") => {
            let all = jobs::list();
            let start = all.len().saturating_sub(30);
            json!({"ok": true, "now": jobs::now(), "jobs": &all[start..]})
        }
        Some("kill") => match job_id(msg) {
            Some(id) => unit(jobs::cancel(id)),
            None => json!({"ok": false, "error": "missing job id"}),
        },
        Some("clear") => json!({"ok": true, "removed": jobs::clear_finished()}),
        Some("open") => shell_action(msg, "open", shell::open),
        Some("reveal") => shell_action(msg, "reveal", shell::reveal),
        Some("status") => json!({
            "ok": true,
            "version": env!("CARGO_PKG_VERSION"),
            "build": if crate::DEV_BUILD { "dev" } else { "release" },
            "ytdlp": tools::ytdlp_version(),
            "settings_path": paths::settings_path().to_string_lossy(),
        }),
        Some("update_ytdlp") => match tools::update_ytdlp() {
            Ok(output) => json!({"ok": true, "output": output}),
            Err(e) => json!({"ok": false, "error": e}),
        },
        Some("get_settings") => match Settings::load() {
            Ok(s) => json!({
                "ok": true,
                "settings": s,
                "path": paths::settings_path().to_string_lossy(),
                "downloads": paths::default_download_dir(),
            }),
            Err(e) => json!({"ok": false, "error": format!("settings.toml: {e}")}),
        },
        Some("save_settings") => {
            let incoming = &msg["settings"];
            // from_value would quietly fill in defaults for anything missing,
            // and a half-empty message must never overwrite real settings.
            if !incoming["general"].is_object() || !incoming["site"].is_array() {
                return json!({"ok": false, "error": "incomplete settings, nothing saved"});
            }
            match serde_json::from_value::<Settings>(incoming.clone()) {
                Err(e) => json!({"ok": false, "error": format!("bad settings: {e}")}),
                Ok(mut new) => {
                    keep_unsent_keys(incoming, &mut new);
                    unit(settings::save(&new))
                }
            }
        }
        Some("pick_folder") => match shell::pick_folder(&paths::expand_env(msg["start"].as_str().unwrap_or(""))) {
            Ok(path) => json!({"ok": true, "path": path}),
            Err(e) => json!({"ok": false, "error": e}),
        },
        _ => json!({"ok": false, "error": "unknown message type"}),
    }
}

/// Starts `reynard run <id>` detached, so the download carries on by itself.
fn spawn_runner(id: u32) -> io::Result<()> {
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.arg("run")
        .arg(id.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        // Try to leave Firefox's job object so downloads survive the browser
        // closing. If Windows refuses, start without breaking away.
        cmd.creation_flags(DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB);
        if cmd.spawn().is_ok() {
            return Ok(());
        }
        cmd.creation_flags(DETACHED_PROCESS);
    }
    #[cfg(unix)]
    {
        // Own process group: signals aimed at Firefox's group don't reach the download.
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().map(|_| ())
}

fn job_id(msg: &Value) -> Option<u32> {
    msg["id"].as_u64().map(|v| v as u32)
}

fn unit(r: Result<(), String>) -> Value {
    match r {
        Ok(()) => json!({"ok": true}),
        Err(e) => json!({"ok": false, "error": e}),
    }
}

/// The finished file of a job, from our own history.
fn finished_file(id: u32) -> Result<PathBuf, String> {
    let job = jobs::get(id).ok_or_else(|| format!("no job #{id}"))?;
    if job.status != "done" || job.file.is_empty() {
        return Err("this download has no file".into());
    }
    let path = PathBuf::from(&job.file);
    if !path.exists() {
        return Err("the file is gone (moved or deleted)".into());
    }
    Ok(path)
}

/// Open / reveal a finished download. Every attempt is written to data\reynard.log,
/// so "nothing happened" can be traced.
fn shell_action(msg: &Value, name: &str, action: fn(&Path) -> io::Result<()>) -> Value {
    let Some(id) = job_id(msg) else {
        return json!({"ok": false, "error": "missing job id"});
    };
    match finished_file(id) {
        Err(e) => {
            paths::log(&format!("{name} #{id}: {e}"));
            json!({"ok": false, "error": e})
        }
        Ok(path) => match action(&path) {
            Ok(()) => {
                paths::log(&format!("{name} #{id}: launched for {}", path.display()));
                json!({"ok": true})
            }
            Err(e) => {
                paths::log(&format!("{name} #{id}: failed for {}: {e}", path.display()));
                json!({"ok": false, "error": e.to_string()})
            }
        },
    }
}

/// A settings page from an older add-on doesn't send the newer keys, and "missing"
/// would otherwise read as "back to the default". Keep what settings.toml already says.
fn keep_unsent_keys(incoming: &Value, new: &mut Settings) {
    let Ok(current) = Settings::load() else { return };
    let general = &incoming["general"];
    if general.get("include_id").is_none() {
        new.general.include_id = current.general.include_id;
    }
    if general.get("rename_existing").is_none() {
        new.general.rename_existing = current.general.rename_existing;
    }
    if current.site.len() == new.site.len() {
        let sent = incoming["site"].as_array().cloned().unwrap_or_default();
        for (i, site) in new.site.iter_mut().enumerate() {
            if sent.get(i).map_or(false, |v| v.get("include_id").is_none()) {
                site.include_id = current.site[i].include_id;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // None of these touch settings.toml or the job history: every message here is
    // answered (or refused) before anything is loaded or saved.

    #[test]
    fn ping_answers_with_the_version() {
        let r = handle(&json!({"type": "ping"}));
        assert_eq!(r["ok"], true);
        assert_eq!(r["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn unknown_or_missing_types_are_refused() {
        for msg in [json!({"type": "nope"}), json!({}), json!({"type": 7})] {
            let r = handle(&msg);
            assert_eq!(r["ok"], false);
            assert_eq!(r["error"], "unknown message type");
        }
    }

    #[test]
    fn download_refuses_an_unknown_kind() {
        let r = handle(&json!({"type": "download", "url": "https://x.test/v", "kind": "mp3"}));
        assert_eq!(r["ok"], false);
        assert_eq!(r["error"], "unknown download kind");
    }

    #[test]
    fn download_refuses_a_url_that_is_not_http() {
        for url in ["", "blob:https://x.test/abc", "file:///etc/passwd"] {
            let r = handle(&json!({"type": "download", "url": url, "kind": "video"}));
            assert_eq!(r["ok"], false, "{url}");
            assert_eq!(r["error"], "not an http(s) URL", "{url}");
        }
    }

    #[test]
    fn half_empty_settings_are_never_saved() {
        let expected = "incomplete settings, nothing saved";
        for settings in [json!(null), json!({}), json!({"general": {}}), json!({"site": []}), json!({"general": [], "site": []})] {
            let r = handle(&json!({"type": "save_settings", "settings": settings}));
            assert_eq!(r["ok"], false);
            assert_eq!(r["error"], expected);
        }
        let r = handle(&json!({"type": "save_settings"}));
        assert_eq!(r["error"], expected);
    }

    #[test]
    fn job_actions_need_a_job_id() {
        for t in ["kill", "open", "reveal"] {
            let r = handle(&json!({"type": t}));
            assert_eq!(r["ok"], false, "{t}");
            assert_eq!(r["error"], "missing job id", "{t}");
        }
    }

    #[test]
    fn job_id_reads_numbers_only() {
        assert_eq!(job_id(&json!({"id": 12})), Some(12));
        assert_eq!(job_id(&json!({"id": "12"})), None);
        assert_eq!(job_id(&json!({})), None);
    }

    #[test]
    fn unit_turns_a_result_into_a_reply() {
        assert_eq!(unit(Ok(())), json!({"ok": true}));
        assert_eq!(unit(Err("boom".into())), json!({"ok": false, "error": "boom"}));
    }
}
