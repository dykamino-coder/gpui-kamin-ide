import { randomBytes, createHash } from 'node:crypto'
import fsp from 'node:fs/promises'
import type { Context } from 'hono'
import { getCookie, setCookie, deleteCookie } from 'hono/cookie'

interface Grant {
  sessionId: string
  at: number
  kind?: 'api' | 'admin'
  user?: string
  tokenId?: string
  caller: string
}
const grants = new Map<string, Grant>()
const COOKIE = 'bridge_jsonl_download'
const TTL = 30_000
const route = (sessionId: string) => `/api/dashboard/sessions/${encodeURIComponent(sessionId)}/jsonl-download`
export function jsonlExportCaller(c: Context): string {
  // An irreversible key bounds work per authenticated caller without retaining
  // their Bearer credential. Auth-disabled local mode shares one budget.
  return createHash('sha256')
    .update(c.req.header('authorization') ?? 'local')
    .digest('hex')
}
export function issueJsonlDownload(c: Context, sessionId: string): string | null {
  const now = Date.now()
  const caller = jsonlExportCaller(c)
  for (const [key, grant] of grants) {
    if (now - grant.at >= TTL || (grant.caller === caller && grant.sessionId === sessionId)) grants.delete(key)
  }
  if (grants.size >= 64) return null
  const key = randomBytes(32).toString('hex')
  grants.set(key, {
    sessionId,
    caller,
    at: now,
    kind: c.get('authKind'),
    user: c.get('apiUserName'),
    tokenId: c.get('apiTokenId'),
  })
  setCookie(c, COOKIE, key, {
    path: route(sessionId),
    httpOnly: true,
    sameSite: 'Strict',
    maxAge: TTL / 1000,
    secure: new URL(c.req.url).protocol === 'https:' || c.req.header('x-forwarded-proto') === 'https',
  })
  return route(sessionId)
}

/** Called before the dashboard Bearer middleware. Only this exact endpoint
 * may consume a one-use, resource-scoped grant; arbitrary API paths cannot. */
export function consumeJsonlDownload(c: Context): boolean {
  if (c.req.method !== 'GET') return false
  const key = getCookie(c, COOKIE)
  if (!key) return false
  const grant = grants.get(key)
  if (!grant || c.req.path !== route(grant.sessionId)) return false
  grants.delete(key)
  deleteCookie(c, COOKIE, { path: route(grant.sessionId) })
  if (Date.now() - grant.at >= TTL) return false
  c.set('authKind', grant.kind)
  c.set('apiUserName', grant.user)
  c.set('apiTokenId', grant.tokenId)
  c.set('jsonlDownloadSession', grant.sessionId)
  c.set('jsonlExportCaller', grant.caller)
  return true
}

const active = new Map<string, number>()
let activeTotal = 0
export class JsonlExportBusy extends Error {}

/** Snapshot means the first stat.size bytes of the opened file: later append
 * is excluded; early EOF/truncation fails the download. No in-place rewrite
 * immutability is promised. At most two 64 KiB buffers per reader, 8 readers
 * globally / 2 per caller. Web-stream pull owns filesystem backpressure. */
export async function jsonlAttachment(
  filePath: string,
  name: string,
  caller: string,
  signal: AbortSignal,
): Promise<Response> {
  if (activeTotal >= 8 || (active.get(caller) ?? 0) >= 2)
    throw new JsonlExportBusy('Export busy; try again after the current download')
  activeTotal++
  active.set(caller, (active.get(caller) ?? 0) + 1)
  let released = false
  const release = () => {
    if (released) return
    released = true
    activeTotal--
    const count = (active.get(caller) ?? 1) - 1
    if (count) active.set(caller, count)
    else active.delete(caller)
  }
  let file: Awaited<ReturnType<typeof fsp.open>>
  try {
    file = await fsp.open(filePath, 'r')
  } catch (error) {
    release()
    throw error
  }
  let closed = false
  let abort: () => void = () => {}
  const finish = async () => {
    if (closed) return
    closed = true
    signal.removeEventListener('abort', abort)
    try {
      await file.close()
    } catch {
      /* close cancellation race */
    } finally {
      release()
    }
  }
  try {
    const stat = await file.stat()
    if (!stat.isFile()) throw new Error('Not a transcript file')
    signal.throwIfAborted()
    let offset = 0
    const stream = new ReadableStream<Uint8Array>(
      {
        start(controller) {
          abort = () => {
            controller.error(signal.reason ?? new Error('Download cancelled'))
            void finish()
          }
          signal.addEventListener('abort', abort, { once: true })
          if (signal.aborted) abort()
        },
        async pull(controller) {
          if (closed) return
          try {
            if (offset >= stat.size) {
              await finish()
              controller.close()
              return
            }
            const buffer = Buffer.alloc(Math.min(64 * 1024, stat.size - offset))
            const { bytesRead } = await file.read(buffer, 0, buffer.length, offset)
            if (closed) return
            if (!bytesRead) throw new Error('Transcript truncated during export')
            offset += bytesRead
            controller.enqueue(buffer.subarray(0, bytesRead))
          } catch (error) {
            if (closed) return
            await finish()
            controller.error(error)
          }
        },
        cancel: finish,
      },
      { highWaterMark: 1 },
    )
    return new Response(stream, {
      headers: {
        'Content-Type': 'application/x-ndjson',
        'Content-Length': String(stat.size),
        'Content-Disposition': `attachment; filename="${name.replace(/[^a-zA-Z0-9_-]/g, '_')}.jsonl"`,
        'Cache-Control': 'no-store',
        'Referrer-Policy': 'no-referrer',
      },
    })
  } catch (error) {
    await finish()
    throw error
  }
}
