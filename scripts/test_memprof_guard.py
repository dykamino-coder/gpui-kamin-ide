"""Tests of memprof_guard on real processes: python -I scripts/test_memprof_guard.py."""

import os
import pathlib
import subprocess
import sys
import tempfile
import time
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import memprof_guard  # noqa: E402

# A launcher chain in miniature: the command starts a grandchild (the "bench"), writes its pid, then
# either waits for it or exits at once and leaves it behind.
CHAIN = r"""
import subprocess, sys, time
grandchild = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(120)"])
open(sys.argv[1], "w").write(str(grandchild.pid))
if sys.argv[2] == "wait":
    grandchild.wait()
"""

GROW = r"""
import sys, time
with open(sys.argv[1], "wb") as out:
    while True:
        out.write(b"x" * 65536)
        out.flush()
        time.sleep(0.01)
"""


def alive(pid):
    if sys.platform == "win32":
        listed = subprocess.run(["tasklist", "/FI", f"PID eq {pid}", "/NH"], capture_output=True, text=True).stdout
        return str(pid) in listed
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


class Guard(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(__import__("shutil").rmtree, self.dir, True)
        self.pid_file = self.dir / "grandchild.pid"

    def grandchild(self):
        deadline = time.monotonic() + 10
        while not self.pid_file.exists() or not self.pid_file.read_text():
            self.assertLess(time.monotonic(), deadline, "the chain never started its grandchild")
            time.sleep(0.02)
        return int(self.pid_file.read_text())

    def test_a_timeout_terminates_the_whole_tree(self):
        with self.assertRaises(memprof_guard.Stopped) as stop:
            memprof_guard.run([sys.executable, "-c", CHAIN, str(self.pid_file), "wait"], timeout=2)
        pid = self.grandchild()
        self.assertIn("--timeout 2 s", str(stop.exception))
        self.assertIn(str(pid), str(stop.exception))
        self.assertFalse(alive(pid))

    def test_a_watched_file_over_its_limit_stops_the_run(self):
        watched = self.dir / "cand.stacks"
        with self.assertRaises(memprof_guard.Stopped) as stop:
            memprof_guard.run([sys.executable, "-c", GROW, str(watched)], timeout=60, watch=watched,
                              max_bytes=1 << 20)
        self.assertIn("--max-stacks-gb", str(stop.exception))
        size = watched.stat().st_size
        time.sleep(0.3)
        self.assertEqual(watched.stat().st_size, size, "the writer still runs")

    def test_processes_left_behind_by_a_finished_command_are_terminated(self):
        code = memprof_guard.run([sys.executable, "-c", CHAIN, str(self.pid_file), "leave"], timeout=30)
        self.assertEqual(code, 0)
        self.assertFalse(alive(self.grandchild()))

    def test_a_command_within_its_limits_returns_its_exit_code(self):
        self.assertEqual(memprof_guard.run([sys.executable, "-c", "raise SystemExit(3)"], timeout=30), 3)


if __name__ == "__main__":
    unittest.main()
