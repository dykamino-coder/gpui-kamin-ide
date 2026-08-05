// Append-only audit log of every hook execution. Format = JSONL — one
// HookExecutionRecord per line. Lives at data/hooks/<yyyy-mm-dd>.jsonl,
// rotated daily. The UI Log tab tails this. A future phase can layer
// tamper-evidence on top (prev_hash chain).

import fsp from 'fs/promises'
import path from 'path'
import os from 'os'
import type { HookExecutionResult, HookEvent, HookSource } from './types'

const LOG_DIR = path.join(os.homedir(), '.claude', 'open-claude-bridge', 'data', 'hooks')

export interface HookExecutionRecord {
  ts: number                    // unix-ms
  tokenId?: string              // API-token owner; legacy rows may omit it
  sessionId: string
  hookId: string
  event: HookEvent | string
  matcher?: string
  source: HookSource
  effectiveHost: 'local' | 'server' | 'http'
  outcome: HookExecutionResult['outcome']
  exitCode: number
  durationMs: number
  stdoutPreview?: string        // first 256 chars
  stderrPreview?: string
}

function todayFile(): string {
  const d = new Date()
  const stamp = `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, '0')}-${String(d.getUTCDate()).padStart(2, '0')}`
  return path.join(LOG_DIR, `${stamp}.jsonl`)
}

// Serialized async write chain: dispatch fires appendExecution per hook and must
// NOT block on disk (the old appendFileSync stalled the event loop on every tool
// call). Fire-and-forget onto this queue keeps writes async AND in order — two
// rapid hooks can't interleave or reorder their lines.
let writeChain: Promise<void> = Promise.resolve()

export function appendExecution(rec: HookExecutionRecord): void {
  const line = JSON.stringify(rec) + '\n'
  writeChain = writeChain.then(async () => {
    try {
      await fsp.mkdir(LOG_DIR, { recursive: true })
      await fsp.appendFile(todayFile(), line, 'utf-8')
    } catch { /* swallow — audit log must never crash dispatch */ }
  })
}

/** Read recent executions (newest first). Across multiple daily files. */
export async function readRecentExecutions(limit = 200, tokenId?: string): Promise<HookExecutionRecord[]> {
  try {
    const files = (await fsp.readdir(LOG_DIR))
      .filter(f => f.endsWith('.jsonl'))
      .sort()
      .reverse()
    const out: HookExecutionRecord[] = []
    for (const f of files) {
      const content = await fsp.readFile(path.join(LOG_DIR, f), 'utf-8')
      const lines = content.split('\n').filter(Boolean).reverse()
      for (const line of lines) {
        try {
          const record = JSON.parse(line) as HookExecutionRecord
          if (!tokenId || record.tokenId === tokenId) out.push(record)
        } catch { /* skip corrupt */ }
        if (out.length >= limit) return out
      }
    }
    return out
  } catch {
    return [] // missing dir / unreadable → no history
  }
}

export async function clearLog(tokenId: string): Promise<void> {
  writeChain = writeChain.then(async () => {
    try {
      for (const f of await fsp.readdir(LOG_DIR)) {
        if (!f.endsWith('.jsonl')) continue
        const filePath = path.join(LOG_DIR, f)
        const content = await fsp.readFile(filePath, 'utf-8')
        const kept = content.split('\n').filter(Boolean).filter((line) => {
          try { return (JSON.parse(line) as HookExecutionRecord).tokenId !== tokenId } catch { return true }
        })
        if (kept.length === 0) await fsp.unlink(filePath)
        else await fsp.writeFile(filePath, `${kept.join('\n')}\n`, 'utf-8')
      }
    } catch { /* ignore — nothing to clear */ }
  })
  await writeChain
}
