// Thinking is drawn as ordinary message text with a lightbulb marker: no
// "Thinking" label, no collapsed one-line teaser, nothing to expand. The
// vnode tree is inspected directly (the suite has no DOM environment).
import { expect, it } from 'vitest'
import type { ContentBlock } from '../../types/jsonl'
import { renderContentBlocks } from './ContentBlocks'
import { JsonlTextBlock } from './JsonlTextBlock'
import { ThinkingBlock } from './ThinkingBlock'

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

it('draws nothing for empty or whitespace-only thinking (signature-only block)', () => {
  expect(ThinkingBlock({ thinking: '' })).toBeNull()
  expect(ThinkingBlock({ thinking: ' \n\t' })).toBeNull()
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
