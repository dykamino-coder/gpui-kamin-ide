import assert from 'node:assert/strict'
import { test } from 'node:test'
import { parseClientFrame } from '../src/protocol.js'

test('existing invoke/send shapes round trip without accepting host frames or invalid identities', () => {
  const invoke = { kind: 'invoke', id: 1, generation: 'document-a', channel: 'get-config', args: [] }
  assert.deepEqual(parseClientFrame(JSON.stringify(invoke)), invoke)
  const send = { kind: 'send', channel: 'pty-submit-text', args: ['tab', 'hello'] }
  assert.deepEqual(parseClientFrame(JSON.stringify(send)), send)
  for (const invalid of [
    null, [], { ...invoke, kind: 'event' }, { ...invoke, kind: 'invoke-reply' },
    { ...invoke, id: 0 }, { ...invoke, id: 1.5 }, { ...invoke, id: Number.MAX_SAFE_INTEGER + 1 },
    { ...invoke, generation: '' }, { ...invoke, channel: '' }, { ...invoke, args: {} },
    { ...invoke, args: Array(33).fill(null) }, { ...invoke, channel: 'a'.repeat(257) },
  ]) assert.throws(() => parseClientFrame(JSON.stringify(invalid)))
  const diagnostic = { kind: 'invoke-diagnostic', id: 0, channel: '', generation: 'doc', stage: 'renderer-ended' }
  assert.deepEqual(parseClientFrame(JSON.stringify(diagnostic)), diagnostic)
  assert.throws(() => parseClientFrame(JSON.stringify({ ...diagnostic, stage: 'invented' })))
})
