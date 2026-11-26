"""Observe permission modes on a synthetic, loopback-only Messages endpoint."""
import http.server
import json
import os
import pathlib
import subprocess
import sys
import threading
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
api.shutdown()
sys.exit(0 if ok else 1)
