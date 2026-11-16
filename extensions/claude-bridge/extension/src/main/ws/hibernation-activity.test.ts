import { describe, expect, it, vi } from 'vitest'

import { handleServerMessage, type HandlerCtx } from './handle-server-message'

/** Idle hibernation: the server reports a chat as asleep / waking / awake on
 *  the activity frame. The webview needs the state (it shows the wake and warms
 *  an asleep chat when the user types); the idle tracker and the session row
 *  must not read a sleeping or waking CLI as a finished turn. */
function ctxWithSpy() {
  const sent: { channel: string; args: unknown[] }[] = []
  const ctx = {
    tabId: 'tab-1',
    window: { webContents: { send: (channel: string, ...args: unknown[]) => sent.push({ channel, args }) } },
    trackActivityForIdle: vi.fn(),
    notifyActivity: vi.fn(),
  } as unknown as HandlerCtx
  return { ctx, sent }
}

describe('hibernation state on session:activity', () => {
  it.each(['asleep', 'waking', 'awake'] as const)('forwards %s to the webview without an idle edge', (hibernation) => {
    const { ctx, sent } = ctxWithSpy()
    handleServerMessage(
      {
        type: 'session:activity',
        sessionId: 's-1',
        hookDriven: true,
        rawTitle: '',
        isWorking: hibernation === 'waking',
        hibernation,
      } as never,
      ctx,
    )
    expect(sent).toEqual([
      {
        channel: 'session:activity',
        args: ['tab-1', expect.objectContaining({ hibernation, isWorking: hibernation === 'waking', hookDriven: true })],
      },
    ])
    expect(ctx.trackActivityForIdle).not.toHaveBeenCalled()
    expect(ctx.notifyActivity).not.toHaveBeenCalled()
  })

  it('keeps the idle tracking for ordinary CLI activity', () => {
    const { ctx, sent } = ctxWithSpy()
    handleServerMessage(
      { type: 'session:activity', sessionId: 's-1', hookDriven: true, rawTitle: '', isWorking: true } as never,
      ctx,
    )
    expect((sent[0]!.args[1] as Record<string, unknown>).hibernation).toBeUndefined()
    expect(ctx.trackActivityForIdle).toHaveBeenCalledWith('', true, true)
    expect(ctx.notifyActivity).toHaveBeenCalledWith(true, true)
  })
})
