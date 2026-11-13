import { describe, expect, it } from 'vitest'
import { readCapped } from './web'

const body = (bytes: Buffer): Response => new Response(new ReadableStream({ start(c) { c.enqueue(bytes); c.close() } }))

describe('WebFetch body cap', () => {
  it('reports a cut body and never ends in a broken character', async () => {
    const text = 'ж'.repeat(10) // 20 bytes
    const cut = await readCapped(body(Buffer.from(text)), 7)
    expect(cut.truncated).toBe(true)
    expect(cut.text).toBe('жжж')
    const whole = await readCapped(body(Buffer.from('short')), 1024)
    expect(whole).toEqual({ text: 'short', truncated: false })
  })
})