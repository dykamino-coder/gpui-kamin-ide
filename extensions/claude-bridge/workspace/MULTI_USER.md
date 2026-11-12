# Several users on one workspace host: state boundaries (INC-2026-0094)

Research result for [INC-2026-0094](../runtime-issues/INC-2026-0094.md).
Source baseline: `origin/main` `81ed673f` (2026-11-13). No behavior change is
part of this document; it fixes the per-item decision, the policy API and the
host tests a later Change PR must add.

## 1. What the host has today

- One process, one runtime: `createSharedBackend` (`src/runtime.ts`) refuses a
  second instance; one `BridgeHost` owns one `TabManager`, one config store and
  one Bridge token (`src/credentials.ts`).
- The sign-in policy returns only an opaque cookie
  (`WorkspaceAuthPolicy.login/validate`, `src/auth.ts`). The built-in
  `WorkspaceAuth` resolves one identity for the workspace token and stores it
  per browser session, but never exposes it. `src/server.ts` keeps the cookie per
  socket only for revalidation and calls `backend.attach(view)` without a
  principal (`src/protocol.ts`).
- Host events reach every attached view: `BridgeHost.broadcast`,
  `TerminalWorkspaceBackend.broadcast` (`src/terminals.ts`), shell layout
  (`src/shell-state.ts`), drafts (`workspace:draft:state` in `src/runtime.ts`)
  and preview state (`src/server.ts` preview feed).
- The active tab is one host value: `TabManager.switchTab` broadcasts
  `tab:switched`, and `runtime.ts` persists it. Switching in one page switches
  every page.
- Effects with an originating request (clipboard, upload, folder picker,
  download, open external) are already per view through `AsyncLocalStorage`
  (`src/browser-environment.ts`). Background notifications without a request go
  to the first registered viewer.
- File views (`src/file-views.ts`): a new document without a remembered id
  adopts the most recent *detached* view, including its panel state and watches.
- `disconnect`/`connect`/`set-config` are ordinary invokes from any page; a
  disconnect terminates owned shell/MCP work for all pages
  (`presentation.forgetSessions`).

## 2. Two product rules

Studio did not state its rule, so each item is decided for both:

- **Team workspace (T):** the container is a shared team machine; everyone
  sees the same sessions, and actions are attributed to the user.
- **Personal sessions (U):** the container and its files are shared, but each
  user sees and drives only their own sessions.

Neither rule can give users separate Bridge identities or separate files inside
one process: the CLI sessions run under the workspace token and tools act on the
same `/workspace`. If agent work must run under each user's own Bridge identity
or files must be private, the answer is one workspace container per user, not a
boundary inside the host.

## 3. Decision per state item

| State | Owner today | T | U | Note |
| --- | --- | --- | --- | --- |
| Bridge token / server identity | process | shared | shared | Per-user identity = container per user |
| Project files, MCP, LSP, hooks, skills, plugins, settings | process | shared | shared | One CLI configuration per workspace |
| Open session set (`workspace-sessions.json`) | process | shared | per user | U: each record gets `owner` principal |
| Active tab | process | **per view** | **per view** | Defect for both rules (and for one user with two pages); host keeps last active per principal only for restore |
| Transcript, streaming, subagents | session | follows session | follows session | U: snapshot/broadcast filtered by session owner |
| Submissions, queue, receipts | session + process | shared, items carry principal | owner only | Receipts stay process-level for duplicate protection; add principal for attribution |
| Permission / question prompts | session | any user settles once, attributed | owner only | `presentation` retains open questions for reattach; settle-once across two pages is still an open INC-2026-0087 acceptance item |
| Composer drafts | session, shared CAS | **per user** | per user | Key `(principal, tabId)`; today two users overwrite one draft |
| Shell layout (`workspace-shell.json`) | process | **per user** | per user | Dock/sidebar are personal preferences |
| File views, panel state, watches | detached-view pool | **per user** | per user | Adopt only a detached view of the same principal |
| Host terminals (PTYs) | tab | follows session | follows session | Input from any viewer of the session |
| Preview descriptor | process | shared | shared | One agent-controlled panel per process |
| Request effects (clipboard, upload, downloads) | view | per view | per view | Already correct |
| Background notifications | first viewer | all viewers of the session | session owner's views | Today an arbitrary page, possibly another user's |
| `connect` / `disconnect` / `set-config` | any page | workspace admin only | workspace admin only | One user's disconnect ends everyone's work |
| Browser sign-in sessions | policy | per user | per user | Already per cookie |

Items in bold are needed under both rules and are the first bounded change.

## 4. Policy API

Additive and backward compatible:

```ts
export interface WorkspacePrincipal {
  /** Stable opaque id chosen by the embedding policy; not a display name. */
  id: string
  /** May run workspace-level actions (connect, disconnect, set-config). */
  admin?: boolean
}

export interface WorkspaceAuthPolicy {
  // existing login / validate / logout / cookieHeader / loginLocal
  /** Principal of a valid cookie. Absent method = single owner principal. */
  principal?(cookie: string): Promise<WorkspacePrincipal | null>
}

export interface WorkspaceBackend {
  attach(view: ClientView, context?: { principal: WorkspacePrincipal }): HostAttachment
}
```

- `server.ts` resolves the principal once at upgrade, next to `validate`, and
  passes it to `attach`; revalidation closes the socket if the principal
  changes or disappears.
- Without `principal()` every socket gets `{ id: 'owner', admin: true }`, so
  current deployments keep their behavior.
- The built-in `WorkspaceAuth` returns the identity it already stores
  (`Session.identity`) with `admin: true`.
- A host option `sessions: 'team' | 'user'` (default `team`) selects T or U for
  the rule-dependent rows.
- The principal is never forwarded to the Bridge server or the CLI; it is host
  routing state only.

## 5. Bounded change plan

1. **Principal plumbing** (no visible change with one principal): API above,
   server resolution and revalidation, `attach` context through
   `withWorkspaceShell`, `WorkspaceFiles`, `TerminalWorkspaceBackend` and the
   shared runtime.
2. **Per-user and per-view items needed under both rules:** per-view active
   tab, drafts keyed by principal, per-user shell layout, same-principal file
   view adoption, notification routing, admin-gated workspace actions.
3. **Rule U:** session ownership in the open set, filtered snapshots and
   broadcasts, rejection of invokes/sends for a foreign session id.
4. **Attribution (rule T):** principal on queue items and settled prompts in
   host state and presentation events.

## 6. Host tests that prove the boundaries

All run with two policy users on one process through the real server and
shared runtime, like the existing `workspace/test` suites:

- the policy principal reaches `attach`; a policy without `principal()` keeps
  today's single-owner behavior (regression for current deployments);
- revalidation closes a socket whose principal changed;
- switching tabs in one page does not change another page's active tab;
- drafts of user A and user B for the same session are independent, survive
  reattach and host restart, and CAS conflicts stay per user;
- shell layout updates reach only the same user's pages;
- a new page of user B never adopts user A's detached file view or watches;
- a background notification reaches only the intended viewers;
- `disconnect`, `connect` and `set-config` from a non-admin are rejected
  without side effects;
- rule U: user B's tab list and replay exclude user A's sessions, and an invoke
  or `pty-submit-text` naming A's session is rejected;
- rule T: both users see one session, each queue item and settled prompt names
  its principal, and a prompt settles exactly once.

## 7. Open product input

Studio chooses rule T or U (or both via the option) and whether workspace
admin is a separate role. Neither choice blocks steps 1 and 2.
