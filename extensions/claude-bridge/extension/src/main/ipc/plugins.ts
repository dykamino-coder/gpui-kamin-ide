// Thin barrel — `electron/main/ipc/plugins.ts` was a 1449-LOC god-file
// containing 13 IPC handlers + 4 helpers. Sprint 2 / Stage C (C2) split the
// implementation into focused modules under `./plugins/`:
//
//   plugins/shared.ts            — types + auth/source helpers
//   plugins/content-scan.ts      — countPluginContents + listPluginContents
//   plugins/handlers-content.ts  — get-contents, list-installed
//   plugins/handlers-options.ts  — get-options-schema, save-options
//   plugins/handlers-browse.ts   — browse-marketplace, browse-nested-marketplace
//   plugins/handlers-install.ts  — install, install-local, uninstall
//   plugins/handlers-source.ts   — get-source-path, sync-cache,
//                                  retry-plugin-source, refresh-plugin-source
//
// This file remains the single entry-point that consumers register from.

import { type PluginsIPCContext } from './plugins/shared'
import { registerContentHandlers } from './plugins/handlers-content'
import { registerOptionsHandlers } from './plugins/handlers-options'
import { registerBrowseHandlers } from './plugins/handlers-browse'
import { registerInstallHandlers } from './plugins/handlers-install'
import { registerSourceHandlers } from './plugins/handlers-source'

export type { PluginsIPCContext } from './plugins/shared'

export function registerPluginsIPC(ctx: PluginsIPCContext = {}): void {
  // Bind ctx.reloadMcpFromPlugins → tolerant async wrapper used by handlers
  // that mutate the installed-plugin set (install / uninstall / sync /
  // refresh). MCP re-discovery picks up plugin-sourced servers without
  // restart. Errors are logged and swallowed so a broken reload doesn't
  // poison an otherwise-successful install.
  async function reloadMcp(): Promise<void> {
    try { await ctx.reloadMcpFromPlugins?.() } catch (err) {
      console.warn('[plugins] reloadMcpFromPlugins failed:', err instanceof Error ? err.message : err)
    }
  }

  registerContentHandlers()
  registerOptionsHandlers()
  registerBrowseHandlers()
  registerInstallHandlers(reloadMcp)
  registerSourceHandlers(reloadMcp)
}
