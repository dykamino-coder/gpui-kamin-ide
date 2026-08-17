import type { ConfigStore } from '../config/store'
import { resetSyncTimers, syncProjectData, syncUserData } from './sync-client'

export interface ForceSyncContext {
  configStore: Pick<ConfigStore, 'get'>
  getProjectPaths: () => string[]
}

export interface ForceSyncResult {
  ok: boolean
  error?: string
  projectPath?: string | null
}

/** Upload a fresh user snapshot and every open project snapshot immediately. */
export async function forceSync(ctx: ForceSyncContext): Promise<ForceSyncResult> {
  const cfg = ctx.configStore.get()
  if (!cfg?.serverUrl || !cfg?.token) {
    return { ok: false, error: 'Server URL or token not configured' }
  }

  resetSyncTimers()
  const user = await syncUserData(cfg.serverUrl, cfg.token)
  if (!user.ok) return { ok: false, error: user.error ?? 'User sync failed' }

  const projectPaths = [...new Set(ctx.getProjectPaths().filter(Boolean))]
  for (const projectPath of projectPaths) {
    const project = await syncProjectData(cfg.serverUrl, cfg.token, projectPath)
    if (!project.ok) return { ok: false, error: project.error ?? 'Project sync failed', projectPath }
  }

  return { ok: true, projectPath: projectPaths[0] ?? null }
}
