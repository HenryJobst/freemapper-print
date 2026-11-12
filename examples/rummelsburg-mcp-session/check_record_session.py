"""Checks that record_session.py refuses a work folder that holds something, and a file, and deletes nothing.

Usage, from the repository root (needs no build, the script stops before it starts the server):

    python3 -I examples/rummelsburg-mcp-session/check_record_session.py
"""

import subprocess
import sys
import tempfile
from pathlib import Path

script = Path(__file__).resolve().parent / "record_session.py"


def refused(work, wanted):
    run = subprocess.run(
        [sys.executable, "-I", str(script), "no-such-binary", str(work)], capture_output=True, text=True
    )
    assert run.returncode != 0, f"{work}: exit code {run.returncode}"
    assert wanted in run.stderr, f"{work}: {run.stderr!r}"


with tempfile.TemporaryDirectory() as tmp:
    full = Path(tmp) / "full"
    (full / "keep").mkdir(parents=True)
    (full / "keep" / "events.txt").write_text("my events\n")
    refused(full, "is not empty")
    assert (full / "keep" / "events.txt").read_text() == "my events\n", "the folder lost its content"

    file = Path(tmp) / "file.txt"
    file.write_text("a file\n")
    refused(file, "is a file")
    assert file.read_text() == "a file\n", "the file changed"

print("record_session.py refuses a folder with content and a file, and deletes nothing")
