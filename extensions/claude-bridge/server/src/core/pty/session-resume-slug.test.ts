import 'reflect-metadata'

import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

/**
 * INC-2026-0070 side issue: a cold resume whose settings directory is gone
 * rebuilt it from the project slug and turned every hyphen of the token id
 * into an underscore, so a duplicate token directory appeared next to the
 * real one. The slug (and so the resume itself) was unaffected; these tests
 * pin that the rebuilt path keeps the slug AND reuses the real token dir.
 */

const holder = vi.hoisted(() => ({ dataDir: '' }))
vi.mock('../stats/database/lifecycle', () => ({ getDataDir: () => holder.dataDir }))

const TOKEN = '8a07ae2d-9d0f-4c1e-9a55-000000000001'
const SESSION = '00000000-0000-4000-8000-0000000000aa'
const CONV = '00000000-0000-4000-8000-000000000001'

let home = ''
let envBefore: { HOME?: string; USERPROFILE?: string } = {}

async function freshHelpers(): Promise<typeof import('./session-resume-helpers')> {
  vi.resetModules()
  await import('reflect-metadata')
  return import('./session-resume-helpers')
}

const slugOf = (p: string): string => path.resolve(p).replace(/[^a-zA-Z0-9]/g, '-')

beforeEach(() => {
  home = fs.mkdtempSync(path.join(os.tmpdir(), 'inc-0070-slug-'))
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
  if (path.basename(home).startsWith('inc-0070-slug-')) fs.rmSync(home, { recursive: true, force: true })
})

describe('INC-2026-0070: reverseSlugToPath keeps the real token directory', () => {
  it('reuses an existing token directory with the same slug', async () => {
    const helpers = await freshHelpers()
    const { SESSIONS_BASE } = await import('./session-settings')
    expect(SESSIONS_BASE.startsWith(home)).toBe(true)
    const tokenDir = `${TOKEN}-C_Projects_my-app`
    fs.mkdirSync(path.join(SESSIONS_BASE, tokenDir, 'other-session'), { recursive: true })
    const original = path.join(SESSIONS_BASE, tokenDir, SESSION)

    const rebuilt = helpers.reverseSlugToPath(slugOf(original), TOKEN)

    expect(rebuilt).toBe(original)
    expect(slugOf(rebuilt!)).toBe(slugOf(original))
  })

  it('restores the caller token id literally when no token directory survives', async () => {
    const helpers = await freshHelpers()
    const { SESSIONS_BASE } = await import('./session-settings')
    const original = path.join(SESSIONS_BASE, `${TOKEN}-C_Projects_cmscompilation`, SESSION)

    const rebuilt = helpers.reverseSlugToPath(slugOf(original), TOKEN)

    expect(rebuilt).toBe(original)
  })

  it('keeps the legacy underscore reconstruction for a foreign or unknown token', async () => {
    const helpers = await freshHelpers()
    const { SESSIONS_BASE } = await import('./session-settings')
    const original = path.join(SESSIONS_BASE, `${TOKEN}-C_Projects_x`, SESSION)

    const rebuilt = helpers.reverseSlugToPath(slugOf(original))

    expect(rebuilt).toBe(path.join(SESSIONS_BASE, `${TOKEN.replace(/-/g, '_')}_C_Projects_x`, SESSION))
    // The slug — what the CLI uses to find the transcript — is preserved anyway.
    expect(slugOf(rebuilt!)).toBe(slugOf(original))
  })

  it('a cold resume after the settings dir vanished creates no duplicate token directory', async () => {
    const helpers = await freshHelpers()
    const { SESSIONS_BASE } = await import('./session-settings')
    const tokenDir = `${TOKEN}-C_Projects_cmscompilation`
    const settingsDir = path.join(SESSIONS_BASE, tokenDir, SESSION)
    const slugDir = path.join(home, '.claude', 'projects', slugOf(settingsDir))
    fs.mkdirSync(slugDir, { recursive: true })
    fs.writeFileSync(path.join(slugDir, `${CONV}.jsonl`), '{"type":"user","uuid":"u1"}\n')
    // The token's other sessions survived; this session's own directory did not.
    fs.mkdirSync(path.join(SESSIONS_BASE, tokenDir, 'sibling'), { recursive: true })

    expect(helpers.findOrRecreateSettingsDir(CONV, TOKEN)).toBe(settingsDir)
    expect(fs.readdirSync(SESSIONS_BASE)).toEqual([tokenDir])
  })
})
