// The context-usage bar must use each model's REAL window.
//
// THE BUG the user hit: contextLimitForModel keyed 1M solely on the `[1m]`
// substring. Fable 5's window is 1M *natively* (no `[1m]` suffix exists for it),
// so `claude-fable-5` fell through to 200K and the usage bar read "full" at a
// fifth of the real window. Sonnet 5 — also 1M-native, also in the picker and
// the default fallback chain — had the identical bug.
//
// The two deliberate distinctions must survive: bare `claude-opus-4-8` is the
// 200K Opus variant (the picker also offers `claude-opus-4-8[1m]` at 1M), and
// Haiku 4.5 is genuinely 200K.
//
// Run: from the kamin-ide repo root →  npx tsx <abs path to this file>

import { contextLimitForModel } from './session-cost.ts'

let failures = 0
function eq(model: string | undefined | null, expected: number, why: string): void {
  const got = contextLimitForModel(model)
  if (got !== expected) { failures++; console.error(`  ✗ ${why}: contextLimitForModel(${JSON.stringify(model)}) = ${String(got)}, expected ${String(expected)}`) }
}

const M = 1_000_000
const K200 = 200_000

console.log('1M-native models (the fix) — no [1m] suffix, still 1M')
eq('claude-fable-5', M, 'Fable 5 is 1M native (the reported bug)')
eq('claude-mythos-5', M, 'Mythos 5 shares Fable behaviour')
eq('claude-sonnet-5', M, 'Sonnet 5 is 1M native (same latent bug)')
eq('claude-fable-5-20260101', M, 'a date-suffixed Fable id still resolves to 1M')

console.log('Opus [1m] convention preserved')
eq('claude-opus-4-8[1m]', M, 'explicit 1M Opus variant')
eq('claude-opus-4-8', K200, 'bare Opus is the deliberate 200K variant — must NOT become 1M')
eq('opus', K200, 'the CLI short name for bare Opus stays 200K')

console.log('Genuinely-200K and empty inputs')
eq('claude-haiku-4-5', K200, 'Haiku 4.5 is really 200K')
eq('haiku', K200, 'Haiku short name')
eq('', K200, 'empty string falls back to the safe 200K default')
eq(null, K200, 'null falls back to 200K')
eq(undefined, K200, 'undefined falls back to 200K')

console.log('')
if (failures === 0) console.log('✅ ALL CONTEXT-LIMIT CHECKS PASSED')
else { console.error(`❌ ${String(failures)} check(s) FAILED`); process.exit(1) }
