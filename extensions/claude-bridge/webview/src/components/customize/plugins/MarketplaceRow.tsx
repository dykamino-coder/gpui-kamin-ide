import type { JSX } from 'preact'
import type { MarketplaceInfo } from '../../../../shared/types'
import { MarketplaceChip } from './MarketplaceChip'
import { MarketplaceAddButton } from './MarketplaceAddButton'
import styles from './MarketplaceRow.module.css'

interface Props {
  marketplaces: MarketplaceInfo[]
  selectedName: string | null
  onSelect: (name: string) => void
  onRefresh: () => void
  onAddGithubOrUrl: () => void
  onAddLocal: () => void
  /** Hides the "+" add button — on the "By Anthropic" tab the list is
   *  curated by Anthropic, users shouldn't inject their own marketplaces
   *  there. Add flow belongs to the Personal tab. */
  allowAdd?: boolean
}

export function MarketplaceRow({
  marketplaces,
  selectedName,
  onSelect,
  onRefresh,
  onAddGithubOrUrl,
  onAddLocal,
  allowAdd = true,
}: Props): JSX.Element {
  return (
    <div class={styles.row}>
      {marketplaces.map((m) => (
        <MarketplaceChip
          key={m.name}
          name={m.name}
          autoUpdate={(m as any).autoUpdate !== false}
          isActive={selectedName === m.name}
          onClick={() => onSelect(m.name)}
          onRefresh={onRefresh}
        />
      ))}
      {allowAdd && <MarketplaceAddButton onGithubOrUrl={onAddGithubOrUrl} onLocal={onAddLocal} />}
    </div>
  )
}
