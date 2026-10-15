"use strict";
const HOST = "io.reynard.host";
const $ = (sel) => document.querySelector(sel);
const send = (msg) => browser.runtime.sendNativeMessage(HOST, msg);

// Text always goes in as text nodes, never HTML (URLs and paths come from outside).
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

let bannerTimer;
function banner(text, bad = true) {
  const box = $("#banner");
  box.textContent = text;
  box.className = "banner " + (bad ? "bad" : "good");
  box.hidden = false;
  clearTimeout(bannerTimer);
  if (!bad) bannerTimer = setTimeout(() => (box.hidden = true), 4000);
}

// ------------------------------------------------------------- flag toggles

// One editor = the toggles (or a raw text box in "Custom flags" mode) for one
// set of yt-dlp flags. Flags it doesn't recognise are kept, never dropped.
function flagsEditor(initialArgs, { additive = false, headLeft = null } = {}) {
  const state = { custom: false, values: {}, extra: [], raw: "", additive };
  let touched = false; // until you touch it, we hand back your original text unchanged
  const root = el("div", { class: "flags" });

  function load(args) {
    const parsed = Flags.parse(args);
    state.values = parsed.values;
    state.extra = parsed.extra;
  }
  load(initialArgs);

  function changed() {
    touched = true;
    root.dispatchEvent(new Event("flagschange", { bubbles: true }));
  }

  function getArgs() {
    if (!touched) return initialArgs;
    return state.custom ? state.raw.trim() : Flags.join(Flags.compose(state.values, state.extra));
  }

  function setCustom(on) {
    if (on === state.custom) return;
    if (on) state.raw = Flags.join(Flags.compose(state.values, state.extra));
    else load(state.raw);
    state.custom = on;
    changed();
    render();
  }

  function render() {
    const customBox = el("input", { type: "checkbox", class: "custom-switch" });
    customBox.checked = state.custom;
    customBox.addEventListener("change", () => setCustom(customBox.checked));
    const parts = [el("div", { class: "head" }, headLeft || el("span", {}), el("label", { class: "check" }, customBox, "Custom flags"))];

    if (state.custom) {
      const area = el("textarea", { spellcheck: "false", placeholder: "--no-playlist --embed-thumbnail ..." });
      area.value = state.raw;
      area.addEventListener("input", () => {
        state.raw = area.value;
        changed();
      });
      parts.push(area, el("div", { class: "hint" }, "Any yt-dlp options. Switch Custom flags off to get toggles again; flags the toggles don't know are kept."));
    } else {
      const grid = el("div", { class: "grid" });
      for (const c of Flags.CONTROLS) {
        if (c.type === "toggle") {
          const box = el("input", { type: "checkbox", "data-id": c.id });
          box.checked = !!state.values[c.id];
          box.addEventListener("change", () => {
            if (box.checked) state.values[c.id] = true;
            else delete state.values[c.id];
            changed();
          });
          grid.append(el("label", { class: "opt", title: c.help }, box, el("span", { class: "name" }, c.label)));
        } else {
          const select = el("select", { "data-id": c.id });
          for (const [value, label] of c.options) {
            select.append(el("option", { value }, value === "" && state.additive ? "Default" : label));
          }
          select.value = state.values[c.id] || "";
          select.addEventListener("change", () => {
            if (select.value) state.values[c.id] = select.value;
            else delete state.values[c.id];
            changed();
          });
          grid.append(el("label", { class: "opt sel", title: c.help }, el("span", { class: "name" }, c.label), select));
        }
      }
      parts.push(grid);
      if (state.extra.length) {
        parts.push(el("div", { class: "hint" }, "Other flags, kept as they are: ", el("code", {}, Flags.join(state.extra)), ". Switch on Custom flags to edit them."));
      }
    }
    root.replaceChildren(...parts);
  }
  render();

  return {
    root,
    getArgs,
    setAdditive(value) {
      state.additive = value;
      render();
    },
  };
}

// -------------------------------------------------------------------- sites

// Commas, semicolons, spaces and line breaks all separate patterns (a link never
// contains a space, so a space can only mean "next pattern").
const parsePatterns = (text) => text.split(/[\s,;]+/).filter(Boolean);
let defaults = null;
let sites = [];

async function browseInto(input) {
  try {
    const reply = await send({ type: "pick_folder", start: input.value.trim() });
    if (!reply || !reply.ok) return banner((reply && reply.error) || "No answer from Reynard");
    if (reply.path) {
      input.value = reply.path;
      input.dispatchEvent(new Event("input", { bubbles: true }));
    }
  } catch (err) {
    banner("Can't reach Reynard: " + err.message);
  }
}

function siteComponent(site) {
  const patterns = el("input", { type: "text", spellcheck: "false", placeholder: "*example.com*, *another.org*   (separate with commas or spaces)", title: "Separate with commas or spaces. Patterns match the whole link, so you usually want a * at both ends, like *youtube.com*." });
  patterns.value = (site.patterns || []).join(", ");
  const dir = el("input", { type: "text", spellcheck: "false", placeholder: "Empty = the default folder" });
  dir.value = site.dir || "";
  const inherit = el("input", { type: "checkbox", class: "inherit" });
  inherit.checked = site.inherit_args !== false;
  const inheritLabel = el("label", { class: "check", title: "Ticked: this rule's flags are added on top of the default flags. Unticked: only this rule's own flags are used." }, inherit, "Start from the default flags");
  const editor = flagsEditor(site.args || "", { additive: inherit.checked, headLeft: inheritLabel });
  inherit.addEventListener("change", () => editor.setAdditive(inherit.checked));

  const title = el("span", { class: "title" });
  const up = el("button", { class: "btn small up", type: "button", title: "Move up (earlier rules win)", onclick: () => move(comp, -1) }, "\u2191");
  const down = el("button", { class: "btn small down", type: "button", title: "Move down", onclick: () => move(comp, 1) }, "\u2193");
  const del = el("button", { class: "btn small danger delete", type: "button", onclick: () => remove(comp) }, "Delete");

  const root = el(
    "div",
    { class: "site" },
    el("div", { class: "site-head" }, title, up, down, del),
    el("label", { class: "field" }, el("span", { class: "label" }, "Patterns"), patterns),
    el("label", { class: "field" }, el("span", { class: "label" }, "Folder"),
      el("span", { class: "with-button" }, dir, el("button", { class: "btn browse", type: "button", onclick: () => browseInto(dir) }, "Browse..."))),
    editor.root
  );

  const comp = {
    root,
    patterns,
    dir,
    inherit,
    editor,
    label(index, count) {
      const all = parsePatterns(patterns.value);
      title.textContent = `Rule ${index + 1}` + (all.length ? `: ${all.join(", ")}` : "");
      title.title = all.length > 1 ? `${all.length} patterns` : "";
      up.disabled = index === 0;
      down.disabled = index === count - 1;
    },
    get: () => ({ patterns: parsePatterns(patterns.value), dir: dir.value.trim(), args: editor.getArgs(), inherit_args: inherit.checked }),
  };
  patterns.addEventListener("input", renderSites);
  return comp;
}

function renderSites() {
  sites.forEach((s, i) => s.label(i, sites.length));
  const nodes = sites.map((s) => s.root);
  const current = [...$("#sites").children];
  if (nodes.length !== current.length || nodes.some((n, i) => n !== current[i])) $("#sites").replaceChildren(...nodes);
  $("#no-sites").hidden = sites.length > 0;
}

function move(comp, delta) {
  const i = sites.indexOf(comp);
  const j = i + delta;
  if (j < 0 || j >= sites.length) return;
  [sites[i], sites[j]] = [sites[j], sites[i]];
  renderSites();
  updateDirty();
}

function remove(comp) {
  if (!confirm("Delete this site rule?")) return;
  sites = sites.filter((s) => s !== comp);
  renderSites();
  updateDirty();
}

$("#add-site").addEventListener("click", () => {
  const comp = siteComponent({ patterns: [], dir: "", args: "", inherit_args: true });
  sites.push(comp);
  renderSites();
  updateDirty();
  if (comp.root.scrollIntoView) comp.root.scrollIntoView({ block: "center" });
  comp.patterns.focus();
});

// ------------------------------------------------------------ whole settings

function collect() {
  return {
    general: {
      yt_dlp: $("#yt-dlp").value.trim(),
      ffmpeg_dir: $("#ffmpeg-dir").value.trim(),
      default_dir: $("#default-dir").value.trim(),
      default_args: defaults ? defaults.getArgs() : "",
      output_template: $("#output-template").value.trim(),
      show_toasts: $("#show-toasts").checked,
    },
    site: sites.map((s) => s.get()),
  };
}

function validate() {
  document.querySelectorAll(".invalid").forEach((n) => n.classList.remove("invalid"));
  const problems = [];
  if (!$("#default-dir").value.trim()) {
    $("#default-dir").classList.add("invalid");
    problems.push("The default download folder can't be empty.");
  }
  sites.forEach((s, i) => {
    if (!parsePatterns(s.patterns.value).length) {
      s.patterns.classList.add("invalid");
      problems.push(`Rule ${i + 1} needs at least one pattern.`);
    }
  });
  return problems;
}

let baseline = "";
function updateDirty() {
  const dirty = JSON.stringify(collect()) !== baseline;
  $("#save").disabled = !dirty;
  $("#revert").disabled = !dirty;
  $("#dirty").textContent = dirty ? "Unsaved changes" : "";
  renderTry();
  return dirty;
}
["input", "change", "flagschange"].forEach((type) => document.addEventListener(type, updateDirty));
window.addEventListener("beforeunload", (e) => {
  if (updateDirty()) e.preventDefault();
});

function fill(settings) {
  const g = settings.general;
  $("#default-dir").value = g.default_dir || "";
  $("#yt-dlp").value = g.yt_dlp || "";
  $("#ffmpeg-dir").value = g.ffmpeg_dir || "";
  $("#output-template").value = g.output_template || "";
  $("#show-toasts").checked = !!g.show_toasts;
  defaults = flagsEditor(g.default_args || "");
  $("#default-flags").replaceChildren(defaults.root);
  sites = (settings.site || []).map(siteComponent);
  renderSites();
}

async function load() {
  try {
    const reply = await send({ type: "get_settings" });
    if (!reply || !reply.ok) throw new Error((reply && reply.error) || "no answer");
    fill(reply.settings);
    $("#path").textContent = reply.path;
    $("#banner").hidden = true;
    baseline = JSON.stringify(collect());
    updateDirty();
  } catch (err) {
    banner("Can't read the settings: " + err.message);
  }
}

async function save() {
  const problems = validate();
  if (problems.length) return banner(problems[0]);
  const settings = collect();
  try {
    const reply = await send({ type: "save_settings", settings });
    if (!reply || !reply.ok) return banner((reply && reply.error) || "No answer from Reynard");
    baseline = JSON.stringify(settings);
    banner("Saved. The next download uses these settings.", false);
    updateDirty();
  } catch (err) {
    banner("Can't reach Reynard: " + err.message);
  }
}

// ------------------------------------------------------------- "try a link"

function renderTry() {
  const url = $("#try").value.trim();
  const out = $("#try-result");
  if (!url) return out.replaceChildren();
  const now = collect();
  const lower = url.toLowerCase();
  let rule = -1;
  let pattern = "";
  now.site.some((s, i) => {
    const hit = s.patterns.find((p) => Flags.wildcard(p.toLowerCase(), lower));
    if (hit) {
      rule = i;
      pattern = hit;
    }
    return !!hit;
  });
  const site = rule >= 0 ? now.site[rule] : null;
  const folder = (site && site.dir) || now.general.default_dir;
  const flags = site ? (site.inherit_args ? [now.general.default_args, site.args] : [site.args]).filter(Boolean).join(" ") : now.general.default_args;
  out.replaceChildren(
    el("div", {}, el("b", {}, site ? `Rule ${rule + 1}` : "No rule matches"), site ? ` (pattern ${pattern})` : ": the defaults are used"),
    el("div", {}, "Folder: ", el("code", {}, folder)),
    el("div", {}, "Flags: ", el("code", {}, flags || "(none)"))
  );
}

// -------------------------------------------------------------------- wiring

$("#browse-default").addEventListener("click", () => browseInto($("#default-dir")));
$("#save").addEventListener("click", save);
$("#revert").addEventListener("click", load);
load();
