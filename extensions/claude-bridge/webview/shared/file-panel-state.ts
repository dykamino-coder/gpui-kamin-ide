/** Data projection with selected context, without CodeMirror instances or document snapshots. */
export interface FilePanelSelection {
  path: string
  startLine: number
  endLine: number
  text: string
  anchor?: number
  head?: number
}
export interface FilePanelTab {
  files: string[]
  active: string | null
  selection: FilePanelSelection | null
}
export interface FilePanelState {
  version: 1
  tabs: Record<string, FilePanelTab>
  previewModes: [string, 'code' | 'preview'][]
  attachActiveFile: boolean
}

const MAX_BYTES = 2 * 1024 * 1024
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid FilePanel state object')
  return value as Record<string, unknown>
}
function path(value: unknown): string {
  if (typeof value !== 'string' || value.length > 4096 || value.includes('\0')
    || !/^(?:\/|[a-z]:[\\/]|\\\\)/i.test(value)) throw new Error('Expected absolute FilePanel path')
  return value
}

/** Whitelist and bound host retention. Invalid frames never partially replace state. */
export function readFilePanelState(value: unknown): FilePanelState {
  const state = object(value)
  if (state.version !== 1 || typeof state.attachActiveFile !== 'boolean') throw new Error('Invalid FilePanel state version/toggle')
  const entries = Object.entries(object(state.tabs))
  if (entries.length > 64) throw new Error('FilePanel tab limit (64)')
  const tabs = Object.fromEntries(entries.map(([id, raw]) => {
    if (!id || id.length > 256) throw new Error('Invalid FilePanel chat identity')
    const tab = object(raw)
    if (!Array.isArray(tab.files) || tab.files.length > 128) throw new Error('FilePanel file limit (128 per chat)')
    const files = tab.files.map(path)
    if (new Set(files).size !== files.length) throw new Error('Duplicate FilePanel path')
    const active = tab.active === null ? null : path(tab.active)
    if (active !== null && !files.includes(active)) throw new Error('Active FilePanel path is not open')
    let selection: FilePanelSelection | null = null
    if (tab.selection !== null) {
      const s = object(tab.selection)
      if (s.path !== active || !Number.isSafeInteger(s.startLine) || !Number.isSafeInteger(s.endLine)
        || (s.startLine as number) < 1 || (s.endLine as number) < (s.startLine as number)
        || typeof s.text !== 'string') throw new Error('Invalid FilePanel selection')
      selection = { path: path(s.path), startLine: s.startLine as number, endLine: s.endLine as number, text: s.text }
      if (s.anchor !== undefined || s.head !== undefined) {
        if (!Number.isSafeInteger(s.anchor) || !Number.isSafeInteger(s.head)
          || (s.anchor as number) < 0 || (s.head as number) < 0) throw new Error('Invalid FilePanel cursor offsets')
        selection.anchor = s.anchor as number
        selection.head = s.head as number
      }
    }
    return [id, { files, active, selection }]
  }))
  if (!Array.isArray(state.previewModes) || state.previewModes.length > 256) throw new Error('FilePanel preview-mode limit (256)')
  const previewModes = state.previewModes.map((entry): [string, 'code' | 'preview'] => {
    if (!Array.isArray(entry) || entry.length !== 2 || !['code', 'preview'].includes(entry[1])) throw new Error('Invalid FilePanel preview mode')
    return [path(entry[0]), entry[1]]
  })
  if (new Set(previewModes.map(([p]) => p)).size !== previewModes.length) throw new Error('Duplicate FilePanel preview mode')
  const result: FilePanelState = { version: 1, tabs, previewModes, attachActiveFile: state.attachActiveFile }
  if (new TextEncoder().encode(JSON.stringify(result)).length > MAX_BYTES) throw new Error('FilePanel state limit (2 MiB)')
  return result
}
