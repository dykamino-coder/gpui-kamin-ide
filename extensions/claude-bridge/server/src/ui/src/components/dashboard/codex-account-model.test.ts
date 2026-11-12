import { describe, expect, it } from 'vitest'
import {
  codexCardView,
  formatPlan,
  isCodexAccountEvent,
  shouldPollCodexAccount,
  toCodexError,
  type CodexAccountStatus,
  type CodexCardUiState,
} from './codex-account-model'

const idle: CodexCardUiState = { loading: false, busy: false, fetchError: null, actionError: null }
const signedOut: CodexAccountStatus = {
  loggedIn: false,
  email: null,
  planType: null,
  accountType: null,
  pendingLogin: null,
  error: null,
}
const pending = {
  loginId: 'l-1',
  verificationUrl: 'https://auth.example.test/device',
  userCode: 'ABCD-1234',
  startedAt: 1,
}

describe('codexCardView', () => {
  it('shows a checking state before the first response', () => {
    const view = codexCardView(null, { ...idle, loading: true })
    expect(view.mode).toBe('loading')
    expect(view.items[0]?.value).toBe('Checking...')
    expect(view.canSignIn).toBe(false)
  })

  it('offers sign-in when signed out', () => {
    const view = codexCardView(signedOut, idle)
    expect(view.mode).toBe('signed-out')
    expect(view.status).toBe('error')
    expect(view.items[0]?.value).toBe('Not signed in')
    expect(view.canSignIn).toBe(true)
    expect(view.canSignOut).toBe(false)
  })

  it('reports a non-ChatGPT auth mode as unused', () => {
    const view = codexCardView({ ...signedOut, accountType: 'apiKey' }, idle)
    expect(view.items.map((i) => i.value)).toContain('apiKey (not used)')
  })

  it('shows the device code while a login is pending', () => {
    const view = codexCardView({ ...signedOut, pendingLogin: pending }, idle)
    expect(view.mode).toBe('pending')
    expect(view.pending).toEqual(pending)
    expect(view.canCancel).toBe(true)
    expect(view.canSignIn).toBe(false)
  })

  it('shows email and plan when signed in', () => {
    const view = codexCardView(
      { ...signedOut, loggedIn: true, email: 'admin@example.test', planType: 'pro', accountType: 'chatgpt' },
      idle,
    )
    expect(view.mode).toBe('signed-in')
    expect(view.status).toBe('ok')
    expect(view.items).toEqual([
      { label: 'Email', value: 'admin@example.test' },
      { label: 'Plan', value: 'Pro' },
    ])
    expect(view.canSignOut).toBe(true)
  })

  it('disables actions while a request is in flight', () => {
    expect(codexCardView(signedOut, { ...idle, busy: true }).canSignIn).toBe(false)
    expect(codexCardView({ ...signedOut, pendingLogin: pending }, { ...idle, busy: true }).canCancel).toBe(false)
  })

  it('prefers the action error over the status error, then the fetch failure', () => {
    const statusError = { kind: 'network' as const, message: 'Server cannot reach ChatGPT.', detail: 'ECONNRESET' }
    const actionError = { kind: 'proxy-auth' as const, message: 'Proxy rejected credentials.', detail: '407' }
    expect(codexCardView({ ...signedOut, error: statusError }, idle).error).toEqual(statusError)
    expect(codexCardView({ ...signedOut, error: statusError }, { ...idle, actionError }).error).toEqual(actionError)
    const offline = codexCardView(null, { ...idle, fetchError: 'HTTP 500' })
    expect(offline.error?.detail).toBe('HTTP 500')
    expect(offline.status).toBe('error')
    expect(offline.canSignIn).toBe(true)
  })
})

describe('shouldPollCodexAccount', () => {
  it('polls only while a login is pending', () => {
    expect(shouldPollCodexAccount(null)).toBe(false)
    expect(shouldPollCodexAccount(signedOut)).toBe(false)
    expect(shouldPollCodexAccount({ ...signedOut, pendingLogin: pending })).toBe(true)
    expect(shouldPollCodexAccount({ ...signedOut, loggedIn: true, pendingLogin: pending })).toBe(false)
  })
})

describe('helpers', () => {
  it('recognises codex account events only', () => {
    expect(isCodexAccountEvent({ engine: 'codex', loggedIn: true })).toBe(true)
    expect(isCodexAccountEvent({ email: 'x@example.test' })).toBe(false)
    expect(isCodexAccountEvent(null)).toBe(false)
  })

  it('formats known plans and keeps unknown ones', () => {
    expect(formatPlan('plus')).toBe('Plus')
    expect(formatPlan('custom-tier')).toBe('custom-tier')
    expect(formatPlan(null)).toBe('—')
  })

  it('extracts the server error body or falls back', () => {
    expect(
      toCodexError({ error: { kind: 'tls', message: 'Certificate rejected.', detail: 'x509' } }, 'HTTP 502'),
    ).toEqual({ kind: 'tls', message: 'Certificate rejected.', detail: 'x509' })
    expect(toCodexError(null, 'HTTP 502')).toEqual({
      kind: 'other',
      message: 'The request failed.',
      detail: 'HTTP 502',
    })
  })
})
