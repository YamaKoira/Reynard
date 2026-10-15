const HOST = "io.reynard.host";
const action = browser.action || browser.browserAction;

// ---------------------------------------------------------------- context menu

browser.runtime.onInstalled.addListener(() => {
  browser.contextMenus.create({
    id: "reynard-download",
    title: "Download with Reynard",
    contexts: ["link", "video", "audio", "image", "page"],
  });
});

// Prefer the link, then the media source, then the page itself.
// blob:/data: addresses are useless to yt-dlp, so they are skipped
// (on YouTube the video element's src is a blob: -> falls back to the page URL).
function pickUrl(info) {
  const candidates = [info.linkUrl, info.srcUrl, info.pageUrl];
  return candidates.find((u) => u && /^https?:/i.test(u)) || null;
}

// Every message is also written to the Browser Console (Ctrl+Shift+J).
// Desktop toasts are optional (settings.toml: show_toasts); the toolbar icon
// below is the main feedback.
function notify(message) {
  console.log("[Reynard]", message);
  browser.notifications
    .create({
      type: "basic",
      title: "Reynard",
      message,
      iconUrl: browser.runtime.getURL("icon.png"),
    })
    .catch((err) => console.error("[Reynard] could not show notification:", err));
}

browser.contextMenus.onClicked.addListener(async (info) => {
  const url = pickUrl(info);
  if (!url) return notify("No downloadable URL here.");
  try {
    const reply = await browser.runtime.sendNativeMessage(HOST, { type: "download", url });
    if (!reply || !reply.ok) {
      return notify("Could not start: " + ((reply && reply.error) || "no answer from Reynard"));
    }
    console.log(`[Reynard] download #${reply.id} started -> ${reply.dir}`);
    if (reply.toast !== false) notify(`Download #${reply.id} started -> ${reply.dir}`);
    schedule(0); // update the icon right away
  } catch (err) {
    notify("Could not reach Reynard: " + err.message);
    schedule(0);
  }
});

// ---------------------------------------------------------------- toolbar icon
//
// idle (sleeping grey fox) -> downloading (orange, count badge if several)
// -> done (green, 5 s) / failed (red, 10 s). Red also means "can't reach Reynard".
// The state is worked out from the job list every time, so nothing is lost if
// the page restarts.

const ICONS = {
  idle: "icons/idle.png",
  active: "icons/active.png",
  done: "icons/done.png",
  error: "icons/error.png",
};
const DONE_SECONDS = 5;
const ERROR_SECONDS = 10;
const IDLE_POLL_MS = 15000; // slow: only to notice downloads started elsewhere
const ACTIVE_POLL_MS = 2000;
const FADE_POLL_MS = 1000; // while done/failed is showing, to time it out

let timer = null;
let shown = "";

function show(state, title, badge = "") {
  const key = `${state}|${title}|${badge}`;
  if (key === shown) return;
  shown = key;
  action.setIcon({ path: ICONS[state] });
  action.setTitle({ title });
  action.setBadgeText({ text: badge });
  action.setBadgeBackgroundColor({ color: "#e8742a" });
}

function schedule(ms) {
  clearTimeout(timer);
  timer = setTimeout(poll, ms);
}

async function poll() {
  let next = IDLE_POLL_MS;
  try {
    const reply = await browser.runtime.sendNativeMessage(HOST, { type: "jobs" });
    if (!reply || !reply.ok) throw new Error((reply && reply.error) || "no answer");
    const { jobs, now } = reply;
    const within = (j, seconds) => j.finished && now - j.finished <= seconds;

    const active = jobs.filter((j) => j.status === "running" || j.status === "queued");
    if (active.length) {
      const many = active.length > 1;
      const detail = active[0].progress ? ` ${active[0].progress}` : "";
      show("active", `Reynard: downloading${many ? ` (${active.length})` : ""}${detail}`, many ? String(active.length) : "");
      next = ACTIVE_POLL_MS;
    } else if (jobs.some((j) => j.status === "error" && within(j, ERROR_SECONDS))) {
      show("error", "Reynard: a download failed");
      next = FADE_POLL_MS;
    } else if (jobs.some((j) => j.status === "done" && within(j, DONE_SECONDS))) {
      show("done", "Reynard: download finished");
      next = FADE_POLL_MS;
    } else {
      show("idle", "Reynard: idle");
    }
  } catch (err) {
    show("error", "Reynard: can't reach reynard.exe (has setup.bat been run?)");
    console.error("[Reynard] status check failed:", err);
  }
  schedule(next);
}

poll();
