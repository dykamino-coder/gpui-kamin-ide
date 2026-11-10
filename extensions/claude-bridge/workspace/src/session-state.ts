import { createHash } from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'

export interface OpenSession {
  id: string
  cwd: string
  conversationId?: string
  sessionTitle?: string
  label: string
  createdAt: string
}

interface SessionState {
  version: 1
  scope: string
  active: string | null
  tabs: OpenSession[]
}

/** Host-owned open set, independent of browser lifetime and renderer pin/order state. */
export class WorkspaceSessionState {
  private readonly file: string
  private readonly scope: string

  constructor(storage: string, root: string, serverUrl: string, token: string) {
    this.file = path.join(storage, 'workspace-sessions.json')
    // Neither connection credentials nor a reusable bearer are written to disk.
    this.scope = createHash('sha256').update(JSON.stringify([root, serverUrl, token])).digest('hex')
  }

  load(): Pick<SessionState, 'active' | 'tabs'> {
    const empty = { active: null, tabs: [] }
    try {
      if (fs.statSync(this.file).size > 1024 * 1024) return empty
      const state = JSON.parse(fs.readFileSync(this.file, 'utf8'))
      const text = (value: unknown, max: number) => typeof value === 'string' && value.length > 0 && value.length <= max
      if (state.version !== 1 || state.scope !== this.scope || !Array.isArray(state.tabs) || state.tabs.length > 500 ||
          !(state.active === null || text(state.active, 128))) return empty
      const ids = new Set<string>()
      for (const tab of state.tabs) {
        if (!tab || !text(tab.id, 128) || ids.has(tab.id) || !text(tab.cwd, 32768) ||
            !text(tab.label, 128) || !/^#\d+$/.test(tab.label) ||
            !Number.isSafeInteger(Number(tab.label.slice(1))) || !text(tab.createdAt, 64) ||
            !Number.isFinite(Date.parse(tab.createdAt)) ||
            !(tab.conversationId === undefined || text(tab.conversationId, 128)) ||
            !(tab.sessionTitle === undefined || typeof tab.sessionTitle === 'string' && tab.sessionTitle.length <= 65536)) return empty
        ids.add(tab.id)
      }
      if (state.active !== null && !ids.has(state.active)) return empty
      return { active: state.active, tabs: state.tabs }
    } catch { return empty }
  }

  save(tabs: OpenSession[], active: string | null): void {
    const state: SessionState = { version: 1, scope: this.scope, active,
      tabs: tabs.map(({ id, cwd, conversationId, sessionTitle, label, createdAt }) =>
        ({ id, cwd, conversationId, sessionTitle, label, createdAt })) }
    const temporary = `${this.file}.tmp`
    fs.writeFileSync(temporary, JSON.stringify(state), { mode: 0o600 })
    fs.renameSync(temporary, this.file)
  }
}
