// The editor page: project tree, course and layout choice, the sheet preview, notes, and the edits: forms for
// layouts and elements, moving an element on the sheet, undo and redo. Every change goes to the server as one
// operation with the revision the page last saw; the page never names a file.
import { get, post } from "../suite/api.js";
import { loadTexts, t, tn } from "../suite/texts.js";
import { buildTree } from "../suite/tree.js";
import { say, showBanner, clearBanner } from "../suite/status.js";
import * as sheet from "./sheet.js";
import { elementForm, layoutForm } from "./forms.js";
import * as masks from "./masks.js";
import * as numbers from "./numbers.js";

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
  // The layouts and shared elements as the project file has them (`api/spec`), for the forms.
  spec: null,
  // The element the person selected: { id, layout } with the layout id, or null for a shared element.
  selected: null,
  zoom: 1,
  canUndo: false,
  canRedo: false,
  // A write is on its way; the poll leaves the revision alone meanwhile.
  writing: false,
  // The revision a conflict banner was shown for, so it is not shown again at every poll.
  notified: null,
  // Shared elements the person agreed to move for every layout, in this session.
  agreed: new Set(),
  // The numbers of the sheet on screen ({ control, centre_mm }) and its conflicts, as the preview lists them.
  numbers: [],
  conflicts: [],
  // The control whose number the keyboard stepped to last.
  numberSelected: null,
};

// What the drawing modes and the number forms need of the page.
const host = {
  info: () => state.info,
  course: () => state.shown.course || state.course,
  layout: () => state.shown.layout || state.layout,
  svg: () => state.svg,
  numbers: () => state.numbers,
  write: (name, body) => write(name, body),
  pick: async (at) => {
    const answer = await post("api/hit", { course: state.shown.course, layout: state.shown.layout, x_mm: at.x, y_mm: at.y });
    return answer.ok ? answer.data : null;
  },
  say: (id) => say(id),
  showProps: (node) => $("props").replaceChildren(node),
  clearProps: () => showProps([]),
  leave: () => leaveOk(),
  acceptProposal: (conflict) => acceptProposal(conflict),
};

// A move: the shift (mm) not yet sent, and whether a request is on its way.
const moves = { pending: null, sending: false };
const SNAP_MM = 0.5;
const DRAG_START_MM = 0.5;
const NUMBER_REACH_MM = 2.5;

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
    items: info.layouts.map((layout) => {
      const scale = String(layout.print_scale ?? info.map_scale);
      const free = state.spec?.layouts.find((l) => l.id === layout.id)?.page_mm;
      return {
        key: `layout:${layout.id}`,
        label: free
          ? t("editor-layout-item-free", { id: layout.id, w: String(free[0]), h: String(free[1]), scale })
          : t("editor-layout-item", { id: layout.id, paper: layout.paper, scale }),
      };
    }),
  });
  // The elements of the layout on screen, so that each element can be reached by keyboard.
  const shown = state.shown.layout || state.layout;
  const own = state.spec?.layouts.find((l) => l.id === shown)?.elements ?? [];
  if (own.length) {
    groups.push({
      id: "layout-elements",
      label: t("editor-tree-layout-elements", { id: shown }),
      items: own.map((element) => ({
        key: `element:${encodeURIComponent(element.id)}@${encodeURIComponent(shown)}`,
        label: t("editor-element-item", { id: element.id, kind: kindName(element.kind) }),
      })),
    });
  }
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
        key: `element:${encodeURIComponent(element.id)}`,
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
    const spec = state.spec.layouts.find((l) => l.id === id);
    state.selected = null;
    if (spec) $("props").replaceChildren(layoutForm(spec, (body) => write("set_layout", body)));
  } else if (type === "mask") {
    const mask = info.masks.find((m) => m.id === id);
    showProps([
      [t("editor-field-id"), mask.id],
      [t("editor-field-polygons"), mask.polygons],
    ]);
    const form = masks.assignForm(host, mask);
    if (form) $("props").append(form);
  } else if (type === "element") {
    const at = id.indexOf("@");
    if (at < 0) selectElement(decodeURIComponent(id), null);
    else selectElement(decodeURIComponent(id.slice(0, at)), decodeURIComponent(id.slice(at + 1)));
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
  state.numbers = data.numbers ?? [];
  state.conflicts = data.conflicts ?? [];
  numbers.renderConflicts(host, state.conflicts);
  if (state.info && state.treeLayout !== (state.shown.layout || state.layout)) renderTree();
  applyZoom();
  refreshOutline();
  masks.redraw();
  renderNotes(data.notes);
  $("status-page").textContent = `${data.page_mm[0]} × ${data.page_mm[1]} mm`;
  $("status-scale").textContent = t("editor-status-scale-value", { scale: String(data.print_scale) });
  say("editor-sheet-drawn");
}

// The element `id` of the layout `layout` (null: of the shared ones), as the project file has it.
function elementSpec(id, layout) {
  const owner = layout ? state.spec.layouts.find((l) => l.id === layout) : null;
  const list = owner ? owner.elements : state.spec.shared;
  return list.find((e) => e.id === id) ?? null;
}

// Where the element `id` lives for the sheet on screen: in the layout when the layout has it, else shared.
function ownerOf(id) {
  const layout = state.shown.layout || state.layout;
  return elementSpec(id, layout) ? layout : elementSpec(id, null) ? null : undefined;
}

// Selects the element: its form in the properties pane and its outline on the sheet.
function selectElement(id, layout) {
  const spec = elementSpec(id, layout);
  if (!spec) return false;
  state.selected = { id, layout };
  const heading = t("editor-pick-element", { id });
  const hadFocus = $("props").contains(document.activeElement);
  $("props").replaceChildren(elementForm(spec, layout, heading, (body) => write("set_element", body)));
  // After Apply the form is built again; the keyboard stays in it.
  if (hadFocus) $("props").querySelector("button[type=submit]")?.focus();
  const found = state.elements.find((e) => e.id === id);
  if (state.svg) sheet.outline(state.svg, found ? found.rect_mm : null);
  return true;
}

async function pick(at) {
  if (!state.svg) return;
  const answer = await post("api/hit", { course: state.shown.course, layout: state.shown.layout, x_mm: at.x, y_mm: at.y });
  if (!answer.ok) return;
  const { kind, id } = answer.data;
  if (!kind) {
    state.selected = null;
    sheet.outline(state.svg, null);
    showProps([[t("editor-pick-nothing"), ""]]);
    return;
  }
  const found = kind === "element" ? state.elements.find((e) => e.id === id) : null;
  if (kind === "element") {
    const owner = ownerOf(id);
    if (owner !== undefined && selectElement(id, owner)) return;
    sheet.outline(state.svg, found ? found.rect_mm : null);
    showProps([[t("editor-pick-footer"), ""]]);
    return;
  }
  state.selected = null;
  sheet.outline(state.svg, null);
  if (kind === "number" && state.shown.course) {
    $("props").replaceChildren(numbers.numberForm(host, state.shown.course, id));
    return;
  }
  showProps([[t(PICK_TEXTS[kind], { id }), ""]]);
}

// The smallest element of the sheet under the point, if it can be moved (the footer line cannot).
function elementAt(at) {
  let best = null;
  for (const element of state.elements) {
    const [x0, y0, x1, y1] = element.rect_mm;
    if (at.x < x0 || at.x > x1 || at.y < y0 || at.y > y1) continue;
    if (ownerOf(element.id) === undefined) continue;
    if (!best || (x1 - x0) * (y1 - y0) < (best.rect_mm[2] - best.rect_mm[0]) * (best.rect_mm[3] - best.rect_mm[1])) {
      best = element;
    }
  }
  return best;
}

// The number of a control within reach of the point, on a course sheet.
function numberAt(at) {
  if (!state.shown.course) return null;
  let best = null;
  for (const number of state.numbers) {
    const distance = Math.hypot(number.centre_mm[0] - at.x, number.centre_mm[1] - at.y);
    if (distance <= NUMBER_REACH_MM && (!best || distance < best.distance)) best = { ...number, distance };
  }
  return best;
}

// Asks where the correction applies, then sends the drop point; the server works out direction and gap.
async function dropNumber(number, at) {
  const scope = await numbers.askScope(number.control);
  if (!scope) {
    masks.redraw();
    return;
  }
  await write("numbers_accept", {
    course: state.shown.course,
    layout: state.shown.layout,
    control: number.control,
    scope,
    to_page_mm: [at.x, at.y],
  });
  masks.redraw();
}

// The proposal of the check for one conflict, after the person said where it applies.
async function acceptProposal(conflict) {
  const scope = await numbers.askScope(conflict.control);
  if (scope) await write("numbers_accept", { course: conflict.course, control: conflict.control, scope });
}

const snap = (value) => Math.round(value / SNAP_MM) * SNAP_MM;

// Pointer on the sheet: a click selects through the server, a drag moves the element under the pointer. The outline
// follows the pointer; only the final position goes to the server, on pointer-up.
let drag = null;

function onPointerDown(event) {
  if (!state.svg || event.button !== 0) return;
  const at = sheet.pageMm(state.svg, event);
  $("sheet").focus();
  if (masks.active()) {
    drag = { start: at, tool: true };
    $("sheet").setPointerCapture(event.pointerId);
    return;
  }
  const candidate = elementAt(at);
  // A number that no element covers can be dragged to a new place.
  const number = candidate ? null : numberAt(at);
  drag = { start: at, candidate, number, moving: false, delta: [0, 0] };
  $("sheet").setPointerCapture(event.pointerId);
}

function onPointerMove(event) {
  if (!drag || !state.svg) return;
  const at = sheet.pageMm(state.svg, event);
  const [dx, dy] = [at.x - drag.start.x, at.y - drag.start.y];
  if (drag.tool) return;
  if (drag.number) {
    if (!drag.moving && Math.hypot(dx, dy) < DRAG_START_MM) return;
    drag.moving = true;
    sheet.draw(state.svg, { marks: [[at.x, at.y]], ring: true });
    return;
  }
  if (!drag.moving) {
    if (!drag.candidate || Math.hypot(dx, dy) < DRAG_START_MM) return;
    drag.moving = true;
  }
  drag.delta = [snap(dx), snap(dy)];
  const [x0, y0, x1, y1] = drag.candidate.rect_mm;
  sheet.outline(state.svg, [x0 + drag.delta[0], y0 + drag.delta[1], x1 + drag.delta[0], y1 + drag.delta[1]]);
}

async function onPointerUp(event) {
  if (!drag) return;
  const finished = drag;
  drag = null;
  if (finished.tool) {
    const at = sheet.pageMm(state.svg, event);
    if (Math.hypot(at.x - finished.start.x, at.y - finished.start.y) < 1) await masks.click(at);
    return;
  }
  if (finished.number && finished.moving) {
    await dropNumber(finished.number, sheet.pageMm(state.svg, event));
    return;
  }
  if (!finished.moving) {
    await pick(sheet.pageMm(state.svg, event));
    return;
  }
  const [dx, dy] = finished.delta;
  const owner = ownerOf(finished.candidate.id);
  if (dx === 0 && dy === 0) {
    sheet.outline(state.svg, finished.candidate.rect_mm);
    return;
  }
  state.selected = { id: finished.candidate.id, layout: owner };
  await moveElement(finished.candidate.id, dx, dy);
}

// Asks once per session before a shared element moves in every layout (a real dialog).
function confirmShared(id) {
  if (state.agreed.has(id)) return Promise.resolve(true);
  const dialog = $("confirm");
  $("confirm-body").textContent = t("editor-move-shared-body", { id });
  return new Promise((resolve) => {
    dialog.addEventListener("close", () => {
      const ok = dialog.returnValue === "ok";
      if (ok) state.agreed.add(id);
      resolve(ok);
    }, { once: true });
    dialog.returnValue = "";
    dialog.showModal();
  });
}

// Sends a shift of the element; shifts that arrive while one is on its way are added up and sent next.
async function moveElement(id, dx, dy) {
  if (ownerOf(id) === null && !(await confirmShared(id))) {
    await refresh();
    return;
  }
  moves.pending = { id, dx: (moves.pending?.id === id ? moves.pending.dx : 0) + dx, dy: (moves.pending?.id === id ? moves.pending.dy : 0) + dy };
  if (moves.sending) return;
  moves.sending = true;
  try {
    while (moves.pending) {
      const next = moves.pending;
      moves.pending = null;
      await write("move_element", { layout: state.shown.layout || state.layout, id: next.id, dx_mm: next.dx, dy_mm: next.dy });
    }
  } finally {
    moves.sending = false;
  }
}

// Asks before a half-drawn corridor line is thrown away (a real dialog); true when nothing is lost or the person agrees.
function leaveOk() {
  if (!masks.dirty()) return Promise.resolve(true);
  const dialog = $("leave");
  // The question is on screen already; a second one would throw.
  if (dialog.open) return Promise.resolve(false);
  return new Promise((resolve) => {
    dialog.addEventListener("close", () => resolve(dialog.returnValue === "ok"), { once: true });
    dialog.returnValue = "";
    dialog.showModal();
  });
}

async function choose(course, layout) {
  if (!(await leaveOk())) {
    $("course-select").value = state.course;
    $("layout-select").value = state.layout;
    return;
  }
  if (masks.active() && (course !== state.course || layout !== state.layout)) masks.stop();
  state.course = course;
  state.layout = layout;
  $("course-select").value = course;
  $("layout-select").value = layout;
  showSheet();
}

async function onSelect(key) {
  if (masks.active()) {
    if (!(await leaveOk())) return;
    masks.stop();
  }
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
  const spec = await get("api/spec");
  if (!spec.ok) {
    showBanner(typeof spec.data === "string" ? spec.data : t("editor-status-lost"));
    return false;
  }
  state.info = answer.data;
  state.spec = spec.data;
  state.revision = answer.data.revision;
  state.sources = answer.data.sources;
  setHistory(answer.data.can_undo, answer.data.can_redo);
  document.documentElement.lang = answer.data.lang;
  $("status-project").textContent = answer.data.title;
  renderTree();
  return true;
}

// The tree, built again; it keeps the item the keyboard is on and the one selected.
function renderTree() {
  const tree = $("pane-tree");
  tree.dataset.treeLabel = t("editor-tree-label");
  state.treeLayout = state.shown.layout || state.layout;
  state.treeApi = buildTree(tree, treeGroups(), onSelect);
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
  const surface = $("sheet");
  surface.addEventListener("pointerdown", onPointerDown);
  surface.addEventListener("pointermove", onPointerMove);
  surface.addEventListener("pointerup", onPointerUp);
  surface.addEventListener("pointercancel", () => {
    drag = null;
    if (state.selected) refreshOutline();
    masks.redraw();
  });
  surface.addEventListener("keydown", onSheetKey);
  document.addEventListener("keydown", onKey);
  addEventListener("resize", applyZoom);
  $("menu-undo").addEventListener("click", () => step("undo"));
  $("menu-redo").addEventListener("click", () => step("redo"));
  $("menu-zoom-in").addEventListener("click", () => zoomBy(1.25));
  $("menu-zoom-out").addEventListener("click", () => zoomBy(0.8));
  $("menu-zoom-fit").addEventListener("click", () => zoomTo(1));
  $("confirm-yes").addEventListener("click", () => $("confirm").close("ok"));
  $("confirm-no").addEventListener("click", () => $("confirm").close("cancel"));
  $("scope-layout").addEventListener("click", () => $("scope").close("layout_wide"));
  $("scope-sheet").addEventListener("click", () => $("scope").close("sheet"));
  $("scope-cancel").addEventListener("click", () => $("scope").close("cancel"));
  $("leave-yes").addEventListener("click", () => $("leave").close("ok"));
  $("leave-no").addEventListener("click", () => $("leave").close("cancel"));
  $("menu-mask-window").addEventListener("click", () => masks.start(host, "window"));
  $("menu-mask-corridor").addEventListener("click", () => masks.start(host, "corridor"));
  $("sheet").addEventListener("dblclick", () => masks.doubleClick());
  addEventListener("beforeunload", (event) => {
    if (masks.dirty()) {
      event.preventDefault();
      event.returnValue = "";
    }
  });
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

// The project file may change under the page (a text editor, an MCP session): the page says so and reads it again
// when the person asks. A changed map or course file only draws the sheet again.
async function poll() {
  if (state.closed || state.writing || document.hidden) return;
  const known = state.revision;
  try {
    const answer = await get("api/revision");
    if (!answer.ok || state.writing || state.revision !== known) return;
    setHistory(answer.data.can_undo, answer.data.can_redo);
    if (answer.data.revision !== known) {
      if (state.notified !== answer.data.revision) {
        state.notified = answer.data.revision;
        conflict(t("editor-changed-outside"));
      }
    } else if (answer.data.sources !== state.sources) {
      await reload();
    }
  } catch {
    // The server is gone or busy; the next poll tries again.
  }
}

// A banner with the words and a button that reads the project file again.
function conflict(text) {
  showBanner(text);
  const box = document.querySelector("#banner .banner-box");
  if (!box) return;
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = t("editor-reload");
  button.addEventListener("click", async () => {
    clearBanner();
    await reload();
  });
  box.insertBefore(button, box.lastElementChild);
}

function setHistory(canUndo, canRedo) {
  state.canUndo = Boolean(canUndo);
  state.canRedo = Boolean(canRedo);
  $("menu-undo").disabled = !state.canUndo;
  $("menu-redo").disabled = !state.canRedo;
}

// The project and the sheet again, keeping the choice of course, layout and element where they still exist.
async function refresh() {
  if (!(await loadProject())) return;
  const info = state.info;
  if (state.course && !info.courses.some((c) => c.name === state.course)) state.course = "";
  if (!info.layouts.some((l) => l.id === state.layout)) state.layout = info.layouts[0]?.id ?? "";
  fillSelects();
  await showSheet();
  if (state.selected && !selectElement(state.selected.id, state.selected.layout)) {
    state.selected = null;
    showProps([]);
  }
}

// One change: `POST api/op/{name}` with the revision the page last saw. Returns whether it was made.
async function write(name, body) {
  state.writing = true;
  try {
    const answer = await post(`api/op/${name}`, body, state.revision);
    return await afterWrite(answer, "editor-status-changed");
  } finally {
    state.writing = false;
  }
}

// Undo or redo of the last change.
async function step(which) {
  if (state.writing) return;
  // The sheet changes under a half-drawn line, so it is thrown away, after asking.
  if (!(await leaveOk())) return;
  if (masks.dirty()) masks.cancel();
  state.writing = true;
  try {
    const answer = await post(`api/${which}`, {}, state.revision);
    return await afterWrite(answer, which === "undo" ? "editor-status-undone" : "editor-status-redone");
  } finally {
    state.writing = false;
  }
}

async function afterWrite(answer, saidId) {
  if (answer.ok) {
    clearBanner();
    state.revision = answer.data.revision;
    setHistory(answer.data.can_undo, answer.data.can_redo);
    await refresh();
    say(saidId);
    return true;
  }
  if (answer.status === 409) {
    // The file is not the one the page saw: say so, and read it again on the person's word.
    state.notified = answer.data.revision;
    conflict(answer.data.error);
  } else {
    showBanner(typeof answer.data === "string" ? answer.data : t("editor-status-refused"));
    say("editor-status-refused");
  }
  const revision = await get("api/revision");
  if (revision.ok) setHistory(revision.data.can_undo, revision.data.can_redo);
  await refreshOutline();
  return false;
}

// The outline of the selected element on the freshly drawn sheet (or none).
function refreshOutline() {
  if (!state.svg) return;
  const found = state.selected ? state.elements.find((e) => e.id === state.selected.id) : null;
  sheet.outline(state.svg, found ? found.rect_mm : null);
}

function applyZoom() {
  if (state.svg) sheet.setZoom($("sheet"), state.svg, state.zoom);
}

function zoomTo(zoom) {
  state.zoom = Math.min(8, Math.max(0.25, zoom));
  applyZoom();
}

const zoomBy = (factor) => zoomTo(state.zoom * factor);

// Keys everywhere on the page: undo and redo, except where a field takes them for its own text.
function onKey(event) {
  if (event.key === "Escape" && masks.active() && !$("leave").open && !$("scope").open) {
    event.preventDefault();
    masks.cancel();
    return;
  }
  const field = event.target instanceof HTMLElement && event.target.closest("input, textarea, select");
  if (field || !(event.ctrlKey || event.metaKey) || event.altKey) return;
  if (event.key.toLowerCase() === "z") {
    event.preventDefault();
    step(event.shiftKey ? "redo" : "undo");
  } else if (event.key.toLowerCase() === "y") {
    event.preventDefault();
    step("redo");
  }
}

// Selects the next (or previous) element of the sheet that can be moved, in the order the sheet lists them.
function nextElement(direction) {
  const movable = state.elements.filter((e) => ownerOf(e.id) !== undefined);
  if (!movable.length) return;
  const at = movable.findIndex((e) => e.id === state.selected?.id);
  const next = movable[(at + direction + movable.length) % movable.length];
  if (selectElement(next.id, ownerOf(next.id))) say(t("editor-pick-element", { id: next.id }), true);
}

// Selects the next (or previous) control number of the sheet and opens its form: the keyboard route to any number,
// with or without a conflict.
function nextNumber(direction) {
  if (!state.shown.course || !state.numbers.length) {
    say("editor-numbers-none");
    return;
  }
  const at = state.numbers.findIndex((n) => n.control === state.numberSelected);
  const next = state.numbers[(at + direction + state.numbers.length) % state.numbers.length];
  state.numberSelected = next.control;
  state.selected = null;
  const [x, y] = next.centre_mm;
  sheet.outline(state.svg, [x - 2.5, y - 2.5, x + 2.5, y + 2.5]);
  $("props").replaceChildren(numbers.numberForm(host, state.shown.course, next.control));
  say(t("editor-pick-number", { id: next.control }), true);
}

// Keys on the focused sheet: arrows nudge the selected element by 0.5 mm (1 mm with Shift), else they pan; N and Shift+N select the next and previous element, M and Shift+M the next and previous control number; + and -
// zoom, 0 fits the page. Drawing on the map is mouse only.
function onSheetKey(event) {
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  const arrows = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
  const arrow = arrows[event.key];
  if (arrow) {
    event.preventDefault();
    if (state.selected) {
      const step = event.shiftKey ? 2 * SNAP_MM : SNAP_MM;
      moveElement(state.selected.id, arrow[0] * step, arrow[1] * step);
    } else if (!state.selected) {
      $("sheet").scrollBy(arrow[0] * 60, arrow[1] * 60);
    }
  } else if (event.key === "n" || event.key === "N") {
    event.preventDefault();
    nextElement(event.shiftKey ? -1 : 1);
  } else if (event.key === "m" || event.key === "M") {
    event.preventDefault();
    nextNumber(event.shiftKey ? -1 : 1);
  } else if (event.key === "+" || event.key === "=") {
    event.preventDefault();
    zoomBy(1.25);
  } else if (event.key === "-") {
    event.preventDefault();
    zoomBy(0.8);
  } else if (event.key === "0") {
    event.preventDefault();
    zoomTo(1);
  }
}

async function reload() {
  state.notified = null;
  await refresh();
  say("editor-status-reloaded");
}

// Ends the server; the page then has nothing left to ask.
async function close() {
  if (!(await leaveOk())) return;
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
