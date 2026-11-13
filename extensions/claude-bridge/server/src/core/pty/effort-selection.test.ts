// Тянет за собой потоковый прокси → @peculiar/x509 → tsyringe, которому нужен
// полифил reflect до декораторов. Тот же импорт, что и у точки входа.
import 'reflect-metadata'

import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// Isolated data/transcript directories; never reads the owner profile.
const holder = vi.hoisted(() => ({ dataDir: '' }))
vi.mock('../stats/database/lifecycle', () => ({ getDataDir: () => holder.dataDir }))

const CONV = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'

let home = ''
let envBefore: { HOME?: string; USERPROFILE?: string } = {}
const dirs: string[] = []

async function fresh(): Promise<typeof import('./effort-selection')> {
  vi.resetModules()
  await import('reflect-metadata')
  return import('./effort-selection')
}

beforeEach(() => {
  home = fs.mkdtempSync(path.join(os.tmpdir(), 'inc-0099-'))
  dirs.push(home)
  holder.dataDir = path.join(home, 'data')
  envBefore = { HOME: process.env.HOME, USERPROFILE: process.env.USERPROFILE }
  process.env.HOME = home
  process.env.USERPROFILE = home
})

afterEach(() => {
  if (envBefore.HOME === undefined) delete process.env.HOME
  else process.env.HOME = envBefore.HOME
  if (envBefore.USERPROFILE === undefined) delete process.env.USERPROFILE
  else process.env.USERPROFILE = envBefore.USERPROFILE
  for (const dir of dirs.splice(0)) fs.rmSync(dir, { recursive: true, force: true })
})

describe('INC-2026-0099: выбранный эффорт переживает респаун', () => {
  function transcript(rows: object[]): string {
    const settingsDir = path.join(home, '.claude', 'bridge-sessions', 'token_x', 'sess-1')
    fs.mkdirSync(settingsDir, { recursive: true })
    const slugDir = path.join(home, '.claude', 'projects', path.resolve(settingsDir).replace(/[^a-zA-Z0-9]/g, '-'))
    fs.mkdirSync(slugDir, { recursive: true })
    fs.writeFileSync(path.join(slugDir, `${CONV}.jsonl`), rows.map((r) => JSON.stringify(r)).join('\n') + '\n')
    return settingsDir
  }

  it('эффорт последнего ответа читается из расшифровки', async () => {
    // Полевой случай: смена на xhigh через /effort, ход на xhigh, затем
    // перезапуск CLI. Раньше респаун шёл с умолчанием `--effort high`.
    const mod = await fresh()
    const settingsDir = transcript([
      { type: 'assistant', timestamp: '2026-11-11T10:00:00.000Z', effort: 'high', message: { model: 'm' } },
      { type: 'user', uuid: 'u2' },
      { type: 'assistant', timestamp: '2026-11-11T10:05:00.000Z', effort: 'xhigh', message: { model: 'm' } },
    ])
    expect(mod.effortForResume(settingsDir, CONV)).toBe('xhigh')
  })

  it('выбор без нового ответа сильнее расшифровки', async () => {
    const mod = await fresh()
    const settingsDir = transcript([
      { type: 'assistant', timestamp: '2026-11-11T10:00:00.000Z', effort: 'high', message: { model: 'm' } },
    ])
    mod.rememberEffortSelection(CONV, 'max', new Date('2026-11-11T10:01:00.000Z'))
    expect(mod.effortForResume(settingsDir, CONV)).toBe('max')
  })

  it('ответ после выбора сильнее выбора', async () => {
    const mod = await fresh()
    mod.rememberEffortSelection(CONV, 'max', new Date('2026-11-11T10:01:00.000Z'))
    const newer = { effort: 'medium', timestamp: '2026-11-11T10:02:00.000Z' }
    expect(mod.effortForResumeWithSelection(CONV, newer)).toBe('medium')
  })

  it('выбор переживает перезапуск сервера и снимается явным забыванием', async () => {
    ;(await fresh()).rememberEffortSelection(CONV, 'xhigh')
    const afterRestart = await fresh()
    expect(afterRestart.readEffortSelection(CONV)?.effort).toBe('xhigh')
    afterRestart.forgetEffortSelection(CONV)
    expect(afterRestart.effortForResumeWithSelection(CONV, null)).toBeNull()
  })

  it('без расшифровки и выбора — null, неизвестный уровень отбрасывается', async () => {
    const mod = await fresh()
    expect(mod.effortForResume(path.join(home, 'none'), CONV)).toBeNull()
    mod.rememberEffortSelection(CONV, 'ultra')
    expect(mod.effortForResume(path.join(home, 'none'), CONV)).toBeNull()
  })

  it('хранилище эффорта не меняет хранилище модели', async () => {
    const mod = await fresh()
    const model = await import('./model-selection')
    model.rememberModelSelection(CONV, 'claude-opus-5-5')
    mod.rememberEffortSelection(CONV, 'xhigh')
    expect(model.readModelSelection(CONV)?.model).toBe('claude-opus-5-5')
    expect(mod.readEffortSelection(CONV)?.effort).toBe('xhigh')
  })
})

it('retains a valid choice when a newer transcript names an unsupported effort', async () => {
  const mod = await fresh()
  mod.rememberEffortSelection(CONV, 'xhigh', new Date('2026-11-11T10:00:00.000Z'))
  expect(mod.effortForResumeWithSelection(CONV, { effort: 'ultra', timestamp: '2026-11-11T11:00:00.000Z' })).toBe(
    'xhigh',
  )
})

it('reads a truncated JSONL tail past invalid/non-assistant rows without changing model recovery', async () => {
  const mod = await fresh()
  const settingsDir = path.join(home, 'transcript')
  const slug = path.resolve(settingsDir).replace(/[^a-zA-Z0-9]/g, '-')
  const jsonl = path.join(home, '.claude', 'projects', slug, `${CONV}.jsonl`)
  fs.mkdirSync(path.dirname(jsonl), { recursive: true })
  fs.writeFileSync(
    jsonl,
    [
      JSON.stringify({
        type: 'assistant',
        effort: 'medium',
        timestamp: '2026-11-11T10:00:00.000Z',
        message: { model: 'claude-test' },
      }),
      JSON.stringify({ type: 'user', effort: 'max' }),
      JSON.stringify({ type: 'assistant', message: { model: '<synthetic>' } }),
      '{"type":"assistant",',
    ].join('\n'),
  )
  expect(mod.effortForResume(settingsDir, CONV)).toBe('medium')
  const helpers = await import('./session-resume-helpers')
  expect(helpers.lastModelEntryFromJsonlTail(jsonl)).toEqual({
    model: 'claude-test',
    timestamp: '2026-11-11T10:00:00.000Z',
  })
})
