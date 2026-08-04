// ============================================================================
// Auth signals — dashboard authentication state
// ============================================================================

import { signal } from '@preact/signals'

const TOKEN_KEY = 'dashboard_session_token'

export const isAuthenticated = signal(false)
export const authChecked = signal(false)
export const loginError = signal('')
export const loginLoading = signal(false)

/** Get stored token */
export function getSessionToken(): string | null {
  return localStorage.getItem(TOKEN_KEY)
}

/** Store token */
function setSessionToken(token: string): void {
  localStorage.setItem(TOKEN_KEY, token)
}

/** Remove stored token */
function clearSessionToken(): void {
  localStorage.removeItem(TOKEN_KEY)
}

/** Check current session against backend */
export async function checkSession(): Promise<void> {
  try {
    const token = getSessionToken()
    const headers: Record<string, string> = {}
    if (token) headers['Authorization'] = `Bearer ${token}`

    const res = await fetch('/api/dashboard/auth/session', { headers })
    const data = await res.json()

    if (!data.authEnabled) {
      // Auth not enabled — treat as authenticated
      isAuthenticated.value = true
    } else {
      isAuthenticated.value = data.authenticated === true
      if (!data.authenticated) clearSessionToken()
    }
  } catch {
    // Network error — assume not authenticated if we had a token
    isAuthenticated.value = !getSessionToken()
  } finally {
    authChecked.value = true
  }
}

/** Login with credentials */
export async function login(username: string, password: string): Promise<boolean> {
  loginError.value = ''
  loginLoading.value = true

  try {
    const res = await fetch('/api/dashboard/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    })

    const data = await res.json()

    if (!res.ok) {
      loginError.value = data.error || 'Login failed'
      return false
    }

    setSessionToken(data.token)
    isAuthenticated.value = true
    return true
  } catch {
    loginError.value = 'Network error'
    return false
  } finally {
    loginLoading.value = false
  }
}

/** Logout */
export async function logout(): Promise<void> {
  const token = getSessionToken()
  if (token) {
    try {
      await fetch('/api/dashboard/auth/logout', {
        method: 'POST',
        headers: { 'Authorization': `Bearer ${token}` },
      })
    } catch { /* ignore */ }
  }
  clearSessionToken()
  isAuthenticated.value = false
}
