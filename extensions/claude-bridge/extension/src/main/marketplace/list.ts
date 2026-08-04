import { readKnownMarketplaces } from './known-store'

export interface MarketplaceListItem {
  name: string
  source: Record<string, unknown>
  installLocation: string
  lastUpdated: string
  autoUpdate: boolean
  isAnthropicOfficial: boolean
}

export function listMarketplaces(): MarketplaceListItem[] {
  const data = readKnownMarketplaces()
  return Object.entries(data).map(([name, info]: [string, any]) => ({
    name,
    source: info.source || {},
    installLocation: info.installLocation || '',
    lastUpdated: info.lastUpdated || '',
    // Mirror known_marketplaces.json semantics: missing ⇒ true (auto-update on).
    autoUpdate: info.autoUpdate !== false,
    isAnthropicOfficial: name.includes('anthropic') || name === 'claude-plugins-official',
  }))
}
