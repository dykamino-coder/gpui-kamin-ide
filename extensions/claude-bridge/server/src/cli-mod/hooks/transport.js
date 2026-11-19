// Runs inside Claude Code's mod sandbox: all effects go through the engine.
// The session transport: prompts in, turn lifecycle out. Read receipts need no
// handler here: the server finds each stored prompt row in the transcript, and a
// handler on every transcript append would cost each tool call (session.append).
/** @typedef {{id: number, rejected?: boolean}} Ack */
/** @typedef {{turnId: string | null}} Turn */
/** @typedef {{turnId: string | null, active: boolean, polling: boolean, waiting: boolean, reporting: boolean, emptyPolls: number, socketPath: string, session: string, instance: string, version: string, acks: Ack[], turns: Turn[], conditionEnabled: boolean, conditionSeen: string | null, conditionSequence: number, conditionAgents: Map<string, Set<{agentId?: string}>>, conditionLocal: {marker?: string, value: string} | null}} State */

/** @param {import('claude-code').EngineInterface} $ @param {State} state @param {string} route @param {boolean} [wait] */
export async function exchange($, state, route, wait = false) {
  const acks = state.acks.splice(0)
  const turns = state.turns.splice(0)
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
        // The server holds an idle poll open until a command arrives (long poll).
        ...(wait ? { wait: true } : {}),
      }),
    })
    if (!response.ok) throw new Error('Bridge transport unavailable')
    return JSON.parse(response.text)
  } catch (error) {
    state.acks.unshift(...acks)
    state.turns.unshift(...turns)
    if (state.acks.length + state.turns.length > 512) state.active = false
    throw error
  }
}

/** Empty answers in a row after which the module stops asking every period
 * and lets the server hold its poll open (long poll). */
const IDLE_POLLS = 5

/** One period, one exchange, as before; once idle the exchange is a long poll.
 * Acks and turns raised while that poll is held go out at once on a
 * second request instead of waiting behind it.
 * @param {import('claude-code').EngineInterface} $ @param {State} state */
function tick($, state) {
  if (!state.active) return
  const news = state.acks.length + state.turns.length > 0
  if (state.polling) {
    if (!state.waiting || state.reporting || !news) return
    state.reporting = true
    return poll($, state, false).finally(() => {
      state.reporting = false
    })
  }
  state.polling = true
  state.waiting = !news && state.emptyPolls >= IDLE_POLLS
  return poll($, state, state.waiting).finally(() => {
    state.polling = false
    state.waiting = false
  })
}

/** @param {import('claude-code').EngineInterface} $ @param {State} state @param {boolean} wait */
async function poll($, state, wait) {
  if (state.acks.length + state.turns.length) state.emptyPolls = 0
  try {
    const { commands } = await exchange($, state, '/poll', wait)
    if (commands.length) state.emptyPolls = 0
    else if (!wait) state.emptyPolls++
    for (const command of commands) {
      if (command.kind === 'submit' && typeof command.text === 'string') {
        // Resolves at turn start. Never hold up heartbeat or abort behind it.
        void $.prompt.submit({ text: command.text, asUser: true }).then(
          (result) => state.acks.push({ id: command.id, ...(result.drop !== undefined ? { rejected: true } : {}) }),
          () => state.acks.push({ id: command.id, rejected: true }),
        )
      } else if (command.kind === 'arm' && typeof command.submissionId === 'string') {
        // The handshake proves the module is alive before the server types the text.
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
  }
}

/** @returns {State} */
export function createState() {
  return {
    turnId: null,
    active: false,
    polling: false,
    waiting: false,
    reporting: false,
    emptyPolls: 0,
    socketPath: '',
    session: '',
    instance: '',
    version: '',
    acks: [],
    turns: [],
    conditionEnabled: false,
    conditionSeen: null,
    conditionSequence: 0,
    conditionAgents: new Map(),
    conditionLocal: null,
  }
}

/** @param {Parameters<import('claude-code').Register>[0]} on @param {State} state */
export function registerTransport(on, state) {
  on('session.start', async ($, e, next) => {
    if (!e.isInteractive || e.surface !== 'terminal') return next(e)
    state.socketPath = (await $.env.get('BRIDGE_CLI_MOD_SOCKET')) ?? ''
    state.session = (await $.env.get('BRIDGE_CLI_MOD_SESSION')) ?? ''
    state.version = (await $.session.version()).version
    if (!state.socketPath || !state.session || state.version !== '2.1.328') return next(e)
    state.instance = `${await $.clock.now()}-${Math.random()}`
    try {
      await exchange($, state, '/hello')
      state.active = true
      state.conditionEnabled = (await $.env.get('BRIDGE_CLI_MOD_HOOK_CONDITIONS')) === '1'
      $.clock.every(100, () => tick($, state))
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
