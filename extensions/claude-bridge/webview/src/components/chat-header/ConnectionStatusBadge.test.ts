import { describe, it, expect } from 'vitest'
import { retryLabel } from './ConnectionStatusBadge'

describe('retryLabel', () => {
  it('says when the next attempt is and that retries do not stop', () => {
    // Without "retries continue" a long backoff reads as "this is dead" — the
    // delay doubles to a 30s ceiling and then stays there forever.
    expect(retryLabel(7, 4)).toBe('Reconnecting — attempt 5 in 7s. Retries continue (up to 30s apart).')
  })

  it('reads "now" while the attempt is firing', () => {
    expect(retryLabel(0, 1)).toBe('Reconnecting — attempt 2 now. Retries continue (up to 30s apart).')
  })

  it('omits the attempt number before any retry has been counted', () => {
    expect(retryLabel(3, 0)).toBe('Reconnecting — in 3s. Retries continue (up to 30s apart).')
  })
})
