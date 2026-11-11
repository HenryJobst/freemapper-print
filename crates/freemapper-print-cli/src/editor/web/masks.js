// Masks: the window mode (click the numbers of the controls that get a window), the corridor mode (click the points
// of a line, double-click ends it) and the assignment of a mask to a course. Drawing is mouse only; every action
// also has fields, so a keyboard needs no mouse: type the controls as codes or places, type the points as text.
// The server does the work (`mask_window`, `mask_corridor`, `assign_mask`); nothing here decides a rule.
import { t } from "../suite/texts.js";
import * as sheet from "./sheet.js";
import { make, row, select, numberInput, read, fail, clear } from "./forms.js";

// The state of the mode on screen. `points` is the corridor line drawn so far, in page mm (y down).
const tool = { mode: null, points: [], host: null, form: null };

export const active = () => tool.mode !== null;

// A line is half drawn: leaving would lose it.
export const dirty = () => tool.mode === "corridor" && tool.points.length > 0;

function courseOptions(host, withNone) {
  const options = host.info().courses.map((c) => [c.name, c.name]);
  return withNone ? [["", t("editor-mask-no-course")], ...options] : options;
}

// A course that prints with a mask already keeps it: the program never replaces one, so assigning is off then.
function assignDefault(host, course, box) {
  const set = () => {
    const taken = host.info().assignments.some((a) => a.course === course.input.value && a.mask);
    box.input.checked = !taken && Boolean(course.input.value);
  };
  course.input.addEventListener("change", set);
  set();
}

// The codes in the controls field, as a list.
function codesOf(input) {
  return input.value.split(",").map((code) => code.trim()).filter(Boolean);
}

// The marks on the sheet: the controls of the window mask, or the corridor line, drawn over the preview.
export function redraw() {
  const svg = tool.host?.svg();
  if (!svg) return;
  if (tool.mode === "window") {
    const chosen = new Set(codesOf(tool.form.controls.input));
    const marks = tool.host.numbers().filter((n) => chosen.has(n.control)).map((n) => n.centre_mm);
    sheet.draw(svg, { marks, ring: true });
  } else if (tool.mode === "corridor") {
    sheet.draw(svg, { line: tool.points, marks: tool.points });
  } else {
    sheet.draw(svg, {});
  }
}

function hint() {
  return make("p", { class: "hint", text: t("editor-mask-draw-hint") });
}

function buttons(...nodes) {
  return make("div", { class: "dialog-buttons" }, nodes);
}

// Ends the mode and empties the panel.
export function stop() {
  tool.mode = null;
  tool.points = [];
  tool.form = null;
  const host = tool.host;
  tool.host = null;
  if (host) {
    host.clearProps();
    const svg = host.svg();
    if (svg) sheet.draw(svg, {});
  }
}

function windowForm(host) {
  const course = row(t("editor-field-course"), select(courseOptions(host, false), host.course()));
  const controls = row(t("editor-mask-controls"), make("input", { type: "text", autocomplete: "off" }));
  const controlsHint = make("p", { class: "hint", text: t("editor-mask-controls-hint") });
  const radius = row(t("editor-mask-radius"), numberInput(""));
  radius.input.placeholder = "15";
  const radiusFor = row(t("editor-mask-radius-for"), make("input", { type: "text", autocomplete: "off" }));
  const noStart = row(t("editor-mask-no-start"), make("input", { type: "checkbox" }));
  const noFinish = row(t("editor-mask-no-finish"), make("input", { type: "checkbox" }));
  const id = row(t("editor-mask-id"), make("input", { type: "text", autocomplete: "off" }));
  const assign = row(t("editor-mask-assign"), make("input", { type: "checkbox", checked: true }));
  assignDefault(host, course, assign);
  controls.input.addEventListener("input", redraw);
  const fields = [radius];
  const form = make("form", { class: "props-form", novalidate: true }, [
    make("h3", { text: t("editor-mask-window-heading") }),
    hint(),
    course.row,
    controls.row,
    controlsHint,
    radius.row,
    radiusFor.row,
    noStart.row,
    noFinish.row,
    id.row,
    assign.row,
    buttons(
      make("button", { type: "submit", text: t("editor-mask-window-add") }),
      make("button", { type: "button", id: "mask-stop", text: t("editor-mask-stop") }),
    ),
  ]);
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    clear(fields);
    const body = { course: course.input.value, assign: assign.input.checked };
    if (controls.input.value.trim()) body.controls = controls.input.value.trim();
    const mm = read(radius.input);
    if (mm !== null && !Number.isFinite(mm)) return fail(radius, t("editor-form-number-error"));
    if (mm !== null) body.radius = mm;
    const own = radiusFor.input.value.split(",").map((item) => item.trim()).filter(Boolean);
    if (own.length) body.radius_for = own;
    if (noStart.input.checked) body.no_start = true;
    if (noFinish.input.checked) body.no_finish = true;
    if (id.input.value.trim()) body.id = id.input.value.trim();
    if (await host.write("mask_window", body)) stop();
  });
  form.querySelector("#mask-stop").addEventListener("click", stop);
  tool.form = { controls };
  return form;
}

// The points of a line in the points field, one `x,y` per point as the server reads them.
function corridorForm(host) {
  const course = row(t("editor-field-course"), select(courseOptions(host, true), host.course()));
  const typed = row(t("editor-mask-points"), make("textarea", { rows: 3 }));
  typed.input.placeholder = "10,-20;40,-25;70,-20";
  const typedHint = make("p", { class: "hint", text: t("editor-mask-points-hint") });
  const status = make("p", { class: "hint", id: "corridor-status", role: "status", "aria-live": "polite" });
  const width = row(t("editor-mask-width"), numberInput(""));
  width.input.placeholder = "10";
  const id = row(t("editor-mask-id"), make("input", { type: "text", autocomplete: "off" }));
  const assign = row(t("editor-mask-assign"), make("input", { type: "checkbox", checked: true }));
  assignDefault(host, course, assign);
  const form = make("form", { class: "props-form", novalidate: true }, [
    make("h3", { text: t("editor-mask-corridor-heading") }),
    hint(),
    status,
    course.row,
    typed.row,
    typedHint,
    width.row,
    id.row,
    assign.row,
    buttons(
      make("button", { type: "submit", text: t("editor-mask-corridor-add") }),
      make("button", { type: "button", id: "corridor-discard", text: t("editor-mask-discard") }),
      make("button", { type: "button", id: "mask-stop", text: t("editor-mask-stop") }),
    ),
  ]);
  const count = () => {
    status.textContent = t("editor-mask-points-drawn", { n: String(tool.points.length) });
  };
  count();
  const submit = async () => {
    clear([typed, width]);
    const text = typed.input.value.trim();
    const body = { course: course.input.value || null };
    if (!body.course) delete body.course;
    if (text) body.points = text;
    else if (tool.points.length >= 2) {
      body.page_points_mm = tool.points;
      body.layout = tool.host.layout();
    } else return fail(typed, t("editor-mask-points-error"));
    const mm = read(width.input);
    if (mm !== null && !Number.isFinite(mm)) return fail(width, t("editor-form-number-error"));
    if (mm !== null) body.width = mm;
    if (id.input.value.trim()) body.id = id.input.value.trim();
    body.assign = assign.input.checked && Boolean(body.course);
    if (await host.write("mask_corridor", body)) stop();
  };
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    submit();
  });
  form.querySelector("#corridor-discard").addEventListener("click", () => {
    tool.points = [];
    count();
    redraw();
  });
  form.querySelector("#mask-stop").addEventListener("click", () => host.leave().then((ok) => ok && stop()));
  tool.form = { count, submit };
  return form;
}

// Opens the mode `mode` (`window` or `corridor`) after the half-drawn line of the one before is dealt with.
export async function start(host, mode) {
  if (tool.mode && !(await host.leave())) return;
  if (tool.mode) stop();
  if (!host.info().courses.length && mode === "window") {
    host.say("editor-mask-needs-courses");
    return;
  }
  tool.mode = mode;
  tool.host = host;
  tool.points = [];
  const form = mode === "window" ? windowForm(host) : corridorForm(host);
  host.showProps(form);
  form.querySelector("input, select, textarea")?.focus();
  redraw();
}

// A click of the mouse on the sheet at `at` (page mm).
export async function click(at) {
  if (tool.mode === "window") {
    const answer = await tool.host.pick(at);
    if (answer?.kind !== "number") {
      tool.host.say("editor-mask-click-number");
      return;
    }
    const input = tool.form.controls.input;
    const codes = codesOf(input);
    const at2 = codes.indexOf(answer.id);
    if (at2 >= 0) codes.splice(at2, 1);
    else codes.push(answer.id);
    input.value = codes.join(",");
    redraw();
  } else if (tool.mode === "corridor") {
    const round = (v) => Math.round(v * 100) / 100;
    tool.points.push([round(at.x), round(at.y)]);
    tool.form.count();
    redraw();
  }
}

// A double-click ends the line and makes the mask. The browser sends two clicks before it, so the second one added
// the last point a second time.
export function doubleClick() {
  if (tool.mode !== "corridor") return;
  tool.points.pop();
  tool.form.count();
  redraw();
  if (tool.points.length >= 2) tool.form.submit();
}

// Escape: the line drawn so far goes; with none, the mode ends.
export function cancel() {
  if (tool.mode === "corridor" && tool.points.length) {
    tool.points = [];
    tool.form.count();
    redraw();
  } else if (tool.mode) stop();
}

// The panel of a mask in the tree: the course that prints with it, or none.
export function assignForm(host, mask) {
  const course = row(t("editor-field-course"), select(courseOptions(host, false), host.course()));
  const form = make("form", { class: "props-form", novalidate: true }, [
    make("h3", { text: t("editor-mask-assign-heading", { id: mask.id }) }),
    course.row,
    buttons(
      make("button", { type: "submit", text: t("editor-mask-assign-button") }),
      make("button", { type: "button", id: "mask-unassign", text: t("editor-mask-unassign") }),
    ),
  ]);
  if (!host.info().courses.length) return null;
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    host.write("assign_mask", { course: course.input.value, mask: mask.id });
  });
  form.querySelector("#mask-unassign").addEventListener("click", () => host.write("assign_mask", { course: course.input.value }));
  return form;
}
