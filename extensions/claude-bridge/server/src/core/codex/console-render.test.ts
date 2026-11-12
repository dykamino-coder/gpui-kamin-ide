import { describe, expect, it } from 'vitest'
import type { AgentEvent } from '../../shared/agent-events'
import { CodexConsoleRenderer, consoleBanner, plainText } from './console-render'

const at = Date.UTC(2026, 10, 12, 10, 0, 0)
const ev = (e: Record<string, unknown>) => ({ threadId: 't', at, ...e }) as AgentEvent

const strip = (s: string) => s.replace(/\x1b\[[0-9;]*m/g, '')

describe('codex console renderer', () => {
  it('prints a banner with the thread and model', () => {
    const text = strip(consoleBanner({ threadId: 'th-1', model: 'gpt-6-luna', resumed: true }))
    expect(text).toContain('ChatGPT (codex) resumed thread th-1')
    expect(text).toContain('model gpt-6-luna')
    expect(text).not.toMatch(/[^\r]\n/)
  })

  it('streams a reply on one line and does not repeat it when the final text arrives', () => {
    const r = new CodexConsoleRenderer()
    const out = [
      r.render(ev({ kind: 'turn.started', turnId: 'u' })),
      r.render(ev({ kind: 'message.user', id: 'm0', turnId: 'u', text: 'hi' })),
      r.render(ev({ kind: 'message.assistant.delta', id: 'm1', turnId: 'u', delta: 'Hel' })),
      r.render(ev({ kind: 'message.assistant.delta', id: 'm1', turnId: 'u', delta: 'lo\nworld' })),
      r.render(ev({ kind: 'message.assistant', id: 'm1', turnId: 'u', text: 'Hello\nworld' })),
      r.render(ev({ kind: 'turn.completed', turnId: 'u', status: 'completed', durationMs: 1500 })),
    ]
      .map(strip)
      .join('')
    expect(out).toBe(
      '10:00:00 ── turn started\r\n' +
        '10:00:00 you hi\r\n' +
        '10:00:00 assistant Hello\r\nworld\r\n' +
        '10:00:00 ── turn completed in 1.5 s\r\n',
    )
  })

  it('prints a reply that did not stream, tool calls with a short result, and errors', () => {
    const r = new CodexConsoleRenderer()
    const tool = { source: 'mcp', server: 'bridge', name: 'Read' }
    const out = [
      r.render(ev({ kind: 'message.assistant', id: 'm2', turnId: 'u', text: 'done' })),
      r.render(ev({ kind: 'tool.started', id: 'c1', turnId: 'u', tool, input: { file_path: '/a' } })),
      r.render(
        ev({
          kind: 'tool.completed',
          id: 'c1',
          turnId: 'u',
          tool,
          input: {},
          status: 'completed',
          output: [{ type: 'text', text: 'x'.repeat(1000) }],
        }),
      ),
      r.render(ev({ kind: 'turn.completed', turnId: 'u', status: 'failed', error: { message: 'HTTP 407' } })),
    ]
      .map(strip)
      .join('')
    expect(out).toContain('assistant done\r\n')
    expect(out).toContain('→ bridge.Read {"file_path":"/a"}')
    expect(out).toContain('← bridge.Read completed')
    expect(out).toContain('… (1000 chars)')
    expect(out).toContain('── turn failed: HTTP 407')
  })

  it('never passes terminal control sequences from the engine through', () => {
    expect(plainText('a\x1b]0;evil\x07b\x1b[2Jc\r\nd\te')).toBe('a]0;evilb[2Jc\nd\te')
    const r = new CodexConsoleRenderer()
    const out = r.render(ev({ kind: 'message.assistant', id: 'm', turnId: 'u', text: '\x1b]0;title\x07hi' }))
    expect(out).not.toContain('\x07')
    expect(out).not.toContain('\x1b]')
  })
})
