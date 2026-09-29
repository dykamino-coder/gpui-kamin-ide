// Тянет за собой потоковый прокси → @peculiar/x509 → tsyringe, которому
// нужен полифил reflect до декораторов. Тот же импорт, что и у точки входа.
import 'reflect-metadata'

import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

/**
 * INC-2026-0054: беседа старше окна очистки Claude Code переставала
 * резюмироваться навсегда.
 *
 * CLI выметает `~/.claude/projects/` по `cleanupPeriodDays` (по документации —
 * 30 дней). После этого `findOrRecreateSettingsDir` не находит `<id>.jsonl` ни
 * в одном slug-каталоге, сервер выбрасывает запрошенный id и заводит новую
 * сессию: строки в базе беседу резюмируемой сделать не могут.
 *
 * Подменяется только источник каталога данных; домашний каталог уводится во
 * временный. Файловая система, сам модуль резюма и архив — настоящие, а
 * истечение изображается ровно тем, что делает CLI: удалением файла.
 */

const holder = vi.hoisted(() => ({ dataDir: '' }))
vi.mock('../stats/database/lifecycle', () => ({ getDataDir: () => holder.dataDir }))

const CONV = '11111111-2222-3333-4444-555555555555'

let home = ''
let envBefore: { HOME?: string; USERPROFILE?: string } = {}
const dirs: string[] = []

function projectsDir(): string {
  return path.join(home, '.claude', 'projects')
}

/** Настоящий settingsDir внутри SESSIONS_BASE + его slug-каталог с расшифровкой,
 *  ровно в той раскладке, которую строит сервер. */
function seedConversation(lines: string[] = ['{"type":"user","uuid":"u1"}']): { settingsDir: string; jsonl: string } {
  const settingsDir = path.join(home, '.claude', 'bridge-sessions', 'token_x', 'sess-1')
  fs.mkdirSync(settingsDir, { recursive: true })
  const slug = path.resolve(settingsDir).replace(/[^a-zA-Z0-9]/g, '-')
  const slugDir = path.join(projectsDir(), slug)
  fs.mkdirSync(slugDir, { recursive: true })
  const jsonl = path.join(slugDir, `${CONV}.jsonl`)
  fs.writeFileSync(jsonl, lines.join('\n') + '\n')
  return { settingsDir, jsonl }
}

/** Свежий экземпляр модулей: `SESSIONS_BASE` вычисляется на импорте из
 *  `os.homedir()`, поэтому переменные окружения обязаны стоять раньше. */
async function freshHelpers(): Promise<typeof import('./session-resume-helpers')> {
  vi.resetModules()
  await import('reflect-metadata')
  return import('./session-resume-helpers')
}

async function freshArchive(): Promise<typeof import('./transcript-archive')> {
  vi.resetModules()
  await import('reflect-metadata')
  return import('./transcript-archive')
}

beforeEach(() => {
  home = fs.mkdtempSync(path.join(os.tmpdir(), 'inc-0054-'))
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

describe('INC-2026-0054: беседа резюмируется, пока её не удалили явно', () => {
  it('песочница действительно подменяет дом и каталог данных', async () => {
    // Иначе набор молча трогал бы настоящие ~/.claude и data/ репозитория.
    const { SESSIONS_BASE } = await import('./session-settings')
    const { archiveRoot } = await freshArchive()
    expect(SESSIONS_BASE.startsWith(home)).toBe(true)
    expect(archiveRoot().startsWith(home)).toBe(true)
  })

  it('обычное резюме заводит долговечную копию', async () => {
    const { settingsDir } = seedConversation()
    const helpers = await freshHelpers()

    expect(helpers.findOrRecreateSettingsDir(CONV, 'token_x')).toBe(settingsDir)

    const { archivedTranscriptPath } = await import('./transcript-archive')
    expect(fs.existsSync(archivedTranscriptPath(CONV)!)).toBe(true)
  })

  it('после выметания расшифровки беседа всё ещё резюмируется', async () => {
    const { settingsDir, jsonl } = seedConversation()
    const helpers = await freshHelpers()
    expect(helpers.findOrRecreateSettingsDir(CONV, 'token_x')).toBe(settingsDir)

    // Ровно то, что делает уборка CLI по `cleanupPeriodDays`.
    fs.rmSync(jsonl)
    expect(fs.existsSync(jsonl)).toBe(false)

    expect(helpers.findOrRecreateSettingsDir(CONV, 'token_x')).toBe(settingsDir)
    expect(fs.existsSync(jsonl)).toBe(true)
    expect(fs.readFileSync(jsonl, 'utf8')).toContain('"uuid":"u1"')
  })

  it('перезапуск сервера восстановлению не мешает', async () => {
    const { settingsDir, jsonl } = seedConversation()
    ;(await freshHelpers()).findOrRecreateSettingsDir(CONV, 'token_x')
    fs.rmSync(jsonl)

    // Новый экземпляр модулей = новый процесс: состояние только на диске.
    const afterRestart = await freshHelpers()

    expect(afterRestart.findOrRecreateSettingsDir(CONV, 'token_x')).toBe(settingsDir)
    expect(fs.existsSync(jsonl)).toBe(true)
  })

  it('живая расшифровка всегда сильнее архива', async () => {
    const { jsonl } = seedConversation()
    ;(await freshHelpers()).findOrRecreateSettingsDir(CONV, 'token_x')

    // Беседу продолжили: живой файл длиннее архива и не имеет права быть
    // затёртым устаревшей копией.
    fs.appendFileSync(jsonl, '{"type":"assistant","uuid":"u2"}\n')
    const helpers = await freshHelpers()
    helpers.findOrRecreateSettingsDir(CONV, 'token_x')

    expect(fs.readFileSync(jsonl, 'utf8')).toContain('"uuid":"u2"')
  })

  it('более короткий живой файл не затирает более длинный архив', async () => {
    const { jsonl } = seedConversation(['{"type":"user","uuid":"u1"}', '{"type":"assistant","uuid":"u2"}'])
    const archive = await freshArchive()
    archive.archiveTranscript(jsonl, CONV)

    // Форк или обрезание: расшифровка стала короче. Архив обязан устоять.
    fs.writeFileSync(jsonl, '{"type":"user","uuid":"u1"}\n')
    archive.archiveTranscript(jsonl, CONV)

    expect(fs.readFileSync(archive.archivedTranscriptPath(CONV)!, 'utf8')).toContain('"uuid":"u2"')
  })

  it('битый архив не восстанавливается', async () => {
    const { jsonl } = seedConversation()
    const archive = await freshArchive()
    archive.archiveTranscript(jsonl, CONV)
    fs.writeFileSync(archive.archivedTranscriptPath(CONV)!, 'это не jsonl\n')
    fs.rmSync(jsonl)

    expect(archive.restoreArchivedTranscript(projectsDir(), CONV)).toBeNull()
    expect(fs.existsSync(jsonl)).toBe(false)
  })

  it('оператор может отключить копию, и тогда поведение прежнее', async () => {
    // Копия держит полный текст расшифровки. Тому, кому она на этом хосте не
    // нужна, нужен выключатель — и после него резюм обязан вести себя как
    // раньше, включая потерю истёкшей беседы.
    process.env.KAMIN_TRANSCRIPT_ARCHIVE = 'off'
    try {
      const { jsonl } = seedConversation()
      const helpers = await freshHelpers()
      helpers.findOrRecreateSettingsDir(CONV, 'token_x')
      fs.rmSync(jsonl)

      expect(helpers.findOrRecreateSettingsDir(CONV, 'token_x')).toBeNull()
    } finally {
      delete process.env.KAMIN_TRANSCRIPT_ARCHIVE
    }
  })

  it('идентификатор с выходом за каталог отвергается, а не чистится', async () => {
    const archive = await freshArchive()
    expect(archive.archivedTranscriptPath('../../etc/passwd')).toBeNull()
    expect(archive.restoreArchivedTranscript(projectsDir(), '../../etc/passwd')).toBeNull()
    expect(archive.dropArchivedTranscript('../../etc/passwd')).toBe(false)
  })
})

describe('INC-2026-0054: беседа, которую больше не открывали', () => {
  it('снимок делается на завершении сессии, а не только на резюме', async () => {
    // Сессия отработала один раз и больше не открывалась: путь резюма её
    // расшифровку не копировал бы никогда, и уборка CLI унесла бы её насовсем.
    const { settingsDir } = seedConversation()
    const { sessions, finalizeSessionTeardown } = await import('./session-core')
    const { archivedTranscriptPath } = await import('./transcript-archive')
    sessions.set('s-1', {
      id: 's-1',
      settingsDir,
      cliConversationId: CONV,
      tokenId: 'token_x',
      userName: 'тест',
      childSessions: [],
    } as never)

    finalizeSessionTeardown('s-1', 'pty-exit', 0)

    expect(fs.existsSync(archivedTranscriptPath(CONV)!)).toBe(true)
    sessions.delete('s-1')
  })
})

describe('INC-2026-0054: явное удаление делает беседу недоступной', () => {
  it('удаление сносит и долговечную копию, так что выметание её не воскресит', async () => {
    const { jsonl } = seedConversation()
    const helpers = await freshHelpers()
    helpers.findOrRecreateSettingsDir(CONV, 'token_x')
    const { archivedTranscriptPath } = await import('./transcript-archive')
    expect(fs.existsSync(archivedTranscriptPath(CONV)!)).toBe(true)

    const { deleteSessionByConversationId } = await import('./session-core')
    await deleteSessionByConversationId(CONV, 'тест')

    expect(fs.existsSync(archivedTranscriptPath(CONV)!)).toBe(false)
    expect(fs.existsSync(jsonl)).toBe(false)
    expect(helpers.findOrRecreateSettingsDir(CONV, 'token_x')).toBeNull()
  })

  it('удаление снимает копию и тогда, когда живой расшифровки уже нет', async () => {
    // Порядок из полевого случая: CLI вымел файл, пользователь удалил беседу.
    // Обход каталогов возвращается на первом совпадении и до архива не дошёл бы.
    const { jsonl } = seedConversation()
    const helpers = await freshHelpers()
    helpers.findOrRecreateSettingsDir(CONV, 'token_x')
    fs.rmSync(jsonl)

    const { deleteSessionByConversationId } = await import('./session-core')
    await deleteSessionByConversationId(CONV, 'тест')

    const { archivedTranscriptPath } = await import('./transcript-archive')
    expect(fs.existsSync(archivedTranscriptPath(CONV)!)).toBe(false)
    expect(helpers.findOrRecreateSettingsDir(CONV, 'token_x')).toBeNull()
  })
})
