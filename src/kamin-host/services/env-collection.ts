// Parent-side store for extensions' `environmentVariableCollection` (#11).
// Extensions mutate PATH and friends (e.g. the Python extension prepends a
// venv's bin dir); those mutations must reach the integrated terminal's spawn
// env. The collection objects live in the forked ext-host CHILD (vscode's
// get/forEach are synchronous), which pushes a full snapshot here on every
// change. pty.ts (also parent-side) reads `applyEnvCollections` at spawn time.
//
// Session-scoped by design: a mutation applies for as long as its owning
// extension is active. On disable/unload the contribution is dropped
// immediately (no stale PATH for new terminals). The `persistent` flag is
// accepted for API compatibility but cross-restart pre-seeding is NOT separately
// maintained — an enabled extension re-applies its mutations when it
// re-activates (which happens early at boot), so the only thing not covered is a
// terminal opened in the sub-second window before activation. Keeping it
// session-only avoids the disable/boot edge cases a disk layer introduces
// (a disabled extension's persisted PATH leaking into fresh terminals).
import type { EnvCollectionSnapshot } from "../../exthost/host-services.js"

// vscode.EnvironmentVariableMutatorType
const MUTATOR_REPLACE = 1
const MUTATOR_APPEND = 2
const MUTATOR_PREPEND = 3

/** extId → live snapshot applied to terminal spawns (insertion order = apply order). */
const session = new Map<string, EnvCollectionSnapshot>()

function isEmpty(snap: EnvCollectionSnapshot): boolean {
  return Object.keys(snap.vars).length === 0
}

/** The child pushed an extension's full collection. Replace the session entry
 *  (an empty collection — cleared — is removed entirely). */
export function syncEnvCollection(extId: string, snapshot: EnvCollectionSnapshot): void {
  if (isEmpty(snapshot)) session.delete(extId)
  else session.set(extId, snapshot)
}

/** Disable/unload: drop the contribution so new terminals are clean. */
export function dropEnvCollection(extId: string): void {
  session.delete(extId)
}

/** Apply every collection's mutations onto a copy of `base`. Replace overwrites;
 *  Append/Prepend concatenate around the current value. */
export function applyEnvCollections(base: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
  const out: NodeJS.ProcessEnv = { ...base }
  for (const snap of session.values()) {
    for (const [name, m] of Object.entries(snap.vars)) {
      const cur = out[name] ?? ""
      if (m.type === MUTATOR_REPLACE) out[name] = m.value
      else if (m.type === MUTATOR_APPEND) out[name] = cur + m.value
      else if (m.type === MUTATOR_PREPEND) out[name] = m.value + cur
    }
  }
  return out
}
