import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

/** INC-2026-0041: исход обновления маркетплейса терялся на границе свода.
 *
 *  Обычный отказ `git pull` — это РАЗРЕШЁННОЕ обещание с `{ok:false}`, а не
 *  исключение. `refreshAllMarketplaces` его не смотрел, IPC безусловно отдавал
 *  `{ok:true}`, а чип по событию завершения гасил спиннер как после успеха.
 *
 *  Подменяется только транспорт — запуск `git`. Сам модуль обновления, хранилище
 *  `known_marketplaces.json`, событие в окно и хук идут настоящие. */

const gitCalls: string[][] = []
let gitBehaviour: (args: string[], cwd: string) => { stdout: string } = () => ({ stdout: 'Already up to date.' })

vi.mock('../lib/git-async', () => ({
  runGit: (args: string[], opts: { cwd: string }) => {
    gitCalls.push(args)
    return Promise.resolve(gitBehaviour(args, opts.cwd)).then(r => ({ stderr: '', code: 0, ...r }))
  },
}))

const hookCalls: { event: string; payload: unknown }[] = []
vi.mock('../hooks/emit-bridge-event', () => ({
  emitBridgeHookEvent: (event: string, payload: unknown) => { hookCalls.push({ event, payload }) },
}))

/** Ошибка ровно того вида, какой строит `runGit`: текст содержит кусок stderr. */
function gitFailure(stderr: string): never {
  const err = new Error(`git exited with code 1: ${stderr.slice(0, 500)}`) as Error & { stderr: string }
  err.stderr = stderr
  throw err
}

let home = ''
let homeBefore: { HOME?: string; USERPROFILE?: string } = {}
const dirs: string[] = []

function marketplaceDir(name: string): string {
  const loc = join(home, 'checkouts', name)
  mkdirSync(loc, { recursive: true })
  // Метка в песочнице чекаута: отказ обновления не имеет права её снести.
  writeFileSync(join(loc, 'marker.txt'), 'не трогать', 'utf-8')
  return loc
}

function writeKnown(entries: Record<string, unknown>): void {
  const file = join(home, '.claude', 'plugins', 'known_marketplaces.json')
  mkdirSync(join(home, '.claude', 'plugins'), { recursive: true })
  writeFileSync(file, JSON.stringify(entries, null, 2), 'utf-8')
}

function gitEntry(name: string, extra: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    source: { source: 'git', url: 'https://example.invalid/repo.git' },
    installLocation: marketplaceDir(name),
    lastUpdated: '2026-01-01T00:00:00.000Z',
    ...extra,
  }
}

interface SentEvent { channel: string; payload: { name: string; ok?: boolean; error?: string } }

function fakeWindow(sent: SentEvent[]): never {
  return {
    isDestroyed: () => false,
    webContents: { send: (channel: string, payload: SentEvent['payload']) => { sent.push({ channel, payload }) } },
  } as never
}

beforeEach(() => {
  gitCalls.length = 0
  hookCalls.length = 0
  gitBehaviour = () => ({ stdout: 'Already up to date.' })
  home = mkdtempSync(join(tmpdir(), 'inc-0041-'))
  dirs.push(home)
  homeBefore = { HOME: process.env.HOME, USERPROFILE: process.env.USERPROFILE }
  process.env.HOME = home
  process.env.USERPROFILE = home
})

afterEach(() => {
  if (homeBefore.HOME === undefined) delete process.env.HOME
  else process.env.HOME = homeBefore.HOME
  if (homeBefore.USERPROFILE === undefined) delete process.env.USERPROFILE
  else process.env.USERPROFILE = homeBefore.USERPROFILE
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true })
})

describe('INC-2026-0041: исход обновления маркетплейса доезжает до вызывающего', () => {
  it('песочница теста действительно подменяет домашний каталог', async () => {
    // Иначе весь файл тихо работал бы с настоящим `~/.claude` пользователя.
    const { knownMarketplacesPath } = await import('./known-store')
    expect(knownMarketplacesPath().startsWith(home)).toBe(true)
  })

  it('отказ одного маркетплейса не выдаётся за общий успех, остальные продолжают', async () => {
    const { refreshAllMarketplaces } = await import('./refresh')
    writeKnown({ alpha: gitEntry('alpha'), beta: gitEntry('beta'), gamma: gitEntry('gamma') })
    gitBehaviour = (_args, cwd) => {
      if (cwd.endsWith('beta')) gitFailure('fatal: unable to access repository')
      return { stdout: 'Updating 1a2b3c..4d5e6f' }
    }
    const sent: SentEvent[] = []

    const sweep = await refreshAllMarketplaces(fakeWindow(sent))

    expect(sweep.ok).toBe(false)
    expect(sweep.results.map(r => [r.name, r.ok])).toEqual([['alpha', true], ['beta', false], ['gamma', true]])
    expect(sweep.results[1]?.error).toContain('unable to access')
    // Следующий маркетплейс после отказавшего обязан быть обновлён.
    expect(gitCalls).toHaveLength(3)
  })

  it('все успешные дают общий успех, все отказавшие — общий отказ', async () => {
    const { refreshAllMarketplaces } = await import('./refresh')
    writeKnown({ alpha: gitEntry('alpha'), beta: gitEntry('beta') })

    const allGood = await refreshAllMarketplaces()
    expect(allGood.ok).toBe(true)
    expect(allGood.results.every(r => r.ok)).toBe(true)

    gitBehaviour = () => gitFailure('fatal: Authentication failed')
    const allBad = await refreshAllMarketplaces()
    expect(allBad.ok).toBe(false)
    expect(allBad.results.every(r => !r.ok)).toBe(true)
  })

  it('неожиданный бросок становится таким же отказом, а не тишиной', async () => {
    const { refreshAllMarketplaces } = await import('./refresh')
    // Недопустимое имя в хранилище роняет `assertValidName` ДО собственного
    // try обновления: прежний пустой catch свода проглатывал такой бросок и
    // маркетплейс просто исчезал из итога, будто его и не было.
    writeKnown({ '../evil': gitEntry('evil'), beta: gitEntry('beta') })

    const sweep = await refreshAllMarketplaces()

    expect(sweep.ok).toBe(false)
    expect(sweep.results.map(r => [r.name, r.ok])).toEqual([['../evil', false], ['beta', true]])
    expect(String(sweep.results[0]?.error)).toBeTruthy()
  })

  it('бросок не-Error не превращается в бесполезное «unknown error»', async () => {
    const { refreshAllMarketplaces } = await import('./refresh')
    writeKnown({ alpha: gitEntry('alpha') })
    gitBehaviour = () => { throw 'сломанный git' }

    const sweep = await refreshAllMarketplaces()

    expect(sweep.ok).toBe(false)
    expect(String(sweep.results[0]?.error)).toContain('сломанный git')
  })

  it('на отказ уходит событие завершения с исходом, но не хук успеха', async () => {
    const { refreshAllMarketplaces } = await import('./refresh')
    writeKnown({ alpha: gitEntry('alpha'), beta: gitEntry('beta') })
    gitBehaviour = (_args, cwd) => {
      if (cwd.endsWith('alpha')) gitFailure('fatal: Authentication failed')
      return { stdout: 'Already up to date.' }
    }
    const sent: SentEvent[] = []

    await refreshAllMarketplaces(fakeWindow(sent))

    expect(sent.map(e => [e.payload.name, e.payload.ok])).toEqual([['alpha', false], ['beta', true]])
    expect(sent[0]?.payload.error).toBeTruthy()
    expect(hookCalls.map(h => (h.payload as { name: string }).name)).toEqual(['beta'])
  })

  it('токен не утекает в текст ошибки', async () => {
    const { refreshAllMarketplaces } = await import('./refresh')
    writeKnown({ alpha: gitEntry('alpha') })
    // Синтетические учётные данные: `runGit` вклеивает кусок stderr прямо в
    // message, поэтому редактировать одно поле stderr было недостаточно.
    gitBehaviour = () => gitFailure('fatal: unable to access https://oauth2:synthetic_pat_value@example.invalid/repo.git/')

    const sweep = await refreshAllMarketplaces()

    const text = JSON.stringify(sweep)
    expect(text).not.toContain('synthetic_pat_value')
    expect(text).toContain('***')
  })

  it('отключённый автообновлением маркетплейс не трогается', async () => {
    const { refreshAllMarketplaces } = await import('./refresh')
    writeKnown({ alpha: gitEntry('alpha', { autoUpdate: false }), beta: gitEntry('beta') })

    const sweep = await refreshAllMarketplaces()

    expect(sweep.results.map(r => r.name)).toEqual(['beta'])
    expect(gitCalls).toHaveLength(1)
  })

  it('каталоговый маркетплейс — успех без вызова git', async () => {
    const { refreshAllMarketplaces } = await import('./refresh')
    const loc = marketplaceDir('local')
    writeKnown({
      local: {
        source: { source: 'directory', path: loc },
        installLocation: loc,
        lastUpdated: '2026-01-01T00:00:00.000Z',
      },
    })

    const sweep = await refreshAllMarketplaces()

    expect(sweep.ok).toBe(true)
    expect(sweep.results[0]).toMatchObject({ name: 'local', ok: true, changed: false })
    expect(gitCalls).toHaveLength(0)
  })

  it('неизменившийся успех отличается от изменившегося', async () => {
    const { refreshAllMarketplaces } = await import('./refresh')
    writeKnown({ alpha: gitEntry('alpha') })
    expect((await refreshAllMarketplaces()).results[0]?.changed).toBe(false)

    gitBehaviour = () => ({ stdout: 'Updating 1a2b3c..4d5e6f\n 1 file changed' })
    expect((await refreshAllMarketplaces()).results[0]?.changed).toBe(true)
  })

  it('отказ не сносит содержимое чекаута', async () => {
    const { refreshAllMarketplaces } = await import('./refresh')
    const entry = gitEntry('alpha')
    writeKnown({ alpha: entry })
    gitBehaviour = () => gitFailure('fatal: Authentication failed')

    await refreshAllMarketplaces()

    const loc = String((entry as { installLocation: string }).installLocation)
    expect(existsSync(join(loc, 'marker.txt'))).toBe(true)
    expect(readFileSync(join(loc, 'marker.txt'), 'utf-8')).toBe('не трогать')
  })
})

describe('INC-2026-0041: подсказки по категориям отказа сохранены', () => {
  it.each([
    ['fatal: Authentication failed for https://example.invalid', 'Personal Access Token'],
    ['hint: Updates were rejected because the tip is behind; non-fast-forward', 'diverged'],
    ['fatal: terminal prompts disabled', 'Re-add the marketplace with a token'],
  ])('категория отказа сохраняет подсказку: %s', async (stderr, hint) => {
    const { refreshMarketplaceOnce } = await import('./refresh')
    writeKnown({ alpha: gitEntry('alpha') })
    gitBehaviour = () => gitFailure(stderr)

    const result = await refreshMarketplaceOnce('alpha')

    expect(result.ok).toBe(false)
    expect(result.error).toContain(hint)
  })
})
