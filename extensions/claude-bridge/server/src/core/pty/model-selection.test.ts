// Тянет за собой потоковый прокси → @peculiar/x509 → tsyringe, которому нужен
// полифил reflect до декораторов. Тот же импорт, что и у точки входа.
import 'reflect-metadata'

import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

/**
 * INC-2026-0053: горячая смена модели терялась на респауне.
 *
 * `/model <id>` уходит в живой CLI и больше нигде не записан. Если PTY
 * перезапускается ДО первого ответа ассистента — отсоединение, перезапуск
 * приложения, смена эффорта, — путь резюма несёт пустую модель, и
 * `createSession` достаёт её из последней строки ассистента в расшифровке.
 * Там лежит ПРЕЖНЯЯ модель, и она уходит в `--model`, который сильнее того,
 * что восстановил бы Claude Code.
 *
 * Подменяется только источник каталога данных; дом уведён во временный.
 */

const holder = vi.hoisted(() => ({ dataDir: '' }))
vi.mock('../stats/database/lifecycle', () => ({ getDataDir: () => holder.dataDir }))

const CONV = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'

let home = ''
let envBefore: { HOME?: string; USERPROFILE?: string } = {}
const dirs: string[] = []

async function fresh(): Promise<typeof import('./model-selection')> {
  vi.resetModules()
  await import('reflect-metadata')
  return import('./model-selection')
}

beforeEach(() => {
  home = fs.mkdtempSync(path.join(os.tmpdir(), 'inc-0053-'))
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

describe('INC-2026-0053: выбор модели переживает респаун', () => {
  it('песочница действительно подменяет каталог данных', async () => {
    const mod = await fresh()
    mod.rememberModelSelection(CONV, 'claude-fable-5-1')
    expect(fs.existsSync(path.join(holder.dataDir, 'model-selection.json'))).toBe(true)
  })

  it('выбор без единого ответа ассистента выигрывает', async () => {
    // Ровно полевой случай: смена модели, затем отсоединение до ответа.
    const mod = await fresh()
    mod.rememberModelSelection(CONV, 'claude-fable-5-1')

    expect(mod.modelForResumeWithSelection(CONV, null)).toBe('claude-fable-5-1')
  })

  it('выбор сильнее расшифровки, пока он свежее последнего ответа', async () => {
    const mod = await fresh()
    mod.rememberModelSelection(CONV, 'claude-fable-5-1', new Date('2026-09-24T12:00:00.000Z'))

    const older = { model: 'claude-opus-5', timestamp: '2026-09-24T11:59:00.000Z' }
    expect(mod.modelForResumeWithSelection(CONV, older)).toBe('claude-fable-5-1')
  })

  it('ответ, пришедший ПОСЛЕ выбора, сильнее выбора', async () => {
    // Ответ несёт модель, которой CLI действительно воспользовался. Иначе
    // устаревший выбор вечно перебивал бы модель, с которой беседа уже ушла.
    const mod = await fresh()
    mod.rememberModelSelection(CONV, 'claude-fable-5-1', new Date('2026-09-24T12:00:00.000Z'))

    const newer = { model: 'claude-opus-5-5', timestamp: '2026-09-24T12:01:00.000Z' }
    expect(mod.modelForResumeWithSelection(CONV, newer)).toBe('claude-opus-5-5')
  })

  it('без выбора поведение прежнее — модель из расшифровки', async () => {
    const mod = await fresh()
    expect(mod.modelForResumeWithSelection(CONV, { model: 'claude-opus-5', timestamp: 'x' })).toBe('claude-opus-5')
    expect(mod.modelForResumeWithSelection(CONV, null)).toBeNull()
  })

  it('выбор переживает перезапуск сервера', async () => {
    ;(await fresh()).rememberModelSelection(CONV, 'claude-fable-5-1')

    // Новый экземпляр модулей = новый процесс: состояние только на диске.
    const afterRestart = await fresh()

    expect(afterRestart.readModelSelection(CONV)?.model).toBe('claude-fable-5-1')
  })

  it('явное забывание снимает выбор', async () => {
    const mod = await fresh()
    mod.rememberModelSelection(CONV, 'claude-fable-5-1')
    mod.forgetModelSelection(CONV)

    expect(mod.readModelSelection(CONV)).toBeNull()
    expect(mod.modelForResumeWithSelection(CONV, null)).toBeNull()
  })

  it('битое хранилище не роняет резюм', async () => {
    const mod = await fresh()
    fs.mkdirSync(holder.dataDir, { recursive: true })
    fs.writeFileSync(path.join(holder.dataDir, 'model-selection.json'), 'не json')

    expect(mod.readModelSelection(CONV)).toBeNull()
    expect(() => {
      mod.rememberModelSelection(CONV, 'claude-opus-5-5')
    }).not.toThrow()
    expect(mod.readModelSelection(CONV)?.model).toBe('claude-opus-5-5')
  })
})

describe('INC-2026-0053: запись в stdin отчитывается честно', () => {
  it('на неживом PTY запись не считается удавшейся', async () => {
    vi.resetModules()
    await import('reflect-metadata')
    const { writeInputToSession } = await import('./session-io')

    const dead = { id: 's-1', state: 'exiting', pty: { write: () => undefined } } as never
    expect(writeInputToSession(dead, '/model claude-fable-5-1\r')).toBe(false)
  })

  it('на живом PTY запись считается удавшейся и доезжает до него', async () => {
    vi.resetModules()
    await import('reflect-metadata')
    const { writeInputToSession } = await import('./session-io')

    const written: string[] = []
    const live = {
      id: 's-2',
      state: 'running',
      pty: {
        write: (data: string) => {
          written.push(data)
        },
      },
    } as never

    expect(writeInputToSession(live, '/model claude-fable-5-1\r')).toBe(true)
    expect(written.join('')).toContain('/model claude-fable-5-1')
  })
})

describe('INC-2026-0053: метка времени последнего ответа читается из расшифровки', () => {
  it('отдаётся модель И время записи, а не только модель', async () => {
    // Вторая половина связки: решение о модели сравнивает время выбора со
    // временем последнего ответа, значит время обязано доезжать из файла.
    vi.resetModules()
    await import('reflect-metadata')
    const { lastModelEntryForResume } = await import('./session-resume-helpers')

    const settingsDir = path.join(home, '.claude', 'bridge-sessions', 'token_x', 'sess-1')
    fs.mkdirSync(settingsDir, { recursive: true })
    const slug = path.resolve(settingsDir).replace(/[^a-zA-Z0-9]/g, '-')
    const slugDir = path.join(home, '.claude', 'projects', slug)
    fs.mkdirSync(slugDir, { recursive: true })
    fs.writeFileSync(
      path.join(slugDir, `${CONV}.jsonl`),
      [
        JSON.stringify({ type: 'user', uuid: 'u1' }),
        JSON.stringify({
          type: 'assistant',
          timestamp: '2026-09-24T12:01:00.000Z',
          message: { id: 'msg_1', model: 'claude-opus-5', stop_reason: 'end_turn' },
        }),
      ].join('\n') + '\n',
    )

    expect(lastModelEntryForResume(settingsDir, CONV)).toEqual({
      model: 'claude-opus-5',
      timestamp: '2026-09-24T12:01:00.000Z',
    })
  })
})
