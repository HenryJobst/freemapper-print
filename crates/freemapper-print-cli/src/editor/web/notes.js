// One note as a list item: the severity as a word of its own (colour is never the only sign), then the message
// without the label it begins with. The server's messages begin with "Info:", "Note:" and their German forms;
// those labels are cut here, in this one place, so the word shown is the note's severity.
import { t } from "../suite/texts.js";

const LABELS = ["Info", "Note", "Warning", "Error", "Zur Information", "Hinweis", "Warnung", "Fehler"];
const SEVERITY_TEXTS = {
  info: "editor-severity-info",
  warning: "editor-severity-warning",
  error: "editor-severity-error",
};

export function stripLabel(message) {
  for (const label of LABELS) {
    if (message.startsWith(`${label}:`)) return message.slice(label.length + 1).trimStart();
  }
  return message;
}

// `prefix` is text before the message, such as the sheet a check note is about.
export function noteItem(note, prefix = "") {
  const item = document.createElement("li");
  item.className = note.severity;
  const word = document.createElement("span");
  word.className = "sev";
  word.textContent = t(SEVERITY_TEXTS[note.severity] ?? "editor-severity-info");
  item.append(word, ` ${prefix}${stripLabel(note.message)}`);
  return item;
}
