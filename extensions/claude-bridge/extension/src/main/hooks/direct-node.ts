// A Bash hook that only starts node with literal arguments runs node directly,
// without a shell, when nothing observable would differ: `node "C:\x\hook.js"`
// is the common hook, and Git Bash costs ~50 ms of its ~125 ms on Windows.
// - The command must be provably simple (simpleNodeArgs): `node` or
//   `node.exe` and words bash passes unchanged, with no expansion, no
//   operator and nothing Git Bash rewrites as a path. Anything else stays on
//   the shell.
// - Used on Windows only (see executeHook).
// - The environment is not reconstructed: it is the one a node started by the
//   shell path saw, captured once per (shell, environment, directory) by
//   running a probe through that same path. Until the capture is there, the
//   hook runs on the shell as before.

/** Unquoted characters bash keeps as they are. */
const PLAIN = /^[A-Za-z0-9_.\/:@%+,=-]$/

/**
 * The arguments after `node` when `command` is `node` (or `node.exe`) followed
 * by literal words; null when the shell could make any difference.
 */
export function simpleNodeArgs(command: string, platform: NodeJS.Platform = process.platform): string[] | null {
  const src = command.trim()
  const words: string[] = []
  let i = 0
  while (i < src.length) {
    while (src[i] === ' ' || src[i] === '\t') i++
    if (i >= src.length) break
    let word = ''
    while (i < src.length && src[i] !== ' ' && src[i] !== '\t') {
      const c = src[i]!
      if (c === "'") {
        const end = src.indexOf("'", i + 1)
        if (end === -1) return null
        word += src.slice(i + 1, end)
        i = end + 1
      } else if (c === '"') {
        i++
        for (;;) {
          const d = src[i]
          if (d === undefined || d === '$' || d === '`' || d === '!' || d === '\n') return null
          if (d === '"') break
          // Inside double quotes a backslash escapes only $ ` " \ and a newline.
          if (d === '\\' && /[$`"\\\n]/.test(src[i + 1] ?? '')) return null
          word += d
          i++
        }
        i++
      } else if (c === '\\' || !PLAIN.test(c)) return null
      else {
        word += c
        i++
      }
    }
    words.push(word)
  }
  const [program, ...args] = words
  if (program !== 'node' && program !== 'node.exe') return null
  // The program word must have been unquoted; quoting it would not call the shim.
  if (!/^node(\.exe)?(\s|$)/.test(src)) return null
  return platform !== 'win32' || args.every(msysUnchanged) ? args : null
}

/** Whether Git Bash (MSYS) passes `arg` to a native program unchanged: it
 *  rewrites what looks like a POSIX path, an `x=/path` or a path list. */
function msysUnchanged(arg: string): boolean {
  if (/^[A-Za-z]:[\\/][^:;=]*$/.test(arg)) return true
  if (/[:;=]/.test(arg)) return false
  return !arg.startsWith('/') && !(arg.startsWith('-') && arg.includes('/')) && !arg.includes('//')
}


/** Prints the probe's environment as the hook would see it. */
export const ENV_PROBE_COMMAND = `node -e "process.stdout.write(JSON.stringify(process.env))"`

const CAPTURED_MAX = 16
const captured = new Map<string, NodeJS.ProcessEnv | 'unusable'>()
const pending = new Map<string, Promise<void>>()

/** The captured environment for `key`, or undefined when there is none yet. */
export function capturedEnv(key: string): NodeJS.ProcessEnv | undefined {
  const env = captured.get(key)
  return env === 'unusable' ? undefined : env
}

/** Captures the environment for `key` once, in the background. */
export function captureEnv(key: string, probe: () => Promise<{ stdout: string; exitCode: number; outcome: string }>): void {
  if (captured.has(key) || pending.has(key)) return
  const p = probe()
    .then((r) => {
      let env: NodeJS.ProcessEnv | 'unusable' = 'unusable'
      if (r.outcome === 'success' && r.exitCode === 0) {
        try {
          const parsed: unknown = JSON.parse(r.stdout)
          if (parsed && typeof parsed === 'object') env = parsed as NodeJS.ProcessEnv
        } catch {
          /* unusable */
        }
      }
      captured.set(key, env)
      if (captured.size > CAPTURED_MAX) captured.delete(captured.keys().next().value!)
    })
    .catch(() => void captured.set(key, 'unusable'))
    .finally(() => pending.delete(key))
  pending.set(key, p)
}

/** Test hook: waits for the captures in flight. */
export async function settleEnvCaptures(): Promise<void> {
  await Promise.all([...pending.values()])
}

/** Test hook: forgets the captured environments. */
export function resetEnvCaptures(): void {
  captured.clear()
}
