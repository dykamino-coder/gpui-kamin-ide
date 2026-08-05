import type { PtySession } from './types'

const INPUT_CLEAR_SETTLE_MS = 50
const INPUT_ECHO_QUIET_MS = 80
const INPUT_POST_ENTER_SETTLE_MS = 80
const INPUT_HARD_MAX_MS = 2000
const MAINTENANCE_INPUT_QUIET_MS = 300

interface QueuedSubmission {
  kind: 'user' | 'maintenance'
  text: string
  maintenanceKey?: string
  maintenanceRevision?: number
}

interface ActiveSubmission extends QueuedSubmission {
  clearTimer: ReturnType<typeof setTimeout> | null
  quietTimer: ReturnType<typeof setTimeout> | null
  hardTimer: ReturnType<typeof setTimeout> | null
  postEnterTimer: ReturnType<typeof setTimeout> | null
  outputSubscription: { dispose(): void } | null
  fired: boolean
}

interface PendingMaintenance {
  text: string
  revision: number
}

interface SessionInputState {
  promptReady: boolean
  rawInputDirty: boolean
  rawPasteMode: boolean
  lastRawInputAt: number
  queue: QueuedSubmission[]
  active: ActiveSubmission | null
  bufferedRawInput: string[]
  pendingMaintenance: Map<string, PendingMaintenance>
  maintenanceRevision: number
  maintenanceTimer: ReturnType<typeof setTimeout> | null
}

const sessionInputStates = new WeakMap<PtySession, SessionInputState>()

function inputState(session: PtySession): SessionInputState {
  let state = sessionInputStates.get(session)
  if (!state) {
    state = {
      promptReady: false,
      rawInputDirty: false,
      rawPasteMode: false,
      lastRawInputAt: 0,
      queue: [],
      active: null,
      bufferedRawInput: [],
      pendingMaintenance: new Map(),
      maintenanceRevision: 0,
      maintenanceTimer: null,
    }
    sessionInputStates.set(session, state)
  }
  return state
}

function cancelMaintenanceTimer(state: SessionInputState): void {
  if (!state.maintenanceTimer) return
  clearTimeout(state.maintenanceTimer)
  state.maintenanceTimer = null
}

function canRunMaintenance(session: PtySession, state: SessionInputState): boolean {
  return session.state === 'running'
    && !session.detachedAt
    && state.promptReady
    && !state.rawInputDirty
    && !state.active
    && state.queue.length === 0
    && state.pendingMaintenance.size > 0
}

function schedulePendingMaintenance(session: PtySession, state = inputState(session)): void {
  if (!canRunMaintenance(session, state)) {
    cancelMaintenanceTimer(state)
    return
  }
  if (state.maintenanceTimer) return

  // Older clients send raw Ctrl+U immediately before session:submitText. Keep
  // this short reservation window so maintenance cannot take that prompt.
  const delay = Math.max(0, state.lastRawInputAt + MAINTENANCE_INPUT_QUIET_MS - Date.now())
  state.maintenanceTimer = setTimeout(() => {
    state.maintenanceTimer = null
    if (!canRunMaintenance(session, state)) return
    const first = state.pendingMaintenance.entries().next().value as [string, PendingMaintenance] | undefined
    if (!first) return
    const [maintenanceKey, pending] = first
    startSubmission(session, state, {
      kind: 'maintenance',
      text: pending.text,
      maintenanceKey,
      maintenanceRevision: pending.revision,
    })
  }, delay)
}

function updateRawInputState(state: SessionInputState, data: string): void {
  const pasteStart = '\x1b[200~'
  const pasteEnd = '\x1b[201~'
  for (let i = 0; i < data.length;) {
    if (data.startsWith(pasteStart, i)) {
      state.rawPasteMode = true
      i += pasteStart.length
      continue
    }
    if (data.startsWith(pasteEnd, i)) {
      state.rawPasteMode = false
      i += pasteEnd.length
      continue
    }

    const ch = data[i]!
    i++
    if (state.rawPasteMode) {
      state.rawInputDirty = true
      continue
    }
    if (ch === '\x15' || ch === '\x03') {
      state.rawInputDirty = false
      continue
    }
    if (ch === '\r' || ch === '\n') {
      state.rawInputDirty = false
      state.promptReady = false
      continue
    }
    // Navigation/control sequences do not prove that the line contains text.
    // Backspace stays conservative because the server cannot count graphemes.
    if (ch >= ' ' && ch !== '\x7f') state.rawInputDirty = true
  }
}

function writeRawNow(session: PtySession, state: SessionInputState, data: string): void {
  if (session.state !== 'running') return
  cancelMaintenanceTimer(state)
  session.pty.write(data)
  state.lastRawInputAt = Date.now()
  updateRawInputState(state, data)
  schedulePendingMaintenance(session, state)
}

function disposeActiveTimers(active: ActiveSubmission): void {
  if (active.clearTimer) clearTimeout(active.clearTimer)
  if (active.quietTimer) clearTimeout(active.quietTimer)
  if (active.hardTimer) clearTimeout(active.hardTimer)
  if (active.postEnterTimer) clearTimeout(active.postEnterTimer)
  try { active.outputSubscription?.dispose() } catch { /* noop */ }
  active.clearTimer = null
  active.quietTimer = null
  active.hardTimer = null
  active.postEnterTimer = null
  active.outputSubscription = null
}

function finishSubmission(
  session: PtySession,
  state: SessionInputState,
  active: ActiveSubmission,
  submitted: boolean,
): void {
  if (state.active !== active) return
  disposeActiveTimers(active)
  state.active = null

  if (submitted && active.kind === 'maintenance' && active.maintenanceKey) {
    const pending = state.pendingMaintenance.get(active.maintenanceKey)
    // Only acknowledge the revision that actually reached Enter. A newer sync
    // may have arrived while the previous reload was being rendered.
    if (pending?.revision === active.maintenanceRevision) {
      state.pendingMaintenance.delete(active.maintenanceKey)
    }
  }

  const buffered = state.bufferedRawInput.splice(0)
  for (const data of buffered) writeRawNow(session, state, data)

  const next = state.queue.shift()
  if (next && session.state === 'running') startSubmission(session, state, next)
  else schedulePendingMaintenance(session, state)
}

function startSubmission(session: PtySession, state: SessionInputState, queued: QueuedSubmission): void {
  if (session.state !== 'running') return
  cancelMaintenanceTimer(state)

  const active: ActiveSubmission = {
    ...queued,
    clearTimer: null,
    quietTimer: null,
    hardTimer: null,
    postEnterTimer: null,
    outputSubscription: null,
    fired: false,
  }
  state.active = active
  state.rawInputDirty = false

  // Clear, paste and Enter are coordinator-owned. No raw or programmatic input
  // can interleave while this transaction is active.
  try {
    session.pty.write('\x15')
  } catch {
    finishSubmission(session, state, active, false)
    return
  }

  active.clearTimer = setTimeout(() => {
    active.clearTimer = null
    if (state.active !== active || session.state !== 'running') {
      finishSubmission(session, state, active, false)
      return
    }

    const body = queued.text.replace(/\r\n?/g, '\n')
    const fireEnter = () => {
      if (active.fired || state.active !== active) return
      active.fired = true
      disposeActiveTimers(active)
      let submitted = false
      if (session.state === 'running') {
        try {
          session.pty.write('\r')
          state.promptReady = false
          submitted = true
        } catch { /* PTY exited between state check and write */ }
      }
      if (!submitted) finishSubmission(session, state, active, false)
      else {
        // Starting another Ctrl+U in the same tick can clear the command before
        // Ink consumes Enter, so retain the transaction for one short settle.
        active.postEnterTimer = setTimeout(
          () => finishSubmission(session, state, active, true),
          INPUT_POST_ENTER_SETTLE_MS,
        )
      }
    }

    active.outputSubscription = session.pty.onData(() => {
      if (active.fired || state.active !== active) return
      if (active.quietTimer) clearTimeout(active.quietTimer)
      active.quietTimer = setTimeout(fireEnter, INPUT_ECHO_QUIET_MS)
    })
    active.hardTimer = setTimeout(fireEnter, INPUT_HARD_MAX_MS)
    try {
      session.pty.write(`\x1b[200~${body}\x1b[201~`)
    } catch {
      finishSubmission(session, state, active, false)
    }
  }, INPUT_CLEAR_SETTLE_MS)
}

function cancelActiveSubmission(state: SessionInputState): void {
  const active = state.active
  if (active) disposeActiveTimers(active)
  state.active = null
  state.queue = []
  state.bufferedRawInput = []
}

export function writeCoordinatedInput(session: PtySession, data: string): void {
  if (session.state !== 'running') return
  const state = inputState(session)
  if (data.includes('\x03')) {
    cancelActiveSubmission(state)
    writeRawNow(session, state, data)
  } else if (state.active) {
    state.bufferedRawInput.push(data)
  } else {
    writeRawNow(session, state, data)
  }
}

export function submitCoordinatedText(session: PtySession, text: string): void {
  if (session.state !== 'running') return
  const state = inputState(session)
  cancelMaintenanceTimer(state)
  const queued: QueuedSubmission = { kind: 'user', text }
  if (state.active) state.queue.push(queued)
  else startSubmission(session, state, queued)
}

export function setSessionPromptReady(session: PtySession, promptReady: boolean): void {
  const state = inputState(session)
  state.promptReady = promptReady
  schedulePendingMaintenance(session, state)
}

export function notifySessionAttachmentChanged(session: PtySession): void {
  schedulePendingMaintenance(session)
}

export function requestMaintenanceSubmission(session: PtySession, key: string, text: string): void {
  const state = inputState(session)
  state.maintenanceRevision++
  state.pendingMaintenance.set(key, { text, revision: state.maintenanceRevision })
  schedulePendingMaintenance(session, state)
}

export function getSessionInputSnapshot(session: PtySession): {
  promptReady: boolean
  rawInputDirty: boolean
  activeKind: 'user' | 'maintenance' | null
  queuedSubmissions: number
  pendingMaintenance: string[]
} {
  const state = inputState(session)
  return {
    promptReady: state.promptReady,
    rawInputDirty: state.rawInputDirty,
    activeKind: state.active?.kind ?? null,
    queuedSubmissions: state.queue.length,
    pendingMaintenance: [...state.pendingMaintenance.keys()],
  }
}

export function clearSessionInputState(session: PtySession): void {
  const state = sessionInputStates.get(session)
  if (!state) return
  cancelMaintenanceTimer(state)
  if (state.active) disposeActiveTimers(state.active)
  sessionInputStates.delete(session)
}
