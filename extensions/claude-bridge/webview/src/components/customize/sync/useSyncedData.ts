// Data-fetch state + actions extracted from SyncedDataPanel.tsx (Sprint
// 5 / Stage E1). Owns: tree fetch, file fetch, selection, force-sync.
// Token hash is sha256-hex truncated to 16 chars — must match the
// server's `ensureUserDir` hashing (see src/core/sync/store.ts).

import { useEffect, useState, useMemo } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { serverFetch } from '../../../lib/server-fetch'
import type { SyncTreePayload, SelectedFile, FilePayload } from './sync-format'

async function computeHash(token: string): Promise<string> {
  const buf = await window.crypto.subtle.digest('SHA-256', new TextEncoder().encode(token))
  return Array.from(new Uint8Array(buf)).map(b => b.toString(16).padStart(2, '0')).join('').slice(0, 16)
}

export interface SyncedDataHandle {
  data: SyncTreePayload | null
  loading: boolean
  error: string | null
  selected: SelectedFile | null
  fileData: FilePayload | null
  fileLoading: boolean
  fileError: string | null
  hasAnything: boolean
  load: () => Promise<void>
  forceSyncAndLoad: () => Promise<void>
  openFile: (sel: SelectedFile) => Promise<void>
}

export function useSyncedData(): SyncedDataHandle {
  const bridge = useBridge()
  const [data, setData] = useState<SyncTreePayload | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [selected, setSelected] = useState<SelectedFile | null>(null)
  const [fileData, setFileData] = useState<FilePayload | null>(null)
  const [fileLoading, setFileLoading] = useState(false)
  const [fileError, setFileError] = useState<string | null>(null)

  async function load(): Promise<void> {
    setLoading(true)
    setError(null)
    try {
      const cfg = await bridge.getConfig()
      if (!cfg?.serverUrl || !cfg?.token) {
        setError('No server URL or token configured')
        setLoading(false)
        return
      }
      const httpUrl = cfg.serverUrl.replace(/^ws(s?):\/\//, 'http$1://')
      const hash = await computeHash(cfg.token)
      const resp = await serverFetch(bridge, httpUrl, `/api/sync/${hash}/tree`, {
        headers: { Authorization: `Bearer ${cfg.token}` },
      })
      if (!resp.ok) {
        setError(resp.error || `Server returned ${resp.status}`)
        setLoading(false)
        return
      }
      setData(resp.data as SyncTreePayload)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  async function openFile(sel: SelectedFile): Promise<void> {
    setSelected(sel)
    setFileLoading(true)
    setFileError(null)
    setFileData(null)
    try {
      const cfg = await bridge.getConfig()
      if (!cfg?.serverUrl || !cfg?.token) {
        setFileError('Not configured')
        return
      }
      const httpUrl = cfg.serverUrl.replace(/^ws(s?):\/\//, 'http$1://')
      const hash = await computeHash(cfg.token)
      const resp = await serverFetch(bridge, httpUrl, `/api/sync/${hash}/file?path=${encodeURIComponent(sel.absPath)}`, {
        headers: { Authorization: `Bearer ${cfg.token}` },
      })
      if (!resp.ok) {
        setFileError(resp.error || `Server returned ${resp.status}`)
        return
      }
      setFileData(resp.data as FilePayload)
    } catch (e) {
      setFileError(e instanceof Error ? e.message : String(e))
    } finally {
      setFileLoading(false)
    }
  }

  async function forceSyncAndLoad(): Promise<void> {
    setLoading(true)
    setError(null)
    try {
      const result = await bridge.forceSync()
      if (!result?.ok) {
        setError(result?.error || 'Force sync failed')
        return
      }
      await load()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  // On mount, just LOAD the tree the server already has — don't force a full
  // re-sync before the first paint (that pushes every local file up and blocks
  // the panel behind a slow network round-trip on every open). The user can
  // trigger a fresh push explicitly via the "Force sync now" button.
  useEffect(() => { void load() }, [])

  const hasAnything = useMemo(() => {
    if (!data) return false
    return data.user.tree.length > 0 || data.projects.some(p => p.tree.length > 0)
  }, [data])

  return {
    data, loading, error, selected, fileData, fileLoading, fileError, hasAnything,
    load, forceSyncAndLoad, openFile,
  }
}
