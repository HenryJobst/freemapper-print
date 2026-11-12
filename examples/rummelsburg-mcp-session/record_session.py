"""Records session.jsonl: one MCP session with `freemapper-print mcp` that turns a training course into window orienteering.

Usage, from the repository root after `cargo build --locked`:

    python3 -I examples/rummelsburg-mcp-session/record_session.py target/debug/freemapper-print [/tmp/mcp-session]

The second argument is a work folder that is missing or empty; without it the script makes a new temporary
folder. The script refuses a folder that holds anything and never deletes a folder. It puts the map and the course
file of this example into <work>/Verein/rummelsburg-training, creates the folder out inside it for the PDF, starts
the server with `--lang de`, sends the calls below one by one and writes every message to session.jsonl next to
this script. The session creates the project file and the PDF there. In session.jsonl the work folder reads
/home/user, and the tool list keeps only the description of each input field.
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
source = here

if len(sys.argv) == 3:
    work = Path(sys.argv[2]).resolve()
    if work.exists() and not work.is_dir():
        sys.exit(f"{work} is a file; give a missing or empty folder")
    if work.is_dir() and any(work.iterdir()):
        sys.exit(f"{work} is not empty; give a missing or empty folder, the script deletes nothing")
    work.mkdir(parents=True, exist_ok=True)
else:
    work = Path(tempfile.mkdtemp(prefix="mcp-session-")).resolve()
project = work / "Verein" / "rummelsburg-training"
project.mkdir(parents=True)
for name in ["rummelsburg.omap", "rummelsburg-training.ppen"]:
    shutil.copy(source / name, project / name)
out = project / "out"
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
call(
    "init_project",
    {
        "folder": str(project),
        "map": str(project / "rummelsburg.omap"),
        "courses": str(project / "rummelsburg-training.ppen"),
        "kind": "sprint",
        "title": "Rummelsburg Training",
    },
)

# No tool changes [map] or [print]. The message of init_project asks the agent to compare the proposed contour
# interval with the map, whose information block says 2.5 m. The map also has no map notes, so the agent adds a
# source line for the footer. Both are edits to the file with its own means. They are no MCP calls and do not show
# in session.jsonl.
project_file = project / "freemapper-print.toml"
text = project_file.read_text()
proposal = "# Vorschlag für Sprintkarten. An der Karte prüfen und bestätigen.\ncontour_interval_m = 2.0\n"
if proposal not in text:
    sys.exit("the project file has no contour interval proposal to correct")
corrected = (
    "# Der Informationsblock der Karte nennt 2,5 m.\n"
    "contour_interval_m = 2.5\n"
    "\n"
    "# Die Karte hat keine Kartennotizen. Die Fußzeile nennt deshalb von Hand die Karte und ihren Autor.\n"
    "[print]\n"
    'copyright = """\n'
    "Karte: Rummelsburger Bucht, Remo Madella, MOO Challenge Berlin 2025\n"
    '"""\n'
)
project_file.write_text(text.replace(proposal, corrected))
call("check_project", {"project": str(project)})
call("set_layout", {"project": str(project), "id": "a4-4000", "paper": "A3"})
call("masks_window", {"project": str(project), "course": "T", "radius": 12, "assign": True})
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
