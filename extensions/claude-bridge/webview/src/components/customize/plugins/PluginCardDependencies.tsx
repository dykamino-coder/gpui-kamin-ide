// Extracted from PluginCard.tsx (Sprint 5 / Stage E1) — yellow warning
// chip listing dependencies that are either uninstalled or installed-but-
// disabled. Renders nothing when every dep is satisfied so the parent can
// just always include it.

import type { JSX } from 'preact'
import styles from './PluginCard.module.css'

interface Dependency {
  name: string
  requested: string
  installed: boolean
  enabled: boolean
}

export function PluginCardDependencies({ dependencies }: { dependencies: Dependency[] }): JSX.Element | null {
  const unresolved = dependencies.filter(d => !d.installed || !d.enabled)
  if (unresolved.length === 0) return null
  return (
    <div class={styles.depsWarning} data-tooltip="Dependencies not installed or disabled. The plugin may not work correctly until these are available.">
      <i class="fas fa-exclamation-triangle" style="margin-right:6px;color:var(--accent-yellow)" />
      Requires: {unresolved.map(d => d.requested).join(', ')}
    </div>
  )
}
