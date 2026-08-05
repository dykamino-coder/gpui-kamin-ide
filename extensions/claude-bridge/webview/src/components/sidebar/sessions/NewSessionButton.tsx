import type { JSX } from 'preact'
import { sidebarMode, activeCustomizePanel } from '../../../signals/ui'

declare const kaminBridge: any

function leaveCustomize(): void {
  if (sidebarMode.value === 'customize') {
    sidebarMode.value = 'sessions'
    activeCustomizePanel.value = null
  }
}

async function handleNewSession(): Promise<void> {
  const bridge = (window as any).kaminBridge
  if (!bridge) return
  const folder = await bridge.selectFolder()
  if (!folder) return
  const config = await bridge.getConfig()
  if (!config.serverUrl || !config.token) return
  await bridge.createTab({ serverUrl: config.serverUrl, token: config.token, cwd: folder })
  leaveCustomize()
}

async function handleNewSessionNoFolder(): Promise<void> {
  const bridge = (window as any).kaminBridge
  if (!bridge) return
  const config = await bridge.getConfig()
  if (!config.serverUrl || !config.token) return
  // Explicit empty cwd — the main process skips the userCwd fallback when
  // the field is present, so the session truly starts "no folder" instead
  // of inheriting the last active project directory.
  await bridge.createTab({ serverUrl: config.serverUrl, token: config.token, cwd: '' })
  leaveCustomize()
}

export function NewSessionButton(): JSX.Element {
  return (
    <>
      <button
        class="sidebar-action"
        onClick={handleNewSessionNoFolder}
        type="button"
        data-tooltip="Start without picking a project folder"
        style="opacity:0.85"
      >
        <i class="fas fa-circle-plus" />
        New session (no folder)
      </button>
      <button class="sidebar-action" onClick={handleNewSession} type="button">
        <i class="fas fa-circle-plus" />
        New session
      </button>
    </>
  )
}
