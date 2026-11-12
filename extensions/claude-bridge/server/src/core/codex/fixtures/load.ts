// Recorded app-server traces (sanitized output of the codex tool gate with
// BRIDGE_CODEX_TRACE, see test-support/codex-trace-sanitize.mjs). Each line is
// `{direction, message}`; `in` frames are what codex sent to Bridge.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

export interface TraceFrame {
  direction: 'in' | 'out'
  message: Record<string, any>
}

const here = path.dirname(fileURLToPath(import.meta.url))

export function loadTrace(name: string): TraceFrame[] {
  return fs
    .readFileSync(path.join(here, name), 'utf8')
    .split('\n')
    .filter(Boolean)
    .map((line) => JSON.parse(line) as TraceFrame)
}

/** Notifications codex sent, in order (no responses, no server requests). */
export function notifications(trace: TraceFrame[]): Array<{ method: string; params: any; emittedAtMs?: number }> {
  return trace
    .filter((f) => f.direction === 'in' && typeof f.message.method === 'string' && f.message.id === undefined)
    .map((f) => ({ method: f.message.method, params: f.message.params, emittedAtMs: f.message.emittedAtMs }))
}

/** Thread id of each gate session, keyed by the prompt its turn/start sent (`probe-a`, …). */
export function threadByPrompt(trace: TraceFrame[]): Record<string, string> {
  const out: Record<string, string> = {}
  for (const f of trace) {
    if (f.direction === 'out' && f.message.method === 'turn/start') {
      out[f.message.params.input[0].text] = f.message.params.threadId
    }
  }
  return out
}
export const GATE_TRACE = 'gate-parallel-0.160.1.jsonl'
