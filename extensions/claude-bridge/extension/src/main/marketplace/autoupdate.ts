import fs from 'fs'
import { assertValidName } from '../validators'
import { knownMarketplacesPath, readKnownMarketplaces, writeKnownMarketplaces } from './known-store'

export interface AutoUpdateResult {
  ok: boolean
  error?: string
}

// Set or clear the autoUpdate flag on a marketplace entry. Stored in the
// same known_marketplaces.json CLI reads, so the toggle carries over.
export function setMarketplaceAutoUpdate(name: string, autoUpdate: boolean): AutoUpdateResult {
  if (!fs.existsSync(knownMarketplacesPath())) return { ok: false, error: 'no known_marketplaces.json' }
  assertValidName(name, 'marketplace name')
  try {
    const known = readKnownMarketplaces() as any
    if (!known[name]) return { ok: false, error: 'marketplace not found' }
    known[name].autoUpdate = autoUpdate
    writeKnownMarketplaces(known)
    return { ok: true }
  } catch (err: any) {
    return { ok: false, error: err?.message || String(err) }
  }
}
