// Toast adapter — KaminIDE VSIX.
//
// The Electron app spawned separate OS-level notification windows when the
// main window was unfocused (electron/main/notifications/toast-window.ts).
// Inside KaminIDE the host owns the chrome, so the faithful equivalent is
// `vscode.window.showInformationMessage` (and the question variant routes its
// click back through the registered handler). The same `spawnToast` surface is
// preserved so the copied callers (config IPC, PushNotification tool, session
// callbacks) work unchanged.
import * as vscode from "vscode"
import { activateSessionForTab } from "../../session-activation"

export type ToastKind = "question" | "finished"

export interface ToastOpts {
  kind: ToastKind
  tabId: string
  title: string
  message: string
  requestId?: string
  sticky?: boolean
}

type RouteHandler = (payload: { tabId: string; requestId?: string }) => void
let routeHandler: RouteHandler | null = null

export function setToastRouteHandler(handler: RouteHandler): void {
  routeHandler = handler
}

let notificationsEnabled: () => boolean = () => true

export function configureToastGate(opts: { notificationsEnabled: () => boolean }): void {
  notificationsEnabled = opts.notificationsEnabled
}

export function spawnToast(
  opts: ToastOpts,
  forceOpts?: { bypassFocusGate?: boolean; bypassEnabledGate?: boolean },
): void {
  if (!forceOpts?.bypassEnabledGate && !notificationsEnabled()) return
  const text = opts.title ? `${opts.title} — ${opts.message}` : opts.message
  if (opts.kind === "question") {
    // Clicking the action focuses the originating chat via the route handler.
    void vscode.window.showInformationMessage(text, "Open").then((picked) => {
      if (picked === "Open") routeHandler?.({ tabId: opts.tabId, requestId: opts.requestId })
    })
  } else {
    // "finished" — offer an Open action that ACTIVATES the owning KaminIDE
    // session (switches to it + opens its chat), rather than just raising the
    // window with no way to jump to the session that finished.
    void vscode.window.showInformationMessage(text, "Open").then((picked) => {
      if (picked === "Open") activateSessionForTab(opts.tabId)
    })
  }
}

// vscode notifications auto-dismiss / aren't individually addressable, so the
// close helper is a no-op kept for call-site compatibility (core-ipc).
export function closeToastsMatching(_predicate: (opts: ToastOpts) => boolean): void {}
