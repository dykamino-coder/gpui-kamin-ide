// Right-hand viewer column extracted from SyncedDataPanel.tsx (Sprint 5
// / Stage E1). Renders an empty placeholder until a tree row is selected,
// then shows the file path/size header + markdown-or-pre body.

import type { JSX } from 'preact'
import { renderMarkdown } from '../../../utils/render-markdown'
import { formatSize } from './sync-format'
import type { FilePayload, SelectedFile } from './sync-format'

interface Props {
  selected: SelectedFile | null
  fileData: FilePayload | null
  fileLoading: boolean
  fileError: string | null
}

export function SyncFileViewer({ selected, fileData, fileLoading, fileError }: Props): JSX.Element {
  if (!selected) {
    return (
      <div style="flex:1;display:flex;align-items:center;justify-content:center;color:var(--text-muted);font-size:12px">
        Select a file from the tree to view its content
      </div>
    )
  }
  const isMarkdown = selected.absPath.toLowerCase().endsWith('.md')
  return (
    <>
      <div style="padding:8px 12px;border-bottom:1px solid var(--bg-surface);display:flex;align-items:center;gap:8px">
        <i class="fas fa-file-lines" style="color:var(--accent-primary);font-size:11px" />
        <span style="color:var(--text-primary);font-size:12px;font-family:ui-monospace,Consolas,monospace;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap" title={selected.absPath}>
          {selected.label}
        </span>
        {fileData && (
          <span style="color:var(--text-muted);font-size:11px">
            {formatSize(fileData.size)}{fileData.truncated ? ' · truncated' : ''}
          </span>
        )}
      </div>
      <div style="flex:1;overflow:auto;padding:10px 16px;color:var(--text-primary)">
        {fileLoading && <div style="color:var(--text-muted);font-size:12px">Loading…</div>}
        {fileError && <div style="color:var(--accent-red);font-size:12px">{fileError}</div>}
        {fileData && isMarkdown ? (
          <div
            class="markdown-body"
            style="font-size:13px;line-height:1.55"
            dangerouslySetInnerHTML={{ __html: renderMarkdown(fileData.content) }}
          />
        ) : fileData && (
          <pre style="margin:0;color:var(--text-primary);font-size:12px;font-family:ui-monospace,Consolas,monospace;white-space:pre-wrap;word-break:break-word">{fileData.content}</pre>
        )}
      </div>
    </>
  )
}
