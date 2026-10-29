# Local shell commands as MCP Tasks

Bridge supports the `io.modelcontextprotocol/tasks` extension on MCP
`2026-07-28`, alongside its existing handshake-based MCP endpoint. The wire
contract follows the [released Tasks schema](https://github.com/modelcontextprotocol/ext-tasks/blob/main/schema/2026-07-28/schema.ts)
and the [core schema](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/schema/2026-07-28/schema.ts).

Each modern request carries protocol version and client capabilities in
`params._meta`, with matching HTTP version, method and resource/tool-name
headers. `server/discover` advertises the extension. For clients that opt into Tasks, `Bash` and `PowerShell` with
`run_in_background: true` return a flat `resultType: "task"` handle immediately.
The client executes the command to completion and returns the same bounded full
output as foreground execution. The CLI registers an `mcp_task`, frees the chat,
and delivers its own completion notification. Foreground commands retain their
120-second inline grace, becoming a task only when they outlive that window.
Other tools and clients without Tasks keep their existing behavior, including
the legacy `BashOutput` / `KillShell` background-shell contract.

`tasks/get` returns current status and the terminal tool result or JSON-RPC
error. `tasks/cancel` acknowledges a cooperative cancellation request; a
delivered command becomes terminal when its executor responds or its deadline
expires. Terminal states cannot be overwritten by late replies. Shell tasks
do not issue MCP input requests; `tasks/update` accepts input-response objects
and ignores unrecognized response keys. Local permission prompts remain owned
by the existing client permission pipeline.

`subscriptions/listen` acknowledges accepted task IDs, sends their current
states and subsequent `notifications/tasks` updates, including terminal output.
Resubscribing fetches persisted state. A controlled stream shutdown sends its
final JSON-RPC response; disconnected clients can poll instead. The CLI's
documented Tasks implementation uses polling and creates its own agent-facing
completion notification.

## Ownership and delivery

The MCP URL and authentication header derive from the authenticated Bridge
token identity and conversation settings directory. They remain stable when
that conversation receives a replacement PTY. Hook credentials remain scoped
to the individual PTY. Different owners cannot fetch, subscribe to, cancel or
settle one another's tasks.

The server stores task records under `~/.claude/bridge-mcp/tasks`, using atomic
replacement and file synchronization (plus directory synchronization on POSIX).
Admission must persist before returning a handle. Terminal output must persist
before the server acknowledges the client result. Disk failures leave the
canonical client result outbox responsible for retry; lost acknowledgements
are idempotent. A reconnect to the same conversation can rebind an outstanding
outcome to its new PTY. Completed results survive server restart until TTL.

The server persists its dispatch boundary before sending a command, preventing
automatic replay of side effects. A crash between that boundary and actual
delivery can leave a task unexecuted until its deadline; it is never silently
rerun. The client outbox is bounded memory, not a disk journal: restarting the
extension host before the server has persisted its result can lose that
unacknowledged result. A WS reconnect with the host still running retains it.

## Limits

| Resource                                  | Limit                                         |
| ----------------------------------------- | --------------------------------------------- |
| Working tasks                             | 16 total, 4 per owner                         |
| Retained records                          | 256 total, 64 per owner                       |
| Serialized task storage                   | 64 MiB total                                  |
| Input / terminal result                   | 64 KiB / 512 KiB per task                     |
| Persistence operations waiting or running | 32                                            |
| Task retention from creation              | 24 hours                                      |
| Foreground deadline / cancellation grace  | 30 minutes / 1 minute                         |
| Explicit background execution deadline    | 24 hours minus 1-minute cancellation grace    |
| Suggested polling interval                | 2 seconds                                     |
| Notification subscription                 | One stream per live PTY, 64 task IDs          |
| Notification queue                        | 4 frames plus one controlled closing response |
| Stalled notification stream               | Closed after three 25-second full-queue ticks |
| Client result outbox                      | 64 outcomes, 16 MiB total, 4 MiB per frame    |
| Unacknowledged task outcome lifetime      | 24 hours                                      |

Notifications coalesce to the latest state per subscribed task. Task records
remain the source of truth after stream loss. An oversized terminal result is
replaced with a bounded tool error. Capacity rejection returns an admission
error rather than an untracked handle. Working tasks prevent idle reaping but
do not bypass session startup or absolute lifetime limits.

Cancellation reaches the existing tool pipeline through `AbortSignal`. On
Windows the foreground shell's own process tree is terminated; on POSIX its
own process group receives TERM followed by KILL after one second. Explicit
background-shell management remains available to clients without Tasks. The
server passes a background task deadline in `mcp:call.taskDeadlineMs`, outside
model-controlled tool input; it overrides the client foreground timeout cap.

The session header Commands button and the server dashboard show all running
MCP calls, durable tasks and legacy shells, with background work first. The
header badge counts only background work. Server snapshots use the existing WS;
legacy shell snapshots do not consume BashOutput cursors. Stop routes to the
existing owner-scoped cancellation path, and output views retain a bounded
16-KiB live tail (terminal task output remains available through tasks/get).

## Real CLI acceptance

[Claude Code's MCP documentation](https://code.claude.com/docs/en/mcp)
describes Tasks as a public beta requiring CLI 2.1.304 or newer and an account
feature flag. The companion CLI update pins 2.1.312. Version alone does not
prove account eligibility. The session environment must permit feature-flag
fetching; Bridge removes inherited `DISABLE_NONESSENTIAL_TRAFFIC` and
`CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC`. Other existing session controls,
including the fullscreen renderer setting, remain applicable.

Acceptance uses a separately named temporary Podman container based on the
pinned Bridge image plus candidate source. Use a fresh synthetic CLI profile,
a loopback-only model fixture and synthetic credentials; never mount the
owner's authentication volume or touch their containers or IDE profiles.
An isolated cached beta flag can exercise the actual CLI Tasks implementation,
but does not establish any real account's beta eligibility.

1. Confirm CLI 2.1.312, modern discovery and Tasks capability on the actual
   `tools/call`. Record missing capability separately from server failures.
   A stream-json driver requires `CLAUDE_AUTO_BACKGROUND_TASKS=1` and open stdin.
2. Return a model tool call with `run_in_background: true` and delay completion.
   Compare baseline and candidate using the same fixture: the candidate must
   return the task immediately, register `mcp_task`, finish the agent turn and
   deliver a later `task-notification` without a BashOutput poll.
3. Cancel another task through `tasks/cancel`; confirm executor termination
   and terminal status. Check WS replacement and late acknowledgement against
   the integration tests, with a single execution and retained terminal result.
4. Repeat without Tasks capability and verify legacy BashOutput/KillShell.
   Foreground calls retain their 120-second inline grace and 30-minute deadline.
5. On a separate native GPUI/CEF profile, inspect the Commands button left of
   diagnostics, background-only badge, all-command ordering, output and Stop.
   Capture only that test window through its own kamin-probe port.

Unit and HTTP/WS integration tests exercise persistence, ownership,
cancellation and reconnect boundaries. Synthetic real-CLI acceptance exercises
the pinned binary with controlled model output; production account eligibility
remains an independent prerequisite.
