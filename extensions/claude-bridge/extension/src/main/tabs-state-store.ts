import fs from 'fs'
import path from 'path'
import os from 'os'
import crypto from 'crypto'

export interface PersistedTab {
  id: string
  title: string
  cwd: string
  conversationId?: string
  pinned: boolean
  order: number
}

interface PersistedState {
  version: 1
  tokenHash: string
  tabs: PersistedTab[]
}

const STATE_DIR = path.join(os.homedir(), '.claude', 'open-claude-bridge')
const STATE_FILE = path.join(STATE_DIR, 'tabs-state.json')

function hashToken(token: string): string {
  return crypto.createHash('sha256').update(token).digest('hex').slice(0, 8)
}

function ensureDir(): void {
  if (!fs.existsSync(STATE_DIR)) {
    fs.mkdirSync(STATE_DIR, { recursive: true })
  }
}

export function saveTabsState(tabs: PersistedTab[], token: string): void {
  if (!token) return
  try {
    ensureDir()
    const payload: PersistedState = {
      version: 1,
      tokenHash: hashToken(token),
      tabs,
    }
    fs.writeFileSync(STATE_FILE, JSON.stringify(payload, null, 2), 'utf-8')
  } catch {
    /* best-effort persistence — ignore failures */
  }
}

export function loadTabsState(token: string): PersistedTab[] | null {
  if (!token) return null
  try {
    if (!fs.existsSync(STATE_FILE)) return null
    const raw = fs.readFileSync(STATE_FILE, 'utf-8')
    const parsed = JSON.parse(raw) as PersistedState
    if (!parsed || parsed.version !== 1) return null
    if (parsed.tokenHash !== hashToken(token)) return null
    if (!Array.isArray(parsed.tabs)) return null
    return parsed.tabs
  } catch {
    return null
  }
}
