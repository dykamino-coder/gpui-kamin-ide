# Credential-free Bridge runtime acceptance

This checkout-only harness drives the real PTY, JSONL watcher, MCP HTTP and hook
relay paths without Claude installation, credentials or external network access.
It does not fix or close the linked incidents. Automated harness tests do not
replace the Windows GPUI/CEF runtime merge gate or prove provider compatibility.

## Start an isolated Bridge

Install the server dependencies once (`npm --prefix extensions/claude-bridge/server ci`).
Build the dashboard with `npm --prefix extensions/claude-bridge/server/src/ui ci` and
`npm --prefix extensions/claude-bridge/server/src/ui run build` if you need the Account UI;
the launcher copies its static dist into the external capture. The run itself is offline; use Node 22, Git and the installed dependencies.
From the checkout, in PowerShell:

```powershell
$repo = (Get-Location).Path
$capture = Join-Path $env:USERPROFILE 'kaminide-private-diagnostics/bridge-fakecli/acceptance-01'
node "$repo/extensions/claude-bridge/server/test-support/run.mjs" $capture
```

Choose a new capture directory for each candidate. The launcher uses that directory
as server cwd and creates an isolated HOME/USERPROFILE, so logs, DuckDB, settings,
transcripts and synthetic plugin data stay outside the checkout. It strips inherited
provider credentials and proxy variables. No Claude account is read or created.
Server listens only on `127.0.0.1:3456`; stop with Ctrl+C. Do not expose this server.
Account/health probes report `fixture@example.invalid`, synthetic plan and
`fake-cli` auth method. The fake TUI is visibly labelled FAKE.

Connect the **exact Windows KaminIDE candidate** to `http://127.0.0.1:3456` using
the ordinary Bridge connection UI. Create a new local Bridge token in first-run
setup; this authenticates the local Bridge only and needs no Claude account.
Use a disposable client profile: start the candidate with HOME, USERPROFILE,
APPDATA and LOCALAPPDATA pointing into a separate directory under this capture,
with the normal host/builtin artifacts for that candidate. Do not reuse a profile
that contains real sessions or synced credentials. Keep the server home and client
home separate, except when explicitly preparing the local plugin fixture below.

The selector requires `BRIDGE_DEV_FAKE_CLI=1`, `NODE_ENV=development` or `test`,
and `BRIDGE_FAKE_CLI_HOME` equal to the server's OS home. Prefer the launcher.
Unset/unknown/production modes ignore the switch. npm release packaging and the
Docker runtime omit `test-support`; even forcing development mode in those artifacts
cannot select it. `dev-cli.test.ts` proves the publish allowlist and fail-closed
selection. No arbitrary executable path is accepted from client SessionConfig.
The fake uses only loopback HTTP and never executes hook/plugin commands.

## Deterministic controls

Enter commands in the chat or Console as normal; no API bypasses the client.
Wait for `[FAKE] turn complete` / Stop before issuing the next command.

| Command                 | Observable fixture                                                                                              |
| ----------------------- | --------------------------------------------------------------------------------------------------------------- |
| `/fake-agents 800`      | Standalone worker, 800 sidechain records, `.meta.json` display name `worker`                                    |
| `/fake-team 5000`       | Named-team worker and large transcript; count is bounded to 1..10000                                            |
| `/fake-idle`            | Worker idle notification and SubagentStop                                                                       |
| `/fake-follow-up`       | Acknowledged SendMessage, then an older emitted idle arriving late, then new child activity                     |
| `/fake-report`          | Child report and fresh idle; no second worker identity                                                          |
| `/fake-replay`          | 3000 records of about 1KB for overlapping replay requests                                                       |
| `/fake-console`         | 80 coloured lines unique to conversation, Cyrillic/Japanese text                                                |
| `/fake-pressure`        | Real assistant usage of 978010 input tokens                                                                     |
| `/compact`              | Same-file boundary, preTokens 978010 / postTokens 15098, synthetic summary, SessionStart:compact, no next usage |
| `/fake-compact-missing` | Boundary without postTokens                                                                                     |
| `/fake-next-usage`      | Later eligible assistant usage of 17000 tokens                                                                  |
| `/fake-mcp`             | initialize, tools/list, Bash tools/call, then waits for actual tool result                                      |
| `/usage`                | Fable/promotion TUI usage layout                                                                                |
| `/exit`                 | SessionEnd and process exit (Ctrl+C interrupts immediately)                                                     |

Ordinary text yields a fixed synthetic response. Prompts are not recorded;
only synthetic control entries are written. `--resume <conversationId>` appends
within the existing CLI slug and rejects unknown IDs. CLI `--output-format stream-json`
accepts newline user envelopes and emits init/assistant/result envelopes for protocol
smoke tests. The Bridge interactive path uses disk JSONL rather than parsing these
stdout envelopes. This is a bounded simulator, not an LLM or a full Claude TUI emulator.

## Interrupted MCP delivery: INC-0057

Start the transparent loopback proxy in another PowerShell:

```powershell
node "$repo/extensions/claude-bridge/server/test-support/transport-proxy.mjs" "$capture/control" 3456
```

Connect KaminIDE to `http://127.0.0.1:3457`. The proxy forwards HTTP/WS, does not
log messages/auth, and consumes one-shot control files. `/fake-mcp` asks the real
client host to run `tool-barrier.mjs` through Bash. Wait for
`$capture/home/tool-started`, then choose a boundary:

```powershell
# Disconnect while the real host tool is running.
New-Item -ItemType File -Force "$capture/control/disconnect" | Out-Null
# Or cut the first outgoing result before Bridge delivery (also applies to denial).
New-Item -ItemType File -Force "$capture/control/cut-result" | Out-Null
# Complete the host tool only after choosing the boundary.
Set-Content "$capture/home/tool-release" 'success' # use 'error' for tool failure
```

For denial, deny the real tool approval while `cut-result` is armed. Confirm
`disconnected` or `result-cut`, reconnect/resume the **same conversation**, and
observe one server settlement for the original requestId, with no second tool
execution. Clear `tool-started` before each next case. The proxy deliberately drops
the result; it never caches/retries on behalf of the product, so an unfixed client
can remain pending. A delivered result followed by loss of its ACK requires a
candidate-specific ACK fault test as well; this proxy covers the pre-delivery cut.
Close-session/timeout/multi-tab cases remain part of INC-0057's acceptance matrix.
The tool barrier has a 90-second fail-safe and fake HTTP has a 120-second fail-safe.

## Windows acceptance matrix

| Task          | Actions in the running exact candidate                                                                                                                                 | Evidence to record                                                                                                                                                                     |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| INC-0007/0008 | Two tabs: `/fake-agents 5000` and `/fake-team 5000`; A -> B -> A; open/Back worker reader; close A, retain B; reconnect/reopen and repeat                              | Parent/agent IDs, no foreign UUIDs, retained entries/UUID/slots and mounted rows, heap and responsiveness                                                                              |
| INC-0009      | Populate `/fake-replay`, reveal Agents, request two closely spaced resyncs, switch tabs, close/reconnect during completion, then `/fake-follow-up` on a spawned worker | Exact replay start/completion ordering, staging empty after latest generation, one published snapshot and live updates; record actual timings, do not infer a 50ms race from bulk data |
| INC-0010      | 1/5/10/30 tabs; `/fake-console` on each, idle/dirty streams; hide Console, switch return, close/reconnect and CEF recreation                                           | Unique Unicode scrollback per tab, serializer calls/bytes, retained keys, heap, responsiveness and declared snapshot lifetime                                                          |
| INC-0013      | Set `BRIDGE_FAKE_CLI_STARTUP_DELAY_MS=5000` before server launch; create then close a tab while startup is pending; repeat replacement/reconnect and failure           | No ownerless session or stale indexes; bounded PTY/watcher/proxy cleanup, recovered session ownership; the delay is at the real async create boundary before spawn                     |
| INC-0056      | Standalone and named team: spawn -> idle -> follow-up -> report; wait past cleanup, repeat after reconnect, use two parents with worker named identically              | Follow-up becomes running, stale idle cannot override newer ACK, fresh idle finishes; no duplicated/cross-parent identity                                                              |
| INC-0045      | Pressure -> compact; inspect meter **before another prompt**, then next usage; repeat/missing metadata, Current/archived, reconnect                                    | Boundary values, selected segment, derived meter freshness, provisional postTokens superseded by real usage; no pointer/focus/resize/restart assistance                                |
| BR-20         | Before launch set `BRIDGE_FAKE_USAGE_LAYOUT=fable`, `legacy`, `unknown` or `error`; refresh Account usage, compare Console `/usage` and dashboard within 10s           | Session 23%, all-model week 42%, optional Fable/Sonnet 7%, reset labels, truthful unknown/error; harness does not certify real account quota                                           |

The payloads come from public synthetic tests (`webview/src/signals/agent-replay.test.ts`,
`server/src/core/pty/jsonl-watcher-live-compact.test.ts`,
`webview/src/utils/session-cost.test.ts`) and the public BR-20 layout description.
They contain no private recorded prompts, paths, names or credentials. Keep task
acceptance requirements in the [incident cards](../../runtime-issues/) and
[BR registry](../../RUNTIME_RELIABILITY.md) authoritative. BR-20's authenticated
provider/browser comparison is an additional compatibility gate which synthetic
usage cannot replace; this harness supplies credential-free UI acceptance only.

## Startup plugin convergence: INC-0044

Plugin startup runs in the **client extension host**, not Claude. Before launching
the candidate, seed a fresh isolated **client** home (never your real profile):

```powershell
node "$repo/extensions/claude-bridge/server/test-support/plugin-fixture.mjs" "$capture/client-home" stale-cache
```

Modes: `stale-cache` has source/sub-clone v2 with installed cache v1 and unchanged
pull (simulates a previous interrupted cache sync); `changed` has source v2 and
sub-clone/cache v1; `disabled` sets autoUpdate false; `offline` points the sub-clone
at a missing local remote. All Git origins are local paths; no remote account or
network is used. Existing plugin registries are never overwritten. The fixture
uses Git identity only for its own commits and does not alter global config.

Launch KaminIDE with USERPROFILE/HOME equal to this client home and APPDATA/
LOCALAPPDATA under the capture. Open Active before the delayed startup sweep;
compare source manifest, sub-clone, installed_plugins.json, cache bytes and visible
version after completion without pointer movement. Compare opening Active afterward,
manual Update recovery, disabled updates, missing remote and a subsequent launch.
Do not confuse a correct version label with runtime MCP/tool publication (INC-0042).

## Checks and evidence

Run server typecheck/lint/test and changed-file Prettier checks from CONTRIBUTING.md.
For an automated real-server smoke after launching the server:

```powershell
node "$repo/extensions/claude-bridge/server/test-support/smoke.mjs" 3456 > "$capture/smoke.log" 2>&1
```

It creates only a synthetic local Bridge token, uses real hooks/PTY/watcher/MCP and
returns one bounded summary; it stands in for the host tool result, so it does not
claim client UI or INC-0057 acceptance. The new tests exercise real native PTY, subprocess transcripts, actual watcher replay,
loopback hook/MCP envelopes, result-cut proxy and npm release packaging. For local
checks whose imports write runtime logs, run Vitest with an external root/config and
absolute include paths; direct stdout/stderr into the same capture directory.

For a Windows merge gate, capture exact head/merge candidate SHA, client executable
and host/builtin provenance, server SHA, Node/OS version, commands/control times,
observed thresholds, and task-specific screenshots/state evidence under the external
capture. Test adjacent hover/click/focus/keyboard behavior and the visual result.
Publish only sanitized outcome plus immutable private evidence link through the
[maintainer flow](../../../../docs/MAINTAINER_PR_FLOW.md). Fill `## Runtime acceptance`
with the actual candidate/scenario/observed/evidence. Green harness tests or
Windows CI alone do not authorize `passed`; no task is closed by this tooling PR.
