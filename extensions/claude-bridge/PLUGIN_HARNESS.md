# Plugin harness support

The bridge splits one Claude Code session across two machines:

- the bridge server owns the Claude CLI PTY and a session-local proxy plugin;
- the KaminIDE client host owns the installed plugin cache and executes local
  processes against the user's project files.

Enabled installed plugins are the single source of truth. Starting a session
syncs non-executable plugin components into a private `.bridge-plugins`
directory and passes every proxy with `--plugin-dir`. Executable components
remain on the client host and use an authenticated relay where needed.

## Component matrix

| Component | Discovery | Execution/loading boundary |
| --- | --- | --- |
| Skills | Default `skills/`, root `SKILL.md`, custom paths | Materialized in the session proxy plugin |
| Commands | Default `commands/` or replacement custom paths | Materialized in the session proxy plugin |
| Agents | Default `agents/` or replacement custom paths | Materialized in the session proxy plugin |
| Output styles | Default `output-styles/` or replacement custom paths | Materialized in the session proxy plugin |
| Themes | Default `themes/` or `experimental.themes` paths | Materialized in the session proxy plugin |
| Workflows | Default `workflows/` or replacement custom paths | Materialized in the session proxy plugin |
| Settings | Root `settings.json` | Materialized in the session proxy plugin |
| Hooks | Canonical `hooks/hooks.json`, compatibility paths, manifest/marketplace declarations | Approved commands run on the client host through the authenticated hook relay |
| MCP servers | Root `.mcp.json` plus manifest/marketplace declarations | Client-host manager; tools/resources/prompts are proxied and plugin-namespaced |
| LSP servers | Root `.lsp.json` plus manifest/marketplace declarations | Client-host stdio JSON-RPC process, one lazy instance per project |
| Monitors | Canonical `monitors/monitors.json`, `experimental.monitors`, compatibility declaration | Client-host process scoped to the tab/project |
| `bin/` | Enabled plugin `bin/` directories | Prepended to client-host shell/hook/monitor/LSP `PATH` |
| `userConfig` | Marketplace plus local manifest schema | Non-sensitive values in settings, secrets in `.credentials.json` (`0600`) |
| Dependencies | Marketplace plus local manifest declarations | Missing configured-marketplace dependencies install recursively; cycles and semver are validated |

The effective declaration combines component fields from the marketplace entry
and `.claude-plugin/plugin.json`. The marketplace identity is the canonical
plugin namespace, and its `defaultEnabled` value wins as required by the plugin
contract.

## Hook safety and lifecycle

Plugin hooks are inert until the user reviews their hashes. Approval is
invalidated when a hook changes, while an explicit Reject All decision is
remembered for the reviewed version. Hook requests carry the real session cwd,
plugin id, and arguments. The client expands `CLAUDE_PLUGIN_ROOT`,
`CLAUDE_PLUGIN_DATA`, and `CLAUDE_PROJECT_DIR`; exec-form arguments may use
`user_config`, while shell-form hook commands and monitors reject it to avoid
injecting option text into a shell program.

Plugin enable/disable, install, uninstall, update, options, MCP, LSP, and monitor
state are reloaded in the client runtime. A new or restarted Claude session is
still required for the CLI-side proxy snapshot and hook registry to change.

## Deliberate boundaries

- LSP `stdio` transport is implemented. `socket` declarations are reported as
  unsupported because the plugin declaration has no standard endpoint/handshake
  for this split-host bridge to connect to.
- `.mcp.json` and inline MCP declarations are implemented. Packaged `.mcpb` and
  `.dxt` bundles require a bundle installer/adapter and are reported rather than
  executed as JSON.
- Channels are not exposed by this bridge. They are a research-preview,
  session-scoped stdio MCP notification surface and require a dedicated direct
  CLI transport rather than the current external-tool proxy.
