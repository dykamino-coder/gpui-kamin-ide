// Reuse possession of the upstream Bridge token; keep browser sessions opaque.
import { createHash, createHmac, randomBytes, timingSafeEqual } from 'node:crypto'
import fs from 'node:fs'
import { privateRead, privateWrite } from './credentials.js'

export const SESSION_COOKIE = 'kamin_workspace'

export interface TokenIdentity { id: string }
export type ResolveToken = (token: string) => Promise<TokenIdentity | null>
interface Session { identity: string; expiresAt: number }
export interface PersistentAuth { secret: string; file: string }

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
    private readonly persistent?: PersistentAuth,
  ) {
    if (!token.trim() || token.length > 4096) throw new Error('A configured Bridge token is required')
    if (!Number.isFinite(ttlMs) || ttlMs <= 0) throw new Error('Invalid session lifetime')
    this.digest = this.hash(token)
    if (persistent && fs.existsSync(persistent.file)) {
      const records = privateRead(persistent.file)
      if (!Array.isArray(records) || records.length > 128) throw new Error('Invalid browser sessions')
      for (const record of records) {
        if (!Array.isArray(record) || record.length !== 2 || !/^[a-f0-9]{64}$/.test(record[0]) ||
          typeof record[1]?.identity !== 'string' || !Number.isFinite(record[1]?.expiresAt)) throw new Error('Invalid browser session')
        if (record[1].expiresAt > this.now()) this.sessions.set(record[0], record[1])
      }
    }
  }

  private hash(value: string): Buffer { return createHash('sha256').update(value).digest() }
  private key(cookie: string): string {
    return this.persistent ? createHmac('sha256', this.persistent.secret).update(cookie).digest('hex') : cookie
  }
  private save(): void { if (this.persistent) privateWrite(this.persistent.file, [...this.sessions]) }
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
    this.sessions.set(this.key(cookie), { identity: identity.id, expiresAt: this.now() + this.ttlMs })
    this.save()
    return cookie
  }

  /** Only the checked loopback HTTP boundary may call this. */
  async loginLocal(): Promise<string | null> { return this.login(this.token) }

  /** Revalidate with the server, including for existing sockets. Fail closed. */
  async validate(cookie: string | undefined): Promise<boolean> {
    this.prune()
    const key = cookie ? this.key(cookie) : ''
    const session = this.sessions.get(key)
    if (!session) return false
    const identity = await this.resolveIdentity()
    // Logout/expiry can occur while the upstream request is outstanding.
    if (!identity || identity.id !== session.identity || session.expiresAt <= this.now() ||
      this.sessions.get(key) !== session) {
      this.sessions.delete(key)
      this.save()
      return false
    }
    return true
  }

  logout(cookie: string | undefined): void { if (cookie) { this.sessions.delete(this.key(cookie)); this.save() } }

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
