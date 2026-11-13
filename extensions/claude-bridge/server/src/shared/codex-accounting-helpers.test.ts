import { describe, expect, it } from 'vitest'
import { codexErrorType } from '../core/stats/api-errors'
import { engineClause, nullableEngineClause, parseEngineFilter } from '../core/stats/engine-filter'
import { limitWindowLabel, parseCodexRateLimits } from './codex-rate-limits'
import { isEngineUsageLine, isUsagelessItemLine, lineEngine } from './engine-accounting'
import { isLongOpenAiRequest, openAiCost, openAiRateFor, openAiRequestCost } from './openai-pricing'

const tokens = (input: number, cacheRead = 0, cacheWrite = 0, output = 0) => ({ input, cacheRead, cacheWrite, output })

describe('OpenAI API-equivalent prices', () => {
  it('matches exact model ids only, ignoring case and the openai/ prefix', () => {
    expect(openAiRateFor('gpt-5.5')).not.toBeNull()
    expect(openAiRateFor('OpenAI/GPT-5.5')).toEqual(openAiRateFor('gpt-5.5'))
    expect(openAiRateFor('gpt-5.5-codex-preview')).toBeNull()
    expect(openAiRateFor('claude-opus-5-5')).toBeNull()
    expect(openAiRateFor(null)).toBeNull()
    expect(openAiRateFor('constructor')).toBeNull()
  })

  it('prices fresh, cached and output tokens per 1M and returns null without a price', () => {
    const r = openAiRateFor('gpt-5.5')!
    expect(openAiCost('gpt-5.5', tokens(1_000_000, 1_000_000, 0, 1_000_000), false)).toBeCloseTo(
      r.input + r.cachedInput! + r.output,
      9,
    )
    expect(openAiCost('unknown-model', tokens(10), false)).toBeNull()
    // A model with no cached price bills cached reads as input.
    const pro = openAiRateFor('gpt-5-pro')!
    expect(pro.cachedInput).toBeNull()
    expect(openAiCost('gpt-5-pro', tokens(0, 1_000_000), false)).toBeCloseTo(pro.input, 9)
  })

  it('bills a request above 272K input at the long-context rate', () => {
    const r = openAiRateFor('gpt-5.5')!
    expect(r.long).toBeDefined()
    const short = tokens(200_000, 72_000)
    const long = tokens(200_000, 72_001)
    expect(isLongOpenAiRequest(short)).toBe(false)
    expect(isLongOpenAiRequest(long)).toBe(true)
    expect(openAiRequestCost('gpt-5.5', short)).toBeCloseTo((200_000 * r.input + 72_000 * r.cachedInput!) / 1e6, 9)
    expect(openAiRequestCost('gpt-5.5', long)).toBeCloseTo(
      (200_000 * r.long!.input + 72_001 * r.long!.cachedInput!) / 1e6,
      9,
    )
  })
})

describe('engine of a transcript line', () => {
  const usage = { type: 'system', subtype: 'api_usage', engine: 'codex', usage: {} }
  it('tells usage lines, codex item lines and Claude lines apart', () => {
    expect(lineEngine({ type: 'assistant' })).toBe('claude')
    expect(lineEngine({ engine: 'codex' })).toBe('codex')
    expect(isEngineUsageLine(usage)).toBe(true)
    expect(isEngineUsageLine({ ...usage, engine: undefined })).toBe(false)
    expect(isEngineUsageLine({ type: 'system', subtype: 'compact_boundary', engine: 'codex' })).toBe(false)
    expect(isUsagelessItemLine({ type: 'assistant', engine: 'codex' })).toBe(true)
    expect(isUsagelessItemLine({ type: 'assistant' })).toBe(false)
    expect(isUsagelessItemLine(usage)).toBe(false)
  })

  it('whitelists the engine filter before it reaches SQL', () => {
    expect(parseEngineFilter('codex')).toBe('codex')
    expect(parseEngineFilter("codex' OR 1=1 --")).toBeNull()
    expect(parseEngineFilter(undefined)).toBeNull()
    expect(engineClause('e.engine', 'claude')).toBe(" AND e.engine = 'claude'")
    expect(engineClause('e.engine', null)).toBe('')
    expect(nullableEngineClause('engine', 'claude')).toBe(" AND COALESCE(engine, 'claude') = 'claude'")
  })
})

describe('codex errors and plan limits', () => {
  it('classifies codex turn errors for the errors chart', () => {
    expect(codexErrorType('unexpected status 429 Too Many Requests')).toBe('429')
    expect(codexErrorType('stream disconnected: request timed out')).toBe('timeout')
    expect(codexErrorType("You've hit your usage limit")).toBe('rate_limit')
    expect(codexErrorType('')).toBe('error')
  })

  it('normalizes account/rateLimits/read windows and labels them', () => {
    const now = 1_700_000_000_000
    const parsed = parseCodexRateLimits(
      {
        rateLimits: {
          limitId: 'codex',
          primary: { usedPercent: 42, windowDurationMins: 300, resetsAt: 1_700_010_000 },
          secondary: { usedPercent: 140, windowDurationMins: 10080, resetsAt: null },
          credits: { hasCredits: true, unlimited: false, balance: '12.5' },
          planType: 'plus',
        },
        rateLimitsByLimitId: {
          codex: { primary: { usedPercent: 1 } },
          other: { limitName: 'Other', primary: { usedPercent: 5, windowDurationMins: 60 } },
          empty: {},
        },
        ordinaryUsageAllowed: true,
      },
      now,
    )
    expect(parsed.buckets).toEqual([
      {
        id: 'codex',
        name: null,
        primary: { usedPercent: 42, windowMinutes: 300, resetsAt: 1_700_010_000_000 },
        secondary: { usedPercent: 100, windowMinutes: 10080, resetsAt: null },
        reached: null,
      },
      {
        id: 'other',
        name: 'Other',
        primary: { usedPercent: 5, windowMinutes: 60, resetsAt: null },
        secondary: null,
        reached: null,
      },
    ])
    expect(parsed).toMatchObject({
      credits: { hasCredits: true, unlimited: false, balance: '12.5' },
      planType: 'plus',
      usageAllowed: true,
      readAt: now,
    })
    expect(parseCodexRateLimits(null, now)).toMatchObject({ buckets: [], credits: null, usageAllowed: null })
    expect(limitWindowLabel(300, 'x')).toBe('5-hour limit')
    expect(limitWindowLabel(10080, 'x')).toBe('Weekly limit')
    expect(limitWindowLabel(2880, 'x')).toBe('2-day limit')
    expect(limitWindowLabel(45, 'x')).toBe('45-minute limit')
    expect(limitWindowLabel(null, 'Short limit')).toBe('Short limit')
  })
})
