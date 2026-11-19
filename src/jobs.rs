//! Download jobs: creating them, running yt-dlp, killing it, remembering history.
//! History lives in data\history.json. Several Reynard processes can touch it
//! at once (one per running download), so every change goes through a lock file.

use crate::modes::{self, Mode};
use crate::paths;
use crate::settings::{General, Settings};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
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
    /// What was asked for: default | video | audio | gif
    pub mode: String,
    /// What it is, for the popup: video | audio | gif ("" in old history = video)
    pub kind: String,
    /// Which videos of an X post to take ("1-2", "3"), sent by the add-on. "" = no choice.
    pub items: String,
    /// queued | running | done | error | cancelled
    pub status: String,
    pub progress: String,
    pub file: String,
    pub error: String,
    /// A friendly remark on a finished download (renamed, or skipped because it already existed).
    pub note: String,
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

fn create(url: &str, site: &str, dir: &str, mode: Mode, kind: &str, items: &str) -> Job {
    with_history(|h| {
        h.next_id = h.next_id.max(1);
        let job = Job {
            id: h.next_id,
            url: url.into(),
            site: site.into(),
            dir: dir.into(),
            mode: mode.as_str().into(),
            kind: kind.into(),
            items: items.into(),
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
}

pub fn is_http_url(u: &str) -> bool {
    let l = u.trim().to_ascii_lowercase();
    l.starts_with("http://") || l.starts_with("https://")
}

/// Matches the URL against the site rules and records a queued job.
/// The caller then runs it (CLI) or hands it to a background runner (Firefox).
pub fn start(url: &str, mode: Mode, items: &str) -> Result<Started, String> {
    let url = url.trim();
    if !is_http_url(url) {
        return Err("not an http(s) URL".into());
    }
    let settings = Settings::load().map_err(|e| format!("settings.toml: {e}"))?;
    let resolved = settings.resolve(url);
    let kind = modes::kind_label(mode, &resolved.args);
    let job = create(url, resolved.matched.as_deref().unwrap_or(""), &resolved.dir, mode, kind, &clean_items(items));
    Ok(Started { job })
}

/// A playlist range the add-on may send: "2" or "1-3" and nothing else.
pub fn clean_items(items: &str) -> String {
    let t = items.trim();
    let num = |s: &str| !s.is_empty() && s.len() <= 3 && s.bytes().all(|b| b.is_ascii_digit()) && s != "0" && !s.starts_with('0');
    let ok = match t.split_once('-') {
        Some((a, b)) => num(a) && num(b) && a.parse::<u32>().unwrap_or(0) <= b.parse::<u32>().unwrap_or(0),
        None => num(t),
    };
    if ok { t.to_string() } else { String::new() }
}

/// X lists a post's own videos first and then the quoted post's, as one playlist. When the
/// add-on says which of them were meant, ask yt-dlp for exactly those. "Single video" in your
/// flags (--no-playlist) still means one video: the first of the range.
pub fn apply_items(args: Vec<String>, items: &str) -> Vec<String> {
    let items = clean_items(items);
    if items.is_empty() {
        return args;
    }
    let single = args.iter().any(|a| a == "--no-playlist");
    let mut out: Vec<String> = args.into_iter().filter(|a| a != "--no-playlist" && a != "--yes-playlist").collect();
    let pick = if single { items.split('-').next().unwrap_or(&items).to_string() } else { items };
    out.push("--playlist-items".into());
    out.push(pick);
    out
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
    // yt-dlp and ffmpeg are started as leaders of their own process group, so one
    // signal to the group (a negative number) stops them and everything they started.
    let group = Command::new("kill").args(["-s", "TERM", "--"]).arg(format!("-{pid}")).stderr(Stdio::null()).status();
    if !group.map(|s| s.success()).unwrap_or(false) {
        let _ = Command::new("kill").arg(pid.to_string()).stderr(Stdio::null()).status();
    }
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
    let mode = Mode::parse(&job.mode).unwrap_or(Mode::Default);

    let yt_dlp = paths::resolve_tool(&g.yt_dlp);
    let data = paths::data_dir();
    let _ = fs::create_dir_all(&data);
    // yt-dlp writes the final file path here (UTF-8), so odd characters survive.
    // (Absolute, because yt-dlp resolves relative paths against the download folder.)
    let path_file = data.join(format!("job-{id}.path"));
    let _ = fs::remove_file(&path_file);

    // A GIF is made from a video that is only an intermediate. It is downloaded into
    // a private scratch folder, so it can never overwrite or delete a video that is
    // already in your download folder.
    // With "Rename if file exists" every download goes through a scratch folder too:
    // yt-dlp can't number files itself, so we move the result in afterwards and pick
    // a free name ("Name (1).ext") ourselves.
    let rename = g.rename_existing;
    let download_dir: PathBuf = if mode == Mode::Gif || rename {
        let scratch = data.join(format!("{}-{id}", if mode == Mode::Gif { "gif" } else { "dl" }));
        let _ = fs::remove_dir_all(&scratch);
        scratch
    } else {
        PathBuf::from(&job.dir)
    };

    // YouTube needs a JavaScript runtime to list any formats. If Deno sits next to
    // yt-dlp (install-local.bat puts it there), tell yt-dlp exactly where it is.
    let deno = find_deno(g, &yt_dlp);
    let ffmpeg_dir = paths::resolve_dir(&g.ffmpeg_dir);
    if !ffmpeg_dir.is_dir() {
        paths::log(&format!("ffmpeg folder not found: {}", ffmpeg_dir.display()));
    }

    let build = |args: &[String]| -> Command {
        let mut cmd = Command::new(&yt_dlp);
        cmd.current_dir(&data)
            .env("PYTHONIOENCODING", "utf-8")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args([
                // Reynard's own settings are the only source of flags: never read the
                // person's yt-dlp config, or a yt-dlp.conf lying in the download folder.
                "--ignore-config",
                "--newline",
                "--progress",
                "--progress-template",
                "download:RFPROG|%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s",
                // Tells us when ffmpeg (or another post-processor) starts, so the job can
                // say "Converting..." instead of keeping the last download percentage.
                "--progress-template",
                "postprocess:RFPOST|%(progress.postprocessor)s|%(progress.status)s",
                "--print-to-file",
                "after_move:filepath",
            ])
            .arg(&path_file)
            .arg("-P")
            .arg(&download_dir)
            .arg("-o")
            .arg(&resolved.template);
        // Only point yt-dlp at the folder if ffmpeg is really in it; otherwise it would
        // not look on PATH, where a system ffmpeg may be.
        if ffmpeg_dir.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }).is_file() {
            cmd.arg("--ffmpeg-location").arg(&ffmpeg_dir);
        }
        if let Some(deno) = &deno {
            cmd.arg("--js-runtimes").arg(format!("deno:{}", deno.display()));
        }
        // Settings args come last so they can override the basics above.
        cmd.args(args).arg("--").arg(&job.url);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0); // its own group, so Cancel can stop everything it started
        }
        cmd
    };

    let base_args = apply_items(modes::args_for(mode, &resolved.args, g), &job.items);
    let mut attempt = execute(id, build(&base_args), &yt_dlp, &path_file, verbose)?;

    // The format you asked for isn't there (say, a resolution limit nothing fits):
    // try once more and let yt-dlp pick the best it can find. Conversions you asked
    // for (mp3, mp4, GIF) still happen afterwards, so you end up with the right kind
    // of file anyway.
    if !attempt.success && wants_format_fallback(&attempt.err_lines) && modes::has_format_flag(&base_args) && !is_cancelled(id) {
        paths::log(&format!("job #{id}: requested format not available, retrying without a format choice"));
        update(id, |j| j.progress = "Retrying with any available format...".into());
        attempt = execute(id, build(&modes::without_format(&base_args)), &yt_dlp, &path_file, verbose)?;
    }
    let Attempt { mut success, err_lines, mut file, files, already } = attempt;
    let mut note = String::new();

    let mut error = explain_error(
        &err_lines,
        err_lines
            .iter()
            .rev()
            .find(|l| l.starts_with("ERROR"))
            .or(err_lines.last())
            .cloned()
            .unwrap_or_else(|| "yt-dlp failed".into()),
    );

    // "Download as GIF": the video is in the scratch folder, now ffmpeg makes the GIF.
    if mode == Mode::Gif {
        if success && !file.is_empty() {
            match make_gif(id, Path::new(&file), Path::new(&job.dir), g) {
                Ok((gif, renamed)) => {
                    if renamed {
                        note = format!("A file with that name was already there, so this one was saved as \"{}\".", file_name_of(&gif));
                    }
                    file = gif.to_string_lossy().into_owned();
                }
                Err(e) => {
                    success = false;
                    // keep the video you did get (unless you cancelled)
                    let kept = if is_cancelled(id) { None } else { move_into(Path::new(&file), Path::new(&job.dir), rename).ok() };
                    error = match kept {
                        Some(path) => format!("The video was saved ({}), but making the GIF failed: {e}", path.display()),
                        None => format!("Making the GIF failed: {e}"),
                    };
                }
            }
        }
        let _ = fs::remove_dir_all(&download_dir);
    } else if rename {
        // Everything that finished goes into your folder. On success that is all files;
        // after a failure or Cancel only the finished downloads, never half-done .part files.
        let failed_before = !success;
        let mains: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
        if failed_before && mains.is_empty() {
            // Nothing finished, so there is nothing to deliver. Keep yt-dlp's own error.
            let _ = fs::remove_dir_all(&download_dir);
        } else {
            match deliver(&download_dir, Path::new(&job.dir), &mains, success) {
                Ok(done) => {
                    if let Some(last) = mains.last().and_then(|m| done.final_path(m)) {
                        file = last.to_string_lossy().into_owned();
                    }
                    if done.renamed && !file.is_empty() {
                        note = format!("A file with that name was already there, so this one was saved as \"{}\".", file_name_of(Path::new(&file)));
                    }
                    if success && done.moves.is_empty() {
                        note = "yt-dlp finished, but no file was saved.".into();
                    }
                    let _ = fs::remove_dir_all(&download_dir);
                }
                Err(e) => {
                    paths::log(&format!("job #{id}: could not deliver the download: {e}"));
                    // If the job had already failed, yt-dlp's own error is the real reason: never hide it.
                    if !failed_before {
                        success = false;
                        let kept = if download_dir.exists() { format!(" The files are in {}", download_dir.display()) } else { String::new() };
                        error = format!("Downloaded, but could not save it to {}: {e}.{kept}", job.dir);
                    }
                }
            }
        }
    } else if already && success {
        note = "A file with this name was already there, so nothing new was saved. Turn on \"Rename if file exists\" in the settings to keep both.".into();
    }

    update(id, |j| {
        j.note = note;
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

fn is_cancelled(id: u32) -> bool {
    get(id).map_or(false, |j| j.status == "cancelled")
}

struct Attempt {
    success: bool,
    err_lines: Vec<String>,
    /// The last finished file yt-dlp reported ("" if none).
    file: String,
    /// Every finished file yt-dlp reported (a playlist has several).
    files: Vec<String>,
    /// yt-dlp said the file was already in the folder.
    already: bool,
}

/// What one yt-dlp output line means for the job's progress text. The bool says the text
/// should be shown at once instead of waiting for the once-a-second update.
fn progress_from_line(line: &str) -> Option<(String, bool)> {
    if let Some(rest) = line.strip_prefix("RFPROG|") {
        let f: Vec<&str> = rest.split('|').map(str::trim).collect();
        let text = format!("{} {} ETA {}", f.first().unwrap_or(&""), f.get(1).unwrap_or(&""), f.get(2).unwrap_or(&""));
        return Some((text, false));
    }
    if let Some(rest) = line.strip_prefix("RFPOST|") {
        let f: Vec<&str> = rest.split('|').map(str::trim).collect();
        // Moving the finished file into place is instant; everything else (ffmpeg merging,
        // audio extraction, remuxing, embedding) is "converting" as far as anyone cares.
        if f.get(1) == Some(&"started") && f.first() != Some(&"MoveFiles") {
            return Some(("Converting...".to_string(), true));
        }
    }
    None
}

/// Runs one yt-dlp process to the end, keeping the job's progress up to date.
fn execute(id: u32, mut cmd: Command, tool: &Path, path_file: &Path, verbose: bool) -> Result<Attempt, String> {
    let mut child = cmd
        .spawn()
        .map_err(|e| fail(id, format!("could not start {}: {e}", tool.display())))?;
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
    let mut already = false;
    read_lines(stdout, |line| {
        if line.contains("has already been downloaded") {
            already = true;
        }
        match progress_from_line(line) {
            Some((progress, urgent)) => {
                if verbose {
                    print!("\r{progress:<60}");
                    let _ = std::io::stdout().flush();
                }
                if urgent || last_write.elapsed() >= Duration::from_secs(1) {
                    last_write = Instant::now();
                    update(id, |j| j.progress = progress);
                }
            }
            None => {}
        }
    });
    if verbose {
        println!();
    }

    let success = child.wait().map(|s| s.success()).unwrap_or(false);
    let err_lines = err_thread.join().unwrap_or_default();

    let files: Vec<String> = fs::read_to_string(path_file)
        .map(|t| t.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    let file = files.last().cloned().unwrap_or_default();
    let _ = fs::remove_file(path_file);
    Ok(Attempt { success, err_lines, file, files, already })
}

/// Deno next to yt-dlp (or in the ffmpeg folder), if there is one.
fn find_deno(g: &General, yt_dlp: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "deno.exe" } else { "deno" };
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(parent) = yt_dlp.parent() {
        if !parent.as_os_str().is_empty() {
            dirs.push(parent.to_path_buf());
        }
    }
    dirs.push(paths::resolve_dir(&g.ffmpeg_dir));
    dirs.into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

/// "Requested format is not available" is worth a second try, unless the real
/// reason is that yt-dlp can't see any formats at all (no JavaScript runtime).
fn wants_format_fallback(err_lines: &[String]) -> bool {
    let any = |needle: &str| err_lines.iter().any(|l| l.contains(needle));
    any("Requested format is not available") && !any("No supported JavaScript runtime") && !any("Only images are available")
}

// Where Deno comes from differs: the Windows installer adds it, on Arch it is a package.
const HINT_ALSO_NO_RUNTIME: &str = if cfg!(windows) {
    "yt-dlp also has no JavaScript runtime: run install-local.bat again to add Deno."
} else {
    "yt-dlp also has no JavaScript runtime: install Deno with sudo pacman -S deno."
};
const HINT_NEEDS_RUNTIME: &str = if cfg!(windows) {
    "YouTube needs a JavaScript runtime (Deno) and yt-dlp could not find one. Run install-local.bat again to add it, or put deno.exe in the bin folder. Updating yt-dlp helps too."
} else {
    "YouTube needs a JavaScript runtime (Deno) and yt-dlp could not find one. Install it with sudo pacman -S deno. Updating yt-dlp helps too."
};

/// Turns yt-dlp's cryptic errors into advice, keeping what yt-dlp said.
fn explain_error(err_lines: &[String], error: String) -> String {
    let any = |needle: &str| err_lines.iter().any(|l| l.contains(needle));
    let no_runtime = any("No supported JavaScript runtime") || any("Only images are available");
    let mut hints: Vec<&str> = Vec::new();
    if any("The page needs to be reloaded") {
        hints.push("YouTube refused the request. Update yt-dlp first (popup, Status, Update yt-dlp).");
        if no_runtime {
            hints.push(HINT_ALSO_NO_RUNTIME);
        } else {
            hints.push("If it keeps happening, try turning off the cookies option for YouTube.");
        }
    } else if no_runtime && any("Requested format is not available") {
        hints.push(HINT_NEEDS_RUNTIME);
    } else if any("unable to access the Reddit API") {
        hints.push("Reddit is blocking this internet address from its API, which Reynard can't fix. Try another network (or turn a VPN off), or turn on Cookies from browser for Reddit while logged in to Reddit there.");
    }
    if hints.is_empty() {
        error
    } else {
        format!("{} (yt-dlp said: {})", hints.join(" "), error.trim_start_matches("ERROR: ").trim())
    }
}

/// Makes sure a folder exists. An existing folder is enough: some drivers (network drives,
/// Linux file systems read through a Windows driver) answer oddly to "create a folder that
/// is already there", and yt-dlp would have been happy with it too.
fn ensure_dir(dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(dir).map_err(|e| io::Error::new(e.kind(), format!("cannot create the folder {}: {e}", dir.display())))
}

/// Plain read-and-write copy. Slower than fs::copy, but it only needs the most basic
/// file calls, which every file system and drive driver supports.
fn copy_stream(from: &Path, to: &Path) -> io::Result<()> {
    let mut reader = fs::File::open(from)?;
    let mut writer = fs::File::create(to)?;
    io::copy(&mut reader, &mut writer)?;
    Ok(())
}

/// Moves a file: rename, else copy (other drive), else a plain stream copy.
/// If all fail, the message says what each attempt answered.
fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    let rename_err = match fs::rename(from, to) {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };
    let copy_err = match fs::copy(from, to) {
        Ok(_) => {
            let _ = fs::remove_file(from); // delivered; the scratch folder is removed afterwards anyway
            return Ok(());
        }
        Err(e) => e,
    };
    let _ = fs::remove_file(to); // a half-written file from the failed copy
    match copy_stream(from, to) {
        Ok(()) => {
            let _ = fs::remove_file(from);
            Ok(())
        }
        Err(e) => Err(io::Error::new(
            e.kind(),
            format!("cannot move {} to {} (rename: {rename_err}; copy: {copy_err}; stream copy: {e})", from.display(), to.display()),
        )),
    }
}

fn file_name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// "Title" + ".mp4" is "Title.mp4"; number 2 makes it "Title (2).mp4" (Windows Explorer style).
fn numbered(stem: &str, n: u32, suffix: &str) -> String {
    if n == 0 {
        format!("{stem}{suffix}")
    } else {
        format!("{stem} ({n}){suffix}")
    }
}

/// The first number (0 = no number at all) for which none of the names is taken in `dir`.
fn free_number(dir: &Path, stem: &str, suffixes: &[String]) -> u32 {
    (0..10_000)
        .find(|&n| suffixes.iter().all(|s| !dir.join(numbered(stem, n, s)).exists()))
        .unwrap_or(10_000)
}

/// Splits "Title [id].mp4" into ("Title [id]", ".mp4").
fn split_name(name: &str) -> (String, String) {
    let stem = Path::new(name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| name.to_string());
    let suffix = name.get(stem.len()..).unwrap_or("").to_string();
    (stem, suffix)
}

/// Moves a file into a folder. With `unique`, a taken name becomes "Name (1).ext".
/// Returns the new path and whether it had to be numbered.
fn move_into_numbered(file: &Path, dir: &Path, unique: bool) -> io::Result<(PathBuf, bool)> {
    let name = file.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no file name"))?;
    ensure_dir(dir)?;
    let (stem, suffix) = split_name(&name.to_string_lossy());
    let n = if unique { free_number(dir, &stem, &[suffix.clone()]) } else { 0 };
    let dest = dir.join(numbered(&stem, n, &suffix));
    move_file(file, &dest)?;
    Ok((dest, n > 0))
}

fn move_into(file: &Path, dir: &Path, unique: bool) -> io::Result<PathBuf> {
    move_into_numbered(file, dir, unique).map(|(p, _)| p)
}

/// What `deliver` did: every (from, to) pair, and whether any name had to be numbered.
struct Delivered {
    moves: Vec<(PathBuf, PathBuf)>,
    renamed: bool,
}

impl Delivered {
    /// Where the file yt-dlp reported as `reported` ended up.
    fn final_path(&self, reported: &Path) -> Option<PathBuf> {
        self.moves
            .iter()
            .find(|(from, _)| from == reported)
            .or_else(|| self.moves.iter().find(|(from, _)| from.file_name() == reported.file_name()))
            .map(|(_, to)| to.clone())
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let list = fs::read_dir(dir).map_err(|e| io::Error::new(e.kind(), format!("cannot list the folder {}: {e}", dir.display())))?;
    for entry in list {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// Moves what a download left in its scratch folder into `target`, never overwriting:
/// a file and its sidecars (thumbnail, subtitles ...) share one number, so
/// "Title.mp4" + "Title.jpg" become "Title (1).mp4" + "Title (1).jpg" together.
/// `mains` are the finished files yt-dlp reported. With `everything` false only those
/// and their sidecars are moved (leftover half-downloads stay behind).
fn deliver(scratch: &Path, target: &Path, mains: &[PathBuf], everything: bool) -> io::Result<Delivered> {
    let mut all = Vec::new();
    if walk(scratch, &mut all).is_err() {
        // Some drives can't be listed this way. The files yt-dlp reported are enough
        // for the main downloads; only extra sidecar files can't be found then.
        all = mains.to_vec();
    }
    all.sort();

    // group = (folder, stem) -> members (path, suffix)
    struct Group {
        dir: PathBuf,
        stem: String,
        members: Vec<(PathBuf, String)>,
    }
    let mut groups: Vec<Group> = Vec::new();
    for m in mains {
        let found = all.iter().find(|a| *a == m).or_else(|| all.iter().find(|a| a.file_name() == m.file_name()));
        if let Some(f) = found {
            let (stem, _) = split_name(&file_name_of(f));
            let dir = f.parent().unwrap_or(scratch).to_path_buf();
            if !groups.iter().any(|g| g.dir == dir && g.stem == stem) {
                groups.push(Group { dir, stem, members: vec![] });
            }
        }
    }
    for f in &all {
        let name = file_name_of(f);
        let dir = f.parent().unwrap_or(scratch).to_path_buf();
        // the longest group stem this file starts with ("Title" for "Title.en.srt")
        let best = groups
            .iter_mut()
            .filter(|g| g.dir == dir && (name == g.stem || (name.starts_with(&g.stem) && name[g.stem.len()..].starts_with('.'))))
            .max_by_key(|g| g.stem.len());
        match best {
            Some(g) => {
                let suffix = name[g.stem.len()..].to_string();
                g.members.push((f.clone(), suffix));
            }
            None if everything => {
                let (stem, suffix) = split_name(&name);
                groups.push(Group { dir, stem, members: vec![(f.clone(), suffix)] });
            }
            None => {}
        }
    }

    let mut done = Delivered { moves: vec![], renamed: false };
    for g in groups.iter().filter(|g| !g.members.is_empty()) {
        let rel = g.dir.strip_prefix(scratch).unwrap_or(Path::new(""));
        // (joining an empty path would add a trailing backslash)
        let out_dir = if rel.as_os_str().is_empty() { target.to_path_buf() } else { target.join(rel) };
        ensure_dir(&out_dir)?;
        let suffixes: Vec<String> = g.members.iter().map(|(_, s)| s.clone()).collect();
        let n = free_number(&out_dir, &g.stem, &suffixes);
        for (from, suffix) in &g.members {
            let to = out_dir.join(numbered(&g.stem, n, suffix));
            move_file(from, &to)?;
            done.moves.push((from.clone(), to));
        }
        done.renamed |= n > 0;
    }
    Ok(done)
}

/// Turns a downloaded video into a GIF in `target_dir`, using a two-step colour
/// palette (much better looking and smaller than ffmpeg's default).
/// Returns the GIF's path.
fn make_gif(id: u32, video: &Path, target_dir: &Path, g: &General) -> Result<(PathBuf, bool), String> {
    ensure_dir(target_dir).map_err(|e| e.to_string())?;
    let unique = g.rename_existing;
    if video.extension().map_or(false, |e| e.eq_ignore_ascii_case("gif")) {
        // the site already served a GIF
        return move_into_numbered(video, target_dir, unique).map_err(|e| e.to_string());
    }
    let stem = video.file_stem().and_then(|s| s.to_str()).unwrap_or("video");
    let video_suffix = video.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    // With "Rename if file exists" the GIF (and the video, if it is kept) get a free number.
    let n = if unique {
        let mut suffixes = vec![".gif".to_string()];
        if g.gif_keep_video {
            suffixes.push(video_suffix.clone());
        }
        free_number(target_dir, stem, &suffixes)
    } else {
        0
    };
    let out = target_dir.join(numbered(stem, n, ".gif"));

    let ffmpeg_dir = paths::resolve_dir(&g.ffmpeg_dir);
    let exe = ffmpeg_dir.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
    let ffmpeg: PathBuf = if exe.exists() { exe } else { PathBuf::from("ffmpeg") };

    // never upscale: min(width, original width)
    let scale = if g.gif_width > 0 { format!("scale='min({},iw)':-1:flags=lanczos,", g.gif_width) } else { String::new() };
    let filter = format!(
        "fps={},{scale}split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5",
        g.gif_fps
    );

    let mut cmd = Command::new(&ffmpeg);
    cmd.args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(video)
        .args(["-an", "-vf", filter.as_str(), "-loop", "0"])
        .arg(&out)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn().map_err(|e| format!("could not run ffmpeg ({}): {e}", ffmpeg.display()))?;
    // from here on, Cancel stops ffmpeg
    let pid = child.id();
    update(id, |j| {
        if j.status == "running" {
            j.pid = pid;
            j.progress = "Converting to GIF...".into();
        }
    });
    let output = child.wait_with_output().map_err(|e| format!("ffmpeg failed: {e}"))?;
    if !output.status.success() {
        let text = String::from_utf8_lossy(&output.stderr);
        let last = text.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("ffmpeg stopped");
        let _ = fs::remove_file(&out); // no half-made GIF left behind
        return Err(last.to_string());
    }
    if g.gif_keep_video {
        let _ = move_file(video, &target_dir.join(numbered(stem, n, &video_suffix)));
    }
    Ok((out, n > 0))
}

#[cfg(test)]
mod tests {
    #[test]
    fn items_ranges_are_checked_and_applied() {
        use super::{apply_items, clean_items};
        let v = |s: &str| s.split_whitespace().map(String::from).collect::<Vec<_>>();
        assert_eq!(clean_items(" 1-2 "), "1-2");
        for bad in ["", "0", "2-1", "a", "1,2", "1-", "-1", "1234", "01", "1;rm"] {
            assert_eq!(clean_items(bad), "", "{bad}");
        }
        assert_eq!(apply_items(v("--embed-metadata"), ""), v("--embed-metadata"));
        assert_eq!(apply_items(v("--embed-metadata"), "2-3"), v("--embed-metadata --playlist-items 2-3"));
        assert_eq!(apply_items(v("--no-playlist -x"), "2-3"), v("-x --playlist-items 2"));
        assert_eq!(apply_items(v("--no-playlist"), "nonsense"), v("--no-playlist"));
    }

    use super::*;

    fn lines(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    // What yt-dlp really prints for YouTube without a JavaScript runtime
    fn no_runtime() -> Vec<String> {
        lines(&[
            "WARNING: [youtube] No supported JavaScript runtime could be found. Only deno is enabled by default; to use another runtime add  --js-runtimes RUNTIME[:PATH]  to your command/config.",
            "WARNING: Only images are available for download.",
            "ERROR: [youtube] zWH_9VRWn8Y: Requested format is not available. Use --list-formats for a list of available formats",
        ])
    }

    #[test]
    fn missing_javascript_runtime_is_explained() {
        let l = no_runtime();
        let msg = explain_error(&l, l[2].clone());
        assert!(msg.contains("Deno") && msg.contains(if cfg!(windows) { "install-local.bat" } else { "pacman -S deno" }), "{msg}");
        assert!(msg.contains("yt-dlp said: [youtube] zWH_9VRWn8Y: Requested format is not available"), "{msg}");
        assert!(!wants_format_fallback(&l), "no point retrying when no formats exist at all");
    }

    #[test]
    fn reload_and_reddit_errors_are_explained() {
        let l = lines(&["ERROR: [youtube] 1uypL1oNChI: The page needs to be reloaded."]);
        let msg = explain_error(&l, l[0].clone());
        assert!(msg.contains("Update yt-dlp") && msg.contains("cookies"), "{msg}");
        let mut both = no_runtime();
        both.push("ERROR: [youtube] x: The page needs to be reloaded.".into());
        assert!(explain_error(&both, "e".into()).contains("Deno"));
        let r = lines(&["ERROR: [Reddit] 1xdykel: Your IP address is unable to access the Reddit API"]);
        let msg = explain_error(&r, r[0].clone());
        assert!(msg.contains("blocking this internet address") && msg.contains("yt-dlp said: [Reddit]"), "{msg}");
    }

    #[test]
    fn other_errors_are_left_alone() {
        let l = lines(&["ERROR: [youtube] abc: Private video. Sign in if you've been granted access to this video"]);
        assert_eq!(explain_error(&l, l[0].clone()), l[0]);
        // a real format problem (no JavaScript warning) does get a second try
        let f = lines(&["ERROR: [generic] x: Requested format is not available. Use --list-formats for a list of available formats"]);
        assert!(wants_format_fallback(&f));
        assert!(!wants_format_fallback(&l));
    }

    #[test]
    fn deno_is_found_next_to_yt_dlp_or_in_the_ffmpeg_folder() {
        let base = std::env::temp_dir().join(format!("reynard-deno-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (a, b) = (base.join("a"), base.join("b"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let name = if cfg!(windows) { "deno.exe" } else { "deno" };
        let g = General { ffmpeg_dir: b.to_string_lossy().into_owned(), ..General::default() };
        assert_eq!(find_deno(&g, &a.join("yt-dlp")), None);
        fs::write(b.join(name), "x").unwrap();
        assert_eq!(find_deno(&g, Path::new("yt-dlp")), Some(b.join(name)), "bare yt-dlp name: ffmpeg folder");
        fs::write(a.join(name), "x").unwrap();
        assert_eq!(find_deno(&g, &a.join("yt-dlp")), Some(a.join(name)), "next to yt-dlp wins");
        let _ = fs::remove_dir_all(&base);
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("reynard-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn numbering_looks_like_windows() {
        assert_eq!(numbered("Title", 0, ".mp4"), "Title.mp4");
        assert_eq!(numbered("Title [abc]", 2, ".mp4"), "Title [abc] (2).mp4");
        assert_eq!(split_name("Title [abc].mp4"), ("Title [abc]".to_string(), ".mp4".to_string()));
        assert_eq!(split_name("v1.2 demo.mkv"), ("v1.2 demo".to_string(), ".mkv".to_string()));
        assert_eq!(split_name("noext"), ("noext".to_string(), String::new()));
    }

    #[test]
    fn a_taken_name_gets_the_next_free_number() {
        let base = scratch("num");
        let (from, to) = (base.join("from"), base.join("to"));
        fs::create_dir_all(&from).unwrap();
        for round in 0..3 {
            let f = from.join("Clip.mp4");
            fs::write(&f, format!("v{round}")).unwrap();
            let (dest, renamed) = move_into_numbered(&f, &to, true).unwrap();
            assert_eq!(renamed, round > 0);
            let want = ["Clip.mp4", "Clip (1).mp4", "Clip (2).mp4"][round];
            assert_eq!(file_name_of(&dest), want);
        }
        // a gap is filled first, like Explorer does
        fs::remove_file(to.join("Clip (1).mp4")).unwrap();
        let f = from.join("Clip.mp4");
        fs::write(&f, "x").unwrap();
        assert_eq!(file_name_of(&move_into_numbered(&f, &to, true).unwrap().0), "Clip (1).mp4");
        // without the option the old behaviour stays: same name, replaced
        fs::write(&f, "new").unwrap();
        assert_eq!(file_name_of(&move_into(&f, &to, false).unwrap()), "Clip.mp4");
        assert_eq!(fs::read_to_string(to.join("Clip.mp4")).unwrap(), "new");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn a_download_and_its_sidecars_share_one_number() {
        let base = scratch("deliver");
        let (sc, target) = (base.join("scratch"), base.join("Downloads"));
        fs::create_dir_all(&sc).unwrap();
        fs::create_dir_all(&target).unwrap();
        let make = |names: &[&str]| {
            for n in names {
                fs::write(sc.join(n), "x").unwrap();
            }
        };
        let names = ["Title [abc].mp4", "Title [abc].jpg", "Title [abc].en.srt"];
        let main = vec![sc.join("Title [abc].mp4")];

        make(&names);
        let d = deliver(&sc, &target, &main, true).unwrap();
        assert!(!d.renamed);
        for n in names {
            assert!(target.join(n).exists(), "{n}");
        }
        assert_eq!(d.final_path(&main[0]).unwrap(), target.join("Title [abc].mp4"));

        // same video again: all three get (1) together, nothing is overwritten
        make(&names);
        let d = deliver(&sc, &target, &main, true).unwrap();
        assert!(d.renamed);
        for n in ["Title [abc] (1).mp4", "Title [abc] (1).jpg", "Title [abc] (1).en.srt"] {
            assert!(target.join(n).exists(), "{n}");
        }
        assert_eq!(file_name_of(&d.final_path(&main[0]).unwrap()), "Title [abc] (1).mp4");

        // a different video with another name is not numbered
        fs::write(sc.join("Other.mp4"), "x").unwrap();
        let d = deliver(&sc, &target, &[sc.join("Other.mp4")], true).unwrap();
        assert!(!d.renamed && target.join("Other.mp4").exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn after_a_failure_only_finished_files_are_kept() {
        let base = scratch("partial");
        let (sc, target) = (base.join("scratch"), base.join("out"));
        fs::create_dir_all(&sc).unwrap();
        fs::write(sc.join("Done one.mp4"), "x").unwrap();
        fs::write(sc.join("Half way.mp4.part"), "x").unwrap();
        fs::write(sc.join("Half way.mp4.ytdl"), "x").unwrap();
        // yt-dlp may print the path with other slashes; the file name still finds it
        let reported = vec![sc.join("Done one.mp4")];
        let d = deliver(&sc, &target, &reported, false).unwrap();
        assert_eq!(d.moves.len(), 1);
        assert!(target.join("Done one.mp4").exists());
        assert!(!target.join("Half way.mp4.part").exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn subfolders_from_your_own_template_are_kept() {
        let base = scratch("sub");
        let (sc, target) = (base.join("scratch"), base.join("out"));
        fs::create_dir_all(sc.join("Channel")).unwrap();
        fs::write(sc.join("Channel").join("Clip.mp4"), "x").unwrap();
        fs::create_dir_all(target.join("Channel")).unwrap();
        fs::write(target.join("Channel").join("Clip.mp4"), "old").unwrap();
        let main = vec![sc.join("Channel").join("Clip.mp4")];
        let d = deliver(&sc, &target, &main, true).unwrap();
        assert!(d.renamed);
        assert!(target.join("Channel").join("Clip (1).mp4").exists());
        assert_eq!(fs::read_to_string(target.join("Channel").join("Clip.mp4")).unwrap(), "old");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn an_existing_folder_is_enough_and_errors_name_the_folder() {
        let base = scratch("ensure");
        assert!(ensure_dir(&base).is_ok(), "already there");
        assert!(ensure_dir(&base.join("a").join("b")).is_ok(), "created");
        fs::write(base.join("file"), "x").unwrap();
        let err = ensure_dir(&base.join("file").join("sub")).unwrap_err().to_string();
        assert!(err.contains("cannot create the folder"), "{err}");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn delivering_to_a_folder_that_does_not_exist_yet_works() {
        let base = scratch("newdir");
        let (sc, target) = (base.join("scratch"), base.join("not").join("there").join("yet"));
        fs::create_dir_all(&sc).unwrap();
        fs::write(sc.join("Clip.mp4"), "x").unwrap();
        let d = deliver(&sc, &target, &[sc.join("Clip.mp4")], true).unwrap();
        assert_eq!(d.final_path(&sc.join("Clip.mp4")).unwrap(), target.join("Clip.mp4"));
        let s = d.moves[0].1.to_string_lossy().into_owned();
        assert!(!s.contains("//") && !s.contains("\\\\"), "no doubled separators: {s}");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn delivery_still_works_when_the_scratch_folder_cannot_be_listed() {
        let base = scratch("nolist");
        let (gone, other, target) = (base.join("gone"), base.join("other"), base.join("out"));
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("Clip.mp4"), "x").unwrap();
        // `gone` does not exist, so listing it fails; the reported file is still delivered
        let d = deliver(&gone, &target, &[other.join("Clip.mp4")], true).unwrap();
        assert!(target.join("Clip.mp4").exists());
        assert_eq!(d.moves.len(), 1);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn the_plain_stream_copy_works_and_move_reports_clearly() {
        let base = scratch("stream");
        fs::write(base.join("a.bin"), vec![7u8; 100_000]).unwrap();
        copy_stream(&base.join("a.bin"), &base.join("b.bin")).unwrap();
        assert_eq!(fs::read(base.join("b.bin")).unwrap().len(), 100_000);
        let err = move_file(&base.join("missing.bin"), &base.join("c.bin")).unwrap_err().to_string();
        assert!(err.contains("rename:") && err.contains("copy:") && err.contains("stream copy:"), "{err}");
        let _ = fs::remove_dir_all(&base);
    }

    #[cfg(unix)]
    #[test]
    fn cancel_stops_the_whole_process_group() {
        use std::os::unix::process::CommandExt;
        // a shell that starts a child, like yt-dlp starting ffmpeg
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 60 & echo $! ; wait"]).stdout(Stdio::piped()).process_group(0);
        let mut parent = cmd.spawn().unwrap();
        let mut line = String::new();
        BufReader::new(parent.stdout.take().unwrap()).read_line(&mut line).unwrap();
        let child_pid: i32 = line.trim().parse().unwrap();
        kill_tree(parent.id());
        let _ = parent.wait();
        std::thread::sleep(Duration::from_millis(300));
        // dead = gone, or a zombie waiting for its parent to collect it
        let state = fs::read_to_string(format!("/proc/{child_pid}/stat")).ok().and_then(|t| t.rsplit(')').next().map(|r| r.trim().chars().next().unwrap_or('?')));
        assert!(matches!(state, None | Some('Z')), "the grandchild must be gone too, state: {state:?}");
    }

    #[test]
    fn deno_hints_follow_the_platform() {
        assert_eq!(HINT_ALSO_NO_RUNTIME.contains("install-local.bat"), cfg!(windows));
        assert_eq!(HINT_NEEDS_RUNTIME.contains("pacman"), !cfg!(windows));
    }

    #[test]
    fn progress_lines_become_text() {
        assert_eq!(progress_from_line("RFPROG| 12.3%|1.2MiB/s|00:05"), Some(("12.3% 1.2MiB/s ETA 00:05".to_string(), false)));
        assert_eq!(progress_from_line("RFPOST|Merger|started"), Some(("Converting...".to_string(), true)));
        assert_eq!(progress_from_line("RFPOST|FFmpegExtractAudio|started"), Some(("Converting...".to_string(), true)));
        assert_eq!(progress_from_line("RFPOST|Merger|finished"), None);
        assert_eq!(progress_from_line("RFPOST|MoveFiles|started"), None);
        assert_eq!(progress_from_line("[download] Destination: x.mp4"), None);
    }
}
