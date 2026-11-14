import fs from 'fs'
import path from 'path'
import crypto from 'crypto'
import { bridgeStatePath } from './bridge-state'

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

function hashToken(token: string): string {
  return crypto.createHash('sha256').update(token).digest('hex').slice(0, 8)
}

export function saveTabsState(tabs: PersistedTab[], token: string): void {
  if (!token) return
  try {
    const file = bridgeStatePath('tabs-state.json')
    fs.mkdirSync(path.dirname(file), { recursive: true })
    const payload: PersistedState = {
      version: 1,
      tokenHash: hashToken(token),
      tabs,
    }
    fs.writeFileSync(file, JSON.stringify(payload, null, 2), 'utf-8')
  } catch {
    /* best-effort persistence — ignore failures */
  }
}

export function loadTabsState(token: string): PersistedTab[] | null {
  if (!token) return null
  try {
    const file = bridgeStatePath('tabs-state.json')
    if (!fs.existsSync(file)) return null
    const raw = fs.readFileSync(file, 'utf-8')
    const parsed = JSON.parse(raw) as PersistedState
    if (!parsed || parsed.version !== 1) return null
    if (parsed.tokenHash !== hashToken(token)) return null
    if (!Array.isArray(parsed.tabs)) return null
    return parsed.tabs
  } catch {
    return null
  }
}
