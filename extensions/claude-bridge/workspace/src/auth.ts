// Reuse possession of the upstream Bridge token; keep browser sessions opaque.
import { createHash, randomBytes, timingSafeEqual } from 'node:crypto'

export const SESSION_COOKIE = 'kamin_workspace'

export interface TokenIdentity { id: string }
export type ResolveToken = (token: string) => Promise<TokenIdentity | null>
interface Session { identity: string; expiresAt: number }

export function serverTokenResolver(serverUrl: string): ResolveToken {
  const base = new URL(serverUrl)
  if (!['http:', 'https:', 'ws:', 'wss:'].includes(base.protocol) || base.username || base.password) {
    throw new Error('Invalid Bridge server URL')
  }
  base.protocol = base.protocol === 'wss:' || base.protocol === 'https:' ? 'https:' : 'http:'
  base.pathname = '/api/dashboard/tokens/resolve'
  base.search = ''
  base.hash = ''
  return async token => {
    try {
      const response = await fetch(base, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ token }),
        redirect: 'error',
        signal: AbortSignal.timeout(5000),
      })
      if (!response.ok) return null
      const result = await response.json() as { id?: unknown }
      return typeof result.id === 'string' && result.id ? { id: result.id } : null
    } catch { return null }
  }
}

export class WorkspaceAuth {
  private readonly sessions = new Map<string, Session>()
  private identity: string | undefined
  private readonly digest: Buffer

  constructor(
    private readonly token: string,
    private readonly resolve: ResolveToken,
    private readonly ttlMs = 60 * 60 * 1000,
    private readonly now: () => number = Date.now,
  ) {
    if (!token.trim() || token.length > 4096) throw new Error('A configured Bridge token is required')
    if (!Number.isFinite(ttlMs) || ttlMs <= 0) throw new Error('Invalid session lifetime')
    this.digest = this.hash(token)
  }

  private hash(value: string): Buffer { return createHash('sha256').update(value).digest() }
  private async resolveIdentity(): Promise<TokenIdentity | null> {
    try { return await this.resolve(this.token) } catch { return null }
  }
  private prune(): void {
    for (const [key, value] of this.sessions) {
      if (value.expiresAt <= this.now()) this.sessions.delete(key)
    }
  }

  async login(candidate: unknown): Promise<string | null> {
    if (typeof candidate !== 'string' || candidate.length > 4096 ||
      !timingSafeEqual(this.hash(candidate), this.digest)) return null
    const identity = await this.resolveIdentity()
    if (!identity || (this.identity && this.identity !== identity.id)) return null
    this.identity = identity.id
    this.prune()
    if (this.sessions.size >= 128) return null
    const cookie = randomBytes(32).toString('base64url')
    this.sessions.set(cookie, { identity: identity.id, expiresAt: this.now() + this.ttlMs })
    return cookie
  }

  /** Revalidate with the server, including for existing sockets. Fail closed. */
  async validate(cookie: string | undefined): Promise<boolean> {
    this.prune()
    const session = cookie ? this.sessions.get(cookie) : undefined
    if (!session) return false
    const identity = await this.resolveIdentity()
    // Logout/expiry can occur while the upstream request is outstanding.
    if (!identity || identity.id !== session.identity || session.expiresAt <= this.now() ||
      this.sessions.get(cookie!) !== session) {
      this.sessions.delete(cookie!)
      return false
    }
    return true
  }

  logout(cookie: string | undefined): void { if (cookie) this.sessions.delete(cookie) }

  cookieHeader(cookie: string, secure: boolean): string {
    return `${SESSION_COOKIE}=${cookie}; Path=/; HttpOnly; SameSite=Strict; Max-Age=${Math.floor(this.ttlMs / 1000)}${secure ? '; Secure' : ''}`
  }
}

export function readSessionCookie(header: string | undefined): string | undefined {
  const matches = (header ?? '').split(';').map(x => x.trim()).filter(x => x.startsWith(`${SESSION_COOKIE}=`))
  if (matches.length !== 1) return undefined
  const value = matches[0].slice(SESSION_COOKIE.length + 1)
  return /^[A-Za-z0-9_-]{43}$/.test(value) ? value : undefined
}
