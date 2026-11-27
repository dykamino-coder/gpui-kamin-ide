// A synced declaration is a session snapshot. Confirm the command against
// this profile at execution time, including plugin enablement and approvals.
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { isDeepStrictEqual } from 'node:util'
import { listEnabledInstalledPlugins, readEffectivePluginManifest } from '../plugin-helpers'
import { collectPluginHooks, filterApprovedHooks, rewritePluginMcpMatchers } from '../sync/plugin-snapshot'
import type { HookExecuteRequest } from './executor'

interface Declaration {
  source: 'user' | 'plugin'
  matcher?: string
  handler: { type: string; command: string; args?: string[]; shell?: string; timeout?: number }
}

export async function verifyCodexToolHook(req: HookExecuteRequest, declaration: Declaration, cwd?: string): Promise<string | undefined> {
  const settingsPath = path.join(os.homedir(), '.claude', 'settings.json')
  let settings
  try {
    settings = JSON.parse(await fs.readFile(settingsPath, 'utf-8'))
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw new Error('Cannot verify the current hook settings')
  }
  let verifiedPluginRoot: string | undefined
  let hooks: Record<string, unknown[]> | undefined
  if (!settings?.disableAllHooks && declaration.source === 'user' && !req.pluginId) hooks = settings?.hooks
  if (!settings?.disableAllHooks && declaration.source === 'plugin' && req.pluginId) {
    const plugin = (await listEnabledInstalledPlugins({cwd})).find((p) => p.key === req.pluginId)
    if (plugin) {
      verifiedPluginRoot = plugin.installPath
      const manifest = await readEffectivePluginManifest(plugin.installPath, plugin.name, plugin.marketplace)
      const actual = await collectPluginHooks(plugin.installPath, manifest)
      hooks = rewritePluginMcpMatchers(plugin.name, filterApprovedHooks(plugin.key, actual))
    }
  }
  const groups = hooks?.[req.event] as Array<{ matcher?: string; hooks?: unknown[] }> | undefined
  const h = declaration.handler
  if (
    !h ||
    h.type !== 'command' ||
    req.command !== h.command ||
    req.shell !== h.shell ||
    req.timeoutMs !== (h.timeout ?? 600) * 1000 ||
    !isDeepStrictEqual(req.args, h.args) ||
    !groups?.some(
      (group) => group.matcher === declaration.matcher && group.hooks?.some((handler) => isDeepStrictEqual(handler, h)),
    )
  ) {
    throw new Error(
      'The local command hook is disabled, changed, or has not been approved in this profile. Resynchronize the session.',
    )
  }
  return verifiedPluginRoot
}
