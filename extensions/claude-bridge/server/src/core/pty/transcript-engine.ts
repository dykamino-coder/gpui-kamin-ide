// Which engine wrote a Bridge transcript. A codex chat's transcript is Bridge's
// normalized projection, every line marked `engine: "codex"`; Claude CLI lines
// carry no such field. Dependency-free so resume helpers can ask cheaply.
import fs from 'fs'

export function transcriptEngine(file: string): 'codex' | 'claude' | null {
  let fd: number | undefined
  try {
    fd = fs.openSync(file, 'r')
    const buf = Buffer.alloc(64 * 1024)
    const n = fs.readSync(fd, buf, 0, buf.length, 0)
    const first = buf
      .subarray(0, n)
      .toString('utf8')
      .split('\n')
      .find((l) => l.trim())
    if (!first) return null
    try {
      return (JSON.parse(first) as { engine?: unknown }).engine === 'codex' ? 'codex' : 'claude'
    } catch {
      return /"engine"\s*:\s*"codex"/.test(first) ? 'codex' : 'claude'
    }
  } catch {
    return null
  } finally {
    if (fd !== undefined) fs.closeSync(fd)
  }
}
