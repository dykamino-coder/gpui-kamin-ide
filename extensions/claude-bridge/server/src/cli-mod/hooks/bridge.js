// Runs inside Claude Code's mod sandbox: all effects go through the engine.
/** @typedef {{id: number, rejected?: boolean}} Ack */
/** @typedef {{turnId: string | null}} Turn */
/** @typedef {{turnId: string | null, active: boolean, polling: boolean, socketPath: string, session: string, instance: string, version: string, acks: Ack[], turns: Turn[], reads: {submissionId: string, uuid: string}[], submitting: (string | null)[], composer: {submissionId: string | null}[], armed: string | null, conditionEnabled: boolean, conditionSeen: string | null, conditionSequence: number, conditionAgents: Map<string, Set<{agentId?: string}>>, conditionLocal: {marker?: string, value: string} | null}} State */

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

// A pure settings fold: identity comes from the immutable flag layer and the
// decisions from the watched local layer. Never select handlers from transient
// closure state; the same settings read must always give the same answer.
function conditionMarker(layers) {
  let session, states
  for (const layer of layers) {
    if (layer.source === 'flag') session = layer.settings.env?.CLAUDE_BRIDGE_HOOK_SESSION
    if (layer.source === 'local') states = layer.settings.env?.CLAUDE_BRIDGE_HOOK_CONDITIONS
  }
  if (!session || !states) return undefined
  try {
    return JSON.parse(states)[session]
  } catch {
    return undefined
  }
}

function localConditionSnapshot(settings) {
  const { CLAUDE_BRIDGE_HOOK_CONDITIONS: marker, ...env } = settings.env ?? {}
  const value = { ...settings, env }
  if (!Object.keys(env).length) delete value.env
  return { marker, value: JSON.stringify(value) }
}

async function conditionRequest($, state, route, data = {}) {
  const send = () =>
    $.http.fetch(`http://bridge/hooks/${route}`, {
      socketPath: state.socketPath,
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        protocol: 1,
        session: state.session,
        instance: state.instance,
        version: state.version,
        ...data,
      }),
    })
  let response = await send()
  if (!response.ok) {
    // The server drops this module's lease after three seconds without a
    // request (a stalled CLI is enough) and then refuses every request. Ask
    // for the lease again and repeat once; without this the conditions stayed
    // off for the rest of the session (INC-2026-0118). Calls stay unguarded
    // only while no lease can be obtained.
    try {
      await exchange($, state, '/hello')
    } catch {
      throw new Error('Bridge hook conditions unavailable')
    }
    response = await send()
  }
  if (!response.ok) throw new Error('Bridge hook conditions unavailable')
  return JSON.parse(response.text)
}

async function awaitConditionSettings($, state, generation) {
  // A direct $.settings.read() does not update the classic-hook catalog. Wait
  // for the engine's watcher read to acknowledge this exact settings version.
  for (let i = 0; i < 100 && state.conditionSeen !== generation; i++) await conditionRequest($, state, 'wait')
  if (state.conditionSeen !== generation) throw new Error('Bridge hook settings reload timed out')
}

async function gateConditions($, state, event, e, next) {
  const nativeHookAgent = e.agent_id
    ? e.agent_id.startsWith('hook-agent-')
    : [...(state.conditionAgents.get(e.tool_use_id) ?? [])].some((call) => call.agentId?.startsWith('hook-agent-'))
  // Native hook agents have their own empty declaration set. They still emit
  // mod events (including for MCP tools); queuing them behind their parent
  // would deadlock. The identifier is pinned with CLI 2.1.322 and covered by
  // the real-CLI nested-agent fixture.
  if (!state.active || !state.conditionEnabled || nativeHookAgent) return next(e)
  const generation = `${state.instance}:${++state.conditionSequence}`
  const payload =
    event === 'PreToolUse'
      ? {
          tool_name: e.tool,
          tool_input: Object.fromEntries(Object.entries(e).filter(([key]) => key !== 'tool' && key !== 'tool_use_id')),
        }
      : e
  const prepared = await conditionRequest($, state, 'prepare', { generation, event, payload })
  if (!prepared.generation) return next(e)
  try {
    await awaitConditionSettings($, state, generation)
    // Exactly one continuation, with the original MCP envelope and result.
    return await next(e)
  } finally {
    try {
      const restored = await conditionRequest($, state, 'restore', { generation })
      if (restored.generation) await awaitConditionSettings($, state, restored.generation)
    } finally {
      await conditionRequest($, state, 'release', { generation })
    }
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
    conditionEnabled: false,
    conditionSeen: null,
    conditionSequence: 0,
    conditionAgents: new Map(),
    conditionLocal: null,
  }
  on('settings.read', async ($, e, next) => {
    const result = await next(e)
    if (e.source) return result
    const marker = conditionMarker(e.layers)
    if (next.origin?.plugin === 'engine') {
      state.conditionSeen = marker?.generation ?? null
      if (!state.conditionLocal) {
        const local = e.layers.find((layer) => layer.source === 'local')
        state.conditionLocal = localConditionSnapshot(local?.settings ?? {})
      }
    }
    const value = result.value
    if (!marker?.event || !marker.rules || !value.hooks?.[marker.event]) return result
    const matchers = value.hooks[marker.event].map((matcher) => ({
      ...matcher,
      hooks: matcher.hooks.flatMap((handler) => {
        if (!['prompt', 'agent', 'mcp_tool'].includes(handler.type) || !Object.hasOwn(marker.rules, handler.if))
          return [handler]
        if (!marker.rules[handler.if]) return []
        return [Object.fromEntries(Object.entries(handler).filter(([key]) => key !== 'if'))]
      }),
    }))
    return { ...result, value: { ...value, hooks: { ...value.hooks, [marker.event]: matchers } } }
  })
  on('classic.ConfigChange', async ($, e, next) => {
    if (!state.conditionEnabled || e.source !== 'local_settings' || !e.file_path) return next(e)
    try {
      // The watcher raises ConfigChange before rebuilding settings. Compare
      // the actual local file with the last snapshot, so a simultaneous user
      // edit still reaches every original handler. Only our marker-only
      // changes are internal and must not manufacture user hook events.
      const current = localConditionSnapshot(JSON.parse(await $.fs.read(e.file_path)))
      const before = state.conditionLocal
      state.conditionLocal = current
      if (before && before.marker !== current.marker && before.value === current.value) return {}
    } catch {
      // A removed or invalid settings file is a real configuration change.
    }
    return next(e)
  })
  on('tool.call', async ($, e, next) => {
    if (!state.conditionEnabled) return next(e)
    const call = { agentId: e.agentId }
    const calls = state.conditionAgents.get(e.tool_use_id) ?? new Set()
    calls.add(call)
    state.conditionAgents.set(e.tool_use_id, calls)
    try {
      return await next(e)
    } finally {
      calls.delete(call)
      if (!calls.size) state.conditionAgents.delete(e.tool_use_id)
    }
  })
  on('classic.PreToolUse', ($, e, next) => gateConditions($, state, 'PreToolUse', e, next))
  on('classic.PostToolUse', ($, e, next) => gateConditions($, state, 'PostToolUse', e, next))
  on('classic.PostToolUseFailure', ($, e, next) => gateConditions($, state, 'PostToolUseFailure', e, next))
  on('classic.PermissionRequest', ($, e, next) => gateConditions($, state, 'PermissionRequest', e, next))
  on('classic.PermissionDenied', ($, e, next) => gateConditions($, state, 'PermissionDenied', e, next))
  on('prompt.submit', async ($, e, next) => {
    if (!state.active) return next(e)
    let pending
    if (e.origin.kind === 'composer') {
      // The server arms exactly one serialized PTY transaction; never compare text.
      const id = state.armed
      state.armed = null
      // After abort, CLI can promote a queued prompt with or without another
      // idle submit. The unarmed replay already has a slot; do not enqueue it twice.
      if (!id && !e.turnId && state.composer.length) pending = state.composer[0]
      else {
        pending = { submissionId: id }
        state.composer.push(pending)
      }
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
      if (e.origin.kind === 'plugin' && 'name' in e.origin && e.origin.name === 'kamin-bridge')
        id = state.submitting.shift() ?? null
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
    if (id && typeof result.uuid === 'string') state.reads.push({ submissionId: id, uuid: result.uuid })
    return result
  })
  on('session.start', async ($, e, next) => {
    if (!e.isInteractive || e.surface !== 'terminal') return next(e)
    state.socketPath = (await $.env.get('BRIDGE_CLI_MOD_SOCKET')) ?? ''
    state.session = (await $.env.get('BRIDGE_CLI_MOD_SESSION')) ?? ''
    state.version = (await $.session.version()).version
    if (!state.socketPath || !state.session || state.version !== '2.1.322') return next(e)
    state.instance = `${await $.clock.now()}-${Math.random()}`
    try {
      await exchange($, state, '/hello')
      state.active = true
      state.conditionEnabled = (await $.env.get('BRIDGE_CLI_MOD_HOOK_CONDITIONS')) === '1'
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
  return undefined
}
