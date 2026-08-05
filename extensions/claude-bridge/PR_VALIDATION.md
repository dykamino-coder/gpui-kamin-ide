# Claude Bridge PR validation

This document covers the plugin-harness branch as a whole, with extra focus on
the PTY input and live skills-reload regressions fixed in this PR.

## What changed

The server now owns every semantic text submission as one serialized
transaction: `Ctrl+U`, Ink settle, bracketed paste, paste-echo quiet window, and
exactly one `Enter`. Raw terminal bytes arriving during a transaction are
buffered. `Ctrl+C` bypasses the queue, cancels delayed `Enter`, and drops queued
submissions so Stop cannot be followed by a surprise send.

`/reload-skills` is deferred maintenance, not an eager submit. It runs only
while the session is running, attached, prompt-ready, free of another submit,
and has no unfinished raw-console line. A sync while the app is closed keeps
the reload pending across the existing detach grace period. Reattach does not
clear or submit the user's line.

The effective session-local skills tree now has one implementation for initial
session creation and live sync:

- user skills are the base;
- project skills override the same relative paths;
- files missing from the new complete snapshot are removed;
- user and project uploads for one token are serialized;
- unchanged skills maps are not rewritten and do not request another reload.

The Cloud Bridge textarea draft remains storage-only until explicit Send. The
webview sends one semantic `submitText` frame; the server owns clear/paste/Enter.
UI-originated `/compact` and `/rename` actions use the same path.

## Preserved behavior

- Normal app/window close still detaches the live CLI for its grace period. It
  does not send `session:end`, kill the PTY, or create a second JSONL writer on
  quick reconnect.
- Explicit tab close/Disconnect still sends `session:end`.
- Paste echo quiet-window and the 2-second hard cap remain in place.
- Raw bridge-console typing remains raw and interactive.
- Per-tab Cloud Bridge textarea drafts still survive tab switch and iframe
  reload without touching PTY stdin.
- Project skills still override user skills; live refresh and removal now match
  a newly-created session.

## One-command automated validation

From the repository root:

```bash
extensions/claude-bridge/verify-pr.sh --install
```

Use `--install` on a fresh checkout; subsequent runs can omit it. The script
runs repository-level typecheck, ESLint, and Vitest plus package-specific
typechecks/suites for server, extension, and webview. It rebuilds all shipped
bridge artifacts and verifies that they match the committed sources.

Focused regression coverage includes:

- rapid submits remain `paste1, Enter, paste2, Enter`;
- raw input is buffered around a programmatic submit;
- interrupt cancels delayed Enter and queued sends;
- raw `abc` plus detach/sync/reattach never writes `/reload-skills`;
- the legacy two-frame `Ctrl+U` client path cannot be stolen by maintenance;
- deleted skills disappear from the live snapshot;
- project/user collisions resolve to the project version, including
  file-versus-directory collisions;
- user/project uploads for one token cannot expose a half-written overlay;
- explicit webview Send emits one semantic frame;
- restoring a per-tab textarea draft has no PTY/bridge dependency.

## Manual acceptance matrix

### 1. Raw bridge-console draft survives skills sync

1. Start a session and wait for the normal prompt.
2. In the bridge console type `KEEP-ME` without pressing Enter.
3. Add, edit, disable, or delete a skill so the client uploads a changed skills
   snapshot.
4. Wait at least two seconds.

Expected: `KEEP-ME` remains the only unfinished input. `/reload-skills` is not
appended and the line is not cleared. Press Enter (or deliberately clear the
line with `Ctrl+U`); after the next prompt, one deferred `/reload-skills` may
run.

### 2. Detach and reattach preserve the same draft

1. Type `KEEP-AFTER-REATTACH` in the raw console without Enter.
2. Close KaminIDE/the bridge UI without explicitly closing the tab.
3. Change a skill during the server's detach grace period.
4. Reopen KaminIDE and switch to the same session.

Expected: the existing PTY is reattached, the text remains, and no reload is
concatenated with it. Do not “fix” this by destroying the session on app close;
that regresses JSONL safety.

### 3. Cloud Bridge textarea draft remains local

1. Type `CHAT-DRAFT` in the composer without Send.
2. Switch to another tab/session and back; optionally reload the webview.
3. Trigger a skills sync while away.

Expected: the textarea restores `CHAT-DRAFT`; neither it nor
`/reload-skillsCHAT-DRAFT` appears in PTY input. Only Send moves it to the
session.

### 4. Submission ordering and Stop

1. Send two short messages rapidly.
2. Repeat with a long message and press Stop before paste echo settles.

Expected: messages remain distinct turns. Stop does not produce a delayed Enter
or unexpectedly deliver a second queued message.

### 5. Effective skills overlay

1. Create the same relative skill path at user and project scope with different
   descriptions.
2. Sync and verify the project version is active.
3. Delete the project copy and sync again.
4. Delete the user copy and sync again.

Expected: project wins first, then user becomes visible, then the skill is gone.
A new session at each step must match the already-running session.

## Review cautions

An unconditional `Ctrl+U` before automatic reload only hides concatenation by
deleting the user's draft. Ending sessions on normal app close regresses the
detach/reattach protection against partial JSONL writes and double writers. A
longer debounce alone does not remove either race.
