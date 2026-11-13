import { beforeEach, describe, expect, it, vi } from 'vitest'

const show = vi.hoisted(() => vi.fn())
vi.mock('vscode', () => ({ window: { showInformationMessage: show } }))

describe('shared notification navigation', () => {
  beforeEach(() => {
    vi.resetModules()
    show.mockReset().mockResolvedValue('Open')
  })

  async function fixture() {
    const toast = await import('./toast-window')
    const activation = await import('../../session-activation')
    const route = vi.fn()
    toast.setToastRouteHandler(route)
    return { ...toast, ...activation, route }
  }
  const finished = { kind: 'finished' as const, tabId: 'finished-tab', title: 'Done', message: 'Ready' }
  const flush = () => Promise.resolve()

  it('routes finished Open to the existing browser chat channel without a native session hook', async () => {
    const f = await fixture()
    f.spawnToast(finished)
    await flush()
    expect(show).toHaveBeenCalledWith('Done — Ready', 'Open')
    expect(f.route).toHaveBeenCalledExactlyOnceWith({ tabId: 'finished-tab' })
  })

  it('preserves native session activation without a duplicate browser route', async () => {
    const f = await fixture()
    const activate = vi.fn()
    f.setSessionActivateHandler(activate)
    f.spawnToast(finished)
    await flush()
    expect(activate).toHaveBeenCalledExactlyOnceWith('finished-tab')
    expect(f.route).not.toHaveBeenCalled()
  })

  it('preserves question routing and its request identity even with native sessions installed', async () => {
    const f = await fixture()
    const activate = vi.fn()
    f.setSessionActivateHandler(activate)
    f.spawnToast({ ...finished, kind: 'question', requestId: 'question-request' })
    await flush()
    expect(f.route).toHaveBeenCalledExactlyOnceWith({ tabId: 'finished-tab', requestId: 'question-request' })
    expect(activate).not.toHaveBeenCalled()
  })

  it('does not navigate a dismissed notification', async () => {
    const f = await fixture()
    show.mockResolvedValue(undefined)
    f.spawnToast(finished)
    await flush()
    expect(f.route).not.toHaveBeenCalled()
  })

  it('shows finished as a self-closing notice when the host has the transient contract (BR-23)', async () => {
    const f = await fixture()
    const transient = vi.fn().mockResolvedValue('Open')
    f.configureTransientNotice(transient)
    f.spawnToast(finished)
    await flush(); await flush()
    expect(transient).toHaveBeenCalledExactlyOnceWith('Done — Ready', { severity: 'info', durationMs: f.FINISHED_TOAST_MS }, 'Open')
    expect(show).not.toHaveBeenCalled()
    expect(f.route).toHaveBeenCalledExactlyOnceWith({ tabId: 'finished-tab' })
  })

  it('keeps a question waiting for the user even with the transient contract', async () => {
    const f = await fixture()
    const transient = vi.fn().mockResolvedValue(undefined)
    f.configureTransientNotice(transient)
    f.spawnToast({ ...finished, kind: 'question', requestId: 'q' })
    await flush()
    expect(transient).not.toHaveBeenCalled()
    expect(show).toHaveBeenCalledWith('Done — Ready', 'Open')
  })

  it.each(['dismiss', 'timeout'])('does not navigate when the transient host resolves %s', async () => {
    const f = await fixture()
    f.configureTransientNotice(vi.fn().mockResolvedValue(undefined))
    f.spawnToast(finished)
    await flush(); await flush()
    expect(f.route).not.toHaveBeenCalled()
  })

  it('keeps a timed webview notice duration on the transient path', async () => {
    const f = await fixture()
    const transient = vi.fn().mockResolvedValue(undefined)
    f.configureTransientNotice(transient)
    f.showTimedNotice('info', 'Anthropic busy', 6000)
    expect(transient).toHaveBeenCalledExactlyOnceWith('Anthropic busy', { severity: 'info', durationMs: 6000 })
  })

  it('keeps the existing enabled gate and explicit bypass', async () => {
    const f = await fixture()
    f.configureToastGate({ notificationsEnabled: () => false })
    f.spawnToast(finished)
    expect(show).not.toHaveBeenCalled()
    f.spawnToast(finished, { bypassEnabledGate: true })
    await flush()
    expect(f.route).toHaveBeenCalledExactlyOnceWith({ tabId: 'finished-tab' })
  })
})
