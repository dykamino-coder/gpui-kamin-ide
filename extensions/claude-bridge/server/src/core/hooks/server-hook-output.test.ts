import 'reflect-metadata'
import { afterEach, expect, it } from 'vitest'
import { dispatchHook } from './dispatcher'
import { clearSession, listSession, registerSessionHooks } from './registry'

// Server-host hooks run inside the Linux container through `bash -c`.
const SESSION = 'server-hook-output'
afterEach(() => clearSession(SESSION))

function run(command: string) {
  registerSessionHooks(
    SESSION,
    { PreToolUse: [{ matcher: 'Read', hooks: [{ type: 'command', command, host: 'server' }] }] },
    { kind: 'user' },
  )
  const hook = listSession(SESSION).at(-1)!
  expect(hook.effectiveHost).toBe('server')
  return dispatchHook(SESSION, hook.id, { hook_event_name: 'PreToolUse' } as never, {} as never)
}

it.skipIf(process.platform === 'win32')(
  'decodes characters split between chunks and reports a cut stdout',
  async () => {
    const split = await run(
      `node -e "const z=Buffer.from(JSON.stringify({r:'Ж'}),'utf8');process.stdout.write(z.subarray(0,7));setTimeout(()=>process.stdout.write(z.subarray(7)),50)"`,
    )
    expect(split.stdout).toBe('{"r":"Ж"}')
    expect(split.jsonOutput).toEqual({ r: 'Ж' })

    clearSession(SESSION)
    const big = await run(`node -e "process.stdout.write(JSON.stringify({a:'x'.repeat(1100000)}))"`)
    expect(big.stdout.length).toBe(1024 * 1024)
    expect(big.jsonOutput).toBeUndefined()
    expect(big.stderr).toMatch(/hook stdout exceeded 1048576 characters; \d+ were dropped/)
  },
  30_000,
)
