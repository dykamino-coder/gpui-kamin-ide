import type { JSX } from 'preact'
import { PluginBadge } from './PluginBadge'
import styles from './PluginBadges.module.css'

interface Counts {
  commands?: number
  agents?: number
  skills?: number
  lspServers?: number
  mcpServers?: number
  hooks?: number
}

interface Props {
  counts: Counts | undefined
}

export function PluginBadges({ counts }: Props): JSX.Element | null {
  if (!counts) return null

  const badges: JSX.Element[] = []
  if (counts.commands) badges.push(<PluginBadge key="cmd" icon="fas fa-terminal" label={`${counts.commands} cmd`} />)
  if (counts.agents) badges.push(<PluginBadge key="agent" icon="fas fa-robot" label={`${counts.agents} agent`} />)
  if (counts.skills) badges.push(<PluginBadge key="skill" icon="fas fa-magic" label={`${counts.skills} skill`} />)
  if (counts.mcpServers) badges.push(<PluginBadge key="mcp" icon="fas fa-plug" label={`${counts.mcpServers} mcp`} />)
  if (counts.lspServers) badges.push(<PluginBadge key="lsp" icon="fas fa-server" label={`${counts.lspServers} LSP`} />)
  if (counts.hooks) badges.push(<PluginBadge key="hooks" icon="fas fa-anchor" label={`${counts.hooks} hook`} />)

  if (badges.length === 0) return null

  return <div class={styles.container}>{badges}</div>
}
