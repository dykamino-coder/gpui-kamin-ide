import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { activeTabId, tabs } from '../../../signals/tabs'
import { openFileInTab } from '../../../signals/file-viewer'
import { filePanelVisible } from '../../../signals/ui'
import tabStyles from '../../titlebar/TabsBar.module.css'
import { useFileTree, type TreeEntry } from './useFileTree'
import { TreeChildren } from './TreeNode'

/** Project tree for the active chat tab's cwd. Uses fs.watch
 *  recursively in main; renderer maintains a Map<dir, children[]> +
 *  Set<expandedDir>. Click on file → open in CodeMirror editor; right-
 *  click → context menu (rename so far). DnD-ready: each row carries
 *  `data-path` + `data-is-dir`, so a future hook can hit-test by
 *  `closest('[data-path]')` without restructuring. */
export function ProjectTree(): JSX.Element {
  const bridge = useBridge()
  const tabId = activeTabId.value
  const tab = tabs.value.find(t => t.id === tabId) ?? null
  const root = tab?.cwd || null
  const tree = useFileTree(root)

  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; entry: TreeEntry } | null>(null)
  const [renamingPath, setRenamingPath] = useState<string | null>(null)
  const [renameDraft, setRenameDraft] = useState<string>('')

  function openFile(path: string): void {
    const id = activeTabId.value
    if (!id) return
    openFileInTab(id, path)
    if (!filePanelVisible.value) {
      filePanelVisible.value = true
      try { bridge.setLayout?.({ filePanelVisible: true }) } catch { /* noop */ }
    }
  }

  function startRename(entry: TreeEntry): void {
    setRenamingPath(entry.path)
    setRenameDraft(entry.name)
    setCtxMenu(null)
  }

  function cancelRename(): void {
    setRenamingPath(null)
    setRenameDraft('')
  }

  async function commitRename(): Promise<void> {
    const oldPath = renamingPath
    const draft = renameDraft.trim()
    setRenamingPath(null)
    setRenameDraft('')
    if (!oldPath || !draft) return
    if (draft.includes('/') || draft.includes('\\')) return // refuse path-walks
    const parent = oldPath.replace(/[\\/][^\\/]+$/, '')
    const sep = oldPath.includes('\\') ? '\\' : '/'
    const newPath = parent + sep + draft
    if (newPath === oldPath) return
    try {
      await bridge.fileTreeRename(oldPath, newPath)
      // The recursive watcher will fire `file-tree:change` for the
      // parent directory; useFileTree refreshes children automatically.
    } catch {
      // ignore — file may already exist or perms denied. Leave the
      // tree as-is, user can retry.
    }
  }

  if (!root) {
    return (
      <div style="flex:1;min-height:0;display:flex;align-items:center;justify-content:center;padding:24px;color:var(--text-disabled);font-size:11px;text-align:center;line-height:1.5">
        Open a project chat to browse its files here.
      </div>
    )
  }

  return (
    <div style="flex:1;min-height:0;display:flex;flex-direction:column;overflow:hidden">
      <div style="flex:1;min-height:0;overflow-y:auto;overflow-x:hidden;padding:4px 0">
        <TreeChildren
          parent={root}
          depth={0}
          tree={tree}
          onOpenFile={openFile}
          onContextMenu={(e, entry) => setCtxMenu({ x: e.clientX, y: e.clientY, entry })}
          renamingPath={renamingPath}
          renameDraft={renameDraft}
          onRenameDraft={setRenameDraft}
          onRenameCommit={commitRename}
          onRenameCancel={cancelRename}
        />
      </div>
      {ctxMenu && (
        <TreeContextMenu
          x={ctxMenu.x}
          y={ctxMenu.y}
          entry={ctxMenu.entry}
          onClose={() => setCtxMenu(null)}
          onRename={() => startRename(ctxMenu.entry)}
          onRevealInExplorer={() => {
            // For files, `showItemInFolder` opens the parent dir with
            // the file highlighted. For directories, fall back to
            // `openFolder` which opens the dir itself in Finder/Explorer.
            if (ctxMenu.entry.isDir) {
              bridge.openFolder?.(ctxMenu.entry.path)
            } else {
              bridge.revealInExplorer?.(ctxMenu.entry.path)
            }
          }}
        />
      )}
    </div>
  )
}

function TreeContextMenu({ x, y, entry, onClose, onRename, onRevealInExplorer }: {
  x: number
  y: number
  entry: TreeEntry
  onClose: () => void
  onRename: () => void
  onRevealInExplorer: () => void
}): JSX.Element {
  useEffect(() => {
    function onDown(): void { onClose() }
    function onKey(e: KeyboardEvent): void { if (e.key === 'Escape') onClose() }
    window.addEventListener('mousedown', onDown)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('mousedown', onDown)
      window.removeEventListener('keydown', onKey)
    }
  }, [onClose])
  void entry
  return (
    <div
      class={tabStyles.ctxMenu}
      style={`left:${x}px;top:${y}px`}
      onMouseDown={(e: MouseEvent) => e.stopPropagation()}
    >
      <button
        type="button"
        class={tabStyles.ctxItem}
        onClick={() => { onRename(); onClose() }}
      >
        <i class="fas fa-pen" /> Rename
      </button>
      <button
        type="button"
        class={tabStyles.ctxItem}
        onClick={() => { onRevealInExplorer(); onClose() }}
      >
        <i class="fas fa-folder-open" /> {entry.isDir ? 'Open in Explorer' : 'Reveal in Explorer'}
      </button>
    </div>
  )
}
