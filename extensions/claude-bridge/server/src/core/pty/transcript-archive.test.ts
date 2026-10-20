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
const TOKEN = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee'

let home = ''
let envBefore: { HOME?: string; USERPROFILE?: string } = {}
const dirs: string[] = []

function projectsDir(): string {
  return path.join(home, '.claude', 'projects')
}

/** Настоящий settingsDir внутри SESSIONS_BASE + его slug-каталог с расшифровкой,
 *  ровно в той раскладке, которую строит сервер. */
function seedConversation(
  lines: string[] = ['{"type":"user","uuid":"u1"}'],
  tokenDir = TOKEN,
): { settingsDir: string; jsonl: string } {
  const settingsDir = path.join(home, '.claude', 'bridge-sessions', tokenDir, 'sess-1')
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
  vi.restoreAllMocks()
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

    expect(helpers.findOrRecreateSettingsDir(CONV, TOKEN)).toBe(settingsDir)

    const { archivedTranscriptPath } = await import('./transcript-archive')
    expect(fs.existsSync(archivedTranscriptPath(CONV)!)).toBe(true)
  })

  it('после выметания расшифровки беседа всё ещё резюмируется', async () => {
    const { settingsDir, jsonl } = seedConversation()
    const helpers = await freshHelpers()
    expect(helpers.findOrRecreateSettingsDir(CONV, TOKEN)).toBe(settingsDir)

    // Ровно то, что делает уборка CLI по `cleanupPeriodDays`.
    fs.rmSync(jsonl)
    expect(fs.existsSync(jsonl)).toBe(false)

    expect(helpers.findOrRecreateSettingsDir(CONV, TOKEN)).toBe(settingsDir)
    expect(fs.existsSync(jsonl)).toBe(true)
    expect(fs.readFileSync(jsonl, 'utf8')).toContain('"uuid":"u1"')
  })

  it('перезапуск сервера восстановлению не мешает', async () => {
    const { settingsDir, jsonl } = seedConversation()
    ;(await freshHelpers()).findOrRecreateSettingsDir(CONV, TOKEN)
    fs.rmSync(jsonl)

    // Новый экземпляр модулей = новый процесс: состояние только на диске.
    const afterRestart = await freshHelpers()

    expect(afterRestart.findOrRecreateSettingsDir(CONV, TOKEN)).toBe(settingsDir)
    expect(fs.existsSync(jsonl)).toBe(true)
  })

  it('живая расшифровка всегда сильнее архива', async () => {
    const { jsonl } = seedConversation()
    ;(await freshHelpers()).findOrRecreateSettingsDir(CONV, TOKEN)

    // Беседу продолжили: живой файл длиннее архива и не имеет права быть
    // затёртым устаревшей копией.
    fs.appendFileSync(jsonl, '{"type":"assistant","uuid":"u2"}\n')
    const helpers = await freshHelpers()
    helpers.findOrRecreateSettingsDir(CONV, TOKEN)

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
      helpers.findOrRecreateSettingsDir(CONV, TOKEN)
      fs.rmSync(jsonl)

      expect(helpers.findOrRecreateSettingsDir(CONV, TOKEN)).toBeNull()
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
      tokenId: TOKEN,
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
    helpers.findOrRecreateSettingsDir(CONV, TOKEN)
    const { archivedTranscriptPath } = await import('./transcript-archive')
    expect(fs.existsSync(archivedTranscriptPath(CONV)!)).toBe(true)

    const { deleteSessionByConversationId } = await import('./session-core')
    await deleteSessionByConversationId(CONV, 'тест', TOKEN)

    expect(fs.existsSync(archivedTranscriptPath(CONV)!)).toBe(false)
    expect(fs.existsSync(jsonl)).toBe(false)
    expect(helpers.findOrRecreateSettingsDir(CONV, TOKEN)).toBeNull()
  })

  it('удаление снимает копию и тогда, когда живой расшифровки уже нет', async () => {
    // Порядок из полевого случая: CLI вымел файл, пользователь удалил беседу.
    // Обход каталогов возвращается на первом совпадении и до архива не дошёл бы.
    const { jsonl } = seedConversation()
    const helpers = await freshHelpers()
    helpers.findOrRecreateSettingsDir(CONV, TOKEN)
    fs.rmSync(jsonl)

    const { deleteSessionByConversationId } = await import('./session-core')
    await deleteSessionByConversationId(CONV, 'тест', TOKEN)

    const { archivedTranscriptPath } = await import('./transcript-archive')
    expect(fs.existsSync(archivedTranscriptPath(CONV)!)).toBe(false)
    expect(helpers.findOrRecreateSettingsDir(CONV, TOKEN)).toBeNull()
  })
})

describe('AB-01 delete namespace isolation', () => {
  it.each([undefined, 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeef'])(
    'refuses unknown/cross-token caller %s without touching victim data',
    async (tokenId) => {
      const { settingsDir, jsonl } = seedConversation()
      if (tokenId)
        fs.mkdirSync(path.join(home, '.claude', 'bridge-sessions', tokenId, 'own-session'), { recursive: true })
      await freshHelpers()
      const archive = await import('./transcript-archive')
      archive.archiveTranscript(jsonl, CONV)
      const { deleteSessionByConversationId } = await import('./session-core')
      const logging = await import('../logging')
      const refusal = vi.spyOn(logging, 'warnLog')
      const remove = vi.spyOn(fs, 'rmSync')
      const scan = vi.spyOn(fs, 'readdirSync')
      await expect(deleteSessionByConversationId(CONV, 'unknown', tokenId!)).rejects.toThrow()
      expect(refusal).toHaveBeenCalledWith(expect.stringContaining('deletion refused'), expect.any(Object))
      expect(remove).not.toHaveBeenCalled()
      const base = path.join(home, '.claude', 'bridge-sessions')
      expect(scan.mock.calls.every(([dir]) => String(dir) === base || String(dir) === path.join(base, tokenId!))).toBe(
        true,
      )
      expect(fs.existsSync(settingsDir)).toBe(true)
      expect(fs.existsSync(jsonl)).toBe(true)
      expect(fs.existsSync(archive.archivedTranscriptPath(CONV)!)).toBe(true)
    },
  )

  it.each([
    ['C:\\Projects\\my-app', false],
    ['C:\\Projects\\my-app', true],
    ['/work/my project-юникод', false],
    ['/', false],
    [`/${'a'.repeat(79)}/tail`, false],
  ] as const)('deletes project history for cwd=%s, CLI transcript expired=%s', async (cwd, expired) => {
    await freshHelpers()
    const { sanitizeDirName } = await import('./session-settings')
    const { settingsDir, jsonl } = seedConversation(undefined, `${TOKEN}-${sanitizeDirName(cwd)}`)
    const archive = await import('./transcript-archive')
    archive.archiveTranscript(jsonl, CONV)
    if (expired) fs.rmSync(jsonl)
    const { deleteSessionByConversationId } = await import('./session-core')

    await deleteSessionByConversationId(CONV, 'project owner', TOKEN)

    expect(fs.existsSync(settingsDir)).toBe(false)
    expect(fs.existsSync(jsonl)).toBe(false)
    expect(fs.existsSync(archive.archivedTranscriptPath(CONV)!)).toBe(false)
  })

  it.each([
    'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeef-project',
    `${TOKEN}extra-project`,
    `${TOKEN}-_invalid`,
    `${TOKEN}-invalid_`,
    `${TOKEN}-double__underscore`,
    `${TOKEN}-${'a'.repeat(81)}`,
  ])('does not enter foreign or invalid namespace %s', async (tokenDir) => {
    await freshHelpers()
    const { settingsDir, jsonl } = seedConversation(undefined, tokenDir)
    const { deleteSessionByConversationId } = await import('./session-core')
    const scan = vi.spyOn(fs, 'readdirSync')
    const stat = vi.spyOn(fs, 'lstatSync')
    const remove = vi.spyOn(fs, 'rmSync')
    await expect(deleteSessionByConversationId(CONV, 'caller', TOKEN)).rejects.toThrow()
    expect(scan.mock.calls.map(([dir]) => String(dir))).toEqual([path.dirname(path.dirname(settingsDir))])
    expect(stat.mock.calls.some(([dir]) => String(dir) === path.dirname(settingsDir))).toBe(false)
    expect(remove).not.toHaveBeenCalled()
    expect(fs.existsSync(jsonl)).toBe(true)
  })

  it.each(['token', 'session', 'project'])('refuses a %s directory link', async (kind) => {
    await freshHelpers()
    const { settingsDir, jsonl } = seedConversation(undefined, `${TOKEN}-project`)
    const linked = kind === 'token' ? path.dirname(settingsDir) : kind === 'session' ? settingsDir : path.dirname(jsonl)
    const destination = path.join(home, 'foreign-data')
    fs.renameSync(linked, destination)
    fs.symlinkSync(destination, linked, 'junction')
    const { deleteSessionByConversationId } = await import('./session-core')
    const remove = vi.spyOn(fs, 'rmSync')
    await expect(deleteSessionByConversationId(CONV, 'caller', TOKEN)).rejects.toThrow()
    expect(remove).not.toHaveBeenCalled()
    expect(fs.existsSync(destination)).toBe(true)
    expect(fs.existsSync(jsonl)).toBe(true)
  })

  it.each([TOKEN.slice(0, 8), TOKEN.slice(0, -1), `${TOKEN}extra`, '../escape'])(
    'refuses malformed or prefix-only token %s without scanning a foreign namespace',
    async (tokenId) => {
      await freshHelpers()
      const { settingsDir, jsonl } = seedConversation(undefined, `${TOKEN}-project`)
      const { deleteSessionByConversationId } = await import('./session-core')
      const scan = vi.spyOn(fs, 'readdirSync')
      const remove = vi.spyOn(fs, 'rmSync')
      await expect(deleteSessionByConversationId(CONV, 'unknown', tokenId)).rejects.toThrow()
      expect(scan).not.toHaveBeenCalled()
      expect(remove).not.toHaveBeenCalled()
      expect(fs.existsSync(settingsDir)).toBe(true)
      expect(fs.existsSync(jsonl)).toBe(true)
    },
  )
})
