"use strict";
const HOST = "io.reynard.host";
const $ = (sel) => document.querySelector(sel);
const send = (msg) => browser.runtime.sendNativeMessage(HOST, msg);

// Small DOM helper. Text always goes in as text nodes, never as HTML, because
// URLs and error messages come from the outside world.
function el(tag, props = {}, ...kids) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === "class") node.className = v;
    else if (k.startsWith("on")) node.addEventListener(k.slice(2), v);
    else node.setAttribute(k, v);
  }
  node.append(...kids.filter((x) => x !== null && x !== undefined && x !== false));
  return node;
}

// ------------------------------------------------------------------ messages

let msgTimer;
function flash(text, bad = true) {
  const box = $("#msg");
  box.textContent = text;
  box.className = bad ? "bad" : "good";
  box.hidden = false;
  clearTimeout(msgTimer);
  msgTimer = setTimeout(() => (box.hidden = true), 4000);
}

async function act(msg, okText) {
  try {
    const reply = await send(msg);
    if (!reply || !reply.ok) flash((reply && reply.error) || "No answer from Reynard");
    else if (okText) flash(okText, false);
  } catch (err) {
    flash("Can't reach Reynard: " + err.message);
  }
  refresh();
}

// ---------------------------------------------------------------------- tabs

function selectTab(name) {
  if (name === "settings") {
    // The settings live on their own page (a popup closes when you click away).
    browser.runtime.openOptionsPage();
    window.close();
    return;
  }
  document.querySelectorAll(".tab").forEach((t) => t.classList.toggle("active", t.dataset.tab === name));
  $("#downloads").hidden = name !== "downloads";
  $("#status").hidden = name !== "status";
  if (name === "status") loadStatus();
  else refresh();
}
document.querySelectorAll(".tab").forEach((t) => t.addEventListener("click", () => selectTab(t.dataset.tab)));

// ---------------------------------------------------------------- downloads

const LABELS = {
  queued: ["QUEUED", "blue"],
  running: ["DOWNLOADING", "blue"],
  done: ["DONE", "green"],
  error: ["FAILED", "red"],
  cancelled: ["CANCELLED", "grey"],
};

function siteName(url) {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

function mini(label, cls, title, onclick) {
  return el("button", { class: `btn mini ${cls}`.trim(), title, onclick }, label);
}

function card(job) {
  const [label, color] = LABELS[job.status] || [String(job.status).toUpperCase(), "grey"];
  const active = job.status === "running" || job.status === "queued";

  // One compact header row: site, status, and the buttons that make sense now.
  const buttons = [];
  if (active) buttons.push(mini("Cancel", "danger", "Stop this download", () => act({ type: "kill", id: job.id })));
  else if (job.status === "done") {
    buttons.push(mini("Open", "", "Open the file", () => act({ type: "open", id: job.id }, "Opening...")));
    buttons.push(mini("Folder", "", "Show the file in its folder", () => act({ type: "reveal", id: job.id }, "Opening the folder...")));
  }
  const kids = [
    el("div", { class: "row" }, el("span", { class: "site", title: siteName(job.url) }, siteName(job.url)), el("span", { class: `pill ${color}` }, label), ...buttons),
    el("div", { class: "line", title: job.url }, job.url),
    el("div", { class: "line", title: job.dir }, job.dir),
  ];

  if (active) {
    const pct = parseFloat(job.progress);
    const fill = el("div", { class: "fill" });
    if (!Number.isNaN(pct)) fill.style.width = Math.max(0, Math.min(100, pct)) + "%";
    kids.push(el("div", { class: "bar" + (Number.isNaN(pct) ? " unknown" : "") }, fill));
    kids.push(el("div", { class: "prog" }, job.progress || "Starting..."));
  } else if (job.status === "error" && job.error) {
    kids.push(el("div", { class: "err" }, job.error));
  }
  return el("div", { class: `card ${color}` }, ...kids);
}

function renderJobs(jobs) {
  const list = $("#list");
  if (!jobs.length) list.replaceChildren(el("div", { class: "empty" }, "No downloads yet. Right-click something to start one."));
  else list.replaceChildren(...[...jobs].reverse().map(card));
  $("#clear").disabled = !jobs.some((j) => j.status !== "running" && j.status !== "queued");
}

let lastJobs = "";
let refreshing = false;
async function refresh() {
  if (refreshing) return;
  refreshing = true;
  try {
    const reply = await send({ type: "jobs" });
    if (!reply || !reply.ok) throw new Error((reply && reply.error) || "no answer");
    const json = JSON.stringify(reply.jobs);
    if (json !== lastJobs) {
      lastJobs = json;
      renderJobs(reply.jobs);
    }
    $("#offline").hidden = true;
  } catch (err) {
    $("#offline").textContent = "Can't reach reynard.exe. Has setup.bat been run? (" + err.message + ")";
    $("#offline").hidden = false;
  } finally {
    refreshing = false;
  }
}

$("#clear").addEventListener("click", async () => {
  await act({ type: "clear" });
  lastJobs = ""; // force a redraw
  refresh();
});

setInterval(() => {
  if (!$("#downloads").hidden) refresh();
}, 1500);

// ------------------------------------------------------------------- status

const LATEST_KEY = "ytdlpLatest";
const SIX_HOURS = 6 * 3600 * 1000;
let installed = null;
let latest = null;

function cmpVersion(a, b) {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const x = pa[i] || 0;
    const y = pb[i] || 0;
    if (x !== y) return x < y ? -1 : 1;
  }
  return 0;
}

async function latestYtdlp(force) {
  const stored = (await browser.storage.local.get(LATEST_KEY))[LATEST_KEY];
  if (!force && stored && Date.now() - stored.at < SIX_HOURS) return stored.tag;
  const res = await fetch("https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest", {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!res.ok) throw new Error("GitHub answered " + res.status);
  const tag = (await res.json()).tag_name;
  await browser.storage.local.set({ [LATEST_KEY]: { tag, at: Date.now() } });
  return tag;
}

function renderYtdlp() {
  const box = $("#st-ytdlp");
  if (!installed) {
    box.textContent = "not found (run setup.bat)";
    $("#update").disabled = true;
    return;
  }
  box.replaceChildren(installed);
  const outdated = latest && cmpVersion(installed, latest) < 0;
  if (latest) {
    box.append(el("span", { class: "tag " + (outdated ? "old" : "fresh") }, outdated ? `outdated, ${latest} available` : "up to date"));
  }
  $("#update").disabled = !outdated;
}

async function checkLatest(force) {
  try {
    latest = await latestYtdlp(force);
  } catch (err) {
    latest = null;
    if (force) flash("Could not check for updates: " + err.message);
  }
  renderYtdlp();
}

async function loadStatus() {
  $("#st-reynard").textContent = "checking...";
  $("#dot").className = "dot";
  try {
    const reply = await send({ type: "status" });
    if (!reply || !reply.ok) throw new Error((reply && reply.error) || "no answer");
    $("#dot").className = "dot ok";
    $("#st-reynard").textContent = "Ready, v" + reply.version;
    $("#st-settings").textContent = reply.settings_path;
    installed = reply.ytdlp;
    renderYtdlp();
    await checkLatest(false);
  } catch (err) {
    $("#dot").className = "dot bad";
    $("#st-reynard").textContent = "Can't reach reynard.exe";
    $("#st-ytdlp").textContent = "-";
    flash(err.message);
  }
}

$("#check").addEventListener("click", () => checkLatest(true));

$("#update").addEventListener("click", async () => {
  const button = $("#update");
  const out = $("#update-out");
  button.disabled = true;
  button.textContent = "Updating...";
  out.hidden = true;
  try {
    const reply = await send({ type: "update_ytdlp" });
    out.textContent = (reply && (reply.output || reply.error)) || "No answer";
    out.hidden = false;
  } catch (err) {
    flash("Can't reach Reynard: " + err.message);
  }
  button.textContent = "Update yt-dlp";
  loadStatus();
});

refresh();
