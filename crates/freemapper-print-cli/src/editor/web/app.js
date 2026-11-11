// The editor shell: texts, tree, status bar, the menu. The project and the sheet come with later modules.
import { post } from "../suite/api.js";
import { loadTexts, t } from "../suite/texts.js";
import { buildTree } from "../suite/tree.js";
import { say } from "../suite/status.js";

async function start() {
  await loadTexts();
  const tree = document.getElementById("pane-tree");
  tree.dataset.treeLabel = t("editor-tree-label");
  buildTree(tree, [], () => {});
  document.getElementById("menu-done").addEventListener("click", close);
  say("editor-status-ready");
}

// Ends the server; the page then has nothing left to ask.
async function close() {
  try {
    const answer = await post("api/done");
    say(answer.ok ? "editor-status-closed" : "editor-status-lost");
  } catch {
    say("editor-status-lost");
  }
}

start().catch(() => {
  document.getElementById("status-message").textContent = "api/texts";
});
