import { describe, it, expect, afterEach } from 'vitest'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { JsonlWatcher } from './jsonl-watcher'
import type { JsonlStatus } from '../../shared/jsonl-types'

/** A brand-new session's transcript is empty by definition, so this path is the
 *  common case, not an edge one. */
describe('JsonlWatcher — empty transcript', () => {
  const dirs: string[] = []
  const watchers: JsonlWatcher[] = []
  afterEach(() => {
    for (const w of watchers.splice(0)) w.stop()
    for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true })
  })

  function emptyFile(): string {
    const dir = mkdtempSync(join(tmpdir(), 'jsonl-watcher-'))
    dirs.push(dir)
    const file = join(dir, 'empty.jsonl')
    writeFileSync(file, '')
    return file
  }

  it('reports replayComplete instead of going silent', async () => {
    // Silence is what left a new chat under the load-cover until the client's
    // 20s give-up watchdog: `replayComplete` is the ONLY signal that binds the
    // tab and enables the composer, and the empty-file path never sent it.
    const statuses: JsonlStatus[] = []
    const w = new JsonlWatcher(Date.now(), () => true, (s) => { statuses.push(s) })
    watchers.push(w)
    ;(w as unknown as { filePath: string }).filePath = emptyFile()

    w.replayAll()
    await new Promise((r) => setTimeout(r, 100))

    const complete = statuses.filter((s) => s.replayComplete === true)
    expect(complete).toHaveLength(1)
    expect(complete[0]?.status).toBe('watching')
  })

  it('answers "nothing to replay" when no transcript exists yet', async () => {
    // Every brand-new chat: the CLI writes its JSONL only on the first turn, so
    // discovery finds nothing. Silence here parked the chat under the load-cover
    // for the client's full 20s give-up watchdog.
    const statuses: JsonlStatus[] = []
    const w = new JsonlWatcher(
      Date.now(), () => true, (s) => { statuses.push(s) },
      undefined, join(tmpdir(), `kamin-no-such-project-${String(Date.now())}`),
    )
    watchers.push(w)

    w.start()
    await new Promise((r) => setTimeout(r, 200))

    expect(statuses.filter((s) => s.replayComplete === true)).toHaveLength(1)
  })

  it('sends no transcript entries for an empty file', async () => {
    let entryBatches = 0
    const w = new JsonlWatcher(Date.now(), () => { entryBatches++; return true }, () => {})
    watchers.push(w)
    ;(w as unknown as { filePath: string }).filePath = emptyFile()

    w.replayAll()
    await new Promise((r) => setTimeout(r, 100))

    expect(entryBatches).toBe(0)
  })
})
