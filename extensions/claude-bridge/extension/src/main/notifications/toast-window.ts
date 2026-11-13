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

export type NoticeSeverity = "info" | "warning" | "error"

/** `kaminide.window.showTransientMessage` when the host has it (API 2.1.0+).
 *  A `vscode.window.show*Message` with buttons waits for the user, as in VS
 *  Code, and must keep doing so for questions and approvals. "Session
 *  finished" is a notice whose Open is a shortcut, so it needs the explicit
 *  self-closing contract instead (BR-23). Hosts without it keep the old path. */
export type TransientNotice = (
  message: string,
  options: { severity?: NoticeSeverity; durationMs?: number },
  ...items: string[]
) => Promise<string | undefined>

let transientNotice: TransientNotice | null = null

export function configureTransientNotice(fn: TransientNotice | null): void {
  transientNotice = fn
}

/** Countdown of the completion toast; equals the shell's default toast life. */
export const FINISHED_TOAST_MS = 8000

function showPlain(severity: NoticeSeverity, text: string, ...items: string[]): Thenable<string | undefined> {
  if (severity === "error") return vscode.window.showErrorMessage(text, ...items)
  if (severity === "warning") return vscode.window.showWarningMessage(text, ...items)
  return vscode.window.showInformationMessage(text, ...items)
}

/** A notice without buttons that keeps the caller's own duration. The shared
 *  API has no duration, so without the transient hook it falls back to the
 *  shell default instead of losing the notice. */
export function showTimedNotice(severity: NoticeSeverity, text: string, durationMs?: number): void {
  if (transientNotice && typeof durationMs === "number" && durationMs > 0) {
    void transientNotice(text, { severity, durationMs })
    return
  }
  void showPlain(severity, text)
}

export function spawnToast(
  opts: ToastOpts,
  forceOpts?: { bypassFocusGate?: boolean; bypassEnabledGate?: boolean },
): void {
  if (!forceOpts?.bypassEnabledGate && !notificationsEnabled()) return
  const text = opts.title ? `${opts.title} — ${opts.message}` : opts.message
  if (opts.kind === "question") {
    // A question waits for the user: always the sticky shared notification.
    void vscode.window.showInformationMessage(text, "Open").then((picked) => {
      if (picked === "Open") routeHandler?.({ tabId: opts.tabId, requestId: opts.requestId })
    })
  } else {
    // "finished" — offer an Open action that ACTIVATES the owning KaminIDE
    // session (switches to it + opens its chat), rather than just raising the
    // window with no way to jump to the session that finished. The browser
    // host has no native session hook and uses the existing chat route.
    const shown = transientNotice
      ? transientNotice(text, { severity: "info", durationMs: FINISHED_TOAST_MS }, "Open")
      : vscode.window.showInformationMessage(text, "Open")
    void Promise.resolve(shown).then((picked) => {
      if (picked === "Open" && !activateSessionForTab(opts.tabId)) routeHandler?.({ tabId: opts.tabId })
    })
  }
}

// vscode notifications auto-dismiss / aren't individually addressable, so the
// close helper is a no-op kept for call-site compatibility (core-ipc).
export function closeToastsMatching(_predicate: (opts: ToastOpts) => boolean): void {}
