// A server that connects and immediately dies must NOT get an infinite retry
// budget.
//
// THE BUG: `reconnectAttempts = 0` ran on every successful CONNECT. unity-mcp
// connects fine — then its named pipe isn't there, stderr floods, the circuit
// breaker kills the child. Reconnect: attempts=0 → delay 2s → connects again →
// counter reset again. The exponential backoff never escalated and MAX_ATTEMPTS
// (10) was never reached, so it respawned a process every 2 seconds for the life
// of the app. It showed up as kaminhost.exe pinned at ~150% of a core in a
// freeze report, and as 10 breaker trips / 10 respawns in host.log.
//
// The fix: connecting proves the transport came up, not that the server works.
// Only a connection that LASTED (>= STABLE_CONNECTION_MS) refills the budget.
//
// Run: from the kamin-ide repo root →  npx tsx <abs path to this file>

const STABLE_CONNECTION_MS = 60_000
const MAX_ATTEMPTS = 10

let failures = 0
function assert(cond: boolean, msg: string): void {
  if (!cond) { failures++; console.error('  ✗ ' + msg) }
}

interface State { reconnectAttempts?: number; connectedAt?: number }

/** The two lines under test, lifted verbatim in behaviour from manager.ts. */
function onConnected(s: State, now: number): void {
  s.connectedAt = now
}
/** Returns the delay, or null when the budget is exhausted (loop stops). */
function onDisconnect(s: State, now: number): number | null {
  const uptime = s.connectedAt !== undefined ? now - s.connectedAt : 0
  if (uptime >= STABLE_CONNECTION_MS) s.reconnectAttempts = 0
  s.connectedAt = undefined
  const attempts = s.reconnectAttempts ?? 0
  if (attempts >= MAX_ATTEMPTS) return null
  s.reconnectAttempts = attempts + 1
  return Math.min(2000 * Math.pow(2, attempts), 60_000)
}

console.log('Scenario 1: connect-then-die-instantly must exhaust the budget, not loop forever')
{
  const s: State = {}
  let now = 0
  let cycles = 0
  let delay: number | null = 0
  // Each cycle: connect, live 500ms, die. The real loop never ended.
  while (cycles < 1000) {
    onConnected(s, now)
    now += 500 // dies almost immediately — this is the failing server
    delay = onDisconnect(s, now)
    if (delay === null) break
    now += delay
    cycles++
  }
  assert(delay === null, 'S1 the budget is exhausted and the loop STOPS')
  assert(cycles === MAX_ATTEMPTS, `S1 it stops after exactly ${String(MAX_ATTEMPTS)} attempts (got ${String(cycles)})`)
  // The whole point of backoff: the last waits are long, not 2s forever.
  assert(now > 5 * 60_000, `S1 backoff actually escalates — >5min of total wait (got ${String(Math.round(now / 1000))}s)`)
}

console.log('Scenario 2: a connection that PROVED itself still gets a full budget back')
{
  const s: State = {}
  // Burn most of the budget on early failures.
  for (let i = 0; i < 5; i++) { onConnected(s, 0); onDisconnect(s, 100) }
  assert((s.reconnectAttempts ?? 0) === 5, 'S2 five failures counted')
  // Now a healthy run: connected, served for an hour, then crashed.
  onConnected(s, 0)
  const delay = onDisconnect(s, 60 * 60_000)
  assert((s.reconnectAttempts ?? 0) === 1, 'S2 a long healthy run refills the budget')
  assert(delay === 2000, `S2 and the next retry is prompt again (got ${String(delay)})`)
}

console.log('Scenario 3: the boundary is uptime, not the fact of connecting')
{
  const justUnder: State = {}
  onConnected(justUnder, 0)
  onDisconnect(justUnder, STABLE_CONNECTION_MS - 1)
  onConnected(justUnder, 0)
  onDisconnect(justUnder, STABLE_CONNECTION_MS - 1)
  assert((justUnder.reconnectAttempts ?? 0) === 2, 'S3 just-under-threshold uptimes accumulate (no reset)')

  const justOver: State = {}
  onConnected(justOver, 0)
  onDisconnect(justOver, STABLE_CONNECTION_MS - 1)
  onConnected(justOver, 0)
  onDisconnect(justOver, STABLE_CONNECTION_MS)
  assert((justOver.reconnectAttempts ?? 0) === 1, 'S3 at the threshold the budget resets')
}

console.log('')
if (failures === 0) console.log('✅ ALL RECONNECT-BUDGET CHECKS PASSED')
else { console.error(`❌ ${failures} check(s) FAILED`); process.exit(1) }
