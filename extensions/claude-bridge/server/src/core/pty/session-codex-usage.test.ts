import { describe, expect, it } from 'vitest'
import { codexPlanUsage, isCompactCommand, modelLine, sessionContextFor, startedContext } from './session-codex'
import type { CodexRateLimits } from '../../shared/codex-rate-limits'

const limits = (primary: number | null, secondary: number | null): CodexRateLimits => ({
  buckets: [
    {
      id: 'codex',
      name: null,
      primary:
        primary === null ? null : { usedPercent: primary, windowMinutes: 300, resetsAt: Date.UTC(2026, 10, 20, 3) },
      secondary:
        secondary === null
          ? null
          : { usedPercent: secondary, windowMinutes: 7 * 24 * 60, resetsAt: Date.UTC(2026, 10, 24) },
      reached: null,
    },
  ],
  credits: null,
  planType: 'plus',
  usageAllowed: true,
  readAt: Date.UTC(2026, 10, 19, 20),
})

describe('codex plan windows for the chat status row', () => {
  it('maps the account windows to a codex-tagged session:usage snapshot', () => {
    expect(codexPlanUsage(limits(12, 40), 'gpt-5.5', 'high')).toEqual({
      windows: [
        { id: 'five_hour', label: '5-hour limit', percent: 12, resetsAt: '2026-11-20T03:00:00.000Z' },
        { id: 'seven_day', label: 'Weekly limit', percent: 40, resetsAt: '2026-11-24T00:00:00.000Z' },
      ],
      contextPercent: null,
      contextWindowSize: null,
      autoCompactAt: null,
      model: 'gpt-5.5',
      effort: 'high',
      observedAt: '2026-11-19T20:00:00.000Z',
      limitsObservedAt: '2026-11-19T20:00:00.000Z',
      engine: 'codex',
    })
  })

  it('sends nothing when the account reports no window', () => {
    expect(codexPlanUsage(limits(null, null), 'gpt-5.5', null)).toBeNull()
    expect(codexPlanUsage({ ...limits(1, 1), buckets: [] }, null, null)).toBeNull()
  })
})

describe('session context a codex thread has not seen', () => {
  const base = '# Session\n\n## User Instructions\n\nAnswer tersely.'
  const line = modelLine('gpt-6.2-sol', 'max')

  it('names the exact model and the codex effort', () => {
    expect(line).toContain('This chat runs on the model `gpt-6.2-sol` with reasoning effort `xhigh`.')
    expect(modelLine('gpt-6.2-sol', null)).toContain('This chat runs on the model `gpt-6.2-sol`.')
    expect(modelLine(null, 'high')).toBe('')
  })

  it('sends nothing for what the thread was started with', () => {
    expect(sessionContextFor(startedContext(base, line), base, line)).toBeNull()
  })

  it('sends the whole instructions when they changed (or are unknown), else only the model line', () => {
    const unknown = sessionContextFor(undefined, base, line)!
    expect(unknown.text.startsWith('<bridge_session_context>\n')).toBe(true)
    expect(unknown.text).toContain('Answer tersely.')
    expect(unknown.text).toContain('`gpt-6.2-sol`')
    expect(unknown.record).toEqual(startedContext(base, line))

    const switched = sessionContextFor(startedContext(base, modelLine('gpt-6.2', 'high')), base, line)!
    expect(switched.text).toContain('`gpt-6.2-sol`')
    expect(switched.text).not.toContain('Answer tersely.')
  })
})

describe('/compact in a codex chat', () => {
  it('is the command alone or with instructions, not a message that mentions it', () => {
    expect(isCompactCommand('/compact')).toBe(true)
    expect(isCompactCommand('/compact Summarise for a fresh agent')).toBe(true)
    expect(isCompactCommand('  /compact\nkeep the plan')).toBe(true)
    expect(isCompactCommand('/compaction notes')).toBe(false)
    expect(isCompactCommand('please /compact later')).toBe(false)
  })
})
