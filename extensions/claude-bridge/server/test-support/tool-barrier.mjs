// Runs on the local KaminIDE host via the real MCP Bash tool. No network.
import fs from 'node:fs'
import path from 'node:path'
import { setTimeout } from 'node:timers/promises'
const root = path.resolve(process.argv[2])
const release = path.join(root, 'tool-release')
fs.mkdirSync(root, { recursive: true })
fs.writeFileSync(path.join(root, 'tool-started'), 'started')
const deadline = Date.now() + 90000
while (!fs.existsSync(release)) {
  if (Date.now() > deadline) throw new Error('Synthetic tool barrier timed out')
  await setTimeout(50)
}
const result = fs.readFileSync(release, 'utf8').trim()
fs.rmSync(release)
if (result === 'error') {
  process.stderr.write('Synthetic tool error\n')
  process.exitCode = 1
} else process.stdout.write('Synthetic tool success\n')
