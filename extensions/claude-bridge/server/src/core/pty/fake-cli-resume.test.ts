// The checkout-only fake CLI after a Bridge server restart: a resumed process must not
// reuse record UUIDs (the client drops a reused UUID as a duplicate, and the queue item
// that row would confirm stays pending), and a queue acceptance run can opt in to rows
// that carry the typed text, as the real CLI writes them.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawn } from 'node:child_process'
import { pathToFileURL } from 'node:url'
import { afterEach, expect, it } from 'vitest'

const support = path.resolve(__dirname, '../../../test-support')
const dirs: string[] = []
afterEach(() => {
  for (const dir of dirs.splice(0)) fs.rmSync(dir, { recursive: true, force: true })
})

async function resumeOnce(
  input: string,
  record: boolean,
): Promise<{ rows: Array<Record<string, unknown>>; before: number }> {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'fake-cli-resume-'))
  dirs.push(root)
  const home = path.join(root, 'home')
  const work = path.join(root, 'work')
  fs.mkdirSync(work, { recursive: true })
  const { projectSlug } = (await import(pathToFileURL(path.join(support, 'fixtures.mjs')).href)) as {
    projectSlug(dir: string): string
  }
  const id = '0b6e1f1c-5a8a-4c4e-9d43-3f6a3c1f2b10'
  const dir = path.join(home, '.claude', 'projects', projectSlug(work))
  fs.mkdirSync(dir, { recursive: true })
  const file = path.join(dir, `${id}.jsonl`)
  // An earlier process numbered up to 7; the file a resume reads holds two lines.
  const old = (n: number): string =>
    JSON.stringify({ type: 'user', uuid: `${id}-${n}`, message: { content: `old ${n}` } })
  fs.writeFileSync(file, `${old(1)}\n${old(7)}\n`)
  const child = spawn(process.execPath, [path.join(support, 'fake-claude.mjs'), '--resume', id], {
    cwd: work,
    env: {
      ...process.env,
      NODE_ENV: 'test',
      BRIDGE_DEV_FAKE_CLI: '1',
      BRIDGE_FAKE_CLI_HOME: home,
      BRIDGE_FAKE_CLI_RECORD_INPUT: record ? '1' : '',
    },
    stdio: ['pipe', 'ignore', 'pipe'],
  })
  child.stdin.write(`${input}\r/exit\r`)
  await new Promise((resolve) => child.on('exit', resolve))
  const rows = fs
    .readFileSync(file, 'utf8')
    .split('\n')
    .filter(Boolean)
    .map((line) => JSON.parse(line) as Record<string, unknown>)
  return { rows, before: 2 }
}

it('a resumed fake CLI continues after the highest record number already used', async () => {
  const { rows, before } = await resumeOnce('typed after the restart', false)
  const uuids = rows.map((row) => row.uuid)
  expect(rows.length).toBeGreaterThan(before)
  expect(new Set(uuids).size).toBe(uuids.length)
  expect(rows.slice(before).every((row) => Number(String(row.uuid).split('-').pop()) > 7)).toBe(true)
  // Without the opt-in the transcript keeps no typed input.
  expect(JSON.stringify(rows.slice(before))).not.toContain('typed after the restart')
})

it('records the typed text only when the queue acceptance opts in', async () => {
  const { rows, before } = await resumeOnce('typed after the restart', true)
  const user = rows.slice(before).find((row) => row.type === 'user') as { message: { content: string } }
  expect(user.message.content).toBe('typed after the restart')
})
