// Extracted from PluginCard.tsx (Sprint 5 / Stage E1) — action row for an
// installed-but-broken-marketplace card: "Open as Marketplace" + double-
// click-confirm Uninstall. Lives next to the dependencies warning so the
// parent PluginCard becomes a thin layout container.

import type { JSX } from 'preact'
import styles from './PluginCard.module.css'

interface Props {
  marketplace: string
  uninstalling: boolean
  confirmUninstall: boolean
  onUninstall: () => void
  onOpenAsMarketplace?: (marketplaceName: string) => void
}

export function PluginCardBrokenActions({
  marketplace, uninstalling, confirmUninstall, onUninstall, onOpenAsMarketplace,
}: Props): JSX.Element {
  return (
    <div class={styles.brokenActions}>
      {onOpenAsMarketplace && marketplace && (
        <button
          class={styles.openMarketplaceBtn}
          onClick={() => onOpenAsMarketplace(marketplace)}
          title={`Browse "${marketplace}" marketplace`}
        >
          <i class="fas fa-store" /> Open as Marketplace
        </button>
      )}
      <button
        class={styles.uninstallBtn}
        onClick={onUninstall}
        disabled={uninstalling}
        title="Uninstall this broken entry"
      >
        {uninstalling
          ? <><i class="fas fa-spinner fa-spin" /> Removing…</>
          : confirmUninstall
            ? <><i class="fas fa-exclamation-triangle" /> Click again</>
            : <><i class="fas fa-trash" /> Uninstall</>}
      </button>
    </div>
  )
}
