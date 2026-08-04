import type { ComponentType } from 'preact'
import type { CustomizePanel } from '../../signals/ui'
import { SettingsPanel } from './settings/SettingsPanel'
import { ConnectorsPanel } from './connectors/ConnectorsPanel'
import { SkillsPanel } from './skills/SkillsPanel'
import { AgentsPanel } from './agents/AgentsPanel'
import { PluginsPanel } from './plugins/PluginsPanel'
import { MonitorsPanel } from './monitors/MonitorsPanel'
import { SyncedDataPanel } from './sync/SyncedDataPanel'
import { LogsPanel } from './logs/LogsPanel'
import { HooksPanel } from './hooks/HooksPanel'
import { StatsPanel } from './stats/StatsPanel'

/** A concrete customize section id (everything except the `landing` hub and null). */
export type CustomizeSectionId = Exclude<CustomizePanel, 'landing' | null>

export interface CustomizeSection {
  /** Panel key — drives routing, the sidebar `id` (`sidebar-<panel>-item`) and the URL suffix. */
  panel: CustomizeSectionId
  /** FontAwesome icon class. */
  icon: string
  /** Short human label — used for both the sidebar nav item and the landing card title. */
  label: string
  /** One-line landing-card description. */
  desc: string
  /** The panel body rendered by the content router. */
  Component: ComponentType
}

/**
 * Single source of truth for the Customize sections. The sidebar nav
 * (`CustomizeMode`), the landing grid (`CustomizeLanding`) and the content
 * router (`CustomizeContentPanel`) all derive from this list, so they can
 * never drift out of sync. Order here is the canonical order everywhere.
 */
export const SECTIONS: CustomizeSection[] = [
  { panel: 'settings', icon: 'fas fa-gear', label: 'Settings', desc: 'Model, effort, permissions', Component: SettingsPanel },
  { panel: 'skills', icon: 'fas fa-wand-magic-sparkles', label: 'Skills', desc: 'Custom slash commands', Component: SkillsPanel },
  { panel: 'agents', icon: 'fas fa-users', label: 'Agents', desc: 'Available subagents', Component: AgentsPanel },
  { panel: 'mcp', icon: 'fas fa-plug', label: 'MCP Servers', desc: 'External tool servers', Component: ConnectorsPanel },
  { panel: 'hooks', icon: 'fas fa-link', label: 'Hooks', desc: 'Pre/post tool, session lifecycle handlers', Component: HooksPanel },
  { panel: 'plugins', icon: 'fas fa-puzzle-piece', label: 'Plugins', desc: 'Community extensions', Component: PluginsPanel },
  { panel: 'monitors', icon: 'fas fa-chart-line', label: 'Monitors', desc: 'Plugin status-bar monitors', Component: MonitorsPanel },
  { panel: 'sync', icon: 'fas fa-cloud-arrow-up', label: 'Synced data', desc: 'Skills / agents / commands on server', Component: SyncedDataPanel },
  { panel: 'logs', icon: 'fas fa-triangle-exclamation', label: 'Logs', desc: 'Errors & warnings since app start', Component: LogsPanel },
  { panel: 'stats', icon: 'fas fa-chart-column', label: 'Stats', desc: 'Token usage overview', Component: StatsPanel },
]
