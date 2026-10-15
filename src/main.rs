// Reynard: one binary, two modes.
//  * Started by Firefox (native messaging)  -> host mode, see host.rs
//  * Started by you or by a background job  -> command line (tray app comes later)

mod host;
mod jobs;
mod paths;
mod settings;
mod shell;
mod tools;

use settings::Settings;
use std::env;
use std::process::ExitCode;

const EXTENSION_ID: &str = "reynard@reynard-yamakoira";
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    // Firefox launches native hosts as: reynard <manifest path> <extension id>
    if env::args().any(|a| a == EXTENSION_ID) {
        host::run();
        return ExitCode::SUCCESS;
    }

    let args: Vec<String> = env::args().skip(1).collect();
    let rest = &args[args.len().min(1)..];
    let result = match args.first().map(String::as_str) {
        Some("download") => cmd_download(rest),
        Some("run") => cmd_run(rest),
        Some("jobs") => cmd_jobs(),
        Some("kill") => cmd_kill(rest),
        Some("clear") => cmd_clear(),
        Some("match") => cmd_match(rest),
        Some("settings") => cmd_settings(),
        _ => {
            help();
            Ok(())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn help() {
    println!("Reynard {VERSION}");
    println!();
    println!("  reynard download <url>   download now, with progress (uses your site rules)");
    println!("  reynard jobs             list downloads and their status");
    println!("  reynard kill <id>        stop a download (yt-dlp and ffmpeg)");
    println!("  reynard clear            forget finished downloads");
    println!("  reynard match <url>      show which site rule a URL would use");
    println!("  reynard settings         show where settings.toml is (creates it if missing)");
    println!();
    println!("Right-click downloads from Firefox start the same jobs in the background.");
}

fn parse_id(rest: &[String]) -> Result<u32, String> {
    rest.first()
        .and_then(|s| s.trim_start_matches('#').parse().ok())
        .ok_or_else(|| "give a job number, e.g. 3".to_string())
}

fn cmd_download(rest: &[String]) -> Result<(), String> {
    let url = rest.first().ok_or("give a URL")?;
    let started = jobs::start(url)?;
    println!("Job #{} -> {}", started.job.id, started.job.dir);
    jobs::run(started.job.id, true)?;
    let job = jobs::get(started.job.id).ok_or("job vanished")?;
    match job.status.as_str() {
        "done" => println!("Done: {}", job.file),
        "cancelled" => println!("Cancelled."),
        _ => return Err(job.error),
    }
    Ok(())
}

/// Used by the background process Firefox starts. No terminal output.
fn cmd_run(rest: &[String]) -> Result<(), String> {
    let id = parse_id(rest)?;
    let result = jobs::run(id, false);
    if let Err(e) = &result {
        paths::log(&format!("run #{id}: {e}"));
    }
    result
}

fn cmd_jobs() -> Result<(), String> {
    let list = jobs::list();
    if list.is_empty() {
        println!("No downloads yet.");
    }
    for j in list {
        println!("#{:<4} {:<9} {}", j.id, j.status, j.url);
        let detail = match j.status.as_str() {
            "done" => j.file,
            "error" => j.error,
            "running" => j.progress,
            _ => String::new(),
        };
        if !detail.is_empty() {
            println!("      {detail}");
        }
    }
    Ok(())
}

fn cmd_kill(rest: &[String]) -> Result<(), String> {
    let id = parse_id(rest)?;
    jobs::cancel(id)?;
    println!("Job #{id} cancelled.");
    Ok(())
}

fn cmd_clear() -> Result<(), String> {
    println!("Removed {} finished job(s).", jobs::clear_finished());
    Ok(())
}

fn cmd_match(rest: &[String]) -> Result<(), String> {
    let url = rest.first().ok_or("give a URL")?;
    let r = Settings::load()?.resolve(url);
    match &r.matched {
        Some(p) => println!("Rule:      {p}"),
        None => println!("Rule:      (none, defaults)"),
    }
    println!("Directory: {}", r.dir);
    println!("Arguments: {}", r.args.join(" "));
    Ok(())
}

fn cmd_settings() -> Result<(), String> {
    Settings::load()?;
    println!("{}", paths::settings_path().display());
    Ok(())
}
