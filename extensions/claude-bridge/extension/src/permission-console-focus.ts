import type { WebviewView } from 'vscode'

/** A reveal may create the Console after the menu action. Only its active xterm
 * can acknowledge readiness and consume the pending keyboard focus request. */
export class PermissionConsoleFocus {
  private pending?: { tabId: string; sessionId: string }
  private view?: WebviewView

  constructor(
    private readonly activeClaudeSession: (tabId: string) => string | undefined,
    private readonly reveal: () => void,
    private readonly focus: () => void,
  ) {}

  request(tabId: string): void {
    const sessionId = this.activeClaudeSession(tabId)
    if (!sessionId) return
    this.pending = { tabId, sessionId }
    this.reveal()
    this.offer()
  }

  private currentRequest() {
    if (this.pending && this.activeClaudeSession(this.pending.tabId) !== this.pending.sessionId) this.pending = undefined
    return this.pending
  }

  private offer(): void {
    const request = this.currentRequest()
    if (!request || !this.view?.visible) return
    void this.view.webview.postMessage({ kind: 'event', channel: 'permission-console:requested', args: [request.tabId, request.sessionId] })
  }

  attach(view: WebviewView): void {
    this.view = view
    const visibility = view.onDidChangeVisibility(() => {
      if (this.view !== view) return
      if (view.visible) this.offer()
      else this.pending = undefined
    })
    const ready = view.webview.onDidReceiveMessage(message => {
      if (message?.kind !== 'send' || message.channel !== 'permission-console:ready' || this.view !== view || !view.visible) return
      const request = this.currentRequest()
      if (!request || message.args?.[0] !== request.tabId || message.args?.[1] !== request.sessionId) return
      this.pending = undefined
      this.focus()
      void view.webview.postMessage({ kind: 'event', channel: 'permission-console:focus', args: [request.tabId, request.sessionId] })
    })
    view.onDidDispose(() => {
      visibility.dispose()
      ready.dispose()
      if (this.view === view) { this.view = undefined; this.pending = undefined }
    })
    this.offer()
  }
}
