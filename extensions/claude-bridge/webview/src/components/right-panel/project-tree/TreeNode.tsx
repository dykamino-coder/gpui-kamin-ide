import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { iconForEntry } from './file-icon'
import type { TreeEntry, UseFileTreeHandle } from './useFileTree'

interface TreeNodeProps {
  entry: TreeEntry
  depth: number
  tree: UseFileTreeHandle
  /** Called on left-click of a file. Folders use the toggle directly. */
  onOpenFile: (path: string) => void
  /** Right-click → opens the context menu. Lifted up so the menu can
   *  live at the panel level (single instance, easy positioning). */
  onContextMenu: (e: MouseEvent, entry: TreeEntry) => void
  /** Inline-rename target (matches `entry.path` while the user is
   *  renaming this node). Lifted up so the panel owns the input state
   *  and can dismiss it on outside-click. */
  renamingPath: string | null
  renameDraft: string
  onRenameDraft: (s: string) => void
  onRenameCommit: () => void
  onRenameCancel: () => void
}

/** Single row of the file tree. Stays a thin presentational unit so
 *  future DnD can attach pointer/HTML5 listeners on the wrapper without
 *  touching the recursion / data plumbing. Each row carries `data-path`
 *  + `data-is-dir` attrs so a sibling DnD hook can hit-test by closest
 *  ancestor. */
export function TreeNode({
  entry, depth, tree, onOpenFile, onContextMenu,
  renamingPath, renameDraft, onRenameDraft, onRenameCommit, onRenameCancel,
}: TreeNodeProps): JSX.Element {
  const expanded = tree.isExpanded(entry.path)
  const focused = tree.focused === entry.path
  const icon = iconForEntry(entry.name, entry.isDir, expanded)
  const isRenaming = renamingPath === entry.path
  const [hover, setHover] = useState(false)

  function onRowClick(): void {
    tree.setFocused(entry.path)
    if (entry.isDir) {
      tree.toggle(entry.path)
    } else {
      onOpenFile(entry.path)
    }
  }

  return (
    <div
      data-path={entry.path}
      data-is-dir={entry.isDir ? '1' : '0'}
      // stopPropagation prevents the right-click from bubbling up
      // through ancestor TreeNode wrappers — without it, a click on a
      // file would fire onContextMenu first for the file, then again
      // for each parent folder up the chain, and the LAST handler
      // (the root folder) would win, opening rename on the wrong row.
      onContextMenu={(e: any) => { e.preventDefault(); e.stopPropagation(); onContextMenu(e, entry) }}
    >
      <div
        onClick={onRowClick}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        title={entry.path}
        style={`
          display:flex;align-items:center;gap:6px;
          padding:4px 12px 4px ${depth * 12 + 12}px;
          border-radius:var(--radius-xs);cursor:pointer;
          background:${focused ? 'color-mix(in srgb, var(--accent-primary) 14%, transparent)' : (hover ? 'var(--bg-surface)' : 'transparent')};
          color:${focused ? 'var(--accent-primary)' : 'var(--text-secondary)'};
          font-family:inherit;
          font-size:var(--fs-base);
          user-select:none;
        `}
      >
        {/* Caret column — kept even for files so file/dir rows align. */}
        <span style="flex-shrink:0;width:10px;display:inline-flex;align-items:center;justify-content:center;color:var(--text-muted)">
          {entry.isDir && (
            <i class={expanded ? 'fas fa-chevron-down' : 'fas fa-chevron-right'} style="font-size:9px" />
          )}
        </span>
        <i class={icon.cls} style={`font-size:12px;color:${icon.color};flex-shrink:0;width:14px;text-align:center`} />
        {isRenaming ? (
          <input
            type="text"
            value={renameDraft}
            autoFocus
            onClick={(e: MouseEvent) => e.stopPropagation()}
            onInput={(e: any) => onRenameDraft((e.currentTarget as HTMLInputElement).value)}
            onKeyDown={(e: KeyboardEvent) => {
              if (e.key === 'Enter') { e.preventDefault(); onRenameCommit() }
              else if (e.key === 'Escape') { e.preventDefault(); onRenameCancel() }
            }}
            onBlur={() => onRenameCommit()}
            style="flex:1;min-width:0;padding:1px 4px;border:1px solid var(--accent-primary);border-radius:var(--radius-xs);background:var(--bg-base);color:var(--text-primary);font-family:inherit;font-size:var(--fs-base);outline:none"
          />
        ) : (
          <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0">
            {entry.name}
          </span>
        )}
      </div>
      {entry.isDir && expanded && (
        <TreeChildren
          parent={entry.path}
          depth={depth + 1}
          tree={tree}
          onOpenFile={onOpenFile}
          onContextMenu={onContextMenu}
          renamingPath={renamingPath}
          renameDraft={renameDraft}
          onRenameDraft={onRenameDraft}
          onRenameCommit={onRenameCommit}
          onRenameCancel={onRenameCancel}
        />
      )}
    </div>
  )
}

/** Renders the children of an expanded directory. Separate component so
 *  recursion stays readable + the loading state can show a placeholder
 *  without polluting TreeNode. */
export function TreeChildren({
  parent, depth, tree, onOpenFile, onContextMenu,
  renamingPath, renameDraft, onRenameDraft, onRenameCommit, onRenameCancel,
}: {
  parent: string
  depth: number
  tree: UseFileTreeHandle
  onOpenFile: (path: string) => void
  onContextMenu: (e: MouseEvent, entry: TreeEntry) => void
  renamingPath: string | null
  renameDraft: string
  onRenameDraft: (s: string) => void
  onRenameCommit: () => void
  onRenameCancel: () => void
}): JSX.Element {
  const children = tree.childrenOf(parent)
  if (children === undefined) {
    return (
      <div style={`padding:2px 6px 2px ${depth * 12 + 30}px;font-size:11px;color:var(--text-muted);font-style:italic`}>
        loading…
      </div>
    )
  }
  if (children.length === 0) {
    return (
      <div style={`padding:2px 6px 2px ${depth * 12 + 30}px;font-size:11px;color:var(--text-disabled);font-style:italic`}>
        empty
      </div>
    )
  }
  return (
    <>
      {children.map(c => (
        <TreeNode
          key={c.path}
          entry={c}
          depth={depth}
          tree={tree}
          onOpenFile={onOpenFile}
          onContextMenu={onContextMenu}
          renamingPath={renamingPath}
          renameDraft={renameDraft}
          onRenameDraft={onRenameDraft}
          onRenameCommit={onRenameCommit}
          onRenameCancel={onRenameCancel}
        />
      ))}
    </>
  )
}
