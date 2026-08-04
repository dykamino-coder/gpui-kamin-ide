// VS Code-parity view-reveal commands. Extensions reveal their contributed
// views via `commands.executeCommand("workbench.view.extension.<containerId>")`
// or `"<viewId>.focus"`. KaminIDE registers those per contributed activity-bar
// container/view so they resolve; each broadcasts `kamin:view:reveal` with the
// owning container id, which the renderer turns into pin+activate+show.
import type { Disposable } from "../api/types.js"
import type { Registry } from "./registry.js"

let broadcast: ((channel: string, payload: unknown) => void) | null = null

/** Wired once at ext-host setup (exthost/index.ts) — the channel back to the
 *  renderer. */
export function setRevealBroadcast(fn: (channel: string, payload: unknown) => void): void {
  broadcast = fn
}

/** Register `workbench.view.extension.<containerId>` for an activity-bar
 *  container. Returns the command's Disposable. */
export function registerContainerRevealCommand(
  registry: Registry,
  container: { id: string; title: string },
): Disposable {
  return registry.registerCommand(
    `workbench.view.extension.${container.id}`,
    () => { broadcast?.("kamin:view:reveal", { container: container.id }) },
    { title: `Show ${container.title}`, category: "View" },
  )
}

/** Register `<viewId>.focus` for a view; revealing focuses its container AND —
 *  for a customize-located view — the specific section (so `<czView>.focus`
 *  lands on that section, not the container's first view). */
export function registerViewFocusCommand(
  registry: Registry,
  viewId: string,
  containerId: string,
): Disposable {
  return registry.registerCommand(
    `${viewId}.focus`,
    () => { broadcast?.("kamin:view:reveal", { container: containerId, view: viewId }) },
    { title: `Focus ${viewId}` },
  )
}
