import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { activeFiles, type FileEntry } from '../../signals/files'
import { useBridge } from '../../hooks/useBridge'
import { CardTabBar } from './CardTabBar'
import { EmptyState } from '../ui/EmptyState'
import { activeTabId } from '../../signals/tabs'
import { openFileInTab } from '../../signals/file-viewer'
import { filePanelVisible } from '../../signals/ui'
import { ProjectTree } from './project-tree/ProjectTree'

function basename(p: string): string {
  const m = p.match(/([^/\\]+)$/)
  return m ? m[1]! : p
}
function dirname(p: string): string {
  return p.slice(0, p.length - basename(p).length).replace(/[/\\]$/, '')
}
function formatRelative(iso: string | undefined): string {
  if (!iso) return ''
  const diff = Date.now() - Date.parse(iso)
  if (!Number.isFinite(diff)) return ''
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return 'now'
  if (mins < 60) return `${mins}m`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h`
  const days = Math.floor(hours / 24)
  return `${days}d`
}

type FileTab = 'files' | 'write' | 'read'

/** Files touched by the agent in the active tab — sorted by recency.
 *  Split into Write (Edit / Write / MultiEdit) and Read tabs so the
 *  user can scan the modified list without scrolling past dozens of
 *  read-only opens. Tab pinning is per-session local state — when
 *  the active tab changes, default goes back to "Write" since that's
 *  what users actually monitor. */
export function FilesList(): JSX.Element {
  const bridge = useBridge()
  const files = activeFiles.value
  const [tab, setTab] = useState<FileTab>('files')

  const writeOrEdit = files.filter(f => f.primaryOp !== 'read')
  const readOnly = files.filter(f => f.primaryOp === 'read')
  const listVisible = tab === 'write' ? writeOrEdit : tab === 'read' ? readOnly : []

  return (
    <div style="display:flex;flex-direction:column;height:100%">
      <CardTabBar
        tabs={[
          // "Files" first — the project browser (always-on, source of
          // truth for the workspace). Write/Read are the agent's
          // touched-files history.
          { id: 'files', label: 'Files', icon: 'fa-folder-tree', accent: 'var(--accent-primary)' },
          { id: 'write', label: 'Write', icon: 'fa-file-pen', accent: 'var(--accent-primary)', count: writeOrEdit.length },
          { id: 'read', label: 'Read', icon: 'fa-eye', accent: 'var(--text-secondary)', count: readOnly.length },
        ]}
        active={tab}
        onSelect={(id) => setTab(id as FileTab)}
      />
      {tab === 'files' ? (
        <ProjectTree />
      ) : (
        <div style="flex:1;overflow-y:auto;padding:var(--space-2) var(--space-2) var(--space-7)">
          {listVisible.length === 0 ? (
            <EmptyState
              icon="fa-file-pen"
              title={tab === 'write' ? 'No files modified yet.' : 'No files read yet.'}
              hint="Edit / Write / Read calls will list here."
            />
          ) : (
            listVisible.map(f => <FileRow key={f.path} file={f} bridge={bridge} />)
          )}
        </div>
      )}
    </div>
  )
}

function FileRow({ file, bridge }: { file: FileEntry; bridge: any }): JSX.Element {
  const style = OP_STYLES[file.primaryOp]
  function open(): void {
    // Click on a Read/Write entry → open in the file panel editor.
    // Auto-pop the file column if the user has it collapsed so the
    // newly-opened file is actually visible.
    const tab = activeTabId.value
    if (tab) {
      openFileInTab(tab, file.path)
      if (!filePanelVisible.value) {
        filePanelVisible.value = true
        try { bridge.setLayout?.({ filePanelVisible: true }) } catch { /* noop */ }
      }
    }
  }
  return (
    <div
      onClick={open}
      data-tooltip={file.path}
      style="display:flex;align-items:center;gap:var(--space-2);padding:4px var(--space-2);border-radius:var(--radius-sm);cursor:pointer;transition:background var(--transition-fast)"
      onMouseEnter={(e: any) => { e.currentTarget.style.background = 'var(--bg-surface)' }}
      onMouseLeave={(e: any) => { e.currentTarget.style.background = 'transparent' }}
    >
      <i class={style.icon} style={`color:${style.color};font-size:var(--fs-sm);flex-shrink:0;width:12px;text-align:center`} />
      <div style="flex:1;min-width:0">
        <div style="font-size:var(--fs-base);color:var(--text-primary);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-weight:500">
          {basename(file.path)}
        </div>
        <div style="font-size:var(--fs-10);color:var(--text-muted);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;direction:rtl;text-align:left">
          {dirname(file.path)}
        </div>
      </div>
      <div style="display:flex;flex-direction:column;align-items:flex-end;gap:2px;flex-shrink:0">
        {file.ops > 1 && (
          <span style={`font-size:var(--fs-xs);color:${style.color};background:${style.bg};padding:1px 6px;border-radius:999px;font-weight:600`}>
            ×{file.ops}
          </span>
        )}
        <span style="font-size:var(--fs-xs);color:var(--text-disabled);font-variant-numeric:tabular-nums">
          {formatRelative(file.lastAt)}
        </span>
      </div>
    </div>
  )
}

const OP_STYLES: Record<FileEntry['primaryOp'], { icon: string; color: string; bg: string }> = {
  write: { icon: 'fas fa-file-circle-plus', color: 'var(--accent-green)', bg: 'var(--tint-green-medium)' },
  multiedit: { icon: 'fas fa-file-pen', color: 'var(--accent-yellow)', bg: 'var(--tint-yellow-medium)' },
  edit: { icon: 'fas fa-pen', color: 'var(--accent-yellow)', bg: 'var(--tint-yellow-medium)' },
  read: { icon: 'fas fa-eye', color: 'var(--text-muted)', bg: 'var(--tint-muted-medium)' },
}
