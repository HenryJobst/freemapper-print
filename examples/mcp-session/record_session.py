"""Records session.jsonl: one MCP session with `freemapper-print mcp` on a copy of berlin-viktoriapark-training.

Usage, from the repository root after `cargo build --locked`:

    python3 -I examples/mcp-session/record_session.py target/debug/freemapper-print [/tmp/mcp-session]

The second argument is a work folder that is missing or empty; without it the script makes a new temporary
folder. The script refuses a folder that holds anything and never deletes a folder. It copies the example into
<work>/Verein/viktoriapark-training, creates <work>/Verein/pdf for the PDF, starts the server with `--lang de`,
sends the calls below one by one and writes every message to session.jsonl next to this script. In the file the
work folder reads /home/user, and the tool list keeps only the description of each input field.
"""

import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

if len(sys.argv) not in (2, 3):
    sys.exit("usage: record_session.py BINARY [WORK_FOLDER]")
binary = Path(sys.argv[1]).resolve()
here = Path(__file__).resolve().parent
source = here.parent / "berlin-viktoriapark-training"

if len(sys.argv) == 3:
    work = Path(sys.argv[2]).resolve()
    if work.exists() and not work.is_dir():
        sys.exit(f"{work} is a file; give a missing or empty folder")
    if work.is_dir() and any(work.iterdir()):
        sys.exit(f"{work} is not empty; give a missing or empty folder, the script deletes nothing")
    work.mkdir(parents=True, exist_ok=True)
else:
    work = Path(tempfile.mkdtemp(prefix="mcp-session-")).resolve()
project = work / "Verein" / "viktoriapark-training"
project.mkdir(parents=True)
for name in ["berlin-viktoriapark.omap", "freemapper.toml", "freemapper-print.toml", "training.ppen", "ATTRIBUTION.txt"]:
    shutil.copy(source / name, project / name)
out = work / "Verein" / "pdf"
out.mkdir()

server = subprocess.Popen(
    [str(binary), "--lang", "de", "mcp"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    text=True,
    bufsize=1,
)
log = []
next_id = 1


def send(message):
    log.append({"from": "client", "message": message})
    server.stdin.write(json.dumps(message, ensure_ascii=False) + "\n")
    server.stdin.flush()


def request(method, params):
    global next_id
    request_id = next_id
    next_id += 1
    send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
    while True:
        line = server.stdout.readline()
        if not line:
            sys.exit("the server closed its output")
        message = json.loads(line)
        log.append({"from": "server", "message": message})
        if message.get("id") == request_id:
            return message


def call(tool, arguments):
    return request("tools/call", {"name": tool, "arguments": arguments})


request(
    "initialize",
    {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "example-session", "version": "1"}},
)
send({"jsonrpc": "2.0", "method": "notifications/initialized"})
request("tools/list", {})
call("project_info", {"project": str(project)})
call("check_project", {"project": str(project)})
call("masks_window", {"project": str(project), "course": "Training", "radius": 12, "assign": True})
call("check_project", {"project": str(project)})
call("export_project", {"project": str(project), "out_dir": str(out)})
server.stdin.close()
if server.wait(timeout=60) != 0:
    sys.exit(f"the server ended with exit code {server.returncode}")

lines = []
for entry in log:
    result = entry["message"].get("result", {})
    if "tools" in result:
        for tool in result["tools"]:
            schema = tool["inputSchema"]
            schema["properties"] = {
                name: field.get("description", "") for name, field in schema.get("properties", {}).items()
            }
            schema.pop("$defs", None)
        entry = {"from": entry["from"], "shortened": "each input field shows only its description", **entry}
    lines.append(json.dumps(entry, ensure_ascii=False).replace(str(work), "/home/user"))
(here / "session.jsonl").write_text("\n".join(lines) + "\n")
print(f"{len(log)} messages, PDF in {out}")
