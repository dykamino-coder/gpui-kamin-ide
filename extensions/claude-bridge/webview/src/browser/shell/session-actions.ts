import { switchTabLocal, tabs } from '../../signals/tabs'
import { activeCustomizePanel, sidebarMode } from '../../signals/ui'
import { showToast } from '../../signals/toasts'

export function activateSession(id: string): void {
  sidebarMode.value = 'sessions'; activeCustomizePanel.value = null
  switchTabLocal(id); window.kaminBridge.switchTab(id)
}
export async function newSession(folder: boolean): Promise<void> {
  try {
    const bridge = window.kaminBridge
    const cwd = folder ? await bridge.selectFolder() : ''
    if (folder && !cwd) return
    const config = await bridge.getConfig()
    const id = await bridge.createTab({ ...config, cwd: cwd || '' })
    if (id) activateSession(id)
  } catch { showToast({ type: 'error', title: 'Session could not be opened', message: 'Check the connection and workspace folder.' }) }
}
export async function resumeSession(conversationId: string, cwd: string, label: string): Promise<void> {
  const open = tabs.value.find(t => t.conversationId === conversationId)
  if (open) { activateSession(open.id); return }
  try {
    const config = await window.kaminBridge.getConfig()
    const id = await window.kaminBridge.resumeSession({ ...config, cwd, conversationId })
    if (id) activateSession(id)
  } catch { showToast({ type: 'error', title: 'Session could not be resumed', message: 'Check the connection and try again.' }) }
}
export function renameSession(id: string): void {
  activateSession(id)
  // Same CLI rename flow as the native host; title events update every view.
  window.kaminBridge.submitText(id, '/rename')
}
export function closeSession(id: string): void {
  window.kaminBridge.closeTab(id)
}
