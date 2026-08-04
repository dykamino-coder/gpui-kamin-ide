import type { JSX } from 'preact'
import { CardTabBar } from '../../right-panel/CardTabBar'
import styles from './PluginsTabs.module.css'

type Tab = 'active' | 'anthropic' | 'personal'

interface Props {
  activeTab: Tab
  onTabChange: (tab: Tab) => void
  searchValue: string
  onSearchChange: (value: string) => void
}

export function PluginsTabs({ activeTab, onTabChange, searchValue, onSearchChange }: Props): JSX.Element {
  return (
    <div class={styles.tabs}>
      <CardTabBar
        tabs={[
          { id: 'active', label: 'Active', icon: 'fa-plug' },
          { id: 'anthropic', label: 'By Anthropic', icon: 'fa-star' },
          { id: 'personal', label: 'Personal', icon: 'fa-user' },
        ]}
        active={activeTab}
        onSelect={(id) => onTabChange(id as Tab)}
      />
      <div class={styles.searchWrapper}>
        <i class={`fas fa-search ${styles.searchIcon}`} />
        <input
          type="text"
          class={styles.search}
          placeholder="Search"
          value={searchValue}
          onInput={(e) => onSearchChange((e.target as HTMLInputElement).value)}
        />
      </div>
    </div>
  )
}
