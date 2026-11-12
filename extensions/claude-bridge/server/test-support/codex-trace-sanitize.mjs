// Turns a raw app-server capture (BRIDGE_CODEX_TRACE from the codex tool gate)
// into a committable fixture: temp paths, host name, installation id and
// user agent are replaced, Windows paths are normalised to '/', and bearer
// headers are redacted. Captures come from offline mocks only, so they hold no
// real prompts or credentials; review the output before committing anyway.
// usage: node codex-trace-sanitize.mjs <raw.jsonl> <fixture.jsonl>
import fs from 'node:fs'

const [input, output] = process.argv.slice(2)
if (!input || !output) throw new Error('usage: codex-trace-sanitize.mjs <raw.jsonl> <fixture.jsonl>')

const ROOT = /(?:[A-Za-z]:)?[\\/][^"]*?codex-gate-[A-Za-z0-9]+/g
const clean = (value, key) => {
  if (typeof value === 'string') {
    if (key === 'serverName') return 'bridge-host'
    if (key === 'installationId') return '00000000-0000-0000-0000-000000000000'
    if (key === 'userAgent') return 'kamin_bridge/fixture'
    if (value.startsWith('Bearer ')) return 'Bearer <redacted>'
    return value.replace(ROOT, '/work').replace(/\\/g, '/')
  }
  if (Array.isArray(value)) return value.map((v) => clean(v, key))
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, clean(v, k)]))
  }
  return value
}

const lines = fs
  .readFileSync(input, 'utf8')
  .split('\n')
  .filter(Boolean)
  .map((line) => JSON.stringify(clean(JSON.parse(line))))
fs.writeFileSync(output, lines.join('\n') + '\n')
console.log(`${lines.length} frames → ${output}`)
