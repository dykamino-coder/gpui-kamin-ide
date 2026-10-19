// Live model / effort switches in a running CLI without a console prompt.
//
// Since 2.1.251 Claude Code asks before a switch that would re-read a warm
// conversation cache:
//   "Switch model? … This conversation is cached for the current model."
//   "Change effort level? … cached for the current effort level."
// Bridge drives the CLI from the chat UI, so that dialog surfaced only in the
// Console panel and blocked the switch until someone answered it there.
//
// Model: the official way out is a `PreModelSwitch` hook (2.1.251,
// https://code.claude.com/docs/en/hooks) answering
// `permissionDecision: "allow"`; the CLI then applies `/model <id>` with
// `skipConfirm` and never renders the dialog (verified on 2.1.296). A user or
// plugin hook that answers `ask`/`deny` still wins — the CLI shows ITS reason,
// which the responder below deliberately does not match.
//
// Effort: there is no hook, setting or `/effort` argument that skips the
// dialog in 2.1.296 (the gate only checks whether the cache miss was already
// acknowledged at the current output-token count). The responder therefore
// confirms the cache-warning dialog — and only that dialog, for the level the
// user picked — when it actually appears in the PTY output after Bridge sent
// `/effort <level>`. Without the dialog (cold cache, first turn) nothing is
// written.

import { debugLog } from '../logging'
import { writeInputToSession } from './session-io'
import type { PtySession } from '../types/pty'

/** Levels the CLI's `/effort` accepts (2.1.111+: xhigh, max). */
export const EFFORT_LEVELS = ['low', 'medium', 'high', 'xhigh', 'max'] as const
export type EffortLevel = (typeof EFFORT_LEVELS)[number]

export function isEffortLevel(value: unknown): value is EffortLevel {
  return typeof value === 'string' && (EFFORT_LEVELS as readonly string[]).includes(value)
}

/** A model id or alias that is safe to type into the CLI prompt: no control
 *  characters or spaces, so it can never submit a second line. Covers
 *  `claude-sonnet-4-6`, `opus[1m]`, `us.anthropic.…-v1:0`, gateway ids. */
export function isSafeModelId(value: unknown): value is string {
  return typeof value === 'string' && /^[A-Za-z0-9][A-Za-z0-9._:@/[\]-]{0,159}$/.test(value)
}

/** Ctrl+U clears whatever is on the CLI input line before the command. */
export function effortCommand(level: EffortLevel): string {
  return `\x15/effort ${level}\r`
}

export function modelCommand(model: string): string {
  return `\x15/model ${model}\r`
}

/** Bridge-owned PreModelSwitch hook: consumes stdin and allows the switch.
 *  Runs inside the container (plain command, not relayed), so it needs no
 *  network, credential or client round-trip. */
export const PRE_MODEL_SWITCH_ALLOW_OUTPUT = JSON.stringify({
  hookSpecificOutput: {
    hookEventName: 'PreModelSwitch',
    permissionDecision: 'allow',
    permissionDecisionReason: 'Model switch requested from the Bridge chat',
  },
})

export function preModelSwitchAllowMatcher(): { hooks: Array<{ type: 'command'; command: string; timeout: number }> } {
  return {
    hooks: [
      {
        type: 'command',
        command: `cat >/dev/null; printf '%s\\n' '${PRE_MODEL_SWITCH_ALLOW_OUTPUT}'`,
        timeout: 10,
      },
    ],
  }
}

// ---------------------------------------------------------------------------
// Cache-warning dialog responder
// ---------------------------------------------------------------------------

const ANSI = /\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[@-Z\\-_]/g

/** Strip escape sequences and ALL whitespace: the full-screen renderer moves
 *  the cursor instead of printing spaces, so words arrive glued together. */
export function normalizeScreenText(raw: string): string {
  return raw.replace(ANSI, '').replace(/\s+/g, '').toLowerCase()
}

export type SwitchKind = 'model' | 'effort'

/** True only for the CLI's own cache-warning dialog for this switch. A
 *  PreModelSwitch hook's `ask` renders a different body and never matches. */
export function isCacheWarningDialog(normalized: string, kind: SwitchKind, target?: string): boolean {
  if (kind === 'effort') {
    if (!normalized.includes('changeeffortlevel?')) return false
    if (!normalized.includes('cachedforthecurrenteffortlevel')) return false
    return target ? normalized.includes(`yes,switchto${target.toLowerCase()}`) : true
  }
  return normalized.includes('switchmodel?') && normalized.includes('cachedforthecurrentmodel')
}

/** Long enough for a command queued behind a running turn to surface. */
export const SWITCH_CONFIRM_TTL_MS = 15 * 60_000
const RAW_TAIL = 16_384

const armed = new WeakMap<PtySession, () => void>()

/**
 * Watch the PTY for the cache-warning dialog of one switch and confirm it
 * once (Enter on the default "Yes, switch to …" row). Replaces a previously
 * armed responder for the same session. Returns the disposer.
 */
export function armSwitchConfirm(
  session: PtySession,
  kind: SwitchKind,
  target: string | undefined,
  opts: { ttlMs?: number; confirmDelayMs?: number; write?: (data: string) => boolean } = {},
): () => void {
  armed.get(session)?.()
  const write = opts.write ?? ((data: string) => writeInputToSession(session, data))
  let tail = ''
  let done = false
  let subscription: { dispose(): void } | null = null
  const dispose = (): void => {
    if (done) return
    done = true
    clearTimeout(ttl)
    subscription?.dispose()
    if (armed.get(session) === dispose) armed.delete(session)
  }
  const ttl = setTimeout(dispose, opts.ttlMs ?? SWITCH_CONFIRM_TTL_MS)
  subscription = session.pty.onData((data: string) => {
    if (done) return
    tail = (tail + data).slice(-RAW_TAIL)
    if (!isCacheWarningDialog(normalizeScreenText(tail), kind, target)) return
    dispose()
    debugLog('Confirming CLI cache-warning dialog for a Bridge switch', { sessionId: session.id, kind, target })
    setTimeout(() => {
      if (session.state === 'running') write('\r')
    }, opts.confirmDelayMs ?? 150)
  })
  armed.set(session, dispose)
  return dispose
}
