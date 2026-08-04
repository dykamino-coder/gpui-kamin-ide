export * from './types'
export { resolveHost, explainHost } from './host-resolver'
export { registerSessionHooks, clearSession, getHook, listSession } from './registry'
export { rewriteHooksForCli } from './proxy-rewriter'
export { mergeAllHooks } from './source-merger'
export { appendExecution, readRecentExecutions, clearLog } from './audit-log'
export type { HookExecutionRecord } from './audit-log'
export { HOOK_LIBRARY, findTemplate } from './library'
export type { HookLibraryTemplate } from './library'
export { emitBridgeEvent } from './bridge-emitter'
export {
  dispatchHook,
  handleLocalHookResponse,
  cancelSessionLocalExecs,
} from './dispatcher'
export { createHooksRoutes } from './routes'
