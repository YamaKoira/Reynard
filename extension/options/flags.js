"use strict";
// Turns a yt-dlp flag string into toggles and back. No DOM in here.
//
// Anything we don't recognise is kept as "extra" tokens, so flags you pasted by
// hand are never lost or hidden by the toggles.

const Flags = (() => {
  const HEIGHTS = ["2160", "1440", "1080", "720", "480"];
  const qualityToken = (h) => `bv*[height<=${h}]+ba/b[height<=${h}]`;
  const QUALITY_RE = /^bv\*\[height<=(\d+)\]\+ba\/b\[height<=\1\]$/;
  const BROWSERS = ["firefox", "chrome", "edge", "brave", "chromium", "opera", "vivaldi"];
  const AUDIO_FORMATS = ["mp3", "m4a", "opus", "flac"];

  // Order here = order in the page = order of the generated flags.
  const CONTROLS = [
    { id: "noPlaylist", type: "toggle", flag: "--no-playlist", label: "Single video only", help: "Never download a whole playlist" },
    { id: "quality", type: "select", label: "Max resolution", help: "Best video up to this height",
      options: [["", "Best available"], ...HEIGHTS.map((h) => [h, h + "p"])] },
    { id: "recode", type: "select", flag: "--recode-video", label: "Convert video to", help: "Re-encode after download",
      options: [["", "Keep original"], ["mp4", "mp4"], ["mkv", "mkv"], ["webm", "webm"]] },
    { id: "audio", type: "select", label: "Audio only", help: "Download just the sound",
      options: [["", "Off (video)"], ["best", "Best (original format)"], ...AUDIO_FORMATS.map((a) => [a, a])] },
    { id: "embedThumbnail", type: "toggle", flag: "--embed-thumbnail", label: "Embed thumbnail", help: "Cover image inside the file" },
    { id: "embedMetadata", type: "toggle", flag: "--embed-metadata", label: "Embed metadata", help: "Title, uploader, date inside the file" },
    { id: "embedSubs", type: "toggle", flag: "--embed-subs", label: "Embed subtitles", help: "Subtitles inside the file, when available" },
    { id: "cookies", type: "select", flag: "--cookies-from-browser", label: "Cookies from browser", help: "Act as if logged in (private or age-gated posts)",
      options: [["", "None"], ...BROWSERS.map((b) => [b, b])] },
  ];

  // Same rules as split_args in the Rust code: spaces separate, "quotes" group,
  // backslashes are literal (Windows paths).
  function tokenize(s) {
    const out = [];
    let cur = "";
    let quoted = false;
    let has = false;
    for (const c of s) {
      if (c === '"') {
        quoted = !quoted;
        has = true;
      } else if (/\s/.test(c) && !quoted) {
        if (has) {
          out.push(cur);
          cur = "";
          has = false;
        }
      } else {
        cur += c;
        has = true;
      }
    }
    if (has) out.push(cur);
    return out;
  }

  const quote = (t) => (t === "" ? '""' : /\s/.test(t) ? `"${t}"` : t);
  const join = (tokens) => tokens.map(quote).join(" ");

  // "--flag value" or "--flag=value" -> { value, used }, else null
  function takeValue(tokens, i, names) {
    for (const name of names) {
      if (tokens[i] === name && tokens[i + 1] !== undefined) return { value: tokens[i + 1], used: 2 };
      if (tokens[i].startsWith(name + "=")) return { value: tokens[i].slice(name.length + 1), used: 1 };
    }
    return null;
  }

  function parse(argString) {
    const tokens = tokenize(argString || "");
    const values = {};
    const extra = [];
    const hasX = tokens.includes("-x") || tokens.includes("--extract-audio");
    const listed = (id, v) => CONTROLS.find((c) => c.id === id).options.some(([val]) => val === v && v !== "");

    for (let i = 0; i < tokens.length; ) {
      const t = tokens[i];
      const toggle = CONTROLS.find((c) => c.type === "toggle" && c.flag === t);
      if (toggle) {
        values[toggle.id] = true;
        i++;
        continue;
      }
      if (t === "-x" || t === "--extract-audio") {
        values.audio = values.audio || "best";
        i++;
        continue;
      }
      let hit;
      if (hasX && (hit = takeValue(tokens, i, ["--audio-format"])) && AUDIO_FORMATS.includes(hit.value)) {
        values.audio = hit.value;
        i += hit.used;
        continue;
      }
      if ((hit = takeValue(tokens, i, ["--recode-video"])) && listed("recode", hit.value)) {
        values.recode = hit.value;
        i += hit.used;
        continue;
      }
      if ((hit = takeValue(tokens, i, ["--cookies-from-browser"])) && listed("cookies", hit.value)) {
        values.cookies = hit.value;
        i += hit.used;
        continue;
      }
      if ((hit = takeValue(tokens, i, ["-f", "--format"]))) {
        const m = QUALITY_RE.exec(hit.value);
        if (m && HEIGHTS.includes(m[1])) {
          values.quality = m[1];
          i += hit.used;
          continue;
        }
      }
      extra.push(t); // not ours: keep it, untouched
      i++;
    }
    return { values, extra };
  }

  function compose(values, extra = []) {
    const t = [];
    if (values.noPlaylist) t.push("--no-playlist");
    if (values.quality) t.push("-f", qualityToken(values.quality));
    if (values.recode) t.push("--recode-video", values.recode);
    if (values.audio) {
      t.push("-x");
      if (values.audio !== "best") t.push("--audio-format", values.audio);
    }
    if (values.embedThumbnail) t.push("--embed-thumbnail");
    if (values.embedMetadata) t.push("--embed-metadata");
    if (values.embedSubs) t.push("--embed-subs");
    if (values.cookies) t.push("--cookies-from-browser", values.cookies);
    return t.concat(extra);
  }

  // Same wildcard rules as the Rust matcher: * and ?, case handled by the caller.
  function wildcard(pat, text) {
    let p = 0, t = 0, star = -1, mark = 0;
    while (t < text.length) {
      if (p < pat.length && (pat[p] === "?" || pat[p] === text[t])) { p++; t++; }
      else if (p < pat.length && pat[p] === "*") { star = p; mark = t; p++; }
      else if (star !== -1) { p = star + 1; mark++; t = mark; }
      else return false;
    }
    while (p < pat.length && pat[p] === "*") p++;
    return p === pat.length;
  }

  return { CONTROLS, tokenize, join, parse, compose, wildcard };
})();
