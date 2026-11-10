import 'reflect-metadata'
import { describe, expect, it } from 'vitest'
import { accountFromAuthStatus, loginExpiresAtFrom } from './account'

describe('account card sign-in expiry', () => {
  it('passes through the stored sign-in end reported by claude auth status', () => {
    const account = accountFromAuthStatus(
      { loggedIn: true, email: 'user@example.test', loginExpiresAt: '2026-12-01T10:00:00.000Z' },
      { expiresAt: 1_700_000_000_000 },
    )
    expect(account).toMatchObject({
      email: 'user@example.test',
      loginExpiresAt: '2026-12-01T10:00:00.000Z',
      expiresAt: 1_700_000_000_000,
    })
  })

  it('reports no sign-in end for older CLIs, API keys and malformed values', () => {
    expect(loginExpiresAtFrom({ loggedIn: true })).toBeNull()
    expect(loginExpiresAtFrom({ loginExpiresAt: null })).toBeNull()
    expect(loginExpiresAtFrom({ loginExpiresAt: 'not a date' })).toBeNull()
    expect(loginExpiresAtFrom({ loginExpiresAt: 1_700_000_000_000 })).toBeNull()
    expect(loginExpiresAtFrom(null)).toBeNull()
  })
})
