import { signal } from '@preact/signals'
import type { ExternalMcpServerInfo } from '../../shared/types'

export interface SkillItem {
  name: string
  fileName: string
  description: string
  path: string
  source: string
}

export const mcpServers = signal<ExternalMcpServerInfo[]>([])
export const selectedConnectorId = signal<string | null>(null)
export const skills = signal<SkillItem[]>([])
export const selectedSkillPath = signal<string | null>(null)
export const pluginsTab = signal<'active' | 'anthropic' | 'personal'>('active')
export const selectedMarketplace = signal<string | null>(null)
