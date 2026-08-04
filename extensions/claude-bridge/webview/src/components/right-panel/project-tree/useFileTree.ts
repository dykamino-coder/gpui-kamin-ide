// File-tree state hook — single source of truth for which directories
// are expanded, which children each directory holds, and which path is
// currently focused. Lives outside ProjectTreeTab.tsx so DnD logic
// (when we add it) can plug in via the same handles without coupling
// to the visual component.

import { useEffect, useState, useCallback } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'

export interface TreeEntry {
  name: string
  path: string
  isDir: boolean
  mtimeMs: number
  size: number
}

export interface UseFileTreeHandle {
  /** Children of `path` if loaded, else undefined. */
  childrenOf: (path: string) => TreeEntry[] | undefined
  /** Lazy-loaded set — toggles expansion + (on first expand) fetches. */
  toggle: (path: string) => Promise<void>
  /** Force re-fetch a directory (used after rename / external change). */
  refresh: (path: string) => Promise<void>
  isExpanded: (path: string) => boolean
  /** Currently-focused row (Tab/Shift+Tab + arrow nav, future DnD). */
  focused: string | null
  setFocused: (p: string | null) => void
}

export function useFileTree(root: string | null): UseFileTreeHandle {
  const bridge = useBridge()
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set())
  const [children, setChildren] = useState<Map<string, TreeEntry[]>>(() => new Map())
  const [focused, setFocused] = useState<string | null>(null)

  const fetchDir = useCallback(async (dir: string) => {
    try {
      const entries = await bridge.fileTreeListDir(dir)
      setChildren(prev => {
        const next = new Map(prev)
        next.set(dir, entries)
        return next
      })
    } catch { /* permission / removed mid-fetch */ }
  }, [bridge])

  // First mount + root change — start a recursive watch and load the
  // root directory listing. Cleanup unwatches.
  useEffect(() => {
    if (!root) return
    setExpanded(new Set([root]))
    setChildren(new Map())
    setFocused(null)
    fetchDir(root)
    bridge.fileTreeWatch(root)
    const off = bridge.onFileTreeChange(({ root: changedRoot, changedDir }) => {
      if (changedRoot !== root) return
      // Refresh just the affected directory's children if we have it
      // loaded. Anything below it that isn't expanded stays untouched.
      setChildren(prev => {
        if (!prev.has(changedDir)) return prev
        // Re-fetch async; result merges in via setChildren above.
        fetchDir(changedDir)
        return prev
      })
      // Always refresh the changed dir's own *parent* listing too (in
      // case it's a rename/delete of `changedDir` itself, the parent's
      // children list needs updating).
      const parent = changedDir.replace(/[\\/][^\\/]+$/, '')
      if (parent && parent !== changedDir) {
        setChildren(prev => {
          if (!prev.has(parent)) return prev
          fetchDir(parent)
          return prev
        })
      }
    })
    return () => {
      off()
      bridge.fileTreeUnwatch(root)
    }
  }, [root, bridge, fetchDir])

  const toggle = useCallback(async (path: string) => {
    setExpanded(prev => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
    if (!children.has(path)) await fetchDir(path)
  }, [children, fetchDir])

  const refresh = useCallback(async (path: string) => {
    await fetchDir(path)
  }, [fetchDir])

  const childrenOf = useCallback((path: string) => children.get(path), [children])
  const isExpanded = useCallback((path: string) => expanded.has(path), [expanded])

  return { childrenOf, toggle, refresh, isExpanded, focused, setFocused }
}
