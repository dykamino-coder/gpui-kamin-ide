import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterAll, beforeEach, describe, expect, it } from 'vitest'
import { resolveHookShell } from '../utils/command-runtime'
import { disposeWarmShells } from '../mcp/tools/warm-shell'
import { executeHook, type HookExecuteRequest } from './executor'
import { resetEnvCaptures, settleEnvCaptures, simpleNodeArgs } from './direct-node'

describe('provably simple node hook commands', () => {
  it('takes node with literal words, quoted or not', () => {
    expect(simpleNodeArgs('node "C:\\Users\\me\\.pixel-agents\\hooks\\claude-hook.js"', 'win32')).toEqual(['C:\\Users\\me\\.pixel-agents\\hooks\\claude-hook.js'])
    expect(simpleNodeArgs("node.exe 'a b' c-d C:/x/y.js", 'win32')).toEqual(['a b', 'c-d', 'C:/x/y.js'])
    expect(simpleNodeArgs('  node  hook.js  ', 'win32')).toEqual(['hook.js'])
    expect(simpleNodeArgs('node /opt/hook.js --flag', 'linux')).toEqual(['/opt/hook.js', '--flag'])
  })

  it('leaves to the shell whatever the shell could change', () => {
    for (const c of [
      'node "$HOME/x.js"', 'node x.js | tee y', 'node x.js && true', 'node x.js > out', 'node ~/x.js', 'node *.js', 'node {a,b}.js',
      'node C:\\x.js', 'node "a\\"b"', 'node `x`', 'node $(x)', 'node x.js # c', 'node "x', "node 'x", '"node" x.js', 'A=1 node x.js',
      'nodejs x.js', 'eslint --fix x', 'node x.js; echo', 'node x.js &', 'node "!x"', 'node x\ny',
    ]) {
      expect(simpleNodeArgs(c, 'linux'), c).toBeNull()
    }
    // Git Bash rewrites what looks like a POSIX path or a path list.
    for (const c of ['node /c/x.js', 'node --require=/x.js y', 'node -r/x y', 'node a:b', 'node a;b', 'node x=1', 'node //srv/x']) {
      expect(simpleNodeArgs(c, 'win32'), c).toBeNull()
    }
  })

})

// Behavior equality against the shell path: the same request run through
// bash and directly must give the hook the same arguments, directory,
// environment and stdin, and report the same stdout, stderr, exit code and
// outcome, including on a timeout.
const tmp = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'hook-direct-')))
const script = path.join(tmp, 'probe.js')
fs.writeFileSync(
  script,
  `let input = ''
process.stdin.on('data', (d) => (input += d))
process.stdin.on('end', () => {
  const [mode, ...rest] = process.argv.slice(2)
  if (mode === 'sleep') return setTimeout(() => {}, 60000)
  const { ppid, ...shown } = { ppid: process.ppid, argv: rest, cwd: process.cwd(), env: process.env, stdin: JSON.parse(input) }
  process.stdout.write(JSON.stringify({ ...shown, ppidIsHost: ppid === Number(process.env.HOST_PID) }))
  process.stderr.write('err ' + rest.length + '\\n')
  process.exit(Number(mode))
})
`,
)
afterAll(() => {
  if (savedDirect === undefined) delete process.env.KAMIN_HOOK_DIRECT_NODE
  else process.env.KAMIN_HOOK_DIRECT_NODE = savedDirect
  disposeWarmShells()
  try {
    // A timed-out hook may still be ending on Windows.
    fs.rmSync(tmp, { recursive: true, force: true, maxRetries: 20, retryDelay: 250 })
  } catch {
    /* left in the temp directory */
  }
})
// The direct path is used on Windows; elsewhere it is forced on for this test.
const savedDirect = process.env.KAMIN_HOOK_DIRECT_NODE
process.env.KAMIN_HOOK_DIRECT_NODE = '1'
beforeEach(() => resetEnvCaptures())

let bashAvailable = true
try {
  await resolveHookShell('bash')
} catch {
  bashAvailable = false
}

const request = (command: string, timeoutMs = 20_000): HookExecuteRequest => ({
  type: 'hook:execute',
  requestId: 'r',
  sessionId: 's',
  hookId: 'h',
  event: 'PreToolUse',
  command,
  shell: 'bash',
  cwd: tmp,
  env: { CLAUDE_BRIDGE_HOOK: '1', HOST_PID: String(process.pid) },
  payload: { tool_name: 'Read', tool_input: { file_path: 'ü.txt' } },
  timeoutMs,
})

const shown = (r: Awaited<ReturnType<typeof executeHook>>) => ({ stdout: r.stdout, stderr: r.stderr, exitCode: r.exitCode, outcome: r.outcome })

/** The shell path started cold and warm (a shell waiting from the previous
 *  run), and the direct path. */
async function both(command: string, timeoutMs?: number) {
  disposeWarmShells()
  const viaShell = await executeHook(request(command, timeoutMs), undefined, { shellOnly: true })
  const viaWarmShell = await executeHook(request(command, timeoutMs), undefined, { shellOnly: true })
  // The first run of a simple command captures its environment; the next runs directly.
  await executeHook(request(command, timeoutMs))
  await settleEnvCaptures()
  const direct = await executeHook(request(command, timeoutMs))
  return { viaShell, viaWarmShell, direct }
}

describe.skipIf(!bashAvailable)('a simple node hook run directly behaves as through bash', () => {
  const quoted = process.platform === 'win32' ? `"${script}"` : `'${script}'`
  for (const [name, command] of [
    ['a quoted script path and words', `node ${quoted} 0 a b-c x.y`],
    ['an exit code', `node ${quoted} 3`],
    ['node.exe and a quoted word with a space', `node.exe ${quoted} 0 'two words'`],
  ] as const) {
    it(name, async () => {
      const { viaShell, viaWarmShell, direct } = await both(command)
      const parsed = (r: typeof direct) => JSON.parse(r.stdout)
      expect(parsed(viaShell).ppidIsHost).toBe(false)
      expect(parsed(viaWarmShell).ppidIsHost).toBe(false)
      expect(parsed(direct).ppidIsHost).toBe(true)
      const comparable = (r: typeof direct) => ({ ...shown(r), stdout: { ...parsed(r), ppidIsHost: null } })
      expect(comparable(viaWarmShell)).toEqual(comparable(viaShell))
      expect(comparable(direct)).toEqual(comparable(viaShell))
    }, 60_000)
  }

  it('times out the same way', async () => {
    const { viaShell, viaWarmShell, direct } = await both(`node ${quoted} sleep`, 1500)
    expect(shown(viaWarmShell)).toEqual(shown(viaShell))
    expect(shown(direct)).toEqual(shown(viaShell))
    expect(direct.outcome).toBe('timeout')
  }, 60_000)

})
