// Content script, runs only on x.com and twitter.com.
// Videos on X are blob: elements, so the browser gives Reynard no usable address for them and
// Reynard would fall back to the page address (the thread you are looking at, or even
// x.com/i/history). This remembers which post you right-clicked in, so Reynard can download
// that post's own video(s). It only reads the page; it changes nothing on it.
//
// yt-dlp lists a post's own videos first and the quoted post's videos after them. So besides
// the address this works out which numbers of that list were meant ("items", e.g. "1" or "2-3").
// Everything here is a best guess at X's page structure: when a guess fails the answer is just
// "don't know" and Reynard behaves as before. Every right-click writes one line to the page's
// console (F12) starting with [Reynard], to make a wrong guess easy to see.
(() => {
  const api = typeof browser !== "undefined" ? browser : chrome;
  const STATUS = /^\/([A-Za-z0-9_]+)\/status\/(\d+)/;
  let last = null; // {url, items} for the last right-click, or null

  function statusUrl(a) {
    try {
      const m = STATUS.exec(new URL(a.getAttribute("href"), location.origin).pathname);
      return m ? `${location.origin}/${m[1]}/status/${m[2]}` : null;
    } catch (_) {
      return null;
    }
  }

  // The quoted post is a box (role="link") that holds another post's header (a user name)
  // but not the header of the post that quotes it (the first user name in the article).
  function quoteBoxes(article) {
    const first = article.querySelector('[data-testid="User-Name"]');
    return [...article.querySelectorAll('[role="link"]')].filter(
      (b) => b !== article && b.querySelector('[data-testid="User-Name"]') && !(first && b.contains(first))
    );
  }

  function inspect(target) {
    const article = target.closest("article");
    if (!article) return { why: "not inside an article" };
    const boxes = quoteBoxes(article);
    const inBox = (el) => boxes.find((b) => b.contains(el));
    const videos = [...article.querySelectorAll("video")];
    const own = videos.filter((v) => !inBox(v)).length;
    const box = inBox(target);

    // address
    let url = null;
    if (box) {
      for (const a of box.querySelectorAll('a[href*="/status/"]')) {
        url = statusUrl(a);
        if (url) break;
      }
    }
    const quoteUrl = url;
    if (!url) {
      const anchors = [...article.querySelectorAll('a[href*="/status/"]')].filter((a) => !inBox(a));
      const pick = anchors.find((a) => a.querySelector("time")) || anchors.find((a) => statusUrl(a));
      url = pick ? statusUrl(pick) : null;
    }
    if (!url) return { why: "no post link found", boxes: boxes.length, videos: videos.length };

    // which videos
    let items = "";
    if (box) {
      const n = videos.filter((v) => box.contains(v)).length;
      if (n > 0) items = quoteUrl ? `1-${n}` : `${own + 1}-${own + n}`;
    } else if (own > 0) {
      items = own === 1 ? "1" : `1-${own}`;
    }
    return { url, items, boxes: boxes.length, videos: videos.length, own, inQuote: !!box, quoteHasLink: !!quoteUrl };
  }

  document.addEventListener(
    "contextmenu",
    (e) => {
      last = null;
      try {
        if (!(e.target instanceof Element)) return;
        const r = inspect(e.target);
        if (r.url) last = { url: r.url, items: r.items };
        console.log("[Reynard] right-click:", JSON.stringify(r));
      } catch (err) {
        console.log("[Reynard] right-click: error", String(err));
      }
    },
    true
  );

  api.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
    if (msg && msg.type === "reynard-post") sendResponse(last ? { url: last.url, items: last.items } : { url: null });
  });
})();
