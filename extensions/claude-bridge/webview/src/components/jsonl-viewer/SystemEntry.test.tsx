// The divider where a chat moved to the other agent (server writes a system
// `engine_switch` entry into the conversation's transcript). The vnode tree is
// inspected directly (the suite has no DOM environment).
import { afterEach, expect, it } from 'vitest'
import type { JsonlEntryData } from '../../types/jsonl'
import { codexCatalog } from '../../signals/agent'
import { SystemEntry } from './SystemEntry'
import { systemEntryDraws } from './entry-draws'

interface VNodeLike { type: unknown; props: Record<string, unknown> & { children?: unknown } }

function render(node: unknown): VNodeLike {
  let n = node as VNodeLike
  while (typeof n.type === 'function') n = (n.type as (p: unknown) => VNodeLike)(n.props)
  return n
}
function text(node: unknown): string {
  if (node === null || node === undefined || typeof node === 'boolean') return ''
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(text).join('')
  const n = node as VNodeLike
  if (typeof n.type === 'function') return ''
  return text(n.props.children)
}

const marker = (model: string, engine: string): JsonlEntryData =>
  ({ type: 'system', subtype: 'engine_switch', uuid: 'm1', engine, model, content: `→ ${model}` }) as unknown as JsonlEntryData

afterEach(() => {
  codexCatalog.value = null
})

it('draws the switch with the model name the selector shows', () => {
  const claude = render(SystemEntry({ entry: marker('claude-sonnet-5-5', 'claude') }))
  expect(claude.props['data-engine-switch']).toBe('claude')
  expect(text(claude)).toContain('→ Sonnet 5.5')

  codexCatalog.value = {
    models: [{ model: 'gpt-6.1-sol', displayName: 'GPT-6.1-Sol', description: '', isDefault: true }],
  } as never
  expect(text(render(SystemEntry({ entry: marker('gpt-6.1-sol', 'codex') })))).toContain('→ GPT-6.1-Sol')
})

it('falls back to the model id the server wrote, and counts as a drawn row', () => {
  const entry = marker('gpt-unknown', 'codex')
  expect(text(render(SystemEntry({ entry })))).toContain('→ gpt-unknown')
  expect(systemEntryDraws(entry)).toBe(true)
})
