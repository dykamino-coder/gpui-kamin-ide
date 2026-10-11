import { describe, expect, it } from 'vitest'
import fs from 'node:fs/promises'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { createUsageProbe, quoteCaptureArgument } from './usage-statusline'

describe('BR-20 actual scoped statusline script', () => {
  it('stores only documented usage/version fields from real child stdin, never other statusline content', async () => {
    const probe = await createUsageProbe()
    const root = path.dirname(probe.settings)
    try {
      const settings = JSON.parse(await fs.readFile(probe.settings, 'utf8'))
      expect(Object.keys(settings)).toEqual(['statusLine'])
      const child = spawnSync(process.execPath, [path.join(root, 'statusline.cjs'), path.join(root, 'result.json')], {
        input: JSON.stringify({
          version: '2.1.284',
          transcript_path: '/synthetic/private',
          cwd: '/private',
          token: 'synthetic-secret',
          rate_limits: {
            five_hour: { used_percentage: 12.5, resets_at: 1900000000, private: 'secret' },
            seven_day: { used_percentage: 27, resets_at: 1900100000 },
            seven_day_fable: { private: 'secret' },
          },
        }),
        encoding: 'utf8',
        timeout: 5000,
      })
      expect(child.status).toBe(0)
      expect(child.stdout).toBe('')
      expect(child.stderr).toBe('')
      const result = (await probe.read()) as any
      expect(result).toMatchObject({
        version: '2.1.284',
        rate_limits: {
          five_hour: { used_percentage: 12.5, resets_at: 1900000000 },
          seven_day: { used_percentage: 27, resets_at: 1900100000 },
        },
      })
      expect(Object.keys(result.rate_limits)).toEqual(['five_hour', 'seven_day'])
      expect(JSON.stringify(result)).not.toMatch(/private|secret|transcript|cwd|token/)
    } finally {
      await probe.cleanup()
    }
    await expect(fs.stat(root)).rejects.toThrow()
  })
  it('fails closed for oversized stdin and rejects command expansion in Windows paths', async () => {
    const probe = await createUsageProbe()
    const root = path.dirname(probe.settings)
    try {
      const child = spawnSync(process.execPath, [path.join(root, 'statusline.cjs'), path.join(root, 'result.json')], {
        input: 'x'.repeat(131073),
        encoding: 'utf8',
        timeout: 5000,
      })
      expect(child.status).toBe(0)
      expect(await probe.read()).toBeNull()
      expect(child.stdout + child.stderr).toBe('')
    } finally {
      await probe.cleanup()
    }
    expect(quoteCaptureArgument("a'b", false)).toBe("'a'\\''b'")
    expect(quoteCaptureArgument('a b', true)).toBe('"a b"')
    expect(() => quoteCaptureArgument('a%SECRET%b', true)).toThrow('Unsupported capture path')
  })
})
