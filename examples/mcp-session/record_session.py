"""Records session.jsonl: one MCP session with `freemapper-print mcp` on a copy of berlin-viktoriapark-training.

Usage, from the repository root after `cargo build --locked`:

    python3 -I examples/mcp-session/record_session.py target/debug/freemapper-print /tmp/mcp-session

The second argument is an empty or missing work folder. The script copies the example into
<work>/Verein/viktoriapark-training, creates <work>/Verein/pdf for the PDF, starts the server with `--lang de`,
sends the calls below one by one and writes every message to session.jsonl next to this script. In the file the
work folder reads /home/user, and the tool list keeps only the description of each input field.
"""

import json
import shutil
import subprocess
import sys
from pathlib import Path

binary = Path(sys.argv[1]).resolve()
work = Path(sys.argv[2]).resolve()
here = Path(__file__).resolve().parent
source = here.parent / "berlin-viktoriapark-training"

if work.exists():
    shutil.rmtree(work)
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
