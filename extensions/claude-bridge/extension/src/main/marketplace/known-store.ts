// Read/write of `~/.claude/plugins/known_marketplaces.json`. This file is the
// single source of truth for marketplace metadata — Claude Code CLI reads
// the same file, so we must preserve its exact shape.

import fs from 'fs'
import os from 'os'
import path from 'path'

export type MarketplaceSource =
  | { source: 'git'; url: string }
  | { source: 'github'; repo: string }
  | { source: 'directory'; path: string }

export interface MarketplaceEntry {
  source: MarketplaceSource | Record<string, unknown>
  installLocation: string
  lastUpdated: string
  autoUpdate?: boolean
}

export type KnownMarketplaces = Record<string, MarketplaceEntry>

export function knownMarketplacesPath(): string {
  return path.join(os.homedir(), '.claude', 'plugins', 'known_marketplaces.json')
}

export function readKnownMarketplaces(): KnownMarketplaces {
  const file = knownMarketplacesPath()
  if (!fs.existsSync(file)) return {}
  try {
    return JSON.parse(fs.readFileSync(file, 'utf-8')) as KnownMarketplaces
  } catch { return {} }
}

export function writeKnownMarketplaces(data: KnownMarketplaces): void {
  const file = knownMarketplacesPath()
  fs.mkdirSync(path.dirname(file), { recursive: true })
  fs.writeFileSync(file, JSON.stringify(data, null, 2), 'utf-8')
}
