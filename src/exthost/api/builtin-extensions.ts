// Facades for the built-in extensions VS Code ships (#22). Third-party
// extensions feature-detect them via `vscode.extensions.getExtension(id)` and
// often read the result at module-load/activation time. We don't run these
// built-ins, but returning a faithful "active" facade instead of `undefined`
// keeps such extensions from crashing — e.g. Vue.volar does
//   const e = extensions.getExtension("vscode.typescript-language-features")
//   if (e.isActive) return  // ← throws when e is undefined
// and bails out cleanly when it sees an active TS extension (so it doesn't try
// to patch a tsserver we don't have).
//
// `isActive` is true (extensions branch on it; an active builtin is the
// least-surprising answer) and `exports` is an empty object — these aren't real,
// so we expose no API, but property reads stay safe (only calling a missing API
// method would throw, and the corpus extensions that reach exports already
// guard for it). Add ids here as more builtin-dependent extensions surface.
import { URI } from "vscode-uri"
import { ExtensionKind } from "./enums.js"
import type { ExtensionFacade } from "./types.js"

interface BuiltinSpec { id: string; displayName: string }

const BUILTINS: readonly BuiltinSpec[] = [
  { id: "vscode.typescript-language-features", displayName: "TypeScript and JavaScript Language Features" },
]

function makeBuiltinFacade(spec: BuiltinSpec): ExtensionFacade {
  const extensionPath = `/builtin/${spec.id}`
  const dot = spec.id.indexOf(".")
  const publisher = dot >= 0 ? spec.id.slice(0, dot) : "vscode"
  const name = dot >= 0 ? spec.id.slice(dot + 1) : spec.id
  return {
    id: spec.id,
    extensionUri: URI.file(extensionPath),
    extensionPath,
    isActive: true,
    packageJSON: { name, publisher, displayName: spec.displayName, version: "1.0.0", engines: { vscode: "*" }, isBuiltin: true },
    extensionKind: ExtensionKind.UI,
    exports: {},
    activate: () => Promise.resolve({}),
  }
}

/** Built-in facades, keyed-unique by id. Real extensions with the same id
 *  always take precedence (see ns-data buildExtensions). */
export const BUILTIN_EXTENSION_FACADES: readonly ExtensionFacade[] = BUILTINS.map(makeBuiltinFacade)
