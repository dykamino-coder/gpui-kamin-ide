# MCP Task retention and overload contract

Decision for the bounded [INC-0080](../runtime-issues/INC-2026-0080.md)
follow-up, source baseline `f386ec9a952a87fc08b2abc0b9f59ae5ca34b33e`.
The owner authorized server implementation, automated acceptance and one release
on 2026-11-03. Deployment observation belongs to the owner.

## Contract

- An issued Task handle keeps its terminal result/error until `createdAt + ttlMs`.
  Reading a result or sending an HTTP response is not a consumption acknowledgement.
  The pinned [MCP Tasks extension](https://modelcontextprotocol.github.io/ext-tasks/specification/2026-07-28/tasks.html)
  provides no result-consumption ACK. Do not delete an unexpired outcome under pressure.
- Admission reserves the maximum bounded terminal record before dispatch. Working
  records, terminal results and expiry use the same serialized store operation.
  Execution capacity and retained-result capacity are separate limits.
- Results remain in atomic on-disk records. The in-memory index keeps terminal
  metadata, not complete result bodies; `tasks/get` reads the body on demand.
- If result retention is full, a foreground call may use the existing ordinary
  SSE request/result path without creating a Task handle. Its lease still counts
  against the shared working limits. It waits for the ordinary execution timeout,
  rather than promising background completion after the two-minute grace period.
  This path has the existing ordinary-call restart/disconnect semantics, not the
  durable Task guarantee. A request without the Tasks capability is unchanged.
- An explicit background request cannot silently become an ordinary call. Reject
  it before execution when no retention reservation fits. Distinguish active,
  retained-count, retained-byte, input and persistence failures in bounded numeric/
  enum diagnostics; do not include command arguments, output or filesystem paths.
- A persistence error is not a safe reason to execute through another route.
  Dispatch uncertainty is never resolved by automatically repeating a command.
- UI clear only hides history. Cancellation is cooperative; the existing owner,
  ACK and late-result rules remain. These changes do not claim durable client
  outbox storage across an extension-host crash or fix historical native hangs.

## Bounds and upgrades

Keep the existing 400 global / 8 per-endpoint working limits and 2048 global /
256 per-endpoint retained-record limits. Retention is finite: explicit background
admission can be refused until expiry, while foreground calls can continue via
bounded ordinary execution. A larger numeric task count is not the fix.

Use a 512 MiB on-disk accounting budget, including reservations. This covers the
worst-case reservations for 400 working results plus the previous 128 MiB store
on upgrade. Each result remains bounded at 512 KiB, input at 64 KiB, plus bounded
record metadata. Do not preload the larger result budget into RAM. Preserve the
existing record format and identity key; no operator deletion or migration of
task files is required. Expiry retains its existing creation-time origin.

A downgrade to the previous 128 MiB implementation can refuse a store which has
grown beyond that old budget. Wait for expiry or restore a coordinated backup
using the deployment runbook; never delete individual task records to force it.

## Acceptance

Automated merge gate; no native/CEF/input or installer behavior changes. Tests
must exercise the actual store and HTTP handler, distinguishing mocked execution
from real local WebSocket/process checks:

- unread completed, failed and cancelled results at owner/global count and byte
  pressure, before/at TTL, and after store restart;
- maximum result reservations, atomic write failures and queue saturation;
- foreground fallback only for retention pressure; explicit background refusal
  before dispatch, active-limit enforcement including ordinary leases;
- loss/duplication of an execution-result ACK, PTY replacement and owner isolation;
- no automatic command re-execution, and UI history clear preserving full output;
- existing modern/legacy and explicit-background/foreground regressions.

Run server typecheck/lint/tests and isolated Linux Docker checks. Required CI and
the coordinated release's Windows installer build remain required; a live Windows
smoke is not claimed. After normal deployment, the owner checks existing sessions,
short shell calls, a background task and its eventual output. A new field hang
still follows the request-correlated capture plan in INC-0080.
