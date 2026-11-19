const action = browser.action || browser.browserAction;

// ---------------------------------------------------------------- context menu

// The right-click menu. Firefox puts several top-level items of one add-on into one
// submenu named after the add-on anyway, so there is exactly one top-level entry,
// "Reynard", with the choices inside it. "Download" is your settings as they are;
// the "as" entries change the kind (audio, GIF) and otherwise still follow your
// site rules.
const MENU_CONTEXTS = ["link", "video", "audio", "image", "page"];
const MENU_ROOT = "reynard-root";
const MENU = [
  { id: "reynard-download", title: "Download", kind: "default" },
  { id: "reynard-sep", type: "separator" },
  // alphabetical, on purpose
  { id: "reynard-audio", title: "Download as audio", kind: "audio" },
  { id: "reynard-gif", title: "Download as GIF", kind: "gif" },
  { id: "reynard-video", title: "Download as video", kind: "video" },
];
const KIND_OF = Object.fromEntries(MENU.filter((m) => m.kind).map((m) => [m.id, m.kind]));

// Firefox never keeps the menu items of a persistent background page, so they
// have to be created every time this page loads. (Creating them only in
// runtime.onInstalled leaves the menu missing after a restart or a reload of the
// background page, until the add-on is reinstalled.) removeAll first keeps this
// safe to run more than once.
function createMenu() {
  return browser.contextMenus
    .removeAll()
    .then(() => {
      browser.contextMenus.create({ id: MENU_ROOT, title: REYNARD_LABEL, contexts: MENU_CONTEXTS });
      for (const item of MENU) {
        const entry = { id: item.id, parentId: MENU_ROOT, contexts: MENU_CONTEXTS };
        if (item.type) entry.type = item.type;
        else entry.title = item.title;
        browser.contextMenus.create(entry);
      }
    })
    .catch((err) => console.error("[Reynard] could not create the menu:", err));
}
if (REYNARD_CHROME) {
  // Chrome keeps the menu items between runs of its service worker, and this script runs
  // again every time the worker wakes up, so they are only made when the add-on is
  // installed or updated.
  browser.runtime.onInstalled.addListener(createMenu);
} else {
  createMenu();
}

// Prefer the link, then the media source, then the page itself.
// blob:/data: addresses are useless to yt-dlp, so they are skipped
// (on YouTube the video element's src is a blob: -> falls back to the page URL).
function pickUrl(info) {
  const candidates = [info.linkUrl, info.srcUrl, info.pageUrl];
  return candidates.find((u) => u && /^https?:/i.test(u)) || null;
}

// On X, a right-click on a video gives no usable address (blob:), so pickUrl lands on the page
// address, which is the wrong post or not a post at all. Then the content script (x-post.js)
// is asked which post was under the cursor. Anything else is left exactly as pickUrl says.
const X_PAGE = /^https?:\/\/(?:www\.)?(?:x|twitter)\.com(?:[/?#]|$)/i;
const X_POST = /^https:\/\/(?:x|twitter)\.com\/[A-Za-z0-9_]+\/status\/\d+$/;
async function pickUrlWithPost(info, tab) {
  const url = pickUrl(info);
  const plain = { url, items: "" };
  if (!url || url !== info.pageUrl || !X_PAGE.test(url) || !tab || tab.id == null) return plain;
  try {
    const reply = await browser.tabs.sendMessage(tab.id, { type: "reynard-post" }, { frameId: info.frameId || 0 });
    if (reply && typeof reply.url === "string" && X_POST.test(reply.url)) {
      const items = typeof reply.items === "string" && /^\d{1,3}(-\d{1,3})?$/.test(reply.items) ? reply.items : "";
      return { url: reply.url, items };
    }
  } catch (err) {
    // Usually: this tab was open before the add-on was installed or reloaded. Reload the tab.
    console.log("[Reynard] no answer from the page (reload the tab?), using the page address:", err.message);
  }
  return plain;
}

// Every message is also written to the Browser Console (Ctrl+Shift+J).
// No desktop notifications: the toolbar fox is the feedback. A download that could not
// be started turns it red for a few seconds (details in the tooltip and the console).
function startFailed(message) {
  console.error("[Reynard]", message);
  show("error", `${REYNARD_LABEL}: ${message}`);
  schedule(ERROR_SECONDS * 1000);
}

browser.contextMenus.onClicked.addListener(async (info, tab) => {
  const kind = KIND_OF[info.menuItemId];
  if (!kind) return; // not one of ours (or the "Reynard" parent)
  const { url, items } = await pickUrlWithPost(info, tab);
  if (!url) return startFailed("no downloadable URL here");
  try {
    const reply = await browser.runtime.sendNativeMessage(REYNARD_HOST, { type: "download", url, kind, items });
    if (!reply || !reply.ok) {
      return startFailed("could not start: " + ((reply && reply.error) || "no answer from Reynard"));
    }
    const what = reply.kind || "video";
    console.log(`[Reynard] ${what} download #${reply.id} started -> ${reply.dir}`);
    schedule(0); // update the icon right away
  } catch (err) {
    startFailed("could not reach Reynard: " + err.message);
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
let lastPoll = 0;

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
  // Chrome stops a service worker that has been idle for about 30 seconds. While a download
  // runs or a state is fading, the polling itself keeps it awake. When idle it is left to
  // sleep, and the alarm below wakes it up for the next look.
  if (REYNARD_CHROME && ms >= IDLE_POLL_MS) return;
  timer = setTimeout(poll, ms);
}

async function poll() {
  lastPoll = Date.now();
  let next = IDLE_POLL_MS;
  try {
    const reply = await browser.runtime.sendNativeMessage(REYNARD_HOST, { type: "jobs" });
    if (!reply || !reply.ok) throw new Error((reply && reply.error) || "no answer");
    const { jobs, now } = reply;
    const within = (j, seconds) => j.finished && now - j.finished <= seconds;

    const active = jobs.filter((j) => j.status === "running" || j.status === "queued");
    if (active.length) {
      const many = active.length > 1;
      const p = active[0].progress || "";
      const count = many ? ` (${active.length})` : "";
      // "Converting..." replaces the word "downloading" instead of following it.
      const text = /^Converting/i.test(p)
        ? `${p.charAt(0).toLowerCase()}${p.slice(1)}${count}`
        : `downloading${count}${p ? ` ${p}` : ""}`;
      show("active", `${REYNARD_LABEL}: ${text}`, many ? String(active.length) : "");
      next = ACTIVE_POLL_MS;
    } else if (jobs.some((j) => j.status === "error" && within(j, ERROR_SECONDS))) {
      show("error", `${REYNARD_LABEL}: a download failed`);
      next = FADE_POLL_MS;
    } else if (jobs.some((j) => j.status === "done" && within(j, DONE_SECONDS))) {
      show("done", `${REYNARD_LABEL}: download finished`);
      next = FADE_POLL_MS;
    } else {
      show("idle", `${REYNARD_LABEL}: idle`);
    }
  } catch (err) {
    show("error", `${REYNARD_LABEL}: can't reach the Reynard program (is it installed?)`);
    console.error("[Reynard] status check failed:", err);
  }
  schedule(next);
}

if (REYNARD_CHROME) {
  // Wakes the sleeping worker; running this script then polls once by itself, so the
  // alarm only polls if the worker was already awake and the last poll is a while ago.
  browser.alarms.create("reynard-poll", { periodInMinutes: 0.5 });
  browser.alarms.onAlarm.addListener(() => {
    if (Date.now() - lastPoll > 5000) poll();
  });
}

poll();
