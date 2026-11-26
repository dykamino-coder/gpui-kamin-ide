import type { WebviewView } from 'vscode'

/** A reveal may create the Console after the menu action. Only its active xterm
 * can acknowledge readiness and consume the pending keyboard focus request. */
export class PermissionConsoleFocus {
  private pending?: string
  private view?: WebviewView

  constructor(
    private readonly activeClaudeTab: (id: string) => boolean,
    private readonly reveal: () => void,
    private readonly focus: () => void,
  ) {}

  request(tabId: string): void {
    if (!this.activeClaudeTab(tabId)) return
    this.pending = tabId
    this.reveal()
    this.offer()
  }

  private offer(): void {
    if (!this.pending || !this.view?.visible) return
    if (!this.activeClaudeTab(this.pending)) { this.pending = undefined; return }
    void this.view.webview.postMessage({ kind: 'event', channel: 'permission-console:requested', args: [this.pending] })
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
      const id = message.args?.[0]
      if (!this.pending || !this.activeClaudeTab(this.pending)) { this.pending = undefined; return }
      if (id !== this.pending) return
      this.pending = undefined
      this.focus()
      void view.webview.postMessage({ kind: 'event', channel: 'permission-console:focus', args: [id] })
    })
    view.onDidDispose(() => {
      visibility.dispose()
      ready.dispose()
      if (this.view === view) { this.view = undefined; this.pending = undefined }
    })
    this.offer()
  }
}
