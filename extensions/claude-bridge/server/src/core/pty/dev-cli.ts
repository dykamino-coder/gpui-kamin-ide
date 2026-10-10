// A fixed checkout-only CLI. Release packages omit test-support entirely.
import { existsSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { setTimeout } from 'node:timers/promises'

export const fakeCliPath = fileURLToPath(new URL('../../../test-support/fake-claude.mjs', import.meta.url))

export function devCli(env = process.env, available = existsSync(fakeCliPath), currentHome = os.homedir()) {
  if (env.BRIDGE_DEV_FAKE_CLI !== '1') return undefined
  // Fail closed for unset/unknown modes, even in a source checkout.
  if (!['development', 'test'].includes(env.NODE_ENV ?? '')) return undefined
  if (!available) throw new Error('Fake CLI is unavailable in release artifacts')
  if (!env.BRIDGE_FAKE_CLI_HOME) throw new Error('Fake CLI requires an isolated BRIDGE_FAKE_CLI_HOME')
  if (
    !path.isAbsolute(env.BRIDGE_FAKE_CLI_HOME) ||
    path.resolve(env.BRIDGE_FAKE_CLI_HOME) !== path.resolve(currentHome)
  ) {
    throw new Error('Bridge and fake CLI must share the isolated home; use test-support/run.mjs')
  }
  const delay = Number(env.BRIDGE_FAKE_CLI_STARTUP_DELAY_MS ?? 0)
  if (!Number.isInteger(delay) || delay < 0 || delay > 30000) throw new Error('Invalid fake CLI startup delay')
  return { command: process.execPath, prefix: [fakeCliPath], startupDelay: delay }
}

export async function waitForDevCliStartup(cli: ReturnType<typeof devCli>, signal?: AbortSignal) {
  if (cli?.startupDelay) await setTimeout(cli.startupDelay, undefined, { signal })
}

/** Shared by account/health probes so the dev harness never falls back to an installed CLI. */
export function claudeInvocation(args: string[], fallback = process.platform === 'win32' ? 'claude.cmd' : 'claude') {
  const developmentCli = devCli()
  return { command: developmentCli?.command ?? fallback, args: [...(developmentCli?.prefix ?? []), ...args] }
}
