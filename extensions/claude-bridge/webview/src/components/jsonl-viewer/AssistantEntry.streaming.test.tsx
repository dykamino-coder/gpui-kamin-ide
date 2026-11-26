// A streaming answer that quotes Bridge's hidden history block must not show
// the block while its closing tag has not arrived yet. The vnode tree is
// inspected directly (the suite has no DOM environment).
import { expect, it } from 'vitest'
import type { JsonlEntryData } from '../../types/jsonl'
import { AssistantEntry } from './AssistantEntry'
import { stripBridgeContext } from '../../utils/bridge-context'
import { withoutBridgeContextWhileStreaming } from '../../../shared/bridge-context'

const block = '<bridge-context>\nBridge note\n<conversation-history>\nUser: SECRET-HISTORY\n</conversation-history>\n</bridge-context>'
const answer = `Your message was:\n${block}\nDone.`

/** Every string a rendered row carries in its props (texts, copy payloads). */
function strings(node: unknown, out: string[] = []): string[] {
  if (typeof node === 'string') out.push(node)
  else if (Array.isArray(node)) for (const n of node) strings(n, out)
  else if (node && typeof node === 'object' && 'props' in node) {
    for (const v of Object.values((node as { props: Record<string, unknown> }).props)) strings(v, out)
  }
  return out
}

function stub(text: string): JsonlEntryData {
  return {
    type: 'assistant',
    uuid: 'stream-msg_1',
    __streaming: true,
    message: { id: 'msg_1', role: 'assistant', content: [{ type: 'text', text }] },
  } as unknown as JsonlEntryData
}

it('never shows the hidden block at any point of the stream', () => {
  for (let n = 1; n <= answer.length; n++) {
    const entry = stub(answer.slice(0, n))
    const shown = strings(AssistantEntry({ entry, toolResults: new Map() })).join('\n')
    expect(shown, `prefix ${String(n)}`).not.toContain('SECRET-HISTORY')
    expect(shown, `prefix ${String(n)}`).not.toContain('<bridge-c')
    // The stub keeps its raw text: deltas go on appending to it.
    expect((entry.message!.content as Array<{ text: string }>)[0]!.text).toBe(answer.slice(0, n))
  }
})

it('shows the text around the block once it is closed, as the stored answer does', () => {
  const settled = stub(answer)
  delete (settled as { __streaming?: unknown }).__streaming
  stripBridgeContext(settled)
  const stored = (settled.message!.content as Array<{ text: string }>)[0]!.text
  expect(withoutBridgeContextWhileStreaming(answer)).toBe(stored)
  expect(stored).not.toContain('SECRET-HISTORY')
  expect(stored.startsWith('Your message was:') && stored.endsWith('Done.')).toBe(true)
})

it('leaves an ordinary streaming answer untouched, without a copy per frame', () => {
  const entry = stub('a < b and 2 > 1')
  const text = strings(AssistantEntry({ entry, toolResults: new Map() })).join('\n')
  expect(text).toContain('a < b and 2 > 1')
  expect(withoutBridgeContextWhileStreaming('plain text')).toBe('plain text')
})
