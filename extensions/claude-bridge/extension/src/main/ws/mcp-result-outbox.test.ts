import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { McpResultOutbox, MCP_RESULT_LIMITS } from './mcp-result-outbox'

let outbox: McpResultOutbox
let frames: string[]
let open: boolean
beforeEach(() => {
  vi.useFakeTimers()
  frames = []
  open = false
  outbox = new McpResultOutbox(frame => { if (!open) return false; frames.push(frame); return true })
})
afterEach(() => { outbox.dispose(); vi.useRealTimers() })

describe('bounded acknowledged MCP outcomes', () => {
  it.each(['success', 'error', 'denial'])('retries %s after disconnect without settling before ack', async kind => {
    const outcome = kind === 'denial'
      ? { type: 'mcp:denied' as const, requestId: 'r', reason: 'denied' }
      : { type: 'mcp:response' as const, requestId: 'r', result: kind }
    let completed = false
    const result = outbox.deliver('s', outcome).then(() => { completed = true })
    await Promise.resolve()
    expect(frames).toHaveLength(0)
    expect(completed).toBe(false)
    open = true
    outbox.attach('s')
    expect(frames).toHaveLength(1)
    outbox.detach()
    vi.advanceTimersByTime(3000)
    expect(frames).toHaveLength(1)
    outbox.attach('s')
    expect(frames[1]).toBe(frames[0])
    // Ack for another tab/session cannot release this result.
    outbox.acknowledge('other', 'r', true)
    expect(completed).toBe(false)
    outbox.acknowledge('s', 'r', true)
    await result
    outbox.acknowledge('s', 'r', true)
    vi.advanceTimersByTime(3000)
    expect(frames).toHaveLength(2)
  })

  it('retains a frame if enqueue succeeds but the acknowledgement is lost', async () => {
    open = true
    outbox.attach('s')
    const result = outbox.deliver('s', { type: 'mcp:response', requestId: 'r', result: 'value' })
    vi.advanceTimersByTime(1000)
    expect(frames).toHaveLength(2)
    expect(frames[0]).toBe(frames[1])
    outbox.acknowledge('s', 'r', true)
    await result
  })

  it('reports deadline, session replacement and explicit teardown failures', async () => {
    const expired = expect(outbox.deliver('s', { type: 'mcp:response', requestId: 'a', result: 1 })).rejects.toThrow('timed out')
    vi.advanceTimersByTime(MCP_RESULT_LIMITS.lifetimeMs)
    await expired
    const replaced = expect(outbox.deliver('s', { type: 'mcp:response', requestId: 'b', result: 1 })).rejects.toThrow('ended server session')
    outbox.attach('new')
    await replaced
    const ended = expect(outbox.deliver('new', { type: 'mcp:response', requestId: 'c', result: 1 })).rejects.toThrow('cancelled')
    outbox.dispose()
    await ended
    expect(vi.getTimerCount()).toBe(0)
  })

  it('bounds count, frame bytes and total bytes, and frees capacity after ack', async () => {
    outbox.attach('s')
    const rejected: Promise<unknown>[] = []
    for (let i = 0; i < MCP_RESULT_LIMITS.count; i++) {
      rejected.push(outbox.deliver('s', { type: 'mcp:response', requestId: String(i), result: 1 }).catch(() => {}))
    }
    await expect(outbox.deliver('s', { type: 'mcp:response', requestId: 'overflow', result: 1 })).rejects.toThrow('capacity')
    outbox.dispose()
    await Promise.all(rejected)
    await expect(outbox.deliver('s', { type: 'mcp:response', requestId: 'big', result: 'x'.repeat(MCP_RESULT_LIMITS.frameBytes) })).rejects.toThrow('capacity')
    const large: Promise<unknown>[] = []
    for (let i = 0; i < 5; i++) large.push(outbox.deliver('s', { type: 'mcp:response', requestId: String(i), result: 'x'.repeat(3 * 1024 * 1024) }).catch(() => {}))
    await expect(outbox.deliver('s', { type: 'mcp:response', requestId: 'total', result: 'x'.repeat(3 * 1024 * 1024) })).rejects.toThrow('capacity')
    outbox.attach('s')
    for (let i = 0; i < 5; i++) outbox.acknowledge('s', String(i), true)
    await Promise.all(large)
    const next = outbox.deliver('s', { type: 'mcp:response', requestId: 'fresh', result: 1 })
    outbox.acknowledge('s', 'fresh', true)
    await next
  })

  it('reports server timeout/destruction rejection instead of local completion', async () => {
    outbox.attach('s')
    const result = expect(outbox.deliver('s', { type: 'mcp:response', requestId: 'r', result: 1 })).rejects.toThrow('no longer accepts')
    outbox.acknowledge('s', 'r', false)
    await result
  })
})
