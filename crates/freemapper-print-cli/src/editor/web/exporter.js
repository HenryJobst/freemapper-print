// The export dialog: the folder the person types, the export itself with its progress in the status bar, Stop,
// and the result (files, notes, the reminder to print at actual size). The browser never names a file: the folder
// is the one path the person types, and the server checks it.
import { get, post } from "../suite/api.js";
import { t } from "../suite/texts.js";
import { say } from "../suite/status.js";
import { noteItem } from "./notes.js";

const FOLDER_KEY = "freemapper-print.export-dir";
const POLL_MS = 500;

const $ = (id) => document.getElementById(id);

// The folder is a convenience only; a browser without storage still works.
function rememberedFolder() {
  try {
    return localStorage.getItem(FOLDER_KEY) ?? "";
  } catch {
    return "";
  }
}

function rememberFolder(folder) {
  try {
    localStorage.setItem(FOLDER_KEY, folder);
  } catch {
    // Nothing to do; the person types it again next time.
  }
}

let request = null;
let timer = null;

// Opens the dialog. `what` is { text } (the sentence of what is exported) and `body` the request without the
// folder.
export function open(what, body) {
  request = body;
  $("export-what").textContent = what;
  $("export-result").replaceChildren();
  $("export-dir").value = rememberedFolder();
  $("export-go").disabled = false;
  $("export-stop").hidden = true;
  $("export").showModal();
  $("export-dir").focus();
}

function line(className, text) {
  const item = document.createElement("li");
  item.className = className;
  item.textContent = text;
  return item;
}

function showResult(data) {
  const box = $("export-result");
  const parts = [];
  const message = document.createElement("p");
  message.textContent = data.stopped ? data.message : data.message ?? t("editor-export-done");
  parts.push(message);
  if (!data.stopped && data.files?.length) {
    const heading = document.createElement("p");
    heading.textContent = t("editor-export-files", { dir: data.out_dir });
    const files = document.createElement("ul");
    for (const file of data.files) {
      const kb = String(Math.max(1, Math.round((file.bytes ?? 0) / 1024)));
      files.append(line("file", file.pages
        ? t("editor-export-file-single", { name: file.path, kb, pages: String(file.pages) })
        : t("editor-export-file", { name: file.path, kb })));
    }
    parts.push(heading, files);
  }
  if (!data.stopped && data.notes?.length) {
    const heading = document.createElement("p");
    heading.textContent = t("editor-export-notes");
    const notes = document.createElement("ul");
    for (const note of data.notes) {
      notes.append(noteItem(note));
    }
    parts.push(heading, notes);
  }
  if (data.reminder) {
    const reminder = document.createElement("p");
    reminder.textContent = data.reminder;
    parts.push(reminder);
  }
  box.replaceChildren(...parts);
}

function showError(text) {
  const message = document.createElement("p");
  message.className = "error";
  message.textContent = text;
  $("export-result").replaceChildren(message);
}

// The progress the server reports while the export runs.
function watch() {
  timer = setInterval(async () => {
    try {
      const answer = await get("api/export/status");
      if (answer.ok && answer.data.running && answer.data.pages > 0) {
        // A message with values is said as text, one without as its id.
        say(t("editor-export-running-status", { pages: String(answer.data.pages), name: answer.data.last ?? "" }), true);
      }
    } catch {
      // The next poll tries again.
    }
  }, POLL_MS);
}

async function go() {
  const folder = $("export-dir").value.trim();
  if (!folder) {
    showError(t("editor-export-no-dir"));
    return;
  }
  rememberFolder(folder);
  $("export-go").disabled = true;
  $("export-stop").hidden = false;
  $("export-result").replaceChildren();
  say("editor-export-starting");
  watch();
  try {
    const answer = await post("api/export", { ...request, out_dir: folder });
    if (answer.ok) {
      showResult(answer.data);
      say(answer.data.stopped ? "editor-export-stopped" : "editor-export-done");
    } else {
      showError(typeof answer.data === "string" ? answer.data : t("editor-export-failed"));
      say("editor-export-failed");
    }
  } catch {
    showError(t("editor-export-failed"));
    say("editor-status-lost");
  } finally {
    clearInterval(timer);
    $("export-go").disabled = false;
    $("export-stop").hidden = true;
  }
}

async function stop() {
  $("export-stop").disabled = true;
  try {
    await post("api/export/cancel");
  } catch {
    showError(t("editor-export-failed"));
  } finally {
    $("export-stop").disabled = false;
  }
}

export function setup() {
  $("export-go").addEventListener("click", go);
  $("export-stop").addEventListener("click", stop);
  $("export-close").addEventListener("click", () => $("export").close());
  $("export-dir").addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      if (!$("export-go").disabled) go();
    }
  });
}

// What `GET api/check` found, as one list for the notices pane.
export async function check(notices) {
  say("editor-check-running");
  const answer = await get("api/check");
  if (!answer.ok) {
    say("editor-check-failed");
    return typeof answer.data === "string" ? answer.data : t("editor-check-failed");
  }
  const data = answer.data;
  const summary = document.createElement("p");
  summary.className = "check-summary";
  summary.textContent = t(data.needs_attention ? "editor-check-attention" : "editor-check-clear");
  const list = document.createElement("ul");
  const add = (sheet, notes) => {
    for (const note of notes) {
      list.append(noteItem(note, `${sheet}: `));
    }
  };
  add(t("editor-check-file"), data.course_file ?? []);
  for (const sheet of [...data.courses, ...data.layouts]) add(sheet.course ?? sheet.layout, sheet.notes);
  notices.replaceChildren(summary, list);
  say(data.needs_attention ? "editor-check-attention" : "editor-check-clear");
  return null;
}
