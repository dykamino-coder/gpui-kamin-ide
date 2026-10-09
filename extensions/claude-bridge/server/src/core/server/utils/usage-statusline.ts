import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'

// Scoped only to the disposable /usage capture. Never modifies global or live
// session settings. Official contract: https://code.claude.com/docs/en/statusline
const SCRIPT = String.raw`
const fs = require('node:fs');
let input = ''; let bytes = 0;
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => { bytes += Buffer.byteLength(chunk); if (bytes <= 131072) input += chunk; });
process.stdin.on('end', () => {
  if (bytes > 131072) return;
  try {
    const data = JSON.parse(input);
    const rate_limits = {};
    for (const name of ['five_hour', 'seven_day']) {
      const value = data.rate_limits?.[name];
      if (value && typeof value.used_percentage === 'number' && Number.isFinite(value.used_percentage) && value.used_percentage >= 0 && value.used_percentage <= 100 && Number.isSafeInteger(value.resets_at) && value.resets_at > 0 && value.resets_at <= 253402300799)
        rate_limits[name] = { used_percentage: value.used_percentage, resets_at: value.resets_at };
    }
    const version = typeof data.version === 'string' && /^\d+\.\d+\.\d+(?:[-+][\w.-]{1,32})?$/.test(data.version) ? data.version : null;
    const result = JSON.stringify({ observedAt: new Date().toISOString(), version, rate_limits });
    const target = process.argv[2]; const temp = target + '.' + process.pid + '.tmp';
    fs.writeFileSync(temp, result, { mode: 0o600 }); fs.renameSync(temp, target);
  } catch { /* no raw stdin, errors, paths or credentials emitted */ }
});
`
export function quoteCaptureArgument(value: string, windows = process.platform === 'win32'): string {
  if (windows) {
    if (/["%\r\n]/.test(value)) throw new Error('Unsupported capture path')
    return `"${value}"`
  }
  return `'${value.replace(/'/g, "'\\''")}'`
}
export async function createUsageProbe() {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'bridge-usage-'))
  const script = path.join(root, 'statusline.cjs')
  const result = path.join(root, 'result.json')
  const settings = path.join(root, 'settings.json')
  const cleanup = async () => {
    if (
      path.dirname(path.resolve(root)) !== path.resolve(os.tmpdir()) ||
      !path.basename(root).startsWith('bridge-usage-')
    )
      throw new Error('Unsafe usage probe cleanup')
    await fs.rm(root, { recursive: true, force: true })
  }
  try {
    await fs.writeFile(script, SCRIPT, { mode: 0o600 })
    const command = [process.execPath, script, result].map((value) => quoteCaptureArgument(value)).join(' ')
    await fs.writeFile(settings, JSON.stringify({ statusLine: { type: 'command', command } }), { mode: 0o600 })
  } catch (error) {
    await cleanup()
    throw error
  }
  return {
    settings,
    cleanup,
    read: async (): Promise<unknown> => {
      try {
        if ((await fs.stat(result)).size > 4096) return null
        return JSON.parse(await fs.readFile(result, 'utf8'))
      } catch {
        return null
      }
    },
  }
}
