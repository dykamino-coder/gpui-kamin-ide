// Thinking is drawn as ordinary message text with a lightbulb marker: no
// "Thinking" label, no collapsed one-line teaser, nothing to expand. The
// vnode tree is inspected directly (the suite has no DOM environment).
import { expect, it } from 'vitest'
import type { ContentBlock, JsonlEntryData } from '../../types/jsonl'
import { renderContentBlocks } from './ContentBlocks'
import { JsonlTextBlock } from './JsonlTextBlock'
import { ThinkingBlock, thoughtLabel } from './ThinkingBlock'
import { contentWithThinkingDuration } from './thinking-duration'
import { mergeAssistantEntriesByMessageId } from './utils'
import { mergeAssistantEntry } from '../../signals/merge-assistant'

interface VNodeLike { type: unknown; props: Record<string, unknown> & { children?: unknown } }

function kids(node: VNodeLike): VNodeLike[] {
  const c = node.props.children
  return (Array.isArray(c) ? c : [c]).filter((k): k is VNodeLike => !!k && typeof k === 'object')
}

const LONG = Array.from({ length: 400 }, (_, i) => `Synthetic reasoning line ${String(i)} with **markdown**.`).join('\n')

it('renders the full thinking text through the assistant text renderer, marked by a lightbulb', () => {
  const node = ThinkingBlock({ thinking: LONG }) as unknown as VNodeLike
  expect(node.props.class).toBe('block-thinking')
  const [mark, body, ...rest] = kids(node)
  expect(rest).toHaveLength(0)
  expect(mark!.type).toBe('i')
  expect(String(mark!.props.class)).toContain('fa-lightbulb')
  expect(mark!.props['aria-label']).toBe('Thinking')
  // Same component as assistant prose, whole text, no preview slicing.
  expect(body!.type).toBe(JsonlTextBlock)
  expect(body!.props.text).toBe(LONG)
  // No visible label and no toggle row.
  expect(kids(node).some((k) => typeof k.props.onClick === 'function')).toBe(false)
  expect(JSON.stringify(node.props.children)).not.toContain('thinking-toggle')
})

it('marks a signature-only thinking block with "thought N s" instead of drawing nothing (INC-2026-0098)', () => {
  const node = ThinkingBlock({ thinking: '', durationMs: 4400 }) as unknown as VNodeLike
  expect(node.props.class).toBe('block-thinking thinking-hidden')
  const [mark, note] = kids(node)
  expect(String(mark!.props.class)).toContain('fa-lightbulb')
  expect(note!.props.children).toBe('thought 4 s')
  const blank = ThinkingBlock({ thinking: ' \n\t' }) as unknown as VNodeLike
  expect(kids(blank)[1]!.props.children).toBe('thought')
  expect(thoughtLabel(200)).toBe('thought 1 s')
  // Still streaming: the text may just not have arrived yet.
  expect(ThinkingBlock({ thinking: '', isStreaming: true })).toBeNull()
})

it('keeps each thinking row\'s duration on its block through the message merges', () => {
  const rows = [
    { type: 'assistant', uuid: 'a', thinkingDurationMs: 7000, message: { id: 'm', content: [{ type: 'thinking', thinking: '', signature: 's' }] } },
    { type: 'assistant', uuid: 'b', message: { id: 'm', content: [{ type: 'text', text: 'answer' }] } },
  ] as unknown as JsonlEntryData[]
  const merged = mergeAssistantEntriesByMessageId(rows)
  expect(merged).toHaveLength(1)
  const blocks = contentWithThinkingDuration(merged[0]!)
  expect(blocks[0]!.durationMs).toBe(7000)
  const [first] = renderContentBlocks(blocks) as unknown as VNodeLike[]
  expect(first!.props.durationMs).toBe(7000)
  expect((rows[0]!.message!.content as ContentBlock[])[0]!.durationMs).toBeUndefined()
  const folded = mergeAssistantEntry(rows[0]!, rows[1]!)
  expect((folded.message!.content as ContentBlock[])[0]!.durationMs).toBe(7000)
})

it('streams only the active thinking block and keeps finished ones static', () => {
  const content = [
    { type: 'thinking', thinking: 'first thought' },
    { type: 'thinking', thinking: 'growing thought' },
  ] as ContentBlock[]
  const live = renderContentBlocks(content, undefined, 1) as unknown as VNodeLike[]
  expect(live.map((n) => n.type)).toEqual([ThinkingBlock, ThinkingBlock])
  expect(live.map((n) => n.props.isStreaming)).toEqual([false, true])
  const restored = renderContentBlocks(content) as unknown as VNodeLike[]
  expect(restored.map((n) => n.props.isStreaming)).toEqual([false, false])
  const streamed = ThinkingBlock({ thinking: 'growing thought', isStreaming: true }) as unknown as VNodeLike
  expect(kids(streamed)[1]!.props.isStreaming).toBe(true)
})

it('keeps the compact fallback for redacted thinking instead of a lightbulb row', () => {
  const content = [{ type: 'redacted_thinking', data: 'opaque' }] as unknown as ContentBlock[]
  const [node] = renderContentBlocks(content) as unknown as VNodeLike[]
  expect(node!.type).toBe('div')
  expect(node!.props.class).toBe('unknown-block')
})
