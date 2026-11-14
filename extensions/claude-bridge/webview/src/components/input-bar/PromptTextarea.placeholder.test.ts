import { describe, expect, it } from 'vitest'
import { getPlaceholder } from './PromptTextarea'

describe('getPlaceholder (BR-05B)', () => {
  it('says why a refused tab is not connecting', () => {
    expect(getPlaceholder(false, 'connecting', false, 'session-limit')).toContain('Session limit reached')
    expect(getPlaceholder(false, 'disconnected', false, 'session-limit')).toContain('Session limit reached')
  })

  it('keeps the plain connecting text without a refusal', () => {
    expect(getPlaceholder(false, 'connecting', false)).toBe('Connecting to bridge...')
  })
})