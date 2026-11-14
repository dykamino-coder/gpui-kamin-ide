import fs from 'fs'
import path from 'path'
import { createHash } from 'crypto'
import type { KaminSession } from 'kaminide'
import type { TabManager } from './main/tab-manager'
import { hostGeneration } from './main/host-generation'
import type { ConnectionConfig, TabInfo } from './shared/types'

type RestoredTab = Pick<TabInfo, 'id' | 'label' | 'createdAt' | 'cwd' | 'conversationId' | 'sessionTitle' | 'engine' | 'model'> & { nativeId?: string }
interface Snapshot {
  parent: number
  generation: number
  scope: string
  tabs: RestoredTab[]
  activeTabId: string | null
  activeNativeId?: string
}

/** A child-replacement journal, not cold-start session restoration. The parent
 * and credential scope must still match, and only a newer child can consume it.
 * Retained CEF pages therefore keep the same Bridge tab identities. */
export class NativeRespawnState {
  private readonly file: string
  private readonly generation = hostGeneration()
  private lastSaved = ''

  constructor(storageDir: string, private readonly tm: TabManager,
    private readonly config: () => ConnectionConfig,
    private readonly tabToNative: Map<string, string>,
    private readonly nativeToTab: Map<string, string>,
    private readonly nativeActive: () => KaminSession | undefined) {
    this.file = path.join(storageDir, 'native-child-tabs.json')
  }

  private scope(): string {
    const cfg = this.config()
    return createHash('sha256').update(JSON.stringify([cfg.serverUrl, cfg.token])).digest('hex')
  }

  /** True when a previous child's journal was consumed (this is a respawn). */
  restore(sessions: readonly KaminSession[], active: KaminSession | undefined): boolean {
    if (this.generation <= 1 || !this.config().token) return false
    let saved: Snapshot
    try { saved = JSON.parse(fs.readFileSync(this.file, 'utf8')) as Snapshot } catch { return false }
    if (!saved || !Array.isArray(saved.tabs) || saved.parent !== process.ppid || saved.generation >= this.generation || saved.scope !== this.scope()) return false
    const nativeSessions = new Map(sessions.map(s => [s.id, s]))
    const cfg = this.config()
    for (const tab of saved.tabs) {
      // A native disconnect/delete during the outage takes precedence.
      if (tab.nativeId && !nativeSessions.get(tab.nativeId)?.open) continue
      const config = { ...cfg, cwd: tab.cwd, engine: tab.engine, model: tab.model }
      const opts = { background: true, restored: { id: tab.id, label: tab.label, createdAt: tab.createdAt } }
      if (tab.conversationId) {
        this.tm.resumeSession({ ...config, conversationId: tab.conversationId, sessionLabel: tab.sessionTitle }, opts)
      } else {
        this.tm.createTab(config, opts)
      }
      if (tab.nativeId) {
        this.tabToNative.set(tab.id, tab.nativeId)
        this.nativeToTab.set(tab.nativeId, tab.id)
      }
    }
    // Prefer a native selection changed during the outage, otherwise preserve
    // the Bridge selection even when the native session service has no active.
    const selected = active && active.id !== saved.activeNativeId
      ? this.nativeToTab.get(active.id) : saved.activeTabId
    if (selected && this.tm.getTab(selected)) this.tm.switchTab(selected)
    return true
  }

  save(): void {
    if (this.generation === 0) return // not the supervised native runtime
    const snapshot: Snapshot = {
      parent: process.ppid, generation: this.generation, scope: this.scope(),
      tabs: this.tm.listTabs().map(({ id, label, createdAt, cwd, conversationId, sessionTitle, engine, model }) => ({
        id, label, createdAt, cwd, conversationId, sessionTitle, engine,
        model: model ?? this.tm.getTab(id)?.config.model, nativeId: this.tabToNative.get(id),
      })),
      activeTabId: this.tm.getActiveTabId(),
      activeNativeId: this.nativeActive()?.id,
    }
    const data = JSON.stringify(snapshot)
    if (data === this.lastSaved) return
    try {
      fs.mkdirSync(path.dirname(this.file), { recursive: true })
      fs.writeFileSync(`${this.file}.tmp`, data, 'utf8')
      fs.renameSync(`${this.file}.tmp`, this.file)
      this.lastSaved = data
    } catch { /* an unavailable journal must not interrupt session interaction */ }
  }
}
