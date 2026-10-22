# Hook decisions and Chat permission acceptance

This is the bounded test plan for [BR-11](../extensions/claude-bridge/RUNTIME_RELIABILITY.md#br-11--inventory-native-cli-only-blocking-states)
and [BR-07](../extensions/claude-bridge/RUNTIME_RELIABILITY.md#br-07--surface-native-claude-attention-in-chat).
[INC-0077](../extensions/claude-bridge/runtime-issues/INC-2026-0077.md) separately
owns misleading MCP denial reasons. This plan registers work, not a passed gate
or an implementation. It does not select or expand an execution batch.

## Start here

The maintainer can begin with current source and an artificial hook. The owner's
original hook, whole profile and production-server access are not prerequisites.
Read the existing BR-11 private INC-0035 packet before requesting old evidence.
Keep [INC-0037](../extensions/claude-bridge/runtime-issues/INC-2026-0037.md)'s
`SessionStart:compact` exception/provenance investigation separate.

Record the source SHA, client/server versions, CLI pin from the Dockerfile,
permission mode, effective matcher and execution host. The audit source
`f819b57b` pins CLI `2.1.296`; recheck the pin when implementing. Current upstream
[hook documentation](https://code.claude.com/docs/en/hooks#pretooluse-decision-control)
is a reference, not evidence that the pinned binary behaves identically.

## Three different consent boundaries

| Boundary | Owner | What its acceptance establishes |
| --- | --- | --- |
| Approving a plugin's hook declaration/hash | BR-10 | The user knowingly enables the declared handler; it is not consent for each invocation. |
| Native CLI decision for an individual tool call after a hook | BR-11, then BR-07 | Classification, supported response protocol and visibility of the current request. |
| Bridge permission for a relayed MCP tool | INC-0077 for reason provenance | Policy, explicit user response and cancellation are not reported as the same actor. Existing `PermissionWidget` alone does not implement the native CLI boundary. |

An intentional hook `deny` or blocking exit is an automatic policy decision;
it does not imply a missed user dialog and must not become an overridable Ask.
An `ask` needs a supported pending decision path. Exceptions, cancellation and
timeouts must not be labeled as a user's click. Preserve fail-closed behavior;
do not solve missing visibility by auto-allowing or repeating the tool.

## Disposable fixture and evidence

Use a separate test profile/project/plugin and a harmless operation with a
unique marker. Use the actual advertised tool name: a native `Bash` matcher
need not match a namespaced MCP tool. Keep local and server execution explicit.
Do not install or run the owner's unknown cleanup/guard against real data.

Provide synchronous `PreToolUse` variants with allow, ask and deny JSON, plus a
separate exit-2/stderr variant and a control without a matching hook. Vary one
decision at a time; do not combine contradictory stdout and exit codes as the
only denial test. Verify a harmless execution marker, not just model prose.
The hook Test runner in Settings proves dispatch/output only: it does not prove
that CLI consumed the result or that a Chat answer reaches the pending CLI call.

Keep a privacy-reviewed paired timeline: invocation and request/session identity,
handler start/outcome, decision origin, UI visibility, explicit response and tool
execution marker. Payloads, prompts, auth and real file contents stay out of
public traces. Raw field captures and full analysis use private intake.

## Required matrix

| Case | Required observation / invariant |
| --- | --- |
| No matching hook / explicit allow | Harmless control runs once under the recorded permission mode. Approval of the declaration is not substituted for a tool decision. |
| JSON deny | Tool does not run; the distinct hook reason reaches CLI. No claim that the user clicked Deny and no override button for a mandatory policy prohibition. |
| Blocking exit 2 + stderr | Preserve the code/reason separately from JSON deny and an ordinary handler exception; verify the pinned CLI's event-specific behavior. |
| JSON ask, no user action | Record whether CLI blocks and what protocol is available. No tool execution or fabricated user response; bounded timeout/cancel has its own reason. Test a mode that permits interaction and document bypass-mode behavior separately. |
| Explicit Allow / Deny | For a supported Chat response channel: answer the original request once, execute once / never, reconcile acknowledged outcome and show the reason to the user and CLI. |
| Console hidden / Chat foreground | The pending state is visible in its own parent tab. If only Console handoff is supported, show BR-07's neutral banner and focus/open action; do not claim a Chat decision was implemented. |
| No user click: rule deny / disconnect / timeout / failed delivery | Preserve distinct origins and outcomes; do not report `user denied`. Fail closed without silently retrying an operation or hiding an indeterminate result. |
| Tab switch, two parents and concurrent calls | Display and answer only the owning session/request. A second prompt must not inherit the first response. |
| Reconnect, renderer replacement, duplicate/late reply | Define pending/answered/cancelled/disconnected ownership. A stale button/reply must not settle a newer request or cause another execution; no indefinitely orphaned pending widget. |
| Mouse and keyboard | For a new interactive widget: Allow/Deny, focus, Tab/Shift+Tab, dismissal and return to composer work in native Windows CEF. Reuse BR-10 only for its declaration modal. |

## Maintainer deliverables and gates

1. **BR-11 research/decision:** actual pinned-CLI reproduction, versioned blocker
   inventory and an event/request/response ownership trace. Determine whether
   `PermissionRequest` or another supported structured channel can answer this
   native request. Record the current Console-only result if none is available.
   ANSI/TUI text scraping, guessed Enter/number keys and sending the composer to
   a raw prompt are not an approval protocol. No production access is needed.
2. **BR-07 change after the decision:** implement the justified Chat visibility
   or supported interaction. Include protocol/lifecycle tests and the applicable
   exact-candidate Windows gate with Console hidden, tab switches and reconnect.
   A banner is not acceptance of an interactive Allow/Deny widget.
3. **INC-0077 independent change:** preserve decision provenance across the real
   MCP pipeline and teardown; test rule deny, explicit user deny, pending cancel,
   lost delivery and duplicate response. Target-behavior regressions must fail
   before the fix. The private audit probes only observe current behavior; they
   are not fix-acceptance tests. Pure reason/state changes can use an automated
   source gate; new CEF interaction needs its native gate. Do not wait for BR-11
   or declare it closed merely because MCP provenance is repaired.

Report each case as passed, failed or not run, with the exact candidate and
owner of any remaining gate. CI compilation and successful hook relay tests do
not prove the native CLI-to-Chat round trip. A new blocker does not stop the
independent source/protocol work above.

## When owner data is useful

Only after existing evidence and the synthetic matrix, request a specific
missing fragment needed to attribute the owner's episode: redacted effective
event/matcher/declaration, local/server ownership, script identity and decision
output/exit code, permission mode, versions and time interval. Do not request
whole settings/environment, credentials or already recovered compact stderr.
If the original invocation is unavailable, retain that historical limitation
and finish the independently reproducible contract without inventing a cause.
