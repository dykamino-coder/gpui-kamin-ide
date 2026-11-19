import { expect, it } from 'vitest'
import { CLI_VERSION_PROBE_TTL_MS, probeCliVersion } from './cli-mod-transport'

const node = (out: string) => ({
  command: process.execPath,
  prefix: ['-e', `console.log(${JSON.stringify(out)})`, '--'],
  env: process.env,
})

it('shares one --version run between concurrent session starts and reuses it within the TTL', async () => {
  const cli = node('9.9.9 (Claude Code)')
  const a = probeCliVersion(cli, 1_000)
  const b = probeCliVersion(cli, 1_001)
  expect(b).toBe(a)
  await expect(a).resolves.toBe('9.9.9 (Claude Code)')
  expect(probeCliVersion(cli, 1_000 + CLI_VERSION_PROBE_TTL_MS - 1)).toBe(a)
  const later = probeCliVersion(cli, 1_000 + CLI_VERSION_PROBE_TTL_MS)
  expect(later).not.toBe(a)
  await expect(later).resolves.toBe('9.9.9 (Claude Code)')
})

it('does not remember a failed probe', async () => {
  const cli = { command: 'definitely-not-a-cli-kacc', prefix: [], env: process.env }
  const first = probeCliVersion(cli, 5_000)
  await expect(first).resolves.toBe('')
  await new Promise((r) => setTimeout(r, 0))
  expect(probeCliVersion(cli, 5_001)).not.toBe(first)
})
