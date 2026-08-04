// File tree extracted from SyncedDataPanel.tsx (Sprint 5 / Stage E1).
// Recursive, depth-aware row renderer with file/dir icons + size labels;
// click on a file row reports the absolute server path back to the parent.

import type { JSX } from 'preact'
import { formatSize, joinServerPath } from './sync-format'
import type { SelectedFile, TreeNode } from './sync-format'

interface Props {
  nodes: TreeNode[]
  depth: number
  rootAbs: string
  rootRel: string
  onSelect: (sel: SelectedFile) => void
  selectedAbs: string | null
}

export function SyncTree({ nodes, depth, rootAbs, rootRel, onSelect, selectedAbs }: Props): JSX.Element {
  return (
    <ul style={`list-style:none;margin:0;padding:0;padding-left:${depth === 0 ? 0 : 14}px`}>
      {nodes.map(n => {
        const absPath = joinServerPath([rootAbs, n.name])
        const relPath = rootRel ? `${rootRel}/${n.name}` : n.name
        const isSel = n.type === 'file' && selectedAbs === absPath
        return (
          <li key={n.name} style="padding:1px 0;font-size:11px;font-family:ui-monospace,Consolas,monospace">
            <span
              onClick={n.type === 'file' ? () => onSelect({ absPath, label: relPath }) : undefined}
              style={`display:inline-flex;align-items:center;gap:5px;padding:2px 6px;border-radius:var(--radius-xs);cursor:${n.type === 'file' ? 'pointer' : 'default'};color:${n.type === 'dir' ? 'var(--accent-primary)' : 'var(--text-primary)'};background:${isSel ? 'var(--bg-overlay)' : 'transparent'}`}
            >
              <i class={n.type === 'dir' ? 'fas fa-folder' : 'fas fa-file'} style={`color:${n.type === 'dir' ? 'var(--accent-yellow)' : 'var(--text-muted)'};font-size:9px;width:10px`} />
              <span>{n.name}</span>
              {n.type === 'file' && <span style="color:var(--text-muted);margin-left:4px">{formatSize(n.size)}</span>}
            </span>
            {n.children && n.children.length > 0 && (
              <SyncTree
                nodes={n.children}
                depth={depth + 1}
                rootAbs={absPath}
                rootRel={relPath}
                onSelect={onSelect}
                selectedAbs={selectedAbs}
              />
            )}
          </li>
        )
      })}
    </ul>
  )
}
