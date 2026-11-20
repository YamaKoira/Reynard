// Reynard: one binary, two modes.
//  * Started by Firefox (native messaging)  -> host mode, see host.rs
//  * Started by you or by a background job  -> command line

mod host;
mod jobs;
mod modes;
mod paths;
mod settings;
mod shell;
mod tools;

use modes::Mode;
use settings::Settings;
use std::env;
use std::process::ExitCode;

// The add-on that may start us. A development build (setup.bat) is compiled with
// REYNARD_EXT_ID set, so it answers to the dev add-on and not to the installed one.
/// True for the development copy (setup.bat), false for the real release build.
const DEV_BUILD: bool = match option_env!("REYNARD_EXT_ID") {
    Some(id) => !id.is_empty(),
    None => false,
};

fn build_label() -> String {
    if DEV_BUILD {
        format!("Reynard {VERSION} (dev build, answers to add-on {EXTENSION_ID})")
    } else {
        format!("Reynard {VERSION} (release)")
    }
}

const EXTENSION_ID: &str = match option_env!("REYNARD_EXT_ID") {
    Some(id) if !id.is_empty() => id,
    _ => "reynard@reynard-yamakoira",
};
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How a browser starts a native host: Firefox passes the manifest path and the add-on ID,
/// Chrome and the other Chromium browsers pass the add-on's origin, "chrome-extension://ID/"
/// (on Windows plus --parent-window=...). Which add-ons may start Reynard is decided by the
/// browser, from the allowed_extensions / allowed_origins in the host's manifest file.
fn is_host_launch(args: &[String]) -> bool {
    args.iter().any(|a| a == EXTENSION_ID || a.starts_with("chrome-extension://"))
}

fn main() -> ExitCode {
    // Firefox launches native hosts as: reynard <manifest path> <extension id>
    if is_host_launch(&env::args().collect::<Vec<_>>()) {
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
        Some("version") | Some("--version") => {
            println!("{}", build_label());
            Ok(())
        }
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
    println!("{}", build_label());
    println!();
    println!("  reynard download <url> [--as video|audio|gif]");
    println!("                           download now, with progress (uses your site rules)");
    println!("  reynard jobs             list downloads and their status");
    println!("  reynard kill <id>        stop a download (yt-dlp and ffmpeg)");
    println!("  reynard clear            forget finished downloads");
    println!("  reynard match <url>      show which site rule a URL would use");
    println!("  reynard settings         show where settings.toml is (creates it if missing)");
    println!("  reynard version          show which build this is (release or dev)");
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
    let mut mode = Mode::Default;
    if let Some(i) = rest.iter().position(|a| a == "--as") {
        let value = rest.get(i + 1).ok_or("--as needs video, audio or gif")?;
        mode = Mode::parse(value).ok_or("--as must be video, audio or gif")?;
    }
    let started = jobs::start(url, mode, "")?;
    println!("Job #{} ({}) -> {}", started.job.id, started.job.kind, started.job.dir);
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

#[cfg(test)]
mod tests {
    #[test]
    fn the_build_says_what_it_is() {
        let label = super::build_label();
        assert!(label.starts_with("Reynard "));
        if super::DEV_BUILD {
            assert!(label.contains("dev build"));
        } else {
            assert!(label.ends_with("(release)"));
        }
    }

    #[test]
    fn extension_id_looks_like_an_addon_id() {
        assert!(super::EXTENSION_ID.contains('@'));
        assert!(!super::EXTENSION_ID.contains(' '));
    }

    #[test]
    fn browsers_start_the_host_their_own_way() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // Firefox: manifest path + add-on ID
        assert!(super::is_host_launch(&a(&["reynard", "/x/io.reynard.host.json", super::EXTENSION_ID])));
        // Chrome / Chromium / Edge / Brave: the origin; Windows adds the parent window
        assert!(super::is_host_launch(&a(&["reynard", "chrome-extension://abcdefghijklmnopabcdefghijklmnop/"])));
        assert!(super::is_host_launch(&a(&["reynard.exe", "chrome-extension://abc/", "--parent-window=1234"])));
        // normal command line use is not a host launch
        assert!(!super::is_host_launch(&a(&["reynard"])));
        assert!(!super::is_host_launch(&a(&["reynard", "download", "https://x.test/v"])));
        assert!(!super::is_host_launch(&a(&["reynard", "version"])));
        assert!(!super::is_host_launch(&a(&["reynard", "https://chrome-extension.example/"])));
    }
}
