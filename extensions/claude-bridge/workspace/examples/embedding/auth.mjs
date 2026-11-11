// Example sign-in policy of an embedding application. The site authenticates
// the user (here: a demo password; in production e.g. Keycloak), then mints a
// short-lived signed assertion that the browser posts to the workspace's
// POST /auth/login. The workspace host only verifies the assertion and keeps
// opaque cookie sessions. Replace this file with the application's own policy.
import { createHmac, randomBytes, timingSafeEqual } from 'node:crypto'
import { createSessionCookie, sessionCookieHeader } from '@kaminide/bridge-workspace-host'

const ASSERTION_TTL_MS = 60_000
const SESSION_TTL_MS = 8 * 60 * 60 * 1000

export function createAssertionAuth(secret = randomBytes(32)) {
  const sign = payload => createHmac('sha256', secret).update(payload).digest('base64url')
  const used = new Map()
  const sessions = new Map()
  const prune = () => {
    const now = Date.now()
    for (const [nonce, expires] of used) if (expires <= now) used.delete(nonce)
    for (const [cookie, session] of sessions) if (session.expires <= now) sessions.delete(cookie)
  }
  return {
    /** Site side: called only after the application authenticated `user`. */
    mint(user) {
      const payload = Buffer.from(JSON.stringify({ user, nonce: randomBytes(16).toString('base64url'), expires: Date.now() + ASSERTION_TTL_MS })).toString('base64url')
      return `${payload}.${sign(payload)}`
    },
    /** Workspace side: the WorkspaceAuthPolicy passed to startHeadlessWorkspace. */
    policy: {
      async login(candidate) {
        if (typeof candidate !== 'string' || candidate.length > 1024) return null
        const [payload, signature] = candidate.split('.')
        if (!payload || !signature) return null
        const expected = Buffer.from(sign(payload))
        const actual = Buffer.from(signature)
        if (actual.length !== expected.length || !timingSafeEqual(actual, expected)) return null
        let claims
        try { claims = JSON.parse(Buffer.from(payload, 'base64url').toString('utf8')) } catch { return null }
        prune()
        if (typeof claims.user !== 'string' || !(claims.expires > Date.now()) || used.has(claims.nonce)) return null
        used.set(claims.nonce, claims.expires)
        const cookie = createSessionCookie()
        sessions.set(cookie, { user: claims.user, expires: Date.now() + SESSION_TTL_MS })
        return cookie
      },
      async validate(cookie) {
        prune()
        return !!cookie && sessions.has(cookie)
      },
      logout(cookie) { if (cookie) sessions.delete(cookie) },
      cookieHeader(cookie, secure) { return sessionCookieHeader(cookie, { secure, maxAgeSeconds: SESSION_TTL_MS / 1000 }) },
    },
  }
}
