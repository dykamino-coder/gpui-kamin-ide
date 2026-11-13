import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { BrowserWindow } from '@kaminide/host-compat'
import type { TabManager } from '../tab-manager'
import type { ConfigStore } from '../config/store'
import type { ConnectionConfig } from '../../shared/types'

const toast = vi.hoisted(() => vi.fn(async (..._args: unknown[]) => undefined))
vi.mock('vscode', () => ({ window: { showInformationMessage: toast } }))
vi.mock('../mcp/permission-manager', () => ({ PermissionManager: class {} }))
vi.mock('../mcp/tools/shell', () => ({ disposeBackgroundShells: vi.fn() }))

import { wireConnectionCallbacks } from '../../core-ipc'
import { reportDisplayedTab, clearDisplayedTab } from '../../displayed-tab'
import { ConnectionManager } from './connection-manager'
import { configureTransientNotice } from '../notifications/toast-window'

let manager: ConnectionManager
let active: string
const tabId = 'finishing-tab'
const activity = (working: boolean, hookDriven = true) => (manager as any).dispatchMessage({
  type: 'session:activity', hookDriven, isWorking: working, promptReady: !working,
})
const replayComplete = () => (manager as any).dispatchMessage({
  type: 'jsonl:status', status: 'watching', replayComplete: true,
})

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-11-13T08:00:00Z'))
  toast.mockClear()
  configureTransientNotice(toast)
  active = tabId
  reportDisplayedTab(tabId)
  const tabManager = {
    getActiveTabId: () => active,
    getTab: (id: string) => id === tabId ? { label: 'Background task' } : undefined,
  } as unknown as TabManager
  wireConnectionCallbacks(tabManager, {} as ConfigStore)
  manager = new ConnectionManager(
    { serverUrl: 'ws://synthetic', token: 'synthetic' } as ConnectionConfig,
    { webContents: { send: vi.fn() } } as unknown as BrowserWindow,
    tabId,
  )
  // Exercise the same settle guard armed by the real WebSocket open handler.
  ;(manager as any).idleTracker.armSettle()
})

afterEach(() => {
  manager.disconnect({ endSession: true })
  ConnectionManager.onSessionIdle = null
  configureTransientNotice(null)
  clearDisplayedTab()
  vi.useRealTimers()
})

function background(): void {
  active = 'other-tab'
  reportDisplayedTab(active)
  manager.setRendererMuted(true)
}

describe('background turn completion through the host notification path', () => {
  it('notifies exactly once when a new turn finishes during the attach/replay grace', () => {
    activity(false) // the CLI is ready; this is not cached in-flight work
    activity(true)
    background()
    vi.advanceTimersByTime(2_000)
    replayComplete() // first transcript discovery can coincide with turn end
    activity(false)
    vi.advanceTimersByTime(501)
    expect(toast).toHaveBeenCalledOnce()
    expect(toast.mock.calls[0]).toEqual([
      'Session finished — Tab «Background task» is ready.',
      { severity: 'info', durationMs: 8_000 }, 'Open',
    ])
    activity(false) // repeated Stop/snapshot must not notify again
    vi.advanceTimersByTime(10_000)
    expect(toast).toHaveBeenCalledOnce()
  })

  it('notifies exactly once after a quiet turn longer than one minute', () => {
    replayComplete()
    vi.advanceTimersByTime(5_000)
    activity(false)
    activity(true)
    background()
    vi.advanceTimersByTime(90_000)
    activity(false)
    vi.advanceTimersByTime(501)
    activity(false)
    vi.advanceTimersByTime(5_000)
    expect(toast).toHaveBeenCalledOnce()
  })

  it('notifies exactly once for the CLI OSC idle/working/done sequence without a Stop hook', () => {
    activity(false) // SessionStart; subsequent activity comes from CLI OSC
    activity(true, false)
    background()
    vi.advanceTimersByTime(2_000)
    replayComplete()
    activity(false, false)
    vi.advanceTimersByTime(3_999)
    expect(toast).not.toHaveBeenCalled()
    vi.advanceTimersByTime(2)
    activity(false, false)
    vi.advanceTimersByTime(10_000)
    expect(toast).toHaveBeenCalledOnce()
  })

  it('suppresses the cached working-to-idle blip on attach', () => {
    background()
    activity(true)
    replayComplete()
    activity(false)
    vi.advanceTimersByTime(10_000)
    expect(toast).not.toHaveBeenCalled()
  })

  it('still notifies for a fresh turn after consuming the attach blip', () => {
    background()
    activity(true)
    activity(false)
    vi.advanceTimersByTime(501)
    expect(toast).not.toHaveBeenCalled()
    activity(true)
    activity(false)
    vi.advanceTimersByTime(501)
    expect(toast).toHaveBeenCalledOnce()
  })

  it('does not mistake a short attach idle blip for the start of a fresh turn', () => {
    background()
    activity(true)
    activity(false)
    vi.advanceTimersByTime(100)
    activity(true)
    activity(false)
    vi.advanceTimersByTime(501)
    expect(toast).not.toHaveBeenCalled()
  })

  it('rearms replay suppression on reconnect', () => {
    activity(false)
    activity(true)
    background()
    ;(manager as any).idleTracker.armSettle()
    activity(false)
    vi.advanceTimersByTime(501)
    expect(toast).not.toHaveBeenCalled()
  })

  it('does not notify for the active, displayed chat', () => {
    activity(false)
    activity(true)
    activity(false)
    vi.advanceTimersByTime(5_000)
    expect(toast).not.toHaveBeenCalled()
  })
})
