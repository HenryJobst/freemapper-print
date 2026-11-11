// Control numbers: the conflicts of the sheet with their proposal, the form for a position of your own, and the
// question where a correction applies (the whole layout or this sheet). A number is moved by dragging it on the sheet
// (app.js) or through the form. The server turns a drop point into direction and gap and refuses what the rules refuse.
import { t } from "../suite/texts.js";
import { make, row, select, numberInput, read, fail, clear } from "./forms.js";

// Asks where a correction applies: resolves to `layout_wide`, `sheet` or null.
export function askScope(control) {
  const dialog = document.getElementById("scope");
  document.getElementById("scope-body").textContent = t("editor-scope-body", { control });
  return new Promise((resolve) => {
    dialog.addEventListener("close", () => resolve(dialog.returnValue === "cancel" ? null : dialog.returnValue || null), { once: true });
    dialog.returnValue = "";
    dialog.showModal();
  });
}

// The list of numbers in conflict, with the proposal of the check one button away.
export function renderConflicts(host, conflicts) {
  const box = document.getElementById("conflicts-pane");
  const list = document.getElementById("conflicts");
  box.hidden = conflicts.length === 0;
  list.replaceChildren();
  for (const conflict of conflicts) {
    const buttons = make("div", { class: "dialog-buttons" });
    if (conflict.searched) {
      const accept = make("button", { type: "button", text: t("editor-number-accept") });
      accept.addEventListener("click", () => host.acceptProposal(conflict));
      buttons.append(accept);
    }
    const own = make("button", { type: "button", text: t("editor-number-own") });
    own.addEventListener("click", () => host.showProps(numberForm(host, conflict.course, conflict.control)));
    buttons.append(own);
    list.append(make("li", {}, [make("span", { text: conflict.message }), buttons]));
  }
}

// The form for a direction and a gap of your own: the keyboard route of a drag.
export function numberForm(host, course, control) {
  const scope = row(
    t("editor-scope-label"),
    select([["layout_wide", t("editor-scope-layout")], ["sheet", t("editor-scope-sheet")]], "sheet"),
  );
  const direction = row(t("editor-number-direction"), numberInput(""));
  const gap = row(t("editor-number-gap"), numberInput(""));
  const fields = [direction, gap];
  const form = make("form", { class: "props-form", novalidate: true }, [
    make("h3", { text: t("editor-pick-number", { id: control }) }),
    make("p", { class: "hint", text: t("editor-number-hint") }),
    scope.row,
    direction.row,
    gap.row,
    make("div", { class: "dialog-buttons" }, [make("button", { type: "submit", text: t("editor-number-set") })]),
  ]);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    clear(fields);
    const [d, g] = [read(direction.input), read(gap.input)];
    if (d === null || !Number.isFinite(d)) return fail(direction, t("editor-form-number-error"));
    if (g === null || !Number.isFinite(g)) return fail(gap, t("editor-form-number-error"));
    host.write("numbers_accept", { course, control, scope: scope.input.value, direction: d, gap: g });
  });
  return form;
}
