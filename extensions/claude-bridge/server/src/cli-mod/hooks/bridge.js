// Runs inside Claude Code's mod sandbox: all effects go through the engine.
// Entry for sessions without hook handler conditions (bridge-conditions.js has them).
import { createState, registerTransport } from './transport.js'

/** @type {import('claude-code').Register} */
export function register(on) {
  registerTransport(on, createState())
}
