//! Things that talk to Windows itself: opening a file, showing it in Explorer,
//! choosing a folder. The extension only ever passes a job number; the path
//! comes from our own history, so a web page can't open arbitrary files.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Starts a program so that it outlives us.
///
/// Firefox runs the native host inside a Windows "job" that can take down
/// everything the host started once the host exits. Breaking away from that job
/// (like the download runner does) avoids it. We also wait a moment for
/// explorer.exe to hand the request to the running desktop and exit, so we
/// don't disappear mid-handover.
#[allow(unused_mut)]
fn launch(mut cmd: Command) -> io::Result<()> {
    #[cfg(windows)]
    let mut child = {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB);
        match cmd.spawn() {
            Ok(child) => child,
            Err(_) => {
                // Windows refused to break away: start it normally instead.
                cmd.creation_flags(DETACHED_PROCESS);
                cmd.spawn()?
            }
        }
    };
    #[cfg(not(windows))]
    let mut child = cmd.spawn()?;

    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            _ => break,
        }
    }
    Ok(())
}

#[cfg(windows)]
pub fn open(path: &Path) -> io::Result<()> {
    // Explorer opens a file with whatever program is associated with it.
    let mut cmd = Command::new("explorer.exe");
    cmd.arg(path);
    launch(cmd)
}

#[cfg(windows)]
pub fn reveal(path: &Path) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    // Explorer wants exactly:  /select,"C:\some folder\file.mp4"
    let mut cmd = Command::new("explorer.exe");
    cmd.raw_arg(format!("/select,\"{}\"", path.display()));
    launch(cmd)
}

#[cfg(not(windows))]
pub fn open(path: &Path) -> io::Result<()> {
    let mut cmd = Command::new("xdg-open");
    cmd.arg(path);
    launch(cmd)
}

#[cfg(not(windows))]
pub fn reveal(path: &Path) -> io::Result<()> {
    let mut cmd = Command::new("xdg-open");
    cmd.arg(path.parent().unwrap_or(path));
    launch(cmd)
}

/// Windows "choose a folder" dialog. Ok(None) means the user cancelled.
/// The dialog is owned by an invisible top-most window so it comes to the front
/// instead of hiding behind Firefox.
#[cfg(windows)]
pub fn pick_folder(start: &str) -> Result<Option<String>, String> {
    use std::os::windows::process::CommandExt;
    const SCRIPT: &str = r#"
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false
Add-Type -AssemblyName System.Windows.Forms
$owner = New-Object System.Windows.Forms.Form -Property @{ TopMost = $true }
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.Description = 'Choose a folder'
$dialog.ShowNewFolderButton = $true
if ($env:REYNARD_START -and (Test-Path -LiteralPath $env:REYNARD_START)) { $dialog.SelectedPath = $env:REYNARD_START }
if ($dialog.ShowDialog($owner) -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $dialog.SelectedPath }
"#;
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-STA", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", SCRIPT])
        .env("REYNARD_START", start)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW (the dialog itself still shows)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("could not open the folder dialog: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let path = text.trim().trim_start_matches('\u{feff}').trim();
    Ok(if path.is_empty() { None } else { Some(path.to_string()) })
}

#[cfg(not(windows))]
pub fn pick_folder(_start: &str) -> Result<Option<String>, String> {
    let _ = Stdio::null();
    Err("The folder picker is only available on Windows - type the path instead.".into())
}
