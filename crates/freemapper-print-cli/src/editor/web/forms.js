// The forms of the properties pane: a layout's keys and a sheet element's keys, as `set_layout` and
// `set_element` take them (the server checks every value). A form calls `onSubmit(body)` with the keys it holds.
import { t } from "../suite/texts.js";

const CORNERS = {
  top_left: "editor-corner-top-left",
  top: "editor-corner-top",
  top_right: "editor-corner-top-right",
  left: "editor-corner-left",
  centre: "editor-corner-centre",
  right: "editor-corner-right",
  bottom_left: "editor-corner-bottom-left",
  bottom: "editor-corner-bottom",
  bottom_right: "editor-corner-bottom-right",
};
const PLACEHOLDERS = [
  "{event}", "{date}", "{organiser}", "{print_scale}", "{map_scale}", "{contour_interval}", "{layout}",
  "{course}", "{length_km}", "{climb_m}", "{controls}",
];
const PAPER_MM = { A4: [210, 297], A3: [297, 420] };

function make(tag, attributes = {}, children = []) {
  const node = document.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) {
    if (name === "text") node.textContent = value;
    else if (value === true) node.setAttribute(name, "");
    else if (value !== false && value !== null && value !== undefined) node.setAttribute(name, String(value));
  }
  node.append(...children);
  return node;
}

let counter = 0;

// A label and its input in one row; returns { row, input }.
function row(label, input) {
  const id = `field-${++counter}`;
  input.id = id;
  const error = make("p", { class: "field-error", id: `${id}-error`, hidden: true });
  return { row: make("div", { class: "row" }, [make("label", { for: id, text: label }), input, error]), input, error };
}

function select(options, value) {
  const node = make("select");
  for (const [optionValue, label] of options) {
    const option = make("option", { value: optionValue, text: label });
    option.selected = optionValue === value;
    node.append(option);
  }
  return node;
}

function numberInput(value, step = "any") {
  return make("input", { type: "number", step, value: value ?? "" });
}

// The number in `input`: null when empty, NaN when it is no number.
function read(input) {
  const text = input.value.trim();
  return text === "" ? null : Number(text);
}

function fail(field, message) {
  field.input.setAttribute("aria-invalid", "true");
  field.input.setAttribute("aria-describedby", field.error.id);
  field.error.textContent = message;
  field.error.hidden = false;
  return false;
}

function clear(fields) {
  for (const field of fields) {
    field.input.removeAttribute("aria-invalid");
    field.input.removeAttribute("aria-describedby");
    field.error.hidden = true;
  }
}

// A number that must be there.
function need(field) {
  const value = read(field.input);
  if (value === null || !Number.isFinite(value)) {
    fail(field, t("editor-form-number-error"));
    return null;
  }
  return value;
}

function finish(heading, rows, fields, build, onSubmit) {
  const form = make("form", { class: "props-form", novalidate: true }, [
    make("h3", { text: heading }),
    ...rows,
    make("div", { class: "dialog-buttons" }, [make("button", { type: "submit", text: t("editor-form-apply") })]),
  ]);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    clear(fields);
    const body = build();
    if (body) onSubmit(body);
  });
  return form;
}

// The form of a layout (`spec` as `api/spec` gives it).
export function layoutForm(spec, onSubmit) {
  const paper = row(t("editor-field-paper"), select([["A4", "A4"], ["A3", "A3"]], spec.paper));
  const free = row(t("editor-field-free-page"), make("input", { type: "checkbox", checked: Boolean(spec.page_mm) }));
  const [w0, h0] = spec.page_mm ?? PAPER_MM[spec.paper] ?? PAPER_MM.A4;
  const width = row(t("editor-field-page-width"), numberInput(w0));
  const height = row(t("editor-field-page-height"), numberInput(h0));
  const sync = () => {
    width.input.disabled = height.input.disabled = !free.input.checked;
    paper.input.disabled = free.input.checked;
  };
  free.input.addEventListener("change", sync);
  sync();
  const orientation = row(
    t("editor-field-orientation"),
    select(
      [["auto", t("editor-option-auto")], ["portrait", t("editor-option-portrait")], ["landscape", t("editor-option-landscape")]],
      spec.orientation,
    ),
  );
  const scale = row(t("editor-field-scale"), numberInput(spec.print_scale, "1"));
  scale.input.placeholder = t("editor-scale-placeholder");
  const margin = row(t("editor-field-margin"), numberInput(spec.map_frame.margin_mm));
  const border = row(t("editor-field-border"), numberInput(spec.map_frame.border_mm));
  const north = row(
    t("editor-field-north-lines"),
    select([["auto", t("editor-option-auto")], ["never", t("editor-option-never")]], spec.north_lines),
  );
  const fields = [free, width, height, scale, margin, border];
  const build = () => {
    const body = { id: spec.id, orientation: orientation.input.value, north_lines: north.input.value };
    let good = true;
    if (free.input.checked) {
      const [x, y] = [need(width), need(height)];
      good = x !== null && y !== null && good;
      body.page_mm = [x, y];
    } else {
      body.paper = paper.input.value;
      body.page_mm = null;
    }
    const denominator = read(scale.input);
    if (denominator !== null && !(Number.isInteger(denominator) && denominator > 0)) {
      good = fail(scale, t("editor-form-scale-error"));
    }
    body.print_scale = denominator;
    for (const [key, field] of [["margin_mm", margin], ["border_mm", border]]) {
      const value = need(field);
      good = value !== null && good;
      body[key] = value;
    }
    return good ? body : null;
  };
  return finish(
    t("editor-layout-form-heading", { id: spec.id }),
    [paper, free, width, height, orientation, scale, margin, border, north].map((f) => f.row),
    fields,
    build,
    onSubmit,
  );
}

// The form of a sheet element. `layout` is the id of the layout the element belongs to, or null if it is shared.
export function elementForm(spec, layout, heading, onSubmit) {
  const rows = [];
  const fields = [];
  const add = (field, track = false) => {
    rows.push(field.row);
    if (track) fields.push(field);
    return field;
  };
  const kind = spec.kind;
  const anchorTo = add(
    row(t("editor-field-anchor-to"), select([["page", t("editor-anchor-page")], ["map_frame", t("editor-anchor-map-frame")]], spec.anchor.to)),
  );
  const corner = add(
    row(t("editor-field-corner"), select(Object.entries(CORNERS).map(([value, id]) => [value, t(id)]), spec.anchor.corner)),
  );
  const [ox, oy] = spec.anchor.offset_mm ?? [0, 0];
  const offsetX = add(row(t("editor-field-offset-x"), numberInput(ox)), true);
  const offsetY = add(row(t("editor-field-offset-y"), numberInput(oy)), true);
  const text = kind === "title_block" || kind === "text";
  let lines = null;
  let size = null;
  let box = null;
  let file = null;
  let logoWidth = null;
  let cell = null;
  if (text) {
    lines = add(row(t("editor-field-lines"), make("textarea", { rows: 4 })));
    lines.input.value = (spec.lines ?? []).join("\n");
    const bar = make("p", { class: "hint" }, [make("span", { text: t("editor-placeholders-label") })]);
    for (const token of PLACEHOLDERS) {
      const button = make("button", { type: "button", class: "token", text: token });
      button.addEventListener("click", () => {
        const input = lines.input;
        const [start, end] = [input.selectionStart, input.selectionEnd];
        input.setRangeText(token, start, end, "end");
        input.focus();
      });
      bar.append(" ", button);
    }
    rows.push(bar);
  }
  if (text || kind === "credit_line" || kind === "copyright_line") {
    size = add(row(t("editor-field-size"), numberInput(spec.size_pt)), true);
  }
  if (text) {
    box = add(row(t("editor-field-box-white"), make("input", { type: "checkbox", checked: spec.box_white ?? true })));
  }
  if (kind === "logo") {
    file = add(row(t("editor-field-logo-file"), make("input", { type: "text", value: spec.file ?? "" })));
    logoWidth = add(row(t("editor-field-logo-width"), numberInput(spec.width_mm)), true);
  }
  if (kind === "control_descriptions") {
    cell = add(row(t("editor-field-cell"), numberInput(spec.cell_mm)), true);
  }
  const build = () => {
    const x = need(offsetX);
    const y = need(offsetY);
    let good = x !== null && y !== null;
    const body = {
      id: spec.id,
      kind,
      anchor: { to: anchorTo.input.value, corner: corner.input.value, offset_mm: [x, y] },
    };
    if (layout) body.layout = layout;
    const optional = (key, field) => {
      const value = read(field.input);
      if (value !== null && !Number.isFinite(value)) good = fail(field, t("editor-form-number-error"));
      else if (value !== null) body[key] = value;
    };
    if (lines) {
      const list = lines.input.value.split("\n");
      while (list.length && list[list.length - 1].trim() === "") list.pop();
      body.lines = list;
    }
    if (size) optional("size_pt", size);
    if (box) body.box_white = box.input.checked;
    if (file && file.input.value.trim()) body.file = file.input.value.trim();
    if (logoWidth) optional("width_mm", logoWidth);
    if (cell) optional("cell_mm", cell);
    return good ? body : null;
  };
  return finish(heading, rows, fields, build, onSubmit);
}
