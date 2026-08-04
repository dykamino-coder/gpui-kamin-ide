// The `require('vscode')` interception, split out of loader.ts to keep that
// file focused on discovery/activation. Patches Node's module loader so any
// extension's `require('vscode')` resolves to a per-extension api shim — the
// same documented trick real VS Code uses.
import type { VscodeApi } from "./api.js"

const VIRTUAL_VSCODE_ID = "__kamin_ide_vscode__"
const VIRTUAL_KAMINIDE_ID = "__kamin_ide_kaminide__"

interface NodeModulePrivate {
  _resolveFilename: (request: string, parent: unknown, ...rest: unknown[]) => string
  _load: (request: string, parent: unknown, ...rest: unknown[]) => unknown
}

/** Install the loader patch. `requireFn` is the loader's own `createRequire`
 *  (used only to grab Node's `module`); `resolveApi` maps the calling module's
 *  filesystem path to its cached/created `vscode` api. */
export function installVscodeRequireHook(
  requireFn: (id: string) => unknown,
  resolveApi: (parentPath: string) => VscodeApi,
  /** `require('kaminide')` resolver (Phase 3) — additive; a single shared api
   *  (session events are global), so no per-extension parent path is needed.
   *  NOTE: `kaminide` is a RESERVED module name (like `vscode`) — the hook
   *  hijacks it unconditionally, so a third-party npm dep named `kaminide`
   *  would be shadowed. Acceptable while only builtin extensions load. */
  resolveKaminide?: () => unknown,
): void {
  const Module = requireFn("module") as NodeModulePrivate
  const originalResolve = Module._resolveFilename
  const originalLoad = Module._load
  Module._resolveFilename = (request, parent, ...rest) => {
    if (request === "vscode") return VIRTUAL_VSCODE_ID
    if (request === "kaminide" && resolveKaminide) return VIRTUAL_KAMINIDE_ID
    return originalResolve(request, parent, ...rest)
  }
  Module._load = (request, parent, ...rest) => {
    if (request === "vscode" || request === VIRTUAL_VSCODE_ID) {
      const parentPath: string = (parent as { filename?: string } | null)?.filename ?? VIRTUAL_VSCODE_ID
      return resolveApi(parentPath)
    }
    if (resolveKaminide && (request === "kaminide" || request === VIRTUAL_KAMINIDE_ID)) {
      return resolveKaminide()
    }
    return originalLoad(request, parent, ...rest)
  }
}
