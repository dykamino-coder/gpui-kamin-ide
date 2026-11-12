import { beforeEach, describe, expect, it } from 'vitest'
import { resetTracesForTests, traceAnswered, traceEnd, traceSent, traceStart } from './mcp-trace'

beforeEach(() => resetTracesForTests())

describe('MCP call trace', () => {
  it('splits a call into server, transport, client and tool time', () => {
    traceStart('r', 1000)
    traceSent('r', 1012)
    traceAnswered('r', { queuedMs: 3, toolMs: 50, totalMs: 55 }, 1090)
    expect(traceEnd('r', 1094)).toEqual({
      serverBeforeMs: 12,
      transportMs: 23,
      clientQueuedMs: 3,
      toolMs: 50,
      clientAfterMs: 2,
      serverAfterMs: 4,
    })
    expect(traceEnd('r')).toBeUndefined()
  })

  it('reports nothing for a client host without timing or a malformed one', () => {
    traceStart('old', 0)
    traceSent('old', 1)
    traceAnswered('old', undefined, 5)
    expect(traceEnd('old', 6)).toBeUndefined()
    traceStart('bad', 0)
    traceSent('bad', 1)
    traceAnswered('bad', { queuedMs: -1, toolMs: 'x', totalMs: 2 }, 5)
    expect(traceEnd('bad', 6)).toBeUndefined()
  })

  it('keeps the first send and answer of a retried frame', () => {
    traceStart('r', 0)
    traceSent('r', 10)
    traceSent('r', 1010)
    traceAnswered('r', { queuedMs: 0, toolMs: 5, totalMs: 5 }, 30)
    traceAnswered('r', { queuedMs: 0, toolMs: 5, totalMs: 5 }, 2000)
    expect(traceEnd('r', 31)).toMatchObject({ serverBeforeMs: 10, transportMs: 15, serverAfterMs: 1 })
  })

  it('ignores stamps for calls nobody traces and stays bounded', () => {
    traceSent('none')
    traceAnswered('none', { queuedMs: 0, toolMs: 0, totalMs: 0 })
    expect(traceEnd('none')).toBeUndefined()
    for (let i = 0; i < 2000; i++) traceStart(`t${i}`, i)
    traceSent('t0', 1)
    traceAnswered('t0', { queuedMs: 0, toolMs: 0, totalMs: 0 }, 2)
    expect(traceEnd('t0', 3)).toBeUndefined()
  })
})
