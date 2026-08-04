// The forward scans in activeTodos/activePlan overwrite their result and keep
// only the last hit, so a backward scan that stops at the first hit must give a
// byte-identical answer. "Must" is the whole point: this is a rewrite of code
// whose output is user-visible state, justified purely by cost.
//
// Run against a REAL transcript when one is available (argv[2]) — synthetic data
// misses the cases that matter: several TodoWrite blocks inside one entry, an
// empty todo list as the newest state, tool_use blocks from other tools between.
import { readFileSync, existsSync } from 'fs'

interface Block { type?: string; name?: string; input?: { todos?: unknown } }
interface Rec { type?: string; timestamp?: string; message?: { content?: Block[] } }

const short = (n: string) => n.replace(/^mcp__[^_]+__/, '')

function forward(es: Rec[]): { n: number; ts: string | undefined } {
  let last: unknown[] | null = null
  let ts: string | undefined
  for (const e of es) {
    if (e.type !== 'assistant') continue
    const c = e.message?.content
    if (!Array.isArray(c)) continue
    for (const b of c) {
      if (b?.type !== 'tool_use' || short(String(b.name ?? '')) !== 'TodoWrite') continue
      if (!Array.isArray(b.input?.todos)) continue
      last = b.input.todos as unknown[]
      ts = e.timestamp
    }
  }
  return { n: last?.length ?? -1, ts }
}

function backward(es: Rec[]): { n: number; ts: string | undefined } {
  for (let i = es.length - 1; i >= 0; i--) {
    const e = es[i]
    if (e.type !== 'assistant') continue
    const c = e.message?.content
    if (!Array.isArray(c)) continue
    for (let j = c.length - 1; j >= 0; j--) {
      const b = c[j]
      if (b?.type !== 'tool_use' || short(String(b.name ?? '')) !== 'TodoWrite') continue
      if (!Array.isArray(b.input?.todos)) continue
      return { n: (b.input.todos as unknown[]).length, ts: e.timestamp }
    }
  }
  return { n: -1, ts: undefined }
}

let failures = 0
const check = (label: string, es: Rec[]) => {
  const f = forward(es), b = backward(es)
  const ok = f.n === b.n && f.ts === b.ts
  if (!ok) { failures++; console.log(`  ❌ ${label}: forward=${JSON.stringify(f)} backward=${JSON.stringify(b)}`) }
  else console.log(`  ✅ ${label}  (n=${f.n})`)
}

const todo = (n: number): Block => ({ type: 'tool_use', name: 'TodoWrite', input: { todos: Array.from({ length: n }, () => ({})) } })
const asst = (ts: string, ...blocks: Block[]): Rec => ({ type: 'assistant', timestamp: ts, message: { content: blocks } })

check('no todos at all', [{ type: 'user' }, asst('t1', { type: 'text' })])
check('single write', [asst('t1', todo(3))])
check('later write wins', [asst('t1', todo(3)), asst('t2', todo(7))])
check('two writes in ONE entry — last block wins', [asst('t1', todo(3), todo(9))])
check('newest list is empty', [asst('t1', todo(4)), asst('t2', todo(0))])
check('other tools after the last write', [asst('t1', todo(5)), asst('t2', { type: 'tool_use', name: 'Read' })])
check('mcp-prefixed name', [asst('t1', { type: 'tool_use', name: 'mcp__x__TodoWrite', input: { todos: [{}, {}] } })])
check('malformed input ignored', [asst('t1', todo(2)), asst('t2', { type: 'tool_use', name: 'TodoWrite', input: {} })])

const real = process.argv[2]
if (real && existsSync(real)) {
  const es: Rec[] = []
  for (const line of readFileSync(real, 'utf8').split('\n')) {
    if (!line) continue
    try { es.push(JSON.parse(line) as Rec) } catch { /* torn line */ }
  }
  check(`REAL transcript (${es.length} records)`, es)
} else {
  console.log('  ⚠️  no real transcript given — synthetic cases only')
}

console.log(failures === 0
  ? '✅ ALL SCAN-DIRECTION CHECKS PASSED — backward+early-exit ≡ forward'
  : `❌ ${failures} MISMATCH(ES)`)
process.exit(failures === 0 ? 0 : 1)
