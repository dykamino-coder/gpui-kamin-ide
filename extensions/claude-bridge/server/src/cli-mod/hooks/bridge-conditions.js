// Runs inside Claude Code's mod sandbox: all effects go through the engine.
// Entry for sessions with hook handler conditions: the transport plus the condition gates.
import { registerConditions } from './conditions.js'
import { createState, registerTransport } from './transport.js'

/** @type {import('claude-code').Register} */
export function register(on) {
  const state = createState()
  registerConditions(on, state)
  registerTransport(on, state)
}
