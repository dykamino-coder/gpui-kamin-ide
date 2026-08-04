// Status-bar (bottom row) MCP indicators: a GREEN "connected count" item and,
// when any server has errored, a RED "error count" item. Clicking either opens
// the MCP Servers settings page. Contributed via the VS Code API, so they render
// in KaminIDE's own status bar next to "N active".
import * as vscode from "vscode"
import type { McpServerManager } from "./main/mcp/manager"

// The manager's change signal (notifyChanged) isn't publicly hookable and its
// onToolsChanged slot is taken, so poll its in-memory server list. MCP state
// changes are infrequent and getServers() is a cheap map read.
const MCP_STATUS_POLL_MS = 2500
const OPEN_MCP_COMMAND = "claude-bridge.openMcpSettings"
const MCP_SETTINGS_VIEW = "claudeBridgeCzMcp"
// Left-aligned; sits just after KaminIDE's own "N active / N cmds". Higher
// priority renders further left, so connected shows before the error item.
const MCP_STATUS_PRIORITY = 50
const COLOR_CONNECTED = "#3fb950" // green (GitHub-dark success)
const COLOR_ERROR = "#f85149"     // red   (GitHub-dark danger)

export function installMcpStatusBar(mgr: Pick<McpServerManager, "getServers">): vscode.Disposable {
  const cmd = vscode.commands.registerCommand(OPEN_MCP_COMMAND, () => {
    void vscode.commands.executeCommand(`${MCP_SETTINGS_VIEW}.focus`)
  })
  const conn = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, MCP_STATUS_PRIORITY)
  conn.command = OPEN_MCP_COMMAND
  conn.color = COLOR_CONNECTED
  const err = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, MCP_STATUS_PRIORITY - 1)
  err.command = OPEN_MCP_COMMAND
  err.color = COLOR_ERROR

  let last = "" // dedupe: only touch the items when the counts change
  const refresh = (): void => {
    // Only servers the user has enabled — a disabled one isn't "off with error".
    const enabled = mgr.getServers().filter((s) => s.enabled !== false)
    const connected = enabled.filter((s) => s.status === "connected").length
    const errored = enabled.filter((s) => s.status === "error").length
    // Still coming up (initial handshake / reconnect). At first launch every
    // server is here for a moment — show a spinner so the bottom bar reads
    // "loading" instead of a flat green 0.
    const connecting = enabled.filter((s) => s.status === "connecting" || s.status === "disconnected").length
    const key = `${enabled.length}|${connected}|${errored}|${connecting}`
    if (key === last) return
    last = key
    if (enabled.length === 0) { conn.hide(); err.hide(); return }
    if (connecting > 0) {
      conn.text = `$(sync~spin) ${connected}/${enabled.length}`
      conn.tooltip = `Connecting MCP servers… ${connected} of ${enabled.length} up. Click to configure.`
    } else {
      conn.text = `$(plug) ${connected}`
      conn.tooltip = `MCP servers — ${connected} of ${enabled.length} connected. Click to configure.`
    }
    conn.show()
    if (errored > 0) {
      err.text = `$(error) ${errored}`
      err.tooltip = `${errored} MCP server${errored === 1 ? "" : "s"} failed to connect. Click to configure.`
      err.show()
    } else {
      err.hide()
    }
  }

  refresh()
  const timer = setInterval(refresh, MCP_STATUS_POLL_MS)
  return { dispose: () => { clearInterval(timer); conn.dispose(); err.dispose(); cmd.dispose() } }
}
