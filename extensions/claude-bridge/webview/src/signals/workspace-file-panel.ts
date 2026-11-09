import { batch, effect } from '@preact/signals'
import { readFilePanelState, type FilePanelState } from '../../shared/file-panel-state'
import { vscodeApi } from '../lib/webview-api'
import { activeTabId } from './tabs'
import { activeFileByTab, activeSelectionByTab, attachActiveFile, getOpenFiles,
  openFileInTab, openFilesByTab, previewModeByPath } from './file-viewer'

/** Install before sync. Browser signals are a projection of the retained logical view. */
export function installWorkspaceFilePanel(): () => void {
  let ready = false
  let applying = false
  let lastSent = ''
  const stop = effect(() => {
    const files = openFilesByTab.value
    const active = activeFileByTab.value
    const selections = activeSelectionByTab.value
    const modes = previewModeByPath.value
    const attach = attachActiveFile.value
    if (!ready || applying) return
    const state: FilePanelState = { version: 1, tabs: Object.fromEntries([...files].map(([id, list]) => {
      const paths = list.map(f => f.path)
      const selected = active.get(id)
      const path = selected && paths.includes(selected) ? selected : null
      const selection = selections.get(id)
      return [id, { files: paths, active: path, selection: selection?.path === path ? selection : null }]
    })), previewModes: [...modes], attachActiveFile: attach }
    try {
      const validated = readFilePanelState(state)
      const serialized = JSON.stringify(validated)
      if (serialized === lastSent) return
      vscodeApi.postMessage({ kind: 'send', channel: 'workspace:file-panel', args: [validated] })
      lastSent = serialized
    } catch { console.warn('FilePanel state could not reach the workspace host; reconnect before closing this page.') }
  })
  const off = vscodeApi.onHostMessage?.((frame: unknown) => {
    const event = frame as { kind?: string; channel?: string; args?: { files?: unknown; panel?: unknown }[] } | null
    if (event?.kind !== 'event' || event.channel !== 'workspace:files-state') return
    const retained = event.args?.[0]
    applying = true
    try {
      batch(() => {
        if (retained?.panel !== undefined) {
          const state = readFilePanelState(retained.panel)
          // Retain transient mtime/dirty indicators for mounted editors. They
          // are refreshed from disk and are not file content or host drafts.
          openFilesByTab.value = new Map(Object.entries(state.tabs).map(([id, tab]) => [id, tab.files.map(path =>
            getOpenFiles(id).find(f => f.path === path) ?? { path, mtimeMs: 0, dirty: false })]))
          activeFileByTab.value = new Map(Object.entries(state.tabs).map(([id, tab]) => [id, tab.active]))
          activeSelectionByTab.value = new Map(Object.entries(state.tabs).map(([id, tab]) => [id, tab.selection]))
          previewModeByPath.value = new Map(state.previewModes)
          attachActiveFile.value = state.attachActiveFile
          lastSent = JSON.stringify(state)
        } else {
          // Compatibility with views retained before the control-state upgrade.
          const tab = activeTabId.peek()
          if (tab && Array.isArray(retained?.files) && getOpenFiles(tab).length === 0) {
            for (const path of retained.files) if (typeof path === 'string') openFileInTab(tab, path)
          }
        }
      })
      ready = true
    } finally { applying = false }
  }) ?? (() => {})
  return () => { stop(); off() }
}
