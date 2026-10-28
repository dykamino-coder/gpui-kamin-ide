# Local shell commands as MCP Tasks

Bridge supports the `io.modelcontextprotocol/tasks` extension on MCP
`2026-07-28`, alongside its existing handshake-based MCP endpoint. The wire
contract follows the [released Tasks schema](https://github.com/modelcontextprotocol/ext-tasks/blob/main/schema/2026-07-28/schema.ts)
and the [core schema](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/schema/2026-07-28/schema.ts).

Each modern request carries protocol version and client capabilities in
`params._meta`, with matching HTTP version, method and resource/tool-name
headers. `server/discover` advertises the extension. A foreground `Bash` or
`PowerShell` call returns a flat `resultType: "task"` handle only when that
request opts into Tasks. Other tools and clients without that capability keep
their existing behavior. Explicit `run_in_background` shell calls retain their
`BashOutput` / `KillShell` contract.

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
| Execution deadline / cancellation grace   | 30 minutes / 1 minute                         |
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
background-shell management is unchanged.

## Real CLI acceptance

[Claude Code's MCP documentation](https://code.claude.com/docs/en/mcp)
describes Tasks as a public beta requiring CLI 2.1.304 or newer and an account
feature flag. The companion CLI update pins 2.1.312. Version alone does not
prove account eligibility. The session environment must permit feature-flag
fetching; Bridge removes inherited `DISABLE_NONESSENTIAL_TRAFFIC` and
`CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC`. Other existing session controls,
including the fullscreen renderer setting, remain applicable.

Acceptance uses the exact candidate in the authorized `kamin-acc-302` container
and `kamin-acc-302-auth` volume, with a separate server port and test conversation:

1. Confirm CLI version, valid test authentication, modern discovery and Tasks
   capability on the actual `tools/call`. Record a missing beta flag separately
   from server failures. Use interactive PTY; a stream-json driver additionally
   requires `CLAUDE_AUTO_BACKGROUND_TASKS=1` and open stdin.
2. Ask the agent to invoke a foreground local command that waits 45 seconds and
   prints a unique test marker. Confirm a task handle arrives before completion,
   another user turn completes while it works, and the agent receives the
   terminal marker without a `BashOutput` poll or duplicate command execution.
3. Repeat with a longer command; cancel through `/tasks` or `tasks/cancel`.
   Confirm the executor process tree stops and status becomes `cancelled`.
4. Disconnect/reconnect WS during execution and again after output but before
   acknowledgement. Confirm a single execution and eventual persisted output.
   Cold-resume the same conversation and retrieve its retained result.
5. Repeat without Tasks client capability and confirm ordinary tool calls still
   work. Check MCP tools, hooks, statusline and resume against the CLI update's
   acceptance scenario. Native GPUI/CEF acceptance belongs to the lead.

Unit and HTTP/WS integration tests exercise synthetic tools, persistence,
ownership, cancellation and reconnect boundaries; they do not establish the
account-gated real CLI acceptance above.
