use crate::paths;
use serde::{Deserialize, Serialize};
use std::fs;
use toml_edit::{value, Array, ArrayOfTables, DocumentMut, Item, Table, Value};

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct General {
    pub yt_dlp: String,
    pub ffmpeg_dir: String,
    pub default_dir: String,
    pub default_args: String,
    pub output_template: String,
    pub show_toasts: bool,
}

impl Default for General {
    fn default() -> Self {
        General {
            yt_dlp: r"bin\yt-dlp.exe".into(),
            ffmpeg_dir: "bin".into(),
            default_dir: paths::default_download_setting(),
            default_args: "--no-playlist".into(),
            output_template: "%(title).150B [%(id)s].%(ext)s".into(),
            show_toasts: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Site {
    pub patterns: Vec<String>,
    pub dir: String,
    pub args: String,
    pub inherit_args: bool,
}

impl Default for Site {
    fn default() -> Self {
        Site { patterns: vec![], dir: String::new(), args: String::new(), inherit_args: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Settings {
    pub general: General,
    pub site: Vec<Site>,
}

/// What a URL resolves to after matching it against the site rules.
pub struct Resolved {
    pub dir: String,
    pub args: Vec<String>,
    pub matched: Option<String>,
}

impl Settings {
    /// Loads settings.toml, creating a commented starter file on first use.
    pub fn load() -> Result<Settings, String> {
        let path = paths::settings_path();
        if !path.exists() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&path, starter_file()).map_err(|e| format!("cannot create {}: {e}", path.display()))?;
        }
        let text = fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        toml::from_str(&text).map_err(|e| e.to_string())
    }

    pub fn resolve(&self, url: &str) -> Resolved {
        let lower = url.to_lowercase();
        for site in &self.site {
            if let Some(p) = site.patterns.iter().find(|p| wildcard(&p.to_lowercase(), &lower)) {
                let dir = if site.dir.trim().is_empty() { self.general.default_dir.clone() } else { site.dir.clone() };
                let dir = paths::expand_env(&dir);
                let mut args = if site.inherit_args { split_args(&self.general.default_args) } else { vec![] };
                args.extend(split_args(&site.args));
                return Resolved { dir, args, matched: Some(p.clone()) };
            }
        }
        Resolved { dir: paths::expand_env(&self.general.default_dir), args: split_args(&self.general.default_args), matched: None }
    }
}

// ---------------------------------------------------------------- saving
//
// The settings page sends the whole settings object. We do NOT regenerate the
// file: we edit the current file in place, so your comments, spacing and any
// keys the page doesn't know about survive.

fn check(settings: &Settings) -> Result<(), String> {
    let bad = |s: &str| s.contains('\n') || s.contains('\r');
    let g = &settings.general;
    if g.default_dir.trim().is_empty() {
        return Err("The default download folder can't be empty.".into());
    }
    for (name, v) in [
        ("yt-dlp path", &g.yt_dlp),
        ("ffmpeg folder", &g.ffmpeg_dir),
        ("default folder", &g.default_dir),
        ("default flags", &g.default_args),
        ("filename template", &g.output_template),
    ] {
        if bad(v) {
            return Err(format!("The {name} can't contain line breaks."));
        }
    }
    for (i, site) in settings.site.iter().enumerate() {
        let n = i + 1;
        if site.patterns.is_empty() || site.patterns.iter().any(|p| p.trim().is_empty()) {
            return Err(format!("Site #{n} needs at least one pattern (and no empty ones)."));
        }
        if bad(&site.dir) || bad(&site.args) || site.patterns.iter().any(|p| bad(p)) {
            return Err(format!("Site #{n} contains a line break."));
        }
    }
    Ok(())
}

fn set_str(t: &mut Table, key: &str, val: &str) {
    if t.get(key).and_then(Item::as_str) == Some(val) {
        return; // unchanged: leave the line exactly as the user wrote it
    }
    let mut v = Value::from(val);
    if let Some(old) = t.get(key).and_then(Item::as_value) {
        *v.decor_mut() = old.decor().clone(); // keep a trailing comment on that line
    }
    t[key] = Item::Value(v);
}

fn set_bool(t: &mut Table, key: &str, val: bool) {
    if t.get(key).and_then(Item::as_bool) == Some(val) {
        return;
    }
    let mut v = Value::from(val);
    if let Some(old) = t.get(key).and_then(Item::as_value) {
        *v.decor_mut() = old.decor().clone();
    }
    t[key] = Item::Value(v);
}

/// Writes the value, or removes the key when the value is empty.
fn set_or_remove(t: &mut Table, key: &str, val: &str) {
    if val.trim().is_empty() {
        t.remove(key);
    } else {
        set_str(t, key, val);
    }
}

/// Pure text-in, text-out, so it can be tested without touching any file.
pub fn apply(current: &str, new: &Settings) -> Result<String, String> {
    let mut doc: DocumentMut = current
        .parse()
        .map_err(|e| format!("settings.toml is not valid TOML, so it was left untouched: {e}"))?;

    if !doc.contains_key("general") {
        doc["general"] = Item::Table(Table::new());
    }
    let general = doc["general"].as_table_mut().ok_or("[general] in settings.toml is not a table")?;
    let g = &new.general;
    set_str(general, "yt_dlp", &g.yt_dlp);
    set_str(general, "ffmpeg_dir", &g.ffmpeg_dir);
    set_str(general, "default_dir", &g.default_dir);
    set_str(general, "default_args", &g.default_args);
    set_str(general, "output_template", &g.output_template);
    set_bool(general, "show_toasts", g.show_toasts);

    if new.site.is_empty() {
        doc.remove("site");
    } else {
        if !doc.contains_key("site") {
            doc["site"] = Item::ArrayOfTables(ArrayOfTables::new());
        }
        let sites = doc["site"].as_array_of_tables_mut().ok_or("'site' in settings.toml is not a list of [[site]] tables")?;
        for (i, site) in new.site.iter().enumerate() {
            if i >= sites.len() {
                sites.push(Table::new());
            }
            let t = sites.get_mut(i).expect("just ensured it exists");
            let wanted: Vec<&str> = site.patterns.iter().map(|p| p.trim()).collect();
            let unchanged = t
                .get("patterns")
                .and_then(Item::as_array)
                .map(|a| a.iter().map(|v| v.as_str().unwrap_or("")).eq(wanted.iter().copied()))
                .unwrap_or(false);
            if !unchanged {
                let mut patterns = Array::new();
                for p in &wanted {
                    patterns.push(*p);
                }
                let mut v = Value::from(patterns);
                if let Some(old) = t.get("patterns").and_then(Item::as_value) {
                    *v.decor_mut() = old.decor().clone();
                }
                t["patterns"] = Item::Value(v);
            }
            set_or_remove(t, "dir", &site.dir);
            set_or_remove(t, "args", &site.args);
            if site.inherit_args {
                t.remove("inherit_args"); // true is the default
            } else if t.get("inherit_args").and_then(Item::as_bool) != Some(false) {
                t["inherit_args"] = value(false);
            }
        }
        while sites.len() > new.site.len() {
            sites.remove(sites.len() - 1);
        }
    }
    Ok(doc.to_string())
}

/// Validates, edits settings.toml in place, keeps a backup in the Reynard folder.
pub fn save(new: &Settings) -> Result<(), String> {
    check(new)?;
    let path = paths::settings_path();
    let current = fs::read_to_string(&path).unwrap_or_else(|_| starter_file());
    let updated = apply(&current, new)?;
    // Never write something we couldn't read back.
    toml::from_str::<Settings>(&updated).map_err(|e| format!("internal error, nothing saved: {e}"))?;

    if path.exists() {
        let _ = fs::create_dir_all(paths::data_dir());
        let _ = fs::copy(&path, paths::data_dir().join("settings.backup.toml"));
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, updated).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, &path).map_err(|e| format!("cannot replace {}: {e}", path.display()))
}

/// `*` and `?` wildcards, nothing else.
pub fn wildcard(pat: &str, text: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Splits on spaces, keeps "quoted parts" together. Backslashes are literal,
/// so Windows paths survive.
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut has_token = false;
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                has_token = true;
            }
            c if c.is_whitespace() && !quoted => {
                if has_token {
                    out.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            c => {
                cur.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        out.push(cur);
    }
    out
}

fn starter_file() -> String {
    format!(
        r#"# Reynard settings. Save the file and the next download uses it.
# Paths use 'single quotes' so backslashes are taken literally.

[general]
# Relative paths are relative to the Reynard folder.
yt_dlp = 'bin\yt-dlp.exe'
ffmpeg_dir = 'bin'
# Folders can use Windows variables such as %USERPROFILE%, so no user name is stored here.
default_dir = '{downloads}'
default_args = "--no-playlist --recode-video mp4 --embed-thumbnail --embed-metadata"
output_template = "%(title).150B [%(id)s].%(ext)s"
show_toasts = true

# Site rules: the first match wins. Patterns are matched against the full URL
# (case-insensitive, * and ? wildcards). A rule's args are added after
# default_args; set inherit_args = false to replace default_args instead.
# You can also manage rules on the settings page (toolbar fox > Settings).
#
# Example: send YouTube links to their own folder (remove the # in front of each line)
# [[site]]
# patterns = ["*youtube.com*", "*youtu.be*"]
# dir = 'C:\Users\you\Videos\YouTube'
# args = "--cookies-from-browser firefox"
"#,
        downloads = paths::default_download_setting()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"# Reynard settings. Save the file and the next download uses it.

[general]
# Relative paths are relative to the Reynard folder.
yt_dlp = 'bin\yt-dlp.exe'
ffmpeg_dir = 'bin'
default_dir = 'C:\Users\someone\Downloads'
default_args = "--no-playlist --embed-metadata"
output_template = "%(title).150B [%(id)s].%(ext)s"
show_toasts = true

# Site rules: the first match wins.
[[site]]
patterns = ["*twitter.com*", "*x.com*"]
dir = 'C:\Users\someone\Videos\twitter'
args = "--cookies-from-browser firefox"

[[site]]
patterns = ["*youtube*", "*youtu.be*"]
dir = 'C:\Users\someone\Videos\youtube'
"#;

    #[test]
    fn wildcards() {
        assert!(wildcard("*youtube*", "https://www.youtube.com/watch?v=1"));
        assert!(wildcard("*x.com*", "https://x.com/a/status/1"));
        assert!(wildcard("https://?.com/*", "https://x.com/a"));
        assert!(!wildcard("*twitter.com*", "https://example.org/"));
        assert!(wildcard("*", ""));
    }

    #[test]
    fn splitting() {
        assert_eq!(split_args(r#"--a "b c" d"#), vec!["--a", "b c", "d"]);
        assert_eq!(split_args(r"-P C:\x\y  --z"), vec!["-P", r"C:\x\y", "--z"]);
        assert_eq!(split_args(r#"--o """#), vec!["--o", ""]);
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn rules() {
        let s: Settings = toml::from_str(
            r#"
[general]
default_dir = "D"
default_args = "--no-playlist"
[[site]]
patterns = ["*x.com*"]
dir = "X"
args = "--cookies-from-browser firefox"
[[site]]
patterns = ["*youtu*"]
inherit_args = false
args = "--foo"
"#,
        )
        .unwrap();
        let r = s.resolve("https://X.com/a");
        assert_eq!(r.dir, "X");
        assert_eq!(r.args, vec!["--no-playlist", "--cookies-from-browser", "firefox"]);
        let r = s.resolve("https://youtu.be/1");
        assert_eq!(r.dir, "D");
        assert_eq!(r.args, vec!["--foo"]);
        let r = s.resolve("https://example.org");
        assert!(r.matched.is_none());
        assert_eq!(r.args, vec!["--no-playlist"]);
    }

    fn same(a: &Settings, b: &Settings) -> bool {
        serde_json::to_value(a).unwrap() == serde_json::to_value(b).unwrap()
    }

    #[test]
    fn saving_keeps_comments_and_round_trips() {
        let original = SAMPLE.to_string();
        let mut s: Settings = toml::from_str(&original).unwrap();
        // untouched settings: file content stays identical
        assert_eq!(apply(&original, &s).unwrap(), original);

        s.general.default_dir = r"D:\new place".into();
        s.general.show_toasts = false;
        s.site[0].patterns = vec!["*twitter.com*".into(), "*x.com*".into(), "*nitter*".into()];
        s.site[0].inherit_args = false;
        s.site[1].dir = String::new(); // falls back to the default folder
        s.site.push(Site { patterns: vec!["*vimeo*".into()], dir: r"H:\v".into(), args: "--embed-subs".into(), inherit_args: true });
        let out = apply(&original, &s).unwrap();
        assert!(out.contains("# Reynard settings."), "top comment lost");
        assert!(out.contains("# Site rules: the first match wins."), "section comment lost");
        let back: Settings = toml::from_str(&out).unwrap();
        assert!(same(&s, &back));
        assert!(!out.contains("inherit_args = true"));

        // deleting sites works, and deleting all of them removes the table
        s.site.truncate(1);
        let back: Settings = toml::from_str(&apply(&out, &s).unwrap()).unwrap();
        assert_eq!(back.site.len(), 1);
        s.site.clear();
        let none = apply(&out, &s).unwrap();
        assert!(!none.contains("[[site]]"));
        assert!(toml::from_str::<Settings>(&none).unwrap().site.is_empty());
    }

    #[test]
    fn hand_written_extras_survive_a_save() {
        let text = "[general]\ndefault_dir = 'C:\\d'  # my folder\nmystery = 5\n";
        let s: Settings = toml::from_str(text).unwrap();
        let out = apply(text, &s).unwrap();
        assert!(out.contains("# my folder"));
        assert!(out.contains("mystery = 5"));
        assert!(apply("this is [not toml", &s).is_err());
    }

    #[test]
    fn validation() {
        let mut s: Settings = toml::from_str(SAMPLE).unwrap();
        assert!(check(&s).is_ok());
        s.site[0].patterns.clear();
        assert!(check(&s).is_err());
        s.site[0].patterns = vec!["  ".into()];
        assert!(check(&s).is_err());
        s.site[0].patterns = vec!["*a*".into()];
        s.general.default_dir = " ".into();
        assert!(check(&s).is_err());
        s.general.default_dir = "x".into();
        s.site[0].args = "a\nb".into();
        assert!(check(&s).is_err());
    }

    #[test]
    fn folders_expand_variables_when_used() {
        std::env::set_var("REYNARD_TEST_HOME", "C:\\Users\\someone");
        let s: Settings = toml::from_str(
            r#"
[general]
default_dir = '%REYNARD_TEST_HOME%\Downloads'
[[site]]
patterns = ["*://a.example/*"]
dir = '%REYNARD_TEST_HOME%\Videos\a'
"#,
        )
        .unwrap();
        assert_eq!(s.resolve("https://a.example/x").dir, r"C:\Users\someone\Videos\a");
        assert_eq!(s.resolve("https://other.example/x").dir, r"C:\Users\someone\Downloads");
        // the settings page must still get the raw text back, to edit
        assert_eq!(s.general.default_dir, r"%REYNARD_TEST_HOME%\Downloads");
    }

    #[test]
    fn starter_file_parses_and_is_anonymous() {
        let text = starter_file();
        let s: Settings = toml::from_str(&text).unwrap();
        assert!(s.site.is_empty(), "the starter file ships with no personal site rules");
        assert!(s.general.show_toasts);
        // the only drive/user paths allowed are the neutral example (C:\Users\you)
        // and the current user's own Downloads folder
        assert!(!text.to_lowercase().contains("h:\\"), "starter file contains a personal drive path");
        // the real home folder (which contains the user name) must not be written into the file
        for var in ["USERPROFILE", "HOME"] {
            if let Ok(home) = std::env::var(var) {
                assert!(!text.contains(&home), "starter file contains the real {var} path");
            }
        }
        assert!(text.contains("default_dir = '%"), "default folder should be a variable");
        // saving the starter file untouched must not alter it
        assert_eq!(apply(&text, &s).unwrap(), text);
    }
}
