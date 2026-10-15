//! Download jobs: creating them, running yt-dlp, killing it, remembering history.
//! History lives in data\history.json. Several Reynard processes can touch it
//! at once (one per running download), so every change goes through a lock file.

use crate::paths;
use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_HISTORY: usize = 200;

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Job {
    pub id: u32,
    pub url: String,
    pub site: String,
    pub dir: String,
    /// queued | running | done | error | cancelled
    pub status: String,
    pub progress: String,
    pub file: String,
    pub error: String,
    pub pid: u32,
    pub started: u64,
    pub finished: u64,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct History {
    next_id: u32,
    jobs: Vec<Job>,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ---------- history file ----------

fn history_path() -> std::path::PathBuf {
    paths::data_dir().join("history.json")
}

fn read_history() -> History {
    fs::read_to_string(history_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn write_history(h: &History) {
    let path = history_path();
    let tmp = path.with_extension("json.tmp");
    if let Ok(text) = serde_json::to_string_pretty(h) {
        if fs::write(&tmp, text).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}

/// Lock, read, change, write, unlock.
fn with_history<R>(f: impl FnOnce(&mut History) -> R) -> R {
    let dir = paths::data_dir();
    let _ = fs::create_dir_all(&dir);
    let lock = dir.join("history.lock");
    let started = Instant::now();
    loop {
        if OpenOptions::new().write(true).create_new(true).open(&lock).is_ok() {
            break;
        }
        let stale = fs::metadata(&lock)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .map_or(false, |age| age > Duration::from_secs(10));
        if stale || started.elapsed() > Duration::from_secs(3) {
            let _ = fs::remove_file(&lock); // left behind by a crash
        }
        thread::sleep(Duration::from_millis(25));
    }
    let mut history = read_history();
    let result = f(&mut history);
    write_history(&history);
    let _ = fs::remove_file(&lock);
    result
}

pub fn list() -> Vec<Job> {
    read_history().jobs
}

pub fn any_active() -> bool {
    read_history().jobs.iter().any(|j| j.status == "queued" || j.status == "running")
}

pub fn get(id: u32) -> Option<Job> {
    read_history().jobs.into_iter().find(|j| j.id == id)
}

fn create(url: &str, site: &str, dir: &str) -> Job {
    with_history(|h| {
        h.next_id = h.next_id.max(1);
        let job = Job {
            id: h.next_id,
            url: url.into(),
            site: site.into(),
            dir: dir.into(),
            status: "queued".into(),
            ..Job::default()
        };
        h.next_id += 1;
        h.jobs.push(job.clone());
        if h.jobs.len() > MAX_HISTORY {
            let extra = h.jobs.len() - MAX_HISTORY;
            h.jobs.drain(0..extra);
        }
        job
    })
}

fn update(id: u32, f: impl FnOnce(&mut Job)) {
    with_history(|h| {
        if let Some(job) = h.jobs.iter_mut().find(|j| j.id == id) {
            f(job);
        }
    });
}

/// The "Clear downloads" button: forgets everything that is no longer active.
pub fn clear_finished() -> usize {
    with_history(|h| {
        let before = h.jobs.len();
        h.jobs.retain(|j| j.status == "queued" || j.status == "running");
        before - h.jobs.len()
    })
}

// ---------- starting, killing ----------

pub struct Started {
    pub job: Job,
    pub toast: bool,
}

pub fn is_http_url(u: &str) -> bool {
    let l = u.trim().to_ascii_lowercase();
    l.starts_with("http://") || l.starts_with("https://")
}

/// Matches the URL against the site rules and records a queued job.
/// The caller then runs it (CLI) or hands it to a background runner (Firefox).
pub fn start(url: &str) -> Result<Started, String> {
    let url = url.trim();
    if !is_http_url(url) {
        return Err("not an http(s) URL".into());
    }
    let settings = Settings::load().map_err(|e| format!("settings.toml: {e}"))?;
    let resolved = settings.resolve(url);
    let job = create(url, resolved.matched.as_deref().unwrap_or(""), &resolved.dir);
    Ok(Started { job, toast: settings.general.show_toasts })
}

/// Kill = stop yt-dlp *and* everything it started (ffmpeg, the PyInstaller child).
pub fn cancel(id: u32) -> Result<(), String> {
    let pid = with_history(|h| match h.jobs.iter_mut().find(|j| j.id == id) {
        None => Err(format!("no job #{id}")),
        Some(j) if j.status == "queued" || j.status == "running" => {
            j.status = "cancelled".into();
            j.finished = now();
            Ok(j.pid)
        }
        Some(j) => Err(format!("job #{id} is already {}", j.status)),
    })?;
    if pid != 0 {
        kill_tree(pid);
    }
    Ok(())
}

#[cfg(windows)]
fn kill_tree(pid: u32) {
    use std::os::windows::process::CommandExt;
    let pid = pid.to_string();
    let _ = Command::new("taskkill")
        .args(["/PID", pid.as_str(), "/T", "/F"])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(windows))]
fn kill_tree(pid: u32) {
    let _ = Command::new("kill").arg(pid.to_string()).status();
}

// ---------- running ----------

fn fail(id: u32, msg: String) -> String {
    update(id, |j| {
        j.status = "error".into();
        j.error = msg.clone();
        j.finished = now();
    });
    msg
}

fn read_lines(reader: impl std::io::Read, mut on_line: impl FnMut(&str)) {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => on_line(String::from_utf8_lossy(&buf).trim()),
        }
    }
}

/// Runs one queued job to the end. `verbose` prints progress to the terminal.
pub fn run(id: u32, verbose: bool) -> Result<(), String> {
    let job = get(id).ok_or_else(|| format!("no job #{id}"))?;
    if job.status != "queued" {
        return Ok(()); // cancelled before it started
    }
    let settings = Settings::load().map_err(|e| fail(id, format!("settings.toml: {e}")))?;
    let resolved = settings.resolve(&job.url);
    let g = &settings.general;

    let yt_dlp = paths::resolve_tool(&g.yt_dlp);
    let data = paths::data_dir();
    let _ = fs::create_dir_all(&data);
    // yt-dlp writes the final file path here (UTF-8), so odd characters survive.
    // (Absolute, because yt-dlp resolves relative paths against the download folder.)
    let path_file = data.join(format!("job-{id}.path"));
    let _ = fs::remove_file(&path_file);

    let mut cmd = Command::new(&yt_dlp);
    cmd.current_dir(&data)
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args([
            "--newline",
            "--progress",
            "--progress-template",
            "download:RFPROG|%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s",
            "--print-to-file",
            "after_move:filepath",
        ])
        .arg(&path_file)
        .arg("-P")
        .arg(&job.dir)
        .arg("-o")
        .arg(&g.output_template);
    let ffmpeg_dir = paths::resolve_dir(&g.ffmpeg_dir);
    if ffmpeg_dir.is_dir() {
        cmd.arg("--ffmpeg-location").arg(&ffmpeg_dir);
    } else {
        paths::log(&format!("ffmpeg folder not found: {}", ffmpeg_dir.display()));
    }
    // Settings args come last so they can override the basics above.
    cmd.args(&resolved.args).arg("--").arg(&job.url);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| fail(id, format!("could not start {}: {e}", yt_dlp.display())))?;
    let pid = child.id();

    let mut cancelled_early = false;
    update(id, |j| {
        if j.status == "cancelled" {
            cancelled_early = true;
        } else {
            j.status = "running".into();
            j.pid = pid;
            j.started = now();
        }
    });
    if cancelled_early {
        kill_tree(pid);
    }

    // stderr on its own thread so neither pipe can fill up and stall yt-dlp.
    let stderr = child.stderr.take().expect("stderr is piped");
    let err_thread = thread::spawn(move || {
        let mut lines: Vec<String> = Vec::new();
        read_lines(stderr, |l| {
            if !l.is_empty() {
                lines.push(l.to_string());
                if lines.len() > 20 {
                    lines.remove(0);
                }
            }
        });
        lines
    });

    let stdout = child.stdout.take().expect("stdout is piped");
    let mut last_write = Instant::now() - Duration::from_secs(5);
    read_lines(stdout, |line| {
        if let Some(rest) = line.strip_prefix("RFPROG|") {
            let f: Vec<&str> = rest.split('|').map(str::trim).collect();
            let progress = format!("{} {} ETA {}", f.first().unwrap_or(&""), f.get(1).unwrap_or(&""), f.get(2).unwrap_or(&""));
            if verbose {
                print!("\r{progress:<60}");
                let _ = std::io::stdout().flush();
            }
            if last_write.elapsed() >= Duration::from_secs(1) {
                last_write = Instant::now();
                update(id, |j| j.progress = progress);
            }
        }
    });
    if verbose {
        println!();
    }

    let success = child.wait().map(|s| s.success()).unwrap_or(false);
    let err_lines = err_thread.join().unwrap_or_default();

    let file = fs::read_to_string(&path_file)
        .ok()
        .and_then(|t| t.lines().rev().find(|l| !l.trim().is_empty()).map(|l| l.trim().to_string()))
        .unwrap_or_default();
    let _ = fs::remove_file(&path_file);

    let error = err_lines
        .iter()
        .rev()
        .find(|l| l.starts_with("ERROR"))
        .or(err_lines.last())
        .cloned()
        .unwrap_or_else(|| "yt-dlp failed".into());

    update(id, |j| {
        if success {
            j.status = "done".into();
            j.progress = "100%".into();
            j.file = file;
        } else if j.status != "cancelled" {
            j.status = "error".into();
            j.error = error;
        }
        j.pid = 0;
        j.finished = now();
    });
    Ok(())
}
