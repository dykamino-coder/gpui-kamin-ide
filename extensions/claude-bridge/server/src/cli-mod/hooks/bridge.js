// Runs inside Claude Code's mod sandbox: all effects go through the engine.
/** @typedef {{id: number, rejected?: boolean}} Ack */
/** @typedef {{turnId: string | null}} Turn */
/** @typedef {{turnId: string | null, active: boolean, polling: boolean, socketPath: string, session: string, instance: string, version: string, acks: Ack[], turns: Turn[], reads: {submissionId: string, uuid: string}[], submitting: (string | null)[], composer: {submissionId: string | null}[], armed: string | null}} State */

/** @param {import('claude-code').EngineInterface} $ @param {State} state @param {string} route */
async function exchange($, state, route) {
  const acks = state.acks.splice(0)
  const turns = state.turns.splice(0)
  const reads = state.reads.splice(0)
  try {
    const response = await $.http.fetch(`http://bridge${route}`, {
      socketPath: state.socketPath,
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        protocol: 1,
        session: state.session,
        instance: state.instance,
        version: state.version,
        turnId: state.turnId,
        acks,
        turns,
        reads,
      }),
    })
    if (!response.ok) throw new Error('Bridge transport unavailable')
    return JSON.parse(response.text)
  } catch (error) {
    state.acks.unshift(...acks)
    state.turns.unshift(...turns)
    state.reads.unshift(...reads)
    if (state.acks.length + state.turns.length + state.reads.length > 512) state.active = false
    throw error
  }
}

/** @param {import('claude-code').EngineInterface} $ @param {State} state */
async function poll($, state) {
  if (!state.active || state.polling) return
  state.polling = true
  try {
    const { commands } = await exchange($, state, '/poll')
    for (const command of commands) {
      if (command.kind === 'submit' && typeof command.text === 'string') {
        state.submitting.push(command.submissionId ?? null)
        // Resolves at turn start. Never hold up heartbeat or abort behind it.
        void $.prompt.submit({ text: command.text, asUser: true }).then(
          (result) => {
            if (result.drop !== undefined) {
              const index = state.submitting.indexOf(command.submissionId ?? null)
              if (index >= 0) state.submitting.splice(index, 1)
            }
            state.acks.push({ id: command.id, ...(result.drop !== undefined ? { rejected: true } : {}) })
          },
          () => {
            const index = state.submitting.indexOf(command.submissionId ?? null)
            if (index >= 0) state.submitting.splice(index, 1)
            state.acks.push({ id: command.id, rejected: true })
          },
        )
      } else if (command.kind === 'arm' && typeof command.submissionId === 'string') {
        state.armed = command.submissionId
        state.acks.push({ id: command.id })
      } else if (command.kind === 'abort' && typeof command.turnId === 'string') {
        void $.turn.abort({ turnId: command.turnId }).then(
          () => state.acks.push({ id: command.id }),
          () => state.acks.push({ id: command.id, rejected: true }),
        )
      }
    }
  } catch {
    // PTY remains alive. Lease expiry on the server activates the legacy path.
  } finally {
    state.polling = false
  }
}

/** @type {import('claude-code').Register} */
export const register = (on) => {
  /** @type {State} */
  const state = {
    turnId: null,
    active: false,
    polling: false,
    socketPath: '',
    session: '',
    instance: '',
    version: '',
    acks: [],
    turns: [],
    reads: [],
    submitting: [],
    composer: [],
    armed: null,
  }
  on('prompt.submit', async ($, e, next) => {
    if (!state.active) return next(e)
    let pending
    if (e.origin.kind === 'composer') {
      // The server arms exactly one serialized PTY transaction; never compare text.
      const id = state.armed
      state.armed = null
      // Native queued prompts survive abort and become door:prompt user rows,
      // without another prompt.submit. Both delivery forms consume this FIFO.
      pending = { submissionId: id }
      state.composer.push(pending)
    }
    const result = await next(e)
    // A gate may refuse a composer prompt. It has no later delivery receipt.
    if (pending && result.drop !== undefined) {
      const index = state.composer.indexOf(pending)
      if (index >= 0) state.composer.splice(index, 1)
    }
    return result
  })
  on('session.append', async ($, e, next) => {
    if (!state.active) return next(e)
    let id = null
    if (!e.agentId && e.door === 'prompt') {
      if (e.origin.kind === 'plugin' && e.origin.name === 'kamin-bridge') id = state.submitting.shift() ?? null
      else if (e.origin.kind === 'composer') id = state.composer.shift()?.submissionId ?? null
    } else if (
      !e.agentId &&
      e.door === 'delivery' &&
      e.message.name === 'queued_command' &&
      e.origin.kind === 'composer'
    ) {
      id = state.composer.shift()?.submissionId ?? null
    }
    const result = await next(e)
    if (id) state.reads.push({ submissionId: id, uuid: result.uuid })
    return result
  })
  on('session.start', async ($, e, next) => {
    if (!e.isInteractive || e.surface !== 'terminal') return next(e)
    state.socketPath = (await $.env.get('BRIDGE_CLI_MOD_SOCKET')) ?? ''
    state.session = (await $.env.get('BRIDGE_CLI_MOD_SESSION')) ?? ''
    state.version = (await $.session.version()).version
    if (!state.socketPath || !state.session || state.version !== '2.1.312') return next(e)
    state.instance = `${await $.clock.now()}-${Math.random()}`
    try {
      await exchange($, state, '/hello')
      state.active = true
      $.clock.every(100, () => poll($, state))
    } catch {
      // Missing listener, incompatible protocol or policy refusal: fail open.
    }
    return next(e)
  })
  on('turn.start', ($, e, next) => {
    if (!e.agentId) {
      state.turnId = e.turnId
      if (state.active) state.turns.push({ turnId: state.turnId })
    }
    return next(e)
  })
  on('turn.complete', ($, e, next) => {
    if (!e.agentId && state.turnId === e.turnId) {
      state.turnId = null
      if (state.active) state.turns.push({ turnId: null })
    }
    return next(e)
  })
}
