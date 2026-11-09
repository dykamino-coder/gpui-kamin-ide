import { expect, it, vi } from 'vitest'
import { handleServerMessage, type HandlerCtx } from './handle-server-message'
import { appendStreamingText } from '../../shared/streaming-delta'

it('accumulates streaming text and thinking in the host while every renderer is muted', () => {
  let cached: any
  const send = vi.fn()
  const ctx = {
    tabId: 'tab', window: { webContents: { send } }, isRendererMuted: () => true,
    setCachedStreamingEntry: (entry: any) => { cached = entry },
    appendCachedStreamingDelta: (delta: any) => appendStreamingText(cached, delta),
  } as unknown as HandlerCtx
  const message = (data: unknown) => handleServerMessage(data as any, ctx)
  message({ type: 'streaming:entry', entry: { __streaming: true, message: { id: 'turn', content: [{ type: 'text', text: 'before' }, { type: 'thinking', thinking: '' }] } } })
  message({ type: 'streaming:delta', msgId: 'turn', blockIdx: 0, appendText: ' while closed', kind: 'text' })
  message({ type: 'streaming:delta', msgId: 'turn', blockIdx: 1, appendText: 'reason', kind: 'thinking' })
  message({ type: 'streaming:delta', msgId: 'old', blockIdx: 0, appendText: 'wrong', kind: 'text' })
  expect(cached.message.content).toEqual([{ type: 'text', text: 'before while closed' }, { type: 'thinking', thinking: 'reason' }])
  expect(send).not.toHaveBeenCalled()
  message({ type: 'streaming:entry', entry: { __streaming: false, message: { id: 'turn' } } })
  expect(cached).toBeNull()
})
