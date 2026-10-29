import { spawn } from 'node:child_process'
import { EventEmitter } from 'node:events'
import { createServer, type Server, type RequestListener } from 'node:http'
import type { AddressInfo } from 'node:net'
import { runInNewContext } from 'node:vm'
import { afterEach, describe, expect, it } from 'vitest'
import { buildHookCommandRelay, HOOK_RELAY_TOKEN_ENV } from './proxy-rewriter'

const servers: Server[] = []
afterEach(async () => {
  for (const server of servers.splice(0)) {
    server.closeAllConnections()
    await new Promise<void>((resolve) => server.close(() => resolve()))
  }
})

async function listen(handler: RequestListener): Promise<string> {
  const server = createServer(handler)
  servers.push(server)
  server.requestTimeout = 0
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  return `http://127.0.0.1:${(server.address() as AddressInfo).port}/hook?token=do-not-print`
}

function relay(url: string): Promise<{ stdout: string; stderr: string; code: number | null }> {
  return new Promise((resolve, reject) => {
    const child = spawn(
      process.execPath,
      [
        '-e',
        buildHookCommandRelay(url, {
          event: 'SessionStart',
          type: 'command',
          host: 'local',
        }),
      ],
      { env: { ...process.env, [HOOK_RELAY_TOKEN_ENV]: 'secret-do-not-print' } },
    )
    let stdout = '',
      stderr = ''
    child.stdout.on('data', (b) => {
      stdout += b
    })
    child.stderr.on('data', (b) => {
      stderr += b
    })
    child.on('error', reject)
    child.on('close', (code) => resolve({ stdout, stderr, code }))
    child.stdin.end('{"hook_event_name":"SessionStart","source":"compact"}')
  })
}

const success = JSON.stringify({ result: { stdout: 'done', stderr: '', exitCode: 0 } })

describe('hook relay transport', () => {
  it('reports connection refusal including the OS errno without exposing the URL', async () => {
    const url = await listen(() => {})
    const server = servers.pop()!
    await new Promise<void>((resolve) => server.close(() => resolve()))
    const result = await relay(url)
    expect(result.code).toBe(1)
    expect(result.stderr).toContain('code=ECONNREFUSED')
    expect(result.stderr).toContain('errno=')
    expect(result.stderr).not.toMatch(/do-not-print|127\.0\.0\.1|Bearer/)
  })

  it.each([
    ['secret-do-not-print', 'INVALID_RESPONSE'],
    ['{}', 'MISSING_RESULT'],
  ])('diagnoses invalid relay envelopes without echoing their body (%s)', async (body, code) => {
    const url = await listen((req, res) => {
      req.resume()
      res.end(body)
    })
    const result = await relay(url)
    expect(result.code).toBe(1)
    expect(result.stderr).toContain(`code=${code}`)
    expect(result.stderr).toContain('"host":"local"')
    expect(result.stderr).not.toContain('do-not-print')
  })

  it('reports a reset with context and does not repeat a potentially executed hook', async () => {
    let requests = 0
    const url = await listen((req) => {
      requests++
      req.socket.destroy()
    })
    const result = await relay(url)
    expect(result.code).toBe(1)
    expect(result.stderr).toContain('code=ECONNRESET')
    expect(result.stderr).toContain('"event":"SessionStart","type":"command","host":"local"')
    expect(result.stderr).not.toMatch(/do-not-print|127\.0\.0\.1|Bearer/)
    expect(requests).toBe(1)
  })

  it('settles a truncated response instead of waiting forever', async () => {
    const url = await listen((req, res) => {
      res.writeHead(200, { 'Content-Length': 1000 })
      res.write('{"result":')
      setImmediate(() => res.destroy())
    })
    const result = await relay(url)
    expect(result.code).toBe(1)
    expect(result.stderr).toContain('code=ECONNRESET')
  })

  it('does not echo an HTTP error body containing a credential or follow a redirect', async () => {
    let requests = 0
    const url = await listen((req, res) => {
      requests++
      res.writeHead(307, { Location: '/somewhere?token=do-not-print' })
      res.end('secret-do-not-print')
    })
    const result = await relay(url)
    expect(result.code).toBe(1)
    expect(result.stderr).toContain('code=HTTP_307')
    expect(result.stderr).not.toContain('do-not-print')
    expect(requests).toBe(1)
  })

  it('extracts nested undici cause codes and errno without printing error messages', async () => {
    const stdin = Object.assign(new EventEmitter(), { setEncoding() {} })
    let stderr = ''
    const cause = Object.assign(new Error('http://host/?token=secret'), { code: 'UND_ERR_SOCKET', errno: -104 })
    const error = new TypeError('fetch failed', { cause })
    const proc = {
      stdin,
      env: { [HOOK_RELAY_TOKEN_ENV]: 'secret' },
      stderr: {
        write(s: string) {
          stderr += s
        },
      },
      exitCode: 0,
    }
    runInNewContext(buildHookCommandRelay('http://127.0.0.1/hook'), {
      process: proc,
      require: () => ({
        request() {
          throw error
        },
      }),
    })
    stdin.emit('end')
    await new Promise((resolve) => setImmediate(resolve))
    expect(proc.exitCode).toBe(1)
    expect(stderr).toContain('code=UND_ERR_SOCKET errno=-104')
    expect(stderr).not.toMatch(/secret|http:|fetch failed/)
  })

  // Длинный gate запускается отдельно: реальный встроенный fetch Node 22
  // обрывал ожидание заголовков после 300 секунд при таймауте хука 600 секунд.
  it.skipIf(process.env.BRIDGE_LONG_HOOK_TEST !== '1')(
    'waits for a valid result beyond the fetch headers deadline',
    async () => {
      const url = await listen((req, res) => {
        req.resume()
        req.on('end', () => setTimeout(() => res.end(success), 310_000))
      })
      expect(await relay(url)).toEqual({ stdout: 'done', stderr: '', code: 0 })
    },
    330_000,
  )
})
