// The `kaminide` virtual module (Phase 3) — what `require('kaminide')` resolves
// to inside an extension. ADDITIVE to `vscode`: an extension that never requires
// it is unaffected, and it is NOT exposed as `vscode.kaminide` (which would
// violate the faithful `@types/vscode` mirror). A single shared instance backs
// every caller because session events are global, not per-extension.
import type { SessionsApi } from "./api/sessions.js"

/** `kaminide.version` — the API contract version, bumped when the surface in
 *  kaminide.d.ts changes (minor = additive; extensions feature-detect on it). */
const KAMINIDE_API_VERSION = "2.1.0"

/** Lifetime of a `shell.showMessage` toast. VS Code keeps every notification
 *  with buttons until the user decides, and `vscode.window.show*Message` keeps
 *  that meaning: approvals and elicitations MUST wait. `transientMs` is the
 *  explicit opt-out for notices whose buttons are a shortcut, not a decision
 *  (BR-23: "Session finished — Open"). On timeout the request settles
 *  `undefined`, like a dismiss. */
export interface ShowMessageOptions {
  transientMs?: number
}

export type ShowMessageHook = (
  severity: "info" | "warning" | "error",
  message: string,
  items: string[],
  options?: ShowMessageOptions,
) => Promise<string | undefined>

export interface TransientMessageOptions {
  severity?: "info" | "warning" | "error"
  /** Countdown before the toast closes itself; the shell bounds it. */
  durationMs?: number
}

export interface KaminideWindowApi {
  showTransientMessage(message: string, options?: TransientMessageOptions, ...items: string[]): Promise<string | undefined>
}

export interface KaminideApi {
  readonly version: string
  readonly sessions: SessionsApi
  readonly window: KaminideWindowApi
}

/** Default countdown of a transient notice; equals the shell's TOAST_MS. */
export const TRANSIENT_DEFAULT_MS = 8000

export function createKaminideApi(sessions: SessionsApi, showMessage: ShowMessageHook): KaminideApi {
  const window: KaminideWindowApi = {
    showTransientMessage: (message, options, ...items) => {
      const ms = options?.durationMs
      const transientMs = typeof ms === "number" && Number.isFinite(ms) && ms > 0 ? Math.max(1, Math.round(ms)) : TRANSIENT_DEFAULT_MS
      const titles = items.filter((it): it is string => typeof it === "string")
      return showMessage(options?.severity ?? "info", message, titles, { transientMs })
    },
  }
  return { version: KAMINIDE_API_VERSION, sessions, window }
}
