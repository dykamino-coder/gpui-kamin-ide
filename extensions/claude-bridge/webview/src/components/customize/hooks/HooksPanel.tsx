// HooksPanel — top-level Customize tab dispatcher (Active / Library / Log).
// Each tab now lives in its own file:
//   ActiveHooks.tsx          — current installed hooks (3-column list/detail)
//   HooksLibraryAndLog.tsx   — Library templates + execution log
//   HookChips.tsx            — shared chip primitives + host resolver
//
// Splitting (Sprint 5 / Stage E1) reduced this file from 658 LOC to ~50.

import type { JSX } from 'preact'
import { signal } from '@preact/signals'
import { CardTabBar } from '../../right-panel/CardTabBar'
import { ActiveHooks } from './ActiveHooks'
import { LibraryHooks, LogPane } from './HooksLibraryAndLog'

type Tab = 'active' | 'library' | 'log'
const activeTab = signal<Tab>('active')

export function HooksPanel(): JSX.Element {
  const tab = activeTab.value
  const tabs = [
    { id: 'active', label: 'Active', icon: 'fa-link', accent: 'var(--accent-primary)' },
    { id: 'library', label: 'Library', icon: 'fa-box-archive', accent: 'var(--accent-purple)' },
    { id: 'log', label: 'Log', icon: 'fa-list-timeline', accent: 'var(--accent-yellow)' },
  ]
  return (
    <div style="display:flex;flex-direction:column;height:100%;background:var(--bg-mantle)">
      <CardTabBar tabs={tabs} active={tab} onSelect={(id) => { activeTab.value = id as Tab }} />
      {tab === 'active' && <ActiveHooks />}
      {tab === 'library' && <LibraryHooks />}
      {tab === 'log' && <LogPane />}
    </div>
  )
}
