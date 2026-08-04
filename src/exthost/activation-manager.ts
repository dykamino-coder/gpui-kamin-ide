// Activation engine (B3). Holds extensions that declare `activationEvents`
// and defers their `activate()` until a matching trigger fires — replacing the
// Phase-A "activate everything at startup" behaviour. The loader registers
// each extension with its (already-normalised) event list and an idempotent
// activate thunk; this class decides WHEN to pull that thunk.
//
// Triggers wired today: `*` / `onStartupFinished` / `workspaceContains:<glob>`
// (at startup) and `onCommand:<id>` (via the registry's missing-command hook).
// `onLanguage` / `onView` / `onUri` / `onWebviewPanel` / … are matched by the
// same `fireEvent` path the moment their subsystems (B5–B7) start firing them.
import { minimatch } from "minimatch"

const STAR = "*"
const WORKSPACE_CONTAINS = "workspaceContains:"
const GLOB_CHARS = ["*", "?", "[", "]", "{", "}"]
// VS Code caps a single activate() at 10s then proceeds; we mirror that so one
// hanging extension can't block startup against the cold-start budget.
const ACTIVATION_TIMEOUT_MS = 10_000

export interface ActivatableExtension {
  id: string
  /** Normalised activation events (loader expands contributed commands into
   *  synthetic `onCommand:<id>` and maps the legacy empty list to `*`). */
  events: readonly string[]
  /** `extensionDependencies` ids — activated before this extension so it can
   *  read their exports in `activate()`. */
  deps: readonly string[]
  /** Idempotent — performs require + `activate(context)`; a no-op if already
   *  active. Errors are the loader's to surface; we just await. */
  activate: () => Promise<void>
}

export class ActivationManager {
  private readonly pending = new Map<string, ActivatableExtension>()
  private readonly activating = new Set<string>()

  register(ext: ActivatableExtension): void {
    this.pending.set(ext.id, ext)
  }

  /** Drop a still-pending registration (an extension disabled before it ever
   *  activated) so its events can't trigger it later. */
  unregister(id: string): void {
    this.pending.delete(id)
  }

  /** Activate `*` extensions and any `workspaceContains:<glob|path>` matched
   *  against the workspace file list (workspace-relative POSIX paths).
   *  `onStartupFinished` is intentionally NOT handled here — the loader fires
   *  it separately AFTER this settles, matching VS Code's ordering. */
  async activateStartup(workspaceFiles: readonly string[]): Promise<void> {
    const ready = [...this.pending.values()].filter((e) => matchesStartup(e.events, workspaceFiles))
    // Parallel (like VS Code) — a slow/hanging activate must not block the
    // others. `run` removes from `pending` before awaiting, so the snapshot
    // can't double-activate.
    await Promise.all(ready.map((ext) => this.run(ext)))
  }

  /** Fire a discrete activation event (`onCommand:x`, `onLanguage:js`, …).
   *  Activates every still-pending extension that declared it. */
  async fireEvent(event: string): Promise<void> {
    const ready = [...this.pending.values()].filter((e) => e.events.includes(event))
    await Promise.all(ready.map((ext) => this.run(ext)))
  }

  private async run(ext: ActivatableExtension): Promise<void> {
    if (!this.pending.has(ext.id)) return // already activated by a dep chain
    // Remove before awaiting so a re-entrant fire (e.g. the extension running
    // its own command during activate) can't double-activate it.
    this.pending.delete(ext.id)
    this.activating.add(ext.id)
    // Activate dependencies first (force-activate if still pending) so this
    // extension can read their exports. `pending.delete` + `activating` break
    // dependency cycles naturally.
    try {
      for (const depId of ext.deps) {
        const dep = this.pending.get(depId)
        if (dep && !this.activating.has(depId)) await this.run(dep)
      }
      await withTimeout(ext.activate(), ACTIVATION_TIMEOUT_MS, () => {
        console.warn(`activation of ${ext.id} exceeded ${String(ACTIVATION_TIMEOUT_MS)}ms — continuing`)
      })
    } finally {
      this.activating.delete(ext.id)
    }
  }
}

/** Race a promise against a timeout; on timeout, log and resolve (the original
 *  keeps running in the background — we just stop blocking startup on it). */
function withTimeout(p: Promise<void>, ms: number, onTimeout: () => void): Promise<void> {
  let timer: ReturnType<typeof setTimeout>
  const t = new Promise<void>((resolve) => {
    timer = setTimeout(() => { onTimeout(); resolve() }, ms)
  })
  return Promise.race([p.finally(() => { clearTimeout(timer) }), t])
}

function matchesStartup(events: readonly string[], files: readonly string[]): boolean {
  for (const e of events) {
    if (e === STAR) return true
    if (e.startsWith(WORKSPACE_CONTAINS) && workspaceContainsMatch(e.slice(WORKSPACE_CONTAINS.length), files)) {
      return true
    }
  }
  return false
}

/** `workspaceContains:` takes a glob OR a bare relative path. VS Code globs
 *  patterns with metacharacters and otherwise checks for plain path existence;
 *  we mirror that against the indexed file list (a bare dir matches anything
 *  under it). Note: paths the index excludes (e.g. `.git`) can't match — a
 *  known approximation of VS Code's direct filesystem probe. */
function workspaceContainsMatch(pattern: string, files: readonly string[]): boolean {
  if (GLOB_CHARS.some((c) => pattern.includes(c))) {
    return files.some((f) => minimatch(f, pattern, { dot: true }))
  }
  return files.some((f) => f === pattern || f.startsWith(`${pattern}/`))
}
