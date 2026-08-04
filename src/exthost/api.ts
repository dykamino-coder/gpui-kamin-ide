// Thin composer for the `vscode` module that extensions import.
// All shape lives in `./api/*` — this file only wires it together.
//
// Architecture:
//   - `./api/shared`     — Disposable, EventEmitter, Uri, noopEvent.
//                          Module-scope singletons: shared identity
//                          across all extensions so `instanceof` works.
//   - `./api/enums`      — All `vscode.*` enums with reverse mapping.
//   - `./api/classes`    — Stub classes (Range, CompletionItem, …).
//                          Extensions extend / construct them at
//                          require-time; full behaviour ships in
//                          Phase D when the host renders the data.
//   - `./api/ns-builders`— commands, window namespaces (per-extension).
//   - `./api/ns-data`    — workspace, languages, env, …
//   - `./api/ns-misc`    — notebooks, tests, scm, chat, lm, …
//
// `createVscodeApi` runs once per extension. Closures capture `extId`
// for things that need it (commands.registerCommand records source);
// the rest just mirrors the pattern for symmetry.
import * as classesCore from "./api/classes-core.js"
import * as classesExt from "./api/classes-ext.js"
import * as classesLang from "./api/classes-lang.js"
import * as enums from "./api/enums.js"
import { buildCommands, buildWindow } from "./api/ns-builders.js"
import type { NsHooks } from "./api/ns-builders.js"
import { buildLanguages, buildEnv, buildExtensions, buildDebug, buildTasks, buildAuthentication } from "./api/ns-data.js"
import { buildDocuments } from "./api/ns-documents.js"
import { buildEditors } from "./api/ns-editor.js"
import { buildL10n } from "./api/ns-l10n.js"
import { buildNotebooks, buildTests, buildScm, buildComments, buildChat, buildLm } from "./api/ns-misc.js"
import { buildWorkspace } from "./api/ns-workspace.js"
import { Disposable, EventEmitter, Uri } from "./api/shared.js"
import type { ExtensionFacade } from "./api/types.js"

export type ExtHostHooks = NsHooks

const TARGET_VSCODE_API_VERSION = "1.95.0"

export function createVscodeApi(hooks: ExtHostHooks, extensionId: string) {
  // Build the document registry ONCE and share it so that
  // `window.activeTextEditor.document` is the same TextDocument identity as
  // `workspace.textDocuments` (B5b-2a).
  const documents = buildDocuments(hooks)
  const editors = buildEditors(hooks, documents)
  const workspace = buildWorkspace(hooks, documents)
  return {
    version: TARGET_VSCODE_API_VERSION,
    Disposable, EventEmitter, Uri,
    ...enums,
    ...classesCore,
    ...classesLang,
    ...classesExt,
    commands: buildCommands(hooks, extensionId),
    window: buildWindow(hooks, extensionId, editors),
    workspace,
    languages: buildLanguages(hooks, documents),
    env: buildEnv(hooks),
    extensions: buildExtensions(hooks),
    debug: buildDebug(),
    tasks: buildTasks(),
    authentication: buildAuthentication(),
    l10n: buildL10n(),
    notebooks: buildNotebooks(),
    tests: buildTests(),
    scm: buildScm(),
    comments: buildComments(),
    chat: buildChat(),
    lm: buildLm(),
  }
}

// VscodeApi is intentionally loose — we composed it from `import *`,
// and TS would lose all member knowledge if we wrote it out. The
// downstream consumer (ExtensionLoader) only treats the result as
// `unknown` payload to swap into `require('vscode')`.
export type VscodeApi = ReturnType<typeof createVscodeApi>

// Augment NsHooks with the loader-side hook so consumers can satisfy
// the contract without a separate `Hooks` type.
declare module "./api/ns-builders.js" {
  interface NsHooks {
    listExtensions: () => readonly ExtensionFacade[]
  }
}
