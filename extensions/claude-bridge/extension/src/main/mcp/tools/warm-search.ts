// Client of `kamin-search` (crates/search): one long-lived process per
// extension host answers the ripgrep command lines of Grep and Glob with what
// ripgrep 15.2.0 would print, so a call does not start `rg.exe` (~25–50 ms on
// Windows). Started on first use; it exits by itself after ten idle minutes
// and when this host's stdin pipe to it closes.
//
// Every failure falls back to ripgrep: no binary, a command it does not
// replicate (`unsupported`), or the process dying under a request. After
// three deaths within a minute it stays off for this host.
import { spawn, type ChildProcess } from 'node:child_process'
import { existsSync } from 'node:fs'
import { dirname, join } from 'node:path'

export interface WarmSearchOptions {
  cwd: string
  timeoutMs: number
  /** Output bytes kept; the answer says `truncated` past it. */
  maxBytes?: number
  /** Output records (lines, or NUL-terminated paths) kept. */
  maxRecords?: number
  signal?: AbortSignal
}

export interface WarmSearchResult {
  stdout: Buffer
  stderr: string
  code: number
  truncated: boolean
  timedOut: boolean
}

/** The header line of an answer; `stdoutLen` raw bytes follow it. */
interface Answer {
  id: number
  unsupported?: string
  code?: number
  stdoutLen: number
  stderr?: string
  truncated?: boolean
  timedOut?: boolean
  stdout?: Buffer
}

/** The binary next to the bundled `rg.exe`; Windows only unless configured. */
export function findWarmSearch(env: NodeJS.ProcessEnv = process.env, execPath = process.execPath, platform = process.platform): string | null {
  if (env.KAMIN_SEARCH === '0') return null
  if (env.KAMIN_SEARCH_PATH) return existsSync(env.KAMIN_SEARCH_PATH) ? env.KAMIN_SEARCH_PATH : null
  if (platform !== 'win32') return null
  const hostDir = dirname(execPath)
  for (const candidate of [join(hostDir, '..', 'kamin-search.exe'), join(hostDir, 'kamin-search.exe')]) {
    if (existsSync(candidate)) return candidate
  }
  return null
}

const DEATHS_ALLOWED = 3
const DEATH_WINDOW_MS = 60_000

export class WarmSearch {
  private child: ChildProcess | null = null
  private pending = new Map<number, (a: Answer | null) => void>()
  private nextId = 1
  private buffer: Buffer = Buffer.alloc(0)
  private header: Answer | null = null
  private chunks: Buffer[] = []
  /** Bytes in `buffer` and `chunks`. */
  private buffered = 0
  private deaths: number[] = []

  constructor(private readonly binary: string | null) {}

  /** Whether calls can go here at all. */
  get available(): boolean {
    const now = Date.now()
    this.deaths = this.deaths.filter((t) => now - t < DEATH_WINDOW_MS)
    return this.binary !== null && this.deaths.length < DEATHS_ALLOWED
  }

  private start(): ChildProcess {
    const child = spawn(this.binary!, [], { stdio: ['pipe', 'pipe', 'ignore'], windowsHide: true })
    child.stdout!.on('data', (d: Buffer) => this.onData(d))
    child.stdin!.on('error', () => {
      /* the process ended; 'exit' settles the requests */
    })
    const ended = (): void => {
      if (this.child !== child) return
      this.child = null
      this.buffer = Buffer.alloc(0)
      this.header = null
      this.chunks = []
      this.buffered = 0
      // Exit while requests were open is a death; an idle exit is not.
      if (this.pending.size) this.deaths.push(Date.now())
      for (const settle of this.pending.values()) settle(null)
      this.pending.clear()
    }
    child.on('exit', ended)
    child.on('error', ended)
    // The host's lifetime must not depend on this helper.
    child.unref()
    ;(child.stdout as unknown as { unref?: () => void }).unref?.()
    ;(child.stdin as unknown as { unref?: () => void }).unref?.()
    return child
  }

  private onData(chunk: Buffer): void {
    // A large body arrives in many chunks: join them once it is complete,
    // not at every chunk.
    if (this.header && this.buffered + chunk.length < this.header.stdoutLen) {
      this.chunks.push(chunk)
      this.buffered += chunk.length
      return
    }
    this.buffer = Buffer.concat([this.buffer, ...this.chunks, chunk])
    this.chunks = []
    this.buffered = this.buffer.length
    for (;;) {
      if (!this.header) {
        const i = this.buffer.indexOf(0x0a)
        if (i === -1) return
        const line = this.buffer.subarray(0, i).toString('utf8')
        this.buffer = this.buffer.subarray(i + 1)
        this.buffered = this.buffer.length
        try {
          this.header = JSON.parse(line) as Answer
        } catch {
          // Not a frame of this protocol: the stream cannot be trusted.
          this.child?.kill()
          return
        }
      }
      const answer = this.header
      if (this.buffer.length < answer.stdoutLen) return
      answer.stdout = Buffer.from(this.buffer.subarray(0, answer.stdoutLen))
      this.buffer = this.buffer.subarray(answer.stdoutLen)
      this.buffered = this.buffer.length
      this.header = null
      const settle = this.pending.get(answer.id)
      if (!settle) continue
      this.pending.delete(answer.id)
      settle(answer)
    }
  }

  /** ripgrep's output for `args`, or null when ripgrep must run. */
  async run(args: string[], o: WarmSearchOptions): Promise<WarmSearchResult | null> {
    if (!this.available || o.signal?.aborted) return null
    this.child ??= this.start()
    const child = this.child
    const id = this.nextId++
    const answer = await new Promise<Answer | null>((resolve) => {
      this.pending.set(id, resolve)
      const onAbort = (): void => void child.stdin?.write(`${JSON.stringify({ id, cancel: true })}\n`)
      o.signal?.addEventListener('abort', onAbort, { once: true })
      // An answer settles the request; so does the end of the process.
      const request = { id, args, cwd: o.cwd, timeoutMs: o.timeoutMs, maxBytes: o.maxBytes, maxRecords: o.maxRecords }
      ;(child as unknown as { ref?: () => void }).ref?.()
      ;(child.stdout as unknown as { ref?: () => void }).ref?.()
      if (!child.stdin?.write(`${JSON.stringify(request)}\n`)) {
        /* backpressure only delays the request */
      }
    }).finally(() => {
      if (!this.pending.size && this.child === child) {
        ;(child as unknown as { unref?: () => void }).unref?.()
        ;(child.stdout as unknown as { unref?: () => void }).unref?.()
      }
    })
    if (!answer || answer.unsupported !== undefined || answer.code === undefined) return null
    return {
      stdout: answer.stdout ?? Buffer.alloc(0),
      stderr: answer.stderr ?? '',
      code: answer.code,
      truncated: answer.truncated === true,
      timedOut: answer.timedOut === true,
    }
  }

  /** Ends the process (host shutdown, tests). */
  dispose(): void {
    this.child?.kill()
    this.child = null
  }

  /** Test hook: the running process. */
  get process(): ChildProcess | null {
    return this.child
  }
}

let shared: WarmSearch | undefined

/** The host's search process client. */
export function warmSearch(): WarmSearch {
  shared ??= new WarmSearch(findWarmSearch())
  return shared
}

/** Test hook: replace the shared client. */
export function setWarmSearchForTests(client: WarmSearch | undefined): void {
  shared?.dispose()
  shared = client
}
