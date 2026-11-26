"""Observe permission modes on a synthetic, loopback-only Messages endpoint."""
import http.server
import errno
import fcntl
import json
import os
import pathlib
import pty
import re
import select
import signal
import struct
import subprocess
import sys
import termios
import threading
import time
import uuid

root, binary = map(pathlib.Path, sys.argv[1:])
project = root / "empty-project"
project.mkdir()
events = root / "events.jsonl"
hook = root / "hook.py"
hook.write_text(
    "import json, pathlib, sys\n"
    "event = json.load(sys.stdin)\n"
    "row = {k:event.get(k) for k in ['hook_event_name','permission_mode','source','session_id']}\n"
    "with pathlib.Path(__file__).with_name('events.jsonl').open('a') as f: f.write(json.dumps(row)+'\\n')\n"
)
config = root / ".claude"
config.mkdir()
settings = {"hooks": {name: [{"hooks": [{"type": "command", "command": "python3 " + str(hook)}]}]
                      for name in ("SessionStart", "UserPromptSubmit", "Stop")}}
(config / "settings.json").write_text(json.dumps(settings))
calls = []


class Api(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send_data(self, data, kind):
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        route = self.path.split("?")[0]
        if route == "/v1/messages/count_tokens":
            self.send_data(b'{"input_tokens":4}', "application/json")
            return
        if route != "/v1/messages":
            self.send_data(b'{}', "application/json")
            return
        request = json.loads(body)
        calls.append(route)
        msg = {"id": "msg_" + uuid.uuid4().hex, "type": "message", "role": "assistant",
               "model": request.get("model", "synthetic"), "content": [], "stop_reason": None,
               "stop_sequence": None, "usage": {"input_tokens": 4, "output_tokens": 0}}
        if not request.get("stream"):
            msg.update(content=[{"type": "text", "text": "fixture reply"}], stop_reason="end_turn")
            self.send_data(json.dumps(msg).encode(), "application/json")
            return
        records = [("message_start", {"message": msg}),
                   ("content_block_start", {"index": 0, "content_block": {"type":"text", "text":""}}),
                   ("content_block_delta", {"index":0, "delta":{"type":"text_delta","text":"fixture reply"}}),
                   ("content_block_stop", {"index":0}),
                   ("message_delta", {"delta":{"stop_reason":"end_turn","stop_sequence":None}, "usage":{"output_tokens":2}}),
                   ("message_stop", {})]
        response = "".join("event: " + name + "\ndata: " + json.dumps(dict(data, type=name)) + "\n\n"
                           for name, data in records)
        self.send_data(response.encode(), "text/event-stream")


api = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Api)
threading.Thread(target=api.serve_forever, daemon=True).start()
env = dict(os.environ, CI="1", CLAUDE_CONFIG_DIR=str(config),
           ANTHROPIC_BASE_URL=f"http://127.0.0.1:{api.server_port}", ANTHROPIC_API_KEY="synthetic-fixture-key",
           CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC="1", DISABLE_AUTOUPDATER="1")
version = subprocess.check_output([binary, "--version"], env=env, cwd=project, text=True).strip()
assert "2.1.328" in version, version
print("cli_version=2.1.328", flush=True)
session_id = str(uuid.uuid4())
ok = True
for mode, resume in [("default", False), ("plan", True), ("acceptEdits", True)]:
    old_events = len(events.read_text().splitlines()) if events.exists() else 0
    old_calls = len(calls)
    flags = ["--resume" if resume else "--session-id", session_id]
    try:
        result = subprocess.run([binary, *flags, "--permission-mode", mode, "--output-format", "json",
                                 "--model", "claude-sonnet-4-6", "-p", "Say hello without tools."],
                                env=env, cwd=project, timeout=45, capture_output=True, text=True)
    except subprocess.TimeoutExpired:
        print(f"requested={mode} timeout=true", flush=True)
        ok = False
        continue
    rows = [json.loads(line) for line in events.read_text().splitlines()[old_events:]] if events.exists() else []
    summary = [(r["hook_event_name"], r["source"], r["permission_mode"]) for r in rows]
    source = "resume" if resume else "startup"
    valid_start = any(r["hook_event_name"] == "SessionStart" and r["source"] == source for r in rows)
    valid_mode = any(r["permission_mode"] == mode and r["session_id"] == session_id for r in rows)
    finished = result.returncode == 0 and len(calls) > old_calls and "fixture reply" in result.stdout
    print(f"requested={mode} exit={result.returncode} requests={len(calls)-old_calls} "
          f"hook_events={summary} same_session_mode={valid_mode} fake_reply={finished}", flush=True)
    if not finished:
        print("synthetic_error=" + (result.stderr or result.stdout)[-1400:], flush=True)
    ok &= valid_start and valid_mode and finished
empty_id = str(uuid.uuid4())
interactive_env = dict(env, TERM="xterm-256color")
interactive_env.pop("CI", None)
onboarding = dict(theme="dark", numStartups=5, effortCalloutDismissed=True, effortCalloutV2Dismissed=True,
                  hasCompletedOnboarding=True, hasCompletedProjectOnboarding=True, hasTrustDialogAccepted=True,
                  customApiKeyResponses={"approved":[env["ANTHROPIC_API_KEY"][-20:]],"rejected":[]},
                  projects={str(project): {"hasTrustDialogAccepted":True, "hasCompletedProjectOnboarding":True}})
for profile in (root / ".claude.json", config / ".claude.json"):
    profile.write_text(json.dumps(onboarding))


def transcript():
    files = list((config / "projects").glob("*/" + empty_id + ".jsonl"))
    return bool(files), sum(p.stat().st_size for p in files)


def interactive(mode, flag, stage):
    old_events = len(events.read_text().splitlines())
    old_calls = len(calls)
    pid, master = pty.fork()
    if pid == 0:
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 36, 120, 0, 0))
        os.chdir(project)
        os.execve(binary, [str(binary), flag, empty_id, "--permission-mode", mode, "--model", "claude-sonnet-4-6"], interactive_env)
    screen = bytearray()

    def drain(seconds):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if not select.select([master], [], [], 0.08)[0]:
                continue
            try:
                fragment = os.read(master, 16384)
            except OSError as err:
                if err.errno == errno.EIO:
                    return
                raise
            if not fragment:
                return
            screen.extend(fragment)
            if b"\x1b[6n" in fragment:
                os.write(master, b"\x1b[1;1R")
            if b"\x1b[c" in fragment:
                os.write(master, b"\x1b[?1;2c")

    def rows():
        return [json.loads(line) for line in events.read_text().splitlines()[old_events:]]

    try:
        end = time.monotonic() + 12
        while time.monotonic() < end and not any(r["hook_event_name"]=="SessionStart" for r in rows()):
            drain(0.15)
        drain(1.2)
        starts = [r for r in rows() if r["hook_event_name"]=="SessionStart" and r["session_id"]==empty_id]
        found, size = transcript()
        print(f"interactive={stage} requested={mode} launch={flag} startup_sources={[r['source'] for r in starts]} "
              f"pre_input_transcript={found} bytes={size} messages_requests={len(calls)-old_calls}", flush=True)
        passed = len(starts) == 1 and starts[0]['source'] == ('resume' if flag == '--resume' else 'startup') and not calls[old_calls:]
        if passed and stage=="second":
            os.write(master, b"Hello fixture")
            drain(0.4)
            os.write(master, b"\r")
            end = time.monotonic() + 12
            while time.monotonic() < end and not any(r["hook_event_name"]=="Stop" for r in rows()):
                drain(0.15)
            confirmation = [r for r in rows() if r["hook_event_name"] in ("UserPromptSubmit","Stop") and r["permission_mode"]==mode and r["session_id"]==empty_id]
            print(f"interactive={stage} first_message_reports={[r['hook_event_name'] for r in confirmation]} "
                  f"same_id_plan={bool(confirmation)} messages_requests={len(calls)-old_calls}", flush=True)
            passed &= bool(confirmation) and len(calls)>old_calls
        if not passed:
            readable = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", screen.decode(errors="replace"))
            print("synthetic_pty_tail=" + repr(readable[-1400:]), flush=True)
        return passed
    finally:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        end = time.monotonic() + 5
        while os.waitpid(pid, os.WNOHANG)[0] == 0:
            if time.monotonic() >= end:
                os.kill(pid, signal.SIGKILL)
                os.waitpid(pid, 0)
                break
            drain(0.08)
        os.close(master)


ok &= interactive("default", "--session-id", "first")
exists, size = transcript()
print(f"after_empty_pty_exit transcript={exists} bytes={size}", flush=True)
ok &= interactive("plan", "--resume" if exists else "--session-id", "second")
api.shutdown()
sys.exit(0 if ok else 1)
