import fs from 'node:fs'
import path from 'node:path'
import { execFileSync } from 'node:child_process'
import { parseArgs } from 'node:util'
import { capabilityInventory } from './capability-inventory.js'
import { bridgeRoot, readMatrix } from './capability-matrix.js'
import { fullParityGaps } from './full-parity.js'

// This command consumes sanitized execution artifacts; it never runs a browser
// on the workstation or prints receipt contents/exception messages.
try {
  const { values } = parseArgs({ options: { receipt: { type: 'string' } } })
  const repo = path.resolve(bridgeRoot, '../..')
  const git = (args: string[]) => execFileSync('git', ['-C', repo, ...args], { encoding: 'utf8', windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] }).trim()
  const candidate = git(['rev-parse', 'HEAD'])
  const dirty = !!git(['status', '--porcelain'])
  let receipt: unknown
  if (values.receipt) {
    const file = fs.realpathSync(values.receipt)
    const relative = path.relative(fs.realpathSync(repo), file)
    const info = fs.statSync(file)
    if ((!relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative)) || !info.isFile() || info.size > 4 * 1024 * 1024) {
      throw new Error('Expected external bounded receipt')
    }
    receipt = JSON.parse(fs.readFileSync(file, 'utf8'))
  }
  const gaps = fullParityGaps(capabilityInventory(), readMatrix(), receipt, candidate, dirty)
  if (gaps.length) {
    console.error(`Full browser parity remains OPEN (${gaps.length} gaps).`)
    console.error(gaps.slice(0, 12).join('\n'))
    process.exitCode = 1
  } else console.log('Full browser parity execution receipt passes for the clean candidate and immutable image.')
} catch {
  console.error('Full parity gate rejected its inputs or capability classification; raw details omitted.')
  process.exitCode = 1
}
