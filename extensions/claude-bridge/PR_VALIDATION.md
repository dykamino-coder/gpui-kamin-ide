# Claude Bridge plugin harness validation

This document covers the plugin-harness changes in this branch. It intentionally
does not define PTY submission or live skills-reload behavior.

## Scope

- install, update, uninstall, and dependency handling for personal plugins;
- namespaced plugin snapshots without cross-plugin component collisions;
- hook relay with token and session isolation;
- MCP tools, resources, resource templates, prompts, and pagination;
- plugin monitors and LSP lifecycle;
- sensitive option storage and redaction;
- the KaminIDE compatibility-host boundary;
- sync authentication, payload limits, and path validation.

## One-command automated validation

From the repository root:

```bash
extensions/claude-bridge/verify-pr.sh --install
```

Use `--install` on a fresh checkout; subsequent runs can omit it. The script
runs repository-level typecheck, ESLint, and Vitest plus package-specific
typechecks and complete suites for server, extension, and webview. It rebuilds
all shipped bridge artifacts, rejects accidental Electron runtime imports, and
verifies that generated output matches the committed sources.

Focused regression coverage includes:

- plugin dependency resolution and safe install/update rollback;
- plugin namespace collisions and complete snapshot replacement;
- sensitive option persistence without plaintext leakage;
- hook ownership and cross-token/session rejection;
- MCP resource-template expansion and paginated discovery;
- monitor/LSP process limits and cleanup;
- sync authentication, body limits, traversal rejection, and symlink escape;
- compatibility-host IPC behavior used by the extension runtime.

## Manual acceptance matrix

### 1. Install and update a personal plugin

1. Add a personal marketplace and install a plugin with dependencies.
2. Confirm its skills, commands, hooks, MCP servers, monitors, and LSP entries
   appear under the plugin's own namespace.
3. Publish or select a newer plugin revision and update it.

Expected: dependency failures are reported without leaving a partial install;
successful updates replace the prior snapshot and preserve configured options.

### 2. Namespace isolation

1. Install two plugins that contribute the same skill or command name.
2. Start a session with both enabled.

Expected: both plugin roots are passed independently to the CLI, and neither
plugin overwrites the other's files.

### 3. Hook relay isolation

1. Enable hooks from two users or tokens.
2. Trigger matching hook events in separate sessions.

Expected: each event reaches only its owning host/session. A token cannot read,
approve, update, or dispatch another token's hooks.

### 4. MCP capabilities

1. Connect a plugin MCP server that exposes tools, resources, templates, and
   prompts over more than one discovery page.
2. Exercise one item of each capability.

Expected: every page is discovered once, calls are routed to the owning plugin,
and reconnect/disconnect cleans up pending work.

### 5. Sensitive options

1. Configure a plugin option marked sensitive.
2. Reopen the configuration UI and inspect logs and synced metadata.

Expected: the value remains usable by the plugin but is never returned as
plaintext or written to diagnostic output.

## Review cautions

Do not flatten plugin components into shared user directories: identical names
from unrelated plugins must coexist. Do not remove the user-snapshot lock
without replacing the plugin directory swap with an atomic mechanism; a session
must not copy a plugin tree between its removal and rewrite phases.
