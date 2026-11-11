// The editor page: project tree, course and layout choice, the sheet preview, notes. Reading only; the
// operations that change the project come with later modules.
import { get, post } from "../suite/api.js";
import { loadTexts, t, tn } from "../suite/texts.js";
import { buildTree } from "../suite/tree.js";
import { say, showBanner, clearBanner } from "../suite/status.js";
import * as sheet from "./sheet.js";

const POLL_MS = 2000;

const state = {
  info: null,
  course: "",
  layout: "",
  revision: null,
  svg: null,
  elements: [],
  // The number of the newest preview request; an older answer is dropped. It starts at the time the page was
  // opened, so the server ranks a reload or a second tab above the requests of the page before it.
  request: Date.now(),
  // The course and layout of the sheet on screen, which a click is resolved against.
  shown: { course: "", layout: "" },
  sources: null,
  texts: false,
  closed: false,
};

const $ = (id) => document.getElementById(id);

const KIND_TEXTS = {
  title_block: "editor-kind-title-block",
  text: "editor-kind-text",
  logo: "editor-kind-logo",
  scale_check_bar: "editor-kind-scale-check-bar",
  credit_line: "editor-kind-credit-line",
  copyright_line: "editor-kind-copyright-line",
  control_descriptions: "editor-kind-control-descriptions",
  footer: "editor-kind-footer",
};
const SEVERITY_TEXTS = {
  info: "editor-severity-info",
  warning: "editor-severity-warning",
  error: "editor-severity-error",
};
const PICK_TEXTS = {
  element: "editor-pick-element",
  number: "editor-pick-number",
  map_object: "editor-pick-map-object",
  mask: "editor-pick-mask",
};

const kindName = (kind) => t(KIND_TEXTS[kind] ?? "editor-kind-text");

function fileName(path) {
  return String(path).split(/[\\/]/).pop();
}

function treeGroups() {
  const info = state.info;
  const groups = [];
  const sources = [{ key: "source:map", label: t("editor-source-map", { file: fileName(info.map) }) }];
  if (info.courses_file) {
    sources.push({ key: "source:courses", label: t("editor-source-courses", { file: fileName(info.courses_file) }) });
  }
  groups.push({ id: "sources", label: t("editor-tree-sources"), items: sources });
  groups.push({
    id: "layouts",
    label: t("editor-tree-layouts"),
    items: info.layouts.map((layout) => ({
      key: `layout:${layout.id}`,
      label: t("editor-layout-item", {
        id: layout.id,
        paper: layout.paper,
        scale: String(layout.print_scale ?? info.map_scale),
      }),
    })),
  });
  if (info.masks.length) {
    groups.push({
      id: "masks",
      label: t("editor-tree-masks"),
      items: info.masks.map((mask) => ({
        key: `mask:${mask.id}`,
        label: tn("editor-mask-item", mask.polygons, { id: mask.id }),
      })),
    });
  }
  if (info.shared_elements.length) {
    groups.push({
      id: "shared",
      label: t("editor-tree-shared"),
      items: info.shared_elements.map((element) => ({
        key: `element:${element.id}`,
        label: t("editor-element-item", { id: element.id, kind: kindName(element.kind) }),
      })),
    });
  }
  if (info.assignments.length) {
    groups.push({
      id: "assignments",
      label: t("editor-tree-assignments"),
      items: info.assignments.map((a) => ({
        key: `assignment:${a.course}`,
        label: a.mask
          ? t("editor-assignment-item-mask", { course: a.course, layout: a.layout, mask: a.mask })
          : t("editor-assignment-item", { course: a.course, layout: a.layout }),
      })),
    });
  }
  return groups;
}

// A list of label/value rows in the properties pane.
function showProps(rows) {
  const list = document.createElement("dl");
  for (const [label, value] of rows) {
    const term = document.createElement("dt");
    term.textContent = label;
    const detail = document.createElement("dd");
    detail.textContent = String(value);
    list.append(term, detail);
  }
  $("props").replaceChildren(list);
}

function propsFor(key) {
  const info = state.info;
  const [type, id] = [key.slice(0, key.indexOf(":")), key.slice(key.indexOf(":") + 1)];
  if (type === "layout") {
    const layout = info.layouts.find((l) => l.id === id);
    showProps([
      [t("editor-field-id"), layout.id],
      [t("editor-field-paper"), layout.paper],
      [t("editor-field-orientation"), layout.orientation],
      [t("editor-field-scale"), t("editor-status-scale-value", { scale: String(layout.print_scale ?? info.map_scale) })],
    ]);
  } else if (type === "mask") {
    const mask = info.masks.find((m) => m.id === id);
    showProps([
      [t("editor-field-id"), mask.id],
      [t("editor-field-polygons"), mask.polygons],
    ]);
  } else if (type === "element") {
    const element = info.shared_elements.find((e) => e.id === id);
    showProps([
      [t("editor-field-id"), element.id],
      [t("editor-field-kind"), kindName(element.kind)],
    ]);
  } else if (type === "assignment") {
    const a = info.assignments.find((x) => x.course === id);
    const course = info.courses.find((c) => c.name === id);
    const rows = [
      [t("editor-field-course"), a.course],
      [t("editor-field-layout"), a.layout],
    ];
    if (a.mask) rows.push([t("editor-field-masks"), a.mask]);
    if (course) rows.push([t("editor-field-controls"), course.controls]);
    showProps(rows);
  } else if (type === "source") {
    showProps([[t("editor-field-file"), fileName(id === "map" ? info.map : info.courses_file)]]);
  }
}

function fillSelects() {
  const info = state.info;
  const courseSelect = $("course-select");
  const none = document.createElement("option");
  none.value = "";
  none.textContent = t("editor-course-none");
  courseSelect.replaceChildren(none);
  for (const course of info.courses) {
    const option = document.createElement("option");
    option.value = course.name;
    option.textContent = course.name;
    courseSelect.append(option);
  }
  const noCourses = info.courses.length === 0;
  courseSelect.hidden = noCourses;
  $("course-label").hidden = noCourses;
  $("no-courses").hidden = !noCourses;
  const layoutSelect = $("layout-select");
  layoutSelect.replaceChildren();
  for (const layout of info.layouts) {
    const option = document.createElement("option");
    option.value = layout.id;
    option.textContent = layout.id;
    layoutSelect.append(option);
  }
  courseSelect.value = state.course;
  layoutSelect.value = state.layout;
}

function renderNotes(notes) {
  const box = $("notices");
  if (!notes.length) {
    const empty = document.createElement("p");
    empty.className = "hint";
    empty.textContent = t("editor-notices-empty");
    box.replaceChildren(empty);
    return;
  }
  const list = document.createElement("ul");
  for (const note of notes) {
    const item = document.createElement("li");
    item.className = note.severity;
    item.textContent = t("editor-note-line", {
      severity: t(SEVERITY_TEXTS[note.severity] ?? "editor-severity-info"),
      message: note.message,
    });
    list.append(item);
  }
  box.replaceChildren(list);
}

async function showSheet() {
  const request = ++state.request;
  say("editor-sheet-loading");
  const query = new URLSearchParams({ course: state.course, layout: state.layout, rev: String(request) });
  const answer = await get(`api/preview?${query}`);
  if (answer.status === 409 && answer.data?.error === "superseded") return;
  if (request !== state.request) return;
  if (!answer.ok) {
    showBanner(typeof answer.data === "string" ? answer.data : t("editor-sheet-failed"));
    say("editor-sheet-failed");
    return;
  }
  clearBanner();
  const data = answer.data;
  state.svg = sheet.show($("sheet"), data.svg);
  state.shown = { course: data.course ?? "", layout: data.layout ?? "" };
  state.elements = data.elements;
  renderNotes(data.notes);
  $("status-page").textContent = `${data.page_mm[0]} × ${data.page_mm[1]} mm`;
  $("status-scale").textContent = t("editor-status-scale-value", { scale: String(data.print_scale) });
  say("editor-sheet-drawn");
}

async function pick(event) {
  if (!state.svg) return;
  const at = sheet.pageMm(state.svg, event);
  const answer = await post("api/hit", { course: state.shown.course, layout: state.shown.layout, x_mm: at.x, y_mm: at.y });
  if (!answer.ok) return;
  const { kind, id } = answer.data;
  if (!kind) {
    sheet.outline(state.svg, null);
    showProps([[t("editor-pick-nothing"), ""]]);
    return;
  }
  const found = kind === "element" ? state.elements.find((e) => e.id === id) : null;
  sheet.outline(state.svg, found ? found.rect_mm : null);
  showProps([[t(PICK_TEXTS[kind], { id }), found ? kindName(found.kind) : ""]]);
}

function choose(course, layout) {
  state.course = course;
  state.layout = layout;
  $("course-select").value = course;
  $("layout-select").value = layout;
  showSheet();
}

function onSelect(key) {
  propsFor(key);
  if (key.startsWith("layout:")) choose("", key.slice("layout:".length));
  else if (key.startsWith("assignment:")) {
    const course = key.slice("assignment:".length);
    const known = state.info.courses.find((c) => c.name === course);
    if (known) choose(course, known.layout);
  }
}

async function loadProject() {
  const answer = await get("api/project");
  if (!answer.ok) {
    showBanner(typeof answer.data === "string" ? answer.data : t("editor-status-lost"));
    return false;
  }
  state.info = answer.data;
  state.revision = answer.data.revision;
  state.sources = answer.data.sources;
  document.documentElement.lang = answer.data.lang;
  $("status-project").textContent = answer.data.title;
  const tree = $("pane-tree");
  tree.dataset.treeLabel = t("editor-tree-label");
  state.treeApi = buildTree(tree, treeGroups(), onSelect);
  return true;
}

async function start() {
  await loadTexts();
  state.texts = true;
  $("menu-done").addEventListener("click", close);
  $("course-select").addEventListener("change", (e) => {
    const course = e.target.value;
    const known = state.info.courses.find((c) => c.name === course);
    choose(course, known ? known.layout : state.layout);
  });
  $("layout-select").addEventListener("change", (e) => choose($("course-select").value, e.target.value));
  $("sheet").addEventListener("click", pick);
  if (!(await loadProject())) return;
  const first = state.info.courses[0];
  state.course = first ? first.name : "";
  state.layout = first ? first.layout : (state.info.layouts[0]?.id ?? "");
  fillSelects();
  say("editor-status-ready");
  await showSheet();
  setInterval(poll, POLL_MS);
  addEventListener("focus", poll);
}

// The project file may change under the page (a text editor, an MCP session): the page then reads it again.
async function poll() {
  if (state.closed || document.hidden) return;
  try {
    const answer = await get("api/revision");
    if (answer.ok && (answer.data.revision !== state.revision || answer.data.sources !== state.sources)) {
      await reload();
    }
  } catch {
    // The server is gone or busy; the next poll tries again.
  }
}

async function reload() {
  if (!(await loadProject())) return;
  const info = state.info;
  if (state.course && !info.courses.some((c) => c.name === state.course)) state.course = "";
  if (!info.layouts.some((l) => l.id === state.layout)) state.layout = info.layouts[0]?.id ?? "";
  fillSelects();
  say("editor-status-reloaded");
  await showSheet();
}

// Ends the server; the page then has nothing left to ask.
async function close() {
  try {
    const answer = await post("api/done");
    if (answer.ok) state.closed = true;
    say(answer.ok ? "editor-status-closed" : "editor-status-lost");
  } catch {
    say("editor-status-lost");
  }
}

start().catch(() => {
  if (state.texts) say("editor-status-lost");
  else $("status-message").textContent = "The editor page could not start.";
});
