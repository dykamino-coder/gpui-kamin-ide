import { expect, it, vi } from 'vitest'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
const fixture = vi.hoisted(() => ({ root: '' }))
vi.mock('../../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn() }))
vi.mock('../../config/settings', () => ({
  injectProxyEnv: (env: Record<string, string>) => {
    // Only the disposable child receives this environment; owner profiles and
    // the test worker's home/environment remain untouched.
    env.HOME = fixture.root
    env.BASH_ENV = path.join(fixture.root, 'capture-env.sh')
  },
}))
vi.mock('node:os', async (original) => {
  const actual = await original<typeof import('node:os')>()
  return { ...actual, default: { ...actual, homedir: () => fixture.root } }
})

// Real Linux PTY and generated statusline command, using our own inert CLI.
// This is pipeline proof, NOT authenticated native Claude quota acceptance.
it.skipIf(process.platform !== 'linux' || process.env.KAMIN_BR20_PTY_GATE !== '1')(
  'BR-20 real disposable Linux PTY final screen and scoped statusline pipeline',
  async () => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), 'br20-live-'))
    fixture.root = root
    const cleanup = async () => {
      if (
        path.dirname(path.resolve(root)) !== path.resolve(os.tmpdir()) ||
        !path.basename(root).startsWith('br20-live-')
      )
        throw new Error('Unsafe Linux fixture cleanup')
      await fs.rm(root, { recursive: true, force: true })
    }
    try {
      await fs.mkdir(path.join(root, 'bin'))
      const q = (value: string) => `'${value.replace(/'/g, "'\\''")}'`
      await fs.writeFile(path.join(root, 'capture-env.sh'), `export PATH=${q(path.join(root, 'bin'))}:$PATH\n`)
      const cli = `#!${process.execPath}
const fs = require('node:fs'); const cp = require('node:child_process');
const args = process.argv.slice(2); const settings = JSON.parse(fs.readFileSync(args[args.indexOf('--settings')+1], 'utf8'));
const epoch = Math.floor(Date.now()/1000)+86400;
const probe = cp.spawnSync('/bin/sh',['-c',settings.statusLine.command],{input:JSON.stringify({version:'2.1.284',token:'synthetic-secret',rate_limits:{five_hour:{used_percentage:41,resets_at:epoch},seven_day:{used_percentage:73,resets_at:epoch+86400},seven_day_fable:{used_percentage:99,resets_at:epoch}}}),encoding:'utf8'});
if(probe.status!==0)process.exit(2);
process.stdout.write('Current session\\r\\n12% used\\r\\nResets old\\r\\nCurrent week (Sonnet only)\\r\\n20% used\\r\\nResets old');
setTimeout(()=>{process.stdout.write('\\x1b[2J\\x1b[HCurrent session\\r\\n23% used\\r\\nResets 6pm (UTC)\\r\\nCurrent week (all models)\\r\\n42% used\\r\\nResets Monday\\r\\nBonus promo\\r\\nCurrent week (Fable)\\r\\n17% used\\r\\nResets Monday\\r\\nEsc to cancel');setTimeout(()=>process.exit(0),80)},40);
`
      await fs.writeFile(path.join(root, 'bin', 'claude'), cli, { mode: 0o700 })
      const { captureUsage } = await import('./usage-capture')
      const data = await captureUsage(true)
      expect(data).toMatchObject({ source: 'statusline', claudeCodeVersion: '2.1.284' })
      expect(data.windows).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ id: 'five_hour', percent: 41, source: 'statusline' }),
          expect.objectContaining({ id: 'seven_day', percent: 73, source: 'statusline' }),
          expect.objectContaining({ model: 'Fable', percent: 17, source: 'tui-compatibility' }),
        ]),
      )
      expect(data.windows.some((window) => window.model === 'Sonnet')).toBe(false)
      expect(JSON.stringify(data)).not.toContain('synthetic-secret')
    } finally {
      await cleanup()
    }
  },
  15000,
)
