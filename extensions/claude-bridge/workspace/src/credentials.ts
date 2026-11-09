// Service-owned secrets live only in the workspace HOME volume, never config/UI.
import fs from 'node:fs'
import path from 'node:path'
import { randomBytes, randomUUID } from 'node:crypto'

export const CREDENTIAL_FILE = 'workspace-credentials.json'
export const SESSION_FILE = 'workspace-sessions.json'
export interface Credentials { version: 1; server: string; token: string; browserSecret: string }

export function privateWrite(file: string, value: unknown): void {
  const temporary = `${file}.${randomUUID()}.tmp`
  const fd = fs.openSync(temporary, fs.constants.O_WRONLY | fs.constants.O_CREAT | fs.constants.O_EXCL, 0o600)
  try {
    try {
      fs.writeFileSync(fd, JSON.stringify(value))
      fs.fsyncSync(fd)
    } finally { fs.closeSync(fd) }
    fs.renameSync(temporary, file)
  } finally {
    try { fs.unlinkSync(temporary) }
    catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error }
  }
}

export function privateRead(file: string): unknown {
  const fd = fs.openSync(file, fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW ?? 0))
  try {
    const stat = fs.fstatSync(fd)
    if (!stat.isFile() || stat.size > 128 * 1024) throw new Error('Invalid private state')
    fs.fchmodSync(fd, 0o600)
    return JSON.parse(fs.readFileSync(fd, 'utf8'))
  } finally { fs.closeSync(fd) }
}

export async function loadCredentials(storage: string, serverUrl: string, env = process.env): Promise<Credentials> {
  fs.mkdirSync(storage, { recursive: true, mode: 0o700 })
  fs.chmodSync(storage, 0o700)
  const base = new URL(serverUrl)
  if (!['http:', 'https:', 'ws:', 'wss:'].includes(base.protocol) || base.username || base.password || base.search || base.hash) {
    throw new Error('Invalid Bridge server')
  }
  base.protocol = ['https:', 'wss:'].includes(base.protocol) ? 'https:' : 'http:'
  const server = base.origin
  const file = path.join(storage, CREDENTIAL_FILE)
  if (fs.existsSync(file)) {
    const saved = privateRead(file) as Credentials
    if (saved.version !== 1 || saved.server !== server || typeof saved.token !== 'string' || !saved.token.trim() ||
      saved.token.length > 4096 || !/^[A-Za-z0-9_-]{43}$/.test(saved.browserSecret)) throw new Error('Invalid credentials or changed server; explicit reset required')
    return saved
  }
  if (env.BRIDGE_TOKEN && env.BRIDGE_TOKEN_FILE) throw new Error('Ambiguous token source')
  let token = env.BRIDGE_TOKEN || (env.BRIDGE_TOKEN_FILE ? fs.readFileSync(env.BRIDGE_TOKEN_FILE, 'utf8').trim() : '')
  if (!token) {
    // Existing first-run registration API. An administrative server may refuse;
    // then import a provided secret once rather than inventing a local token.
    const response = await fetch(new URL('/api/dashboard/tokens', server), {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name: `workspace-${randomUUID()}` }),
      redirect: 'error', signal: AbortSignal.timeout(10_000),
    })
    if (![200, 201].includes(response.status)) throw new Error('Server registration refused; provide a bootstrap secret')
    const result = await response.json() as { token?: unknown }
    if (typeof result.token !== 'string') throw new Error('Invalid registration response')
    token = result.token
  }
  if (!token.trim() || token.length > 4096) throw new Error('Invalid Bridge token')
  const credentials: Credentials = { version: 1, server, token, browserSecret: randomBytes(32).toString('base64url') }
  privateWrite(file, credentials)
  return credentials
}

export function resetCredentials(storage: string): void {
  for (const name of [CREDENTIAL_FILE, SESSION_FILE]) {
    try { fs.unlinkSync(path.join(storage, name)) }
    catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error }
  }
}
