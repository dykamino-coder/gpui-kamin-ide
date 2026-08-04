// Claude Bridge — KaminIDE extension (CLIENT). Native-integrated layout.
//
// No duplicate session list: KaminIDE's native sessions are the one list; this
// extension binds each to a Claude conversation (SessionsBridge) and renders the
// conversation in the Claude Bridge TOOL — a webview-view activity (dockable /
// resizable like Search/Tree), NOT an editor-area file tab. Sessions/projects/
// files/editor/terminal/themes are all KaminIDE-native.
// See docs/BRIDGE_VSIX_INTEGRATION.md.
import * as vscode from "vscode"
import * as kaminide from "kaminide"
import fs from "fs"
import path from "path"
import { BridgeHost, type WebviewRole } from "./bridge-host"
import { installEditorContext } from "./editor-context"
import { installMcpStatusBar } from "./mcp-status-bar"
import { SessionsBridge } from "./sessions-bridge"
import { ipcMain } from "./shim/electron"

const CHAT_VIEW_ID = "claudeBridgeChat"
// Plan / Todos / Agents / Console are SEPARATE tools (own auxiliarybar container
// each, independently pinnable). All load the one `tools.html`; it reads its view
// id from the iframe URL and renders only that section. Console shows the raw
// Claude CLI terminal for the active session. Match contributes.views.
const TOOLS_VIEW_IDS = ["claudeBridgePlanView", "claudeBridgeTodosView", "claudeBridgeAgentsView", "claudeBridgeConsoleView", "claudeBridgeToolsUsageView"]
// Each customize section is its OWN webview view (a separate page) under the
// `claudeBridgeCustomize` container — KaminIDE renders them as a TOC tree. They
// all load the same bundle; the bundle reads its view id from the iframe URL and
// renders only that section. Must match contributes.views in package.json.
const CUSTOMIZE_VIEW_IDS = [
  "claudeBridgeCzSettings", "claudeBridgeCzSkills", "claudeBridgeCzAgents", "claudeBridgeCzMcp",
  "claudeBridgeCzHooks", "claudeBridgeCzPlugins", "claudeBridgeCzMonitors", "claudeBridgeCzSync",
  "claudeBridgeCzLogs", "claudeBridgeCzStats",
]

export function activate(context: vscode.ExtensionContext): void {
  // Reveal the Claude Bridge tool (its activity), creating/resolving the view.
  const openChat = (): void => {
    void vscode.commands.executeCommand(`${CHAT_VIEW_ID}.focus`)
      .then(undefined, () => vscode.commands.executeCommand("workbench.view.extension.claudeBridge"))
  }
  const openTools = (): void => { void vscode.commands.executeCommand(`${TOOLS_VIEW_IDS[0]}.focus`) }

  const host = new BridgeHost(context, { onOpenChat: openChat })
  const sessionsBridge = new SessionsBridge(host, openChat)
  sessionsBridge.install()
  context.subscriptions.push({ dispose: () => { sessionsBridge.dispose(); host.dispose() } })
  // Push the host Monaco editor's active file + selection to the composer's
  // "attach file" toggle (editor-context on send).
  context.subscriptions.push(installEditorContext(host))
  // Status-bar item: MCP servers connected/errored → click opens the MCP page.
  context.subscriptions.push(installMcpStatusBar(host.getMcpManager()))

  // Each contributed webview-view (chat + the Plan/Todos/Agents side panel) is
  // its own iframe; both attach to the one BridgeHost (shared session state).
  // Session-consuming views report visibility to the host: a hidden webview view
  // drops postMessage (VS Code semantics), so the host records the gap per view
  // at the dropped send and catches that view up when it's revealed.
  const registerView = (viewId: string, htmlFile: string, role: WebviewRole, trackVisibility = false): void => {
    const provider: vscode.WebviewViewProvider = {
      resolveWebviewView(view) {
        console.log(`[bridge] resolveWebviewView ${viewId}`)
        view.webview.options = { enableScripts: true, localResourceRoots: [] }
        view.webview.html = loadWebviewHtml(context, htmlFile)
        // The role withholds the per-session firehose (jsonl/streaming/pty) from
        // panels that render none of it — the customize sections get none, the
        // tools panels keep only a projected slice on the webview side. This is
        // what stopped every panel from holding its own ~1GB copy of the store.
        const attached = host.attach(view.webview, role)
        // Tie the fan-out registration to THIS view's lifetime, not the
        // extension's: a disposed view must leave the broadcast Set (and its
        // visibility/stale bookkeeping) rather than linger as a dead target.
        // dispose() is idempotent, so also keeping it in subscriptions
        // (extension-teardown cleanup) is safe.
        view.onDidDispose(() => attached.dispose())
        context.subscriptions.push(attached)
        // Message DELIVERY to a hidden view is refused by the host, faithfully to
        // VS Code ("You cannot send messages to a hidden webview, even with
        // retainContextWhenHidden enabled"), so a panel that missed entries while
        // hidden is caught up on reveal — not by delivery-while-hidden.
        if (trackVisibility) {
          host.setWebviewVisible(view.webview, view.visible) // seed (visible at resolve)
          const vis = view.onDidChangeVisibility(() => host.setWebviewVisible(view.webview, view.visible))
          view.onDidDispose(() => vis.dispose())
          context.subscriptions.push(vis)
        }
      },
    }
    context.subscriptions.push(
      vscode.window.registerWebviewViewProvider(viewId, provider, { webviewOptions: { retainContextWhenHidden: true } }),
    )
  }
  host.setResyncHandler(() => sessionsBridge.resyncActive())
  registerView(CHAT_VIEW_ID, "chat.html", "chat", true)
  // Tools panels consume the session stream too and also drop posts while hidden,
  // so each gets the same resync-on-reveal as chat.
  for (const id of TOOLS_VIEW_IDS) registerView(id, "tools.html", "tools", true)
  // Bridge's Settings/Skills/MCP/… pages, hosted inside KaminIDE's Customize as a
  // tree of separate pages. One bundle (customize.html) per view; it self-selects
  // the section from its iframe URL. No visibility wiring — customize consumes no
  // session data, so it has nothing to resync.
  for (const id of CUSTOMIZE_VIEW_IDS) registerView(id, "customize.html", "customize")

  context.subscriptions.push(
    vscode.commands.registerCommand("claude-bridge.openChat", openChat),
    vscode.commands.registerCommand("claude-bridge.openTools", openTools),
    vscode.commands.registerCommand("claude-bridge.reconnect", openChat),
    // Native session context menu → regenerate the AI title from chat (/rename).
    vscode.commands.registerCommand("claude-bridge.regenerateTitle", (kaminId: unknown) => {
      if (typeof kaminId === "string") sessionsBridge.regenerateTitleByKamin(kaminId)
    }),
    // Системные команды (правило тестируемости: ВСЁ системное доступно через
    // executeCommand — e2e зовут их вместо эмуляции кликов).
    vscode.commands.registerCommand("claude-bridge.sendMessage", (text: unknown) =>
      typeof text === "string" && host.cmdSendMessage(text)),
    vscode.commands.registerCommand("claude-bridge.changeModel", (model: unknown) =>
      typeof model === "string" && host.cmdChangeModel(model)),
    vscode.commands.registerCommand("claude-bridge.changeEffort", (effort: unknown) =>
      typeof effort === "string" && host.cmdChangeEffort(effort)),
    vscode.commands.registerCommand("claude-bridge.interrupt", () => host.cmdInterrupt()),
    vscode.commands.registerCommand("claude-bridge.getSessionInfo", () => host.cmdGetSessionInfo()),
    // Панель Tools/e2e: раскрыть применения тула фуллскрином в чате.
    vscode.commands.registerCommand("claude-bridge.openToolUsage", (name: unknown) => {
      if (typeof name === "string") host.broadcast("toolusage-open", name)
    }),
    // Жизненный цикл сессий (правило тестируемости): создание/активация идут
    // через нативный sessions-API хоста — sessionsBridge сам поднимет вкладку.
    vscode.commands.registerCommand("claude-bridge.newSession", async (projectId: unknown, name: unknown) =>
      await kaminide.sessions.createSession({
        ...(typeof projectId === "string" ? { projectId } : {}),
        ...(typeof name === "string" ? { name } : {}),
      })),
    vscode.commands.registerCommand("claude-bridge.activateSession", async (kaminId: unknown) =>
      typeof kaminId === "string" ? await kaminide.sessions.setActiveSession(kaminId) : false),
    vscode.commands.registerCommand("claude-bridge.closeSession", () => host.cmdCloseSession()),
    vscode.commands.registerCommand("claude-bridge.setPermissionMode", (mode: unknown) =>
      typeof mode === "string" && host.cmdSetPermissionMode(mode)),
  )
  // Кросс-вебвью relay: клик в панели Tools (свой iframe) → событие всем вью;
  // чат откроет fullscreen применений.
  ipcMain.on("toolusage:open", (_e: unknown, name: unknown) => {
    if (typeof name === "string") host.broadcast("toolusage-open", name)
  })
  // Chat-panel header button → same, but keyed by the active Bridge tab id.
  ipcMain.on("bridge:regenerate-title", (_e: unknown, tabId: string) => sessionsBridge.regenerateTitleByTab(tabId))
}

export function deactivate(): void {}

/** Load a built single-file webview bundle produced by
 *  extensions/claude-bridge/webview. */
function loadWebviewHtml(context: vscode.ExtensionContext, file: string): string {
  const p = path.join(context.extensionPath, file)
  try {
    return fs.readFileSync(p, "utf8")
  } catch {
    return "<html><body style='font-family:system-ui;padding:16px;color:#e1e4e8'>"
      + `<h2>Claude Bridge</h2><p>${file} missing — run <code>npm run build</code> in extensions/claude-bridge/webview.</p></body></html>`
  }
}
