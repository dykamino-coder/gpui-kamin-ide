// ESM counterpart of loader-hook.ts. The CJS hook only patches
// `require('vscode')`; ESM extensions (esbuild `type:module` bundles like
// prettier 12.x) do `import { window } from "vscode"`, which goes through Node's
// ESM resolver and would fail with ERR_MODULE_NOT_FOUND. registerHooks (Node
// 22.15+, synchronous + in-thread, so the load hook can read our keys directly)
// resolves "vscode" to a virtual module that re-exports the live API from a
// global the host sets. The vscode *namespace* is identical for every
// extension (per-extension state arrives via activate(context), not via
// `import "vscode"`), so a single global API is faithful here.
import { registerHooks } from "node:module"
import { pathToFileURL } from "node:url"
import type { VscodeApi } from "./api.js"

const VIRTUAL_URL = "kaminvscode:vscode"
const GLOBAL_KEY = "__KAMIN_VSCODE_API__"
let installed = false

/** Point the ESM virtual `vscode` module at `api` (idempotent install of the
 *  resolver). The api object is the global namespace shim — its keys become the
 *  named exports ESM extensions destructure. */
export function installEsmVscodeHook(api: VscodeApi): void {
  ;(globalThis as Record<string, unknown>)[GLOBAL_KEY] = api
  if (installed) return
  installed = true
  // Only valid JS identifiers can be `export const`-named; the rest stay
  // reachable via the default export.
  const keys = Object.keys(api as Record<string, unknown>).filter((k) => /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(k))
  const named = keys.map((k) => `export const ${k} = __a[${JSON.stringify(k)}];`).join("\n")
  const source = `const __a = globalThis[${JSON.stringify(GLOBAL_KEY)}];\nexport default __a;\n${named}`
  registerHooks({
    resolve(specifier, context, nextResolve) {
      if (specifier === "vscode") return { url: VIRTUAL_URL, shortCircuit: true }
      return nextResolve(specifier, context)
    },
    load(url, context, nextLoad) {
      if (url === VIRTUAL_URL) return { format: "module", source, shortCircuit: true }
      return nextLoad(url, context)
    },
  })
}

/** Load an extension's main module: ESM (`type:module` manifest or `.mjs` main)
 *  via dynamic import (its `import "vscode"` hits the hook above); everything
 *  else via the CJS `require` path. */
export function loadExtensionModule(mainPath: string, manifest: Record<string, unknown>, requireFrom: (p: string) => unknown): unknown {
  const isEsm = manifest.type === "module" || mainPath.toLowerCase().endsWith(".mjs")
  return isEsm ? import(/* @vite-ignore */ pathToFileURL(mainPath).href) : requireFrom(mainPath)
}
